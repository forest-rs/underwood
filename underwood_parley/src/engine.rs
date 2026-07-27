// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained paragraph-engine orchestration and cache ownership.
//!
//! This module owns formation invalidation and retained Parley preparation; it
//! explicitly does not own line-breaking, shaping, lowering, or interaction
//! algorithms.

use alloc::{collections::BTreeMap, vec::Vec};
use core::{
    mem::{self, size_of, size_of_val},
    ops::Range,
};

use parley_engine::{Analysis, Analyzer, ShapedText, Shaper};
use underwood::adapter::{
    FormationWork, LineBreakReason, LineShapingWork, ParagraphConstraints, ParagraphFormation,
    ParagraphFormationCacheDiagnostics, ParagraphFormationOutput, ParagraphFormationReuse,
    ParagraphInput, ParagraphPreparationId, PreparationError, PreparedLine,
    PreparedParagraphBuilder, PreparedParagraphCapacity, PreparedRun,
};
use underwood::{RegionAttempt, RegionTranscript, ResolvedDirection};

use crate::font::FontSet;
use crate::interaction::{collect_analysis_units_into, lower_visual_units};
use crate::line_break::{
    FormedLine, LineFormationScratch, LineFormationWork, LogicalCluster, apply_wrap_policy,
    collect_logical_clusters_into, form_lines, line_run_pieces, reorder_visual_pieces,
    shaped_text_accounted_bytes, update_line_metrics,
};
use crate::lowering::{
    append_unrendered_source, checked_source_range, index_char_starts, lower_glyphs_into,
    lowered_glyph_count, portable_synthesis,
};
use crate::shaping::{
    analyze_text_with_styles_into, prepare_inline_flow_indices,
    reshape_paragraph_with_retained_fonts, shape_line, shape_paragraph, shaped_glyph_count,
};
use crate::spacing::{
    apply_spacing, capture_base_advances, collect_joining_units, restore_base_advances,
};
use crate::validation::{
    validate_inline_flow_runs, validate_input_runs, validate_paint_runs, validate_shaping_runs,
};

/// Retained Parley Engine paragraph adapter.
#[derive(Debug)]
pub struct ParleyParagraphEngine {
    fonts: FontSet,
    analyzer: Analyzer,
    shaper: Shaper,
    line_scratch: LineFormationScratch,
    reshape_scratch: ShapedText,
    cache: BTreeMap<ParagraphPreparationId, PreparationCache>,
    scratch_cache: Option<PreparationCache>,
    retention_budget: usize,
    retention_clock: u64,
    retention_work: RetentionWork,
}

impl ParleyParagraphEngine {
    /// Creates a retained adapter from an immutable font snapshot.
    #[must_use]
    pub fn new(fonts: FontSet) -> Self {
        Self {
            fonts,
            analyzer: Analyzer::new(),
            shaper: Shaper::default(),
            line_scratch: LineFormationScratch::default(),
            reshape_scratch: ShapedText::new(),
            cache: BTreeMap::new(),
            scratch_cache: None,
            retention_budget: 0,
            retention_clock: 0,
            retention_work: RetentionWork::default(),
        }
    }

    fn scratch_accounted_bytes(&self) -> usize {
        self.line_scratch
            .accounted_owned_bytes()
            .saturating_add(shaped_text_accounted_bytes(&self.reshape_scratch))
            .saturating_add(
                self.scratch_cache
                    .as_ref()
                    .map_or(0, PreparationCache::calculate_accounted_owned_bytes),
            )
    }

    fn remove_cache(&mut self, preparation: ParagraphPreparationId, release: bool) -> bool {
        let Some(cache) = self.cache.remove(&preparation) else {
            return false;
        };
        self.retention_work.resident_bytes = self
            .retention_work
            .resident_bytes
            .saturating_sub(cache.accounted_bytes);
        self.recycle_cache(cache);
        if release {
            self.retention_work.releases = self.retention_work.releases.saturating_add(1);
        }
        true
    }

    fn recycle_cache(&mut self, mut cache: PreparationCache) {
        cache.last_used = 0;
        cache.accounted_bytes = 0;
        let replacement_bytes = cache.calculate_accounted_owned_bytes();
        let retained_bytes = self
            .scratch_cache
            .as_ref()
            .map_or(0, PreparationCache::calculate_accounted_owned_bytes);
        if replacement_bytes >= retained_bytes {
            self.scratch_cache = Some(cache);
        }
    }

    fn release_entry(&mut self, preparation: ParagraphPreparationId, release: bool) {
        self.remove_cache(preparation, release);
    }

    fn enforce_retention_budget(&mut self) {
        while self.retention_work.resident_bytes > self.retention_budget {
            let oldest = self
                .cache
                .iter()
                .map(|(id, entry)| (entry.last_used, *id))
                .min();
            let Some((_, preparation)) = oldest else {
                break;
            };
            let removed = self.remove_cache(preparation, false);
            if !removed {
                break;
            }
            self.retention_work.evictions = self.retention_work.evictions.saturating_add(1);
        }
    }
}

impl ParagraphFormation for ParleyParagraphEngine {
    fn shared_preparation_epoch(&self) -> Option<u64> {
        Some(0)
    }

    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError> {
        let preparation_id = input.preparation();
        let change = input.change();
        let retained = self.cache.contains_key(&preparation_id);
        let formation_reuse = if retained {
            self.retention_work.hits = self.retention_work.hits.saturating_add(1);
            ParagraphFormationReuse::RetainedFacts
        } else {
            self.retention_work.misses = self.retention_work.misses.saturating_add(1);
            ParagraphFormationReuse::Cold
        };
        let text_len =
            u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
        if !retained || change.analysis_changed() {
            validate_input_runs(&input)?;
        } else {
            if change.font_selection_changed() {
                validate_shaping_runs(&input, text_len)?;
            }
            if change.inline_flow_projection_changed() {
                validate_inline_flow_runs(&input, text_len)?;
            }
            if change.paint_changed() {
                validate_paint_runs(&input, text_len)?;
            }
        }
        if !retained {
            self.cache.insert(
                preparation_id,
                self.scratch_cache
                    .take()
                    .unwrap_or_else(PreparationCache::empty),
            );
        }
        let analyzed = !retained || change.analysis_changed();
        if analyzed {
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            analyze_text_with_styles_into(
                &mut self.analyzer,
                input.text(),
                input.paragraph_style().base_direction(),
                input.analysis_styles(),
                input.analysis_runs(),
                &mut cache.analysis,
            )?;
            collect_analysis_units_into(
                input.text(),
                &cache.analysis,
                &mut cache.interaction_units,
            )?;
            collect_joining_units(
                input.text(),
                &cache.analysis,
                &cache.interaction_units,
                &mut cache.joining_units,
            )?;
            index_char_starts(input.text(), &mut cache.char_starts)?;
            cache.style_indices.clear();
            cache.inline_flow_indices.clear();
            cache.shaped_text.clear();
            cache.scripts.clear();
            cache.base_cluster_advances.clear();
            cache.base_glyph_advances.clear();
            cache.logical_clusters.clear();
            cache.shaped_glyphs = 0;
            cache.formed_lines.clear();
            cache.region_transcript = None;
        }

        let font_queried = analyzed || change.font_selection_changed();
        let flow_projection_changed = analyzed || change.inline_flow_projection_changed();
        let ligature_policy_changed = !retained || change.ligature_policy_changed();
        let spacing_changed = !retained || change.spacing_changed();
        let line_height_changed = !retained || change.line_metrics_changed();
        let break_policy_changed = !retained || change.break_policy_changed();
        let shaped = font_queried || ligature_policy_changed;
        let mut selected_clusters = 0_u32;
        if shaped {
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            if font_queried {
                selected_clusters = shape_paragraph(
                    &mut self.shaper,
                    &cache.analysis,
                    &mut self.fonts,
                    input.text(),
                    input.shaping_styles(),
                    input.shaping_runs(),
                    input.inline_flow_styles(),
                    input.inline_flow_runs(),
                    &mut cache.shaped_text,
                    &mut cache.scripts,
                    &mut cache.style_indices,
                    &mut cache.inline_flow_indices,
                )?;
            } else {
                self.reshape_scratch.clear();
                reshape_paragraph_with_retained_fonts(
                    &mut self.shaper,
                    &cache.analysis,
                    &cache.shaped_text,
                    input.text(),
                    input.shaping_styles(),
                    input.shaping_runs(),
                    input.inline_flow_styles(),
                    input.inline_flow_runs(),
                    &mut self.reshape_scratch,
                    &mut cache.scripts,
                    &mut cache.style_indices,
                    &mut cache.inline_flow_indices,
                )?;
                mem::swap(&mut cache.shaped_text, &mut self.reshape_scratch);
            }
            cache.shaped_glyphs = shaped_glyph_count(&cache.shaped_text);
            capture_base_advances(
                &cache.shaped_text,
                &mut cache.base_cluster_advances,
                &mut cache.base_glyph_advances,
            );
        } else if flow_projection_changed {
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            prepare_inline_flow_indices(
                input.text(),
                input.inline_flow_runs(),
                &mut cache.inline_flow_indices,
            )?;
        }

        if shaped || spacing_changed {
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            if !shaped {
                restore_base_advances(
                    &mut cache.shaped_text,
                    &cache.base_cluster_advances,
                    &cache.base_glyph_advances,
                )?;
            }
            apply_spacing(
                &mut cache.shaped_text,
                input.inline_flow_styles(),
                input.inline_flow_runs(),
                &cache.interaction_units,
                &cache.joining_units,
            )?;
            collect_logical_clusters_into(
                input.text(),
                &cache.shaped_text,
                &mut cache.logical_clusters,
            )?;
            cache.formed_lines.clear();
            cache.region_transcript = None;
        }
        if shaped || spacing_changed || break_policy_changed {
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            apply_wrap_policy(
                &mut cache.logical_clusters,
                input.inline_flow_styles(),
                input.inline_flow_runs(),
            )?;
        }

        let constraints_match = retained && !change.constraints_changed();
        let needs_formation = shaped
            || spacing_changed
            || line_height_changed
            || break_policy_changed
            || !constraints_match;
        let mut line_work = LineFormationWork::default();
        if needs_formation {
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            if !shaped
                && !spacing_changed
                && !break_policy_changed
                && constraints.region_flow().is_none()
                && constraints_match
            {
                update_line_metrics(
                    input.text(),
                    &mut cache.formed_lines,
                    input.inline_flow_styles(),
                    input.inline_flow_runs(),
                )?;
            } else {
                let analysis = &cache.analysis;
                let canonical_text = &cache.shaped_text;
                let scripts = &cache.scripts;
                let logical_clusters = &cache.logical_clusters;
                let style_indices = &cache.style_indices;
                let inline_flow_indices = &cache.inline_flow_indices;
                let interaction_units = &cache.interaction_units;
                let joining_units = &cache.joining_units;
                let formed_lines = &mut cache.formed_lines;
                let (work, transcript) = form_lines(
                    input.paragraph(),
                    input.text(),
                    canonical_text,
                    scripts,
                    logical_clusters,
                    input.inline_flow_styles(),
                    input.inline_flow_runs(),
                    &constraints,
                    formed_lines,
                    &mut self.line_scratch,
                    |source, line_text, scripts| {
                        let output = shape_line(
                            &mut self.shaper,
                            analysis,
                            canonical_text,
                            input.text(),
                            input.shaping_styles(),
                            style_indices,
                            input.inline_flow_styles(),
                            inline_flow_indices,
                            source,
                            line_text,
                            scripts,
                        )?;
                        apply_spacing(
                            line_text,
                            input.inline_flow_styles(),
                            input.inline_flow_runs(),
                            interaction_units,
                            joining_units,
                        )?;
                        Ok(output)
                    },
                )?;
                line_work = work;
                cache.region_transcript = transcript;
            }
        }

        let preparation = self
            .cache
            .get(&preparation_id)
            .ok_or_else(PreparationError::invalid_output)?;
        let work = FormationWork::new(
            analyzed,
            shaped,
            selected_clusters,
            if shaped {
                u32::try_from(preparation.shaped_text.runs().len()).unwrap_or(u32::MAX)
            } else {
                0
            },
            if shaped { preparation.shaped_glyphs } else { 0 },
            if needs_formation {
                u32::try_from(preparation.formed_lines.len()).unwrap_or(u32::MAX)
            } else {
                0
            },
            if needs_formation {
                LineShapingWork::new(
                    line_work.reshapes,
                    line_work.resolved_clusters,
                    line_work.shaped_runs,
                    line_work.shaped_glyphs,
                )
                .with_formation(
                    line_work.candidates,
                    line_work.rejected_candidates,
                    line_work.checkpoint_restores,
                )
            } else {
                LineShapingWork::default()
            },
        );
        let resolved_direction = if preparation.analysis.is_rtl() {
            ResolvedDirection::Rtl
        } else {
            ResolvedDirection::Ltr
        };
        let mut paragraph = PreparedParagraphBuilder::with_features(
            input.paragraph(),
            text_len,
            resolved_direction,
            input.features(),
        );
        paragraph.reserve_exact(prepared_capacity(input.text(), preparation)?);
        for formed in &preparation.formed_lines {
            let plan = &formed.plan;
            let shaped_text = &formed.shaped_text;
            if shaped_text.runs().len() != formed.scripts.len() {
                return Err(PreparationError::invalid_output());
            }
            let mut pieces = line_run_pieces(shaped_text, plan.clusters.clone())?;
            reorder_visual_pieces(shaped_text, &mut pieces);
            let mut line = paragraph.begin_line(PreparedLine::try_new_in_slot(
                plan.slot,
                checked_source_range(&plan.source)?,
                plan.reason,
                plan.advance,
                plan.baseline,
                plan.height,
                plan.content_ascent,
                plan.content_descent,
            )?)?;
            lower_visual_units(
                input.text(),
                shaped_text,
                &formed.scripts,
                &pieces,
                &preparation.interaction_units,
                &plan.source,
                plan.reason == LineBreakReason::Mandatory,
                &mut line,
            )?;
            for piece in pieces {
                let run = shaped_text
                    .runs()
                    .get(piece.run)
                    .ok_or_else(PreparationError::invalid_output)?;
                let script = formed
                    .scripts
                    .get(piece.run)
                    .ok_or_else(PreparationError::invalid_output)?;
                let font = shaped_text
                    .fonts()
                    .get(run.font_index)
                    .ok_or_else(PreparationError::invalid_output)?;
                let normalized_coords = shaped_text
                    .normalized_coords()
                    .get(run.normalized_coords_range.clone())
                    .ok_or_else(PreparationError::invalid_output)?;
                let clusters = shaped_text
                    .clusters()
                    .get(piece.clusters.clone())
                    .ok_or_else(PreparationError::invalid_output)?;
                let first = clusters
                    .first()
                    .ok_or_else(PreparationError::invalid_output)?;
                let last = clusters
                    .last()
                    .ok_or_else(PreparationError::invalid_output)?;
                let source = run.range.byte_range.start + usize::from(first.text_offset)
                    ..run.range.byte_range.start
                        + usize::from(last.text_offset)
                        + usize::from(last.text_len);
                let synthesis = portable_synthesis(font.synthesis)?;
                let mut prepared_run = line.begin_run(PreparedRun::try_new(
                    checked_source_range(&source)?,
                    run.bidi_level,
                    *script,
                    font.font.clone(),
                    run.font_size,
                    synthesis,
                )?);
                prepared_run.extend_normalized_coords(
                    normalized_coords.iter().map(|coord| coord.to_bits()),
                );
                lower_glyphs_into(
                    input.text(),
                    &preparation.analysis,
                    &preparation.char_starts,
                    shaped_text,
                    run,
                    piece.clusters.clone(),
                    input.paint_runs(),
                    &mut prepared_run,
                )?;
                append_unrendered_source(
                    input.text(),
                    &preparation.analysis,
                    &preparation.char_starts,
                    source.clone(),
                    &mut prepared_run,
                )?;
                prepared_run.finish()?;
            }
            line.finish()?;
        }
        let paragraph = paragraph.finish()?;
        let region_transcript = preparation.region_transcript.clone();
        match region_transcript {
            Some(transcript) => Ok(ParagraphFormationOutput::in_regions(
                paragraph, work, transcript,
            )
            .with_reuse(formation_reuse)),
            None => Ok(ParagraphFormationOutput::new(paragraph, work).with_reuse(formation_reuse)),
        }
    }

    fn release(&mut self, preparation: ParagraphPreparationId) {
        self.release_entry(preparation, true);
    }

    fn clear(&mut self) {
        self.retention_work.releases = self
            .retention_work
            .releases
            .saturating_add(self.cache.len());
        let retained = mem::take(&mut self.cache);
        for cache in retained.into_values() {
            self.recycle_cache(cache);
        }
        self.retention_work.resident_bytes = 0;
    }

    fn set_retained_facts_budget(&mut self, bytes: usize) {
        self.retention_budget = bytes;
        self.enforce_retention_budget();
    }

    fn commit_preparation(&mut self, preparation: ParagraphPreparationId) {
        self.retention_clock = self.retention_clock.saturating_add(1);
        let current_use = self.retention_clock;
        if let Some(cache) = self.cache.get_mut(&preparation) {
            self.retention_work.resident_bytes = self
                .retention_work
                .resident_bytes
                .saturating_sub(cache.accounted_bytes);
            cache.last_used = current_use;
            cache.accounted_bytes = cache.calculate_accounted_owned_bytes();
            self.retention_work.resident_bytes = self
                .retention_work
                .resident_bytes
                .saturating_add(cache.accounted_bytes);
        }
        self.retention_work.peak_bytes = self
            .retention_work
            .peak_bytes
            .max(self.retention_work.resident_bytes);
        self.enforce_retention_budget();
    }

    fn trim_retained_facts(&mut self) {
        self.clear();
    }

    fn retained_facts(&self) -> Option<ParagraphFormationCacheDiagnostics> {
        Some(ParagraphFormationCacheDiagnostics::new(
            self.retention_budget,
            self.cache.len(),
            self.retention_work.resident_bytes,
            self.retention_work.peak_bytes,
            self.scratch_accounted_bytes(),
            self.retention_work.hits,
            self.retention_work.misses,
            self.retention_work.evictions,
            self.retention_work.releases,
        ))
    }
}

#[derive(Debug)]
struct PreparationCache {
    analysis: Analysis,
    interaction_units: Vec<Range<usize>>,
    joining_units: Vec<bool>,
    char_starts: Vec<u32>,
    style_indices: Vec<u16>,
    inline_flow_indices: Vec<u16>,
    shaped_text: ShapedText,
    scripts: Vec<[u8; 4]>,
    base_cluster_advances: Vec<f32>,
    base_glyph_advances: Vec<f32>,
    logical_clusters: Vec<LogicalCluster>,
    shaped_glyphs: u32,
    formed_lines: Vec<FormedLine>,
    region_transcript: Option<RegionTranscript>,
    last_used: u64,
    accounted_bytes: usize,
}

fn prepared_capacity(
    text: &str,
    preparation: &PreparationCache,
) -> Result<PreparedParagraphCapacity, PreparationError> {
    let mut runs = 0_usize;
    let mut glyphs = 0_usize;
    let mut normalized_coords = 0_usize;
    for formed in &preparation.formed_lines {
        for run in formed.shaped_text.runs() {
            let start = run.clusters_range.start.max(formed.plan.clusters.start);
            let end = run.clusters_range.end.min(formed.plan.clusters.end);
            if start < end {
                runs = runs.saturating_add(1);
                normalized_coords =
                    normalized_coords.saturating_add(run.normalized_coords_range.len());
                glyphs = glyphs.saturating_add(lowered_glyph_count(
                    text,
                    &preparation.analysis,
                    &preparation.char_starts,
                    &formed.shaped_text,
                    run,
                    start..end,
                )?);
            }
        }
    }
    Ok(PreparedParagraphCapacity::new(
        preparation.formed_lines.len(),
        runs,
        glyphs,
        preparation.interaction_units.len(),
    )
    .with_normalized_coords(normalized_coords))
}

impl PreparationCache {
    fn empty() -> Self {
        Self {
            analysis: Analysis::new(),
            interaction_units: Vec::new(),
            joining_units: Vec::new(),
            char_starts: Vec::new(),
            style_indices: Vec::new(),
            inline_flow_indices: Vec::new(),
            shaped_text: ShapedText::new(),
            scripts: Vec::new(),
            base_cluster_advances: Vec::new(),
            base_glyph_advances: Vec::new(),
            logical_clusters: Vec::new(),
            shaped_glyphs: 0,
            formed_lines: Vec::new(),
            region_transcript: None,
            last_used: 0,
            accounted_bytes: 0,
        }
    }

    fn calculate_accounted_owned_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>()
            .saturating_add(size_of_val(self.analysis.char_info()))
            .saturating_add(size_of_val(self.analysis.bidi_levels()))
            .saturating_add(vec_bytes::<Range<usize>>(self.interaction_units.capacity()))
            .saturating_add(vec_bytes::<bool>(self.joining_units.capacity()))
            .saturating_add(vec_bytes::<u32>(self.char_starts.capacity()))
            .saturating_add(vec_bytes::<u16>(self.style_indices.capacity()))
            .saturating_add(vec_bytes::<u16>(self.inline_flow_indices.capacity()))
            .saturating_add(shaped_text_accounted_bytes(&self.shaped_text))
            .saturating_add(vec_bytes::<[u8; 4]>(self.scripts.capacity()))
            .saturating_add(vec_bytes::<f32>(self.base_cluster_advances.capacity()))
            .saturating_add(vec_bytes::<f32>(self.base_glyph_advances.capacity()))
            .saturating_add(vec_bytes::<LogicalCluster>(
                self.logical_clusters.capacity(),
            ))
            .saturating_add(vec_bytes::<FormedLine>(self.formed_lines.capacity()));
        for line in &self.formed_lines {
            bytes = bytes.saturating_add(line.accounted_owned_bytes());
        }
        if let Some(transcript) = &self.region_transcript {
            bytes = bytes.saturating_add(
                size_of::<RegionAttempt>().saturating_mul(transcript.attempts().len()),
            );
        }
        bytes
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct RetentionWork {
    resident_bytes: usize,
    peak_bytes: usize,
    hits: usize,
    misses: usize,
    evictions: usize,
    releases: usize,
}

const fn vec_bytes<T>(capacity: usize) -> usize {
    size_of::<T>().saturating_mul(capacity)
}
