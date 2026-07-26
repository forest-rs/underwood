// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained paragraph-engine orchestration and cache ownership.
//!
//! This module owns formation invalidation and retained Parley preparation; it
//! explicitly does not own line-breaking, shaping, lowering, or interaction
//! algorithms.

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::{mem, ops::Range};

use parley_engine::{Analysis, Analyzer, ShapedText, Shaper};
use underwood::adapter::{
    AnalysisRun, FormationWork, InlineFlowRun, LineBreakReason, LineShapingWork,
    ParagraphConstraints, ParagraphFormation, ParagraphFormationOutput, ParagraphInput,
    ParagraphPreparationId, PreparationError, PreparedLine, PreparedParagraph, PreparedRun,
    ShapingRun,
};
use underwood::{
    AnalysisStyle, InlineFlowStyle, ParagraphStyle, RegionTranscript, ResolvedDirection,
    ShapingStyle,
};

use crate::font::FontSet;
use crate::interaction::{collect_analysis_units, lower_visual_units, prepared_cursor_movements};
use crate::line_break::{
    FormedLine, LineFormationScratch, LineFormationWork, LogicalCluster, apply_wrap_policy,
    collect_logical_clusters, form_lines, line_run_pieces, reorder_visual_pieces,
    update_line_metrics,
};
use crate::lowering::{
    checked_source_range, index_char_starts, lower_glyphs, paint_coverage, portable_synthesis,
    unrendered_source,
};
use crate::shaping::{
    analyze_text_with_styles, prepare_inline_flow_indices, reshape_paragraph_with_retained_fonts,
    shape_line, shape_paragraph, shaped_glyph_count,
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
    outputs: BTreeMap<ParagraphPreparationId, RetainedOutput>,
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
            outputs: BTreeMap::new(),
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
        if change.output_retained() {
            let output = self
                .cache
                .get(&preparation_id)
                .and_then(|cache| {
                    cache.prepared.as_ref().map(|prepared| RetainedOutput {
                        prepared: prepared.clone(),
                        region_transcript: cache.region_transcript.clone(),
                    })
                })
                .or_else(|| self.outputs.get(&preparation_id).cloned());
            if let Some(output) = output {
                return Ok(output.into_formation_output());
            }
        }
        if let Some(source) = input.reusable_preparation() {
            let output = self
                .cache
                .get(&source)
                .and_then(|cache| {
                    cache.prepared.as_ref().map(|prepared| RetainedOutput {
                        prepared: prepared.clone(),
                        region_transcript: cache.region_transcript.clone(),
                    })
                })
                .or_else(|| self.outputs.get(&source).cloned());
            if let Some(output) = output {
                self.cache.remove(&preparation_id);
                self.outputs.insert(preparation_id, output.clone());
                return Ok(output.into_formation_output());
            }
        }
        self.outputs.remove(&preparation_id);
        let retained = self.cache.contains_key(&preparation_id);
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
        let analyzed = !retained || change.analysis_changed();
        if analyzed {
            let analysis = analyze_text_with_styles(
                &mut self.analyzer,
                input.text(),
                input.paragraph_style().base_direction(),
                input.analysis_styles(),
                input.analysis_runs(),
            )?;
            let interaction_units = collect_analysis_units(input.text(), &analysis)?;
            let mut joining_units = Vec::new();
            collect_joining_units(
                input.text(),
                &analysis,
                &interaction_units,
                &mut joining_units,
            )?;
            let mut char_starts = self
                .cache
                .get_mut(&preparation_id)
                .map(|cache| mem::take(&mut cache.char_starts))
                .unwrap_or_default();
            index_char_starts(input.text(), &mut char_starts)?;
            if let Some(cache) = self.cache.get_mut(&preparation_id) {
                cache.text = Arc::from(input.text());
                cache.paragraph_style = input.paragraph_style();
                cache.analysis_styles = input.analysis_styles().to_vec();
                cache.analysis_runs = input.analysis_runs().to_vec();
                cache.analysis = analysis;
                cache.interaction_units = interaction_units;
                cache.joining_units = joining_units;
                cache.char_starts = char_starts;
                cache.shaping_styles.clear();
                cache.shaping_runs.clear();
                cache.style_indices.clear();
                cache.inline_flow_indices.clear();
                cache.shaped_text.clear();
                cache.scripts.clear();
                cache.base_cluster_advances.clear();
                cache.base_glyph_advances.clear();
                cache.logical_clusters.clear();
                cache.shaped_glyphs = 0;
                cache.inline_flow_styles.clear();
                cache.inline_flow_runs.clear();
                cache.constraints = None;
                cache.formed_lines.clear();
                cache.region_transcript = None;
            } else {
                self.cache.insert(
                    preparation_id,
                    PreparationCache {
                        text: Arc::from(input.text()),
                        paragraph_style: input.paragraph_style(),
                        analysis_styles: input.analysis_styles().to_vec(),
                        analysis_runs: input.analysis_runs().to_vec(),
                        analysis,
                        interaction_units,
                        joining_units,
                        char_starts,
                        shaping_styles: Vec::new(),
                        shaping_runs: Vec::new(),
                        style_indices: Vec::new(),
                        inline_flow_indices: Vec::new(),
                        shaped_text: ShapedText::new(),
                        scripts: Vec::new(),
                        base_cluster_advances: Vec::new(),
                        base_glyph_advances: Vec::new(),
                        logical_clusters: Vec::new(),
                        shaped_glyphs: 0,
                        inline_flow_styles: Vec::new(),
                        inline_flow_runs: Vec::new(),
                        constraints: None,
                        formed_lines: Vec::new(),
                        region_transcript: None,
                        prepared: None,
                        prepared_paint_runs: Vec::new(),
                    },
                );
            }
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
            // Invalidate the retained key before any fallible shaping work.
            // A failed font query clears `shaped_text`; keeping the previously
            // successful key would let a later paint-only request reuse that
            // incomplete cache entry.
            cache.shaping_styles.clear();
            cache.shaping_runs.clear();
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
            cache.shaping_styles = input.shaping_styles().to_vec();
            cache.shaping_runs = input.shaping_runs().to_vec();
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
            cache.logical_clusters = collect_logical_clusters(input.text(), &cache.shaped_text)?;
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
            // Once formation inputs change, no previously lowered handle can
            // prove the new line and interaction facts. Invalidate it before
            // fallible work so a later request cannot observe stale output.
            cache.prepared = None;
            cache.prepared_paint_runs.clear();
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
            cache.constraints = Some(constraints.clone());
        }
        if flow_projection_changed {
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            cache.inline_flow_styles = input.inline_flow_styles().to_vec();
            cache.inline_flow_runs = input.inline_flow_runs().to_vec();
        }
        self.cache
            .get_mut(&preparation_id)
            .ok_or_else(PreparationError::invalid_output)?
            .paragraph_style = input.paragraph_style();

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
        if !needs_formation
            && !change.paint_changed()
            && let Some(prepared) = &preparation.prepared
        {
            return match preparation.region_transcript.clone() {
                Some(transcript) => Ok(ParagraphFormationOutput::in_regions(
                    prepared.clone(),
                    work,
                    transcript,
                )),
                None => Ok(ParagraphFormationOutput::new(prepared.clone(), work)),
            };
        }
        if !needs_formation
            && change.paint_changed()
            && let Some(prepared) = &preparation.prepared
        {
            let repainted = prepared
                .try_map_glyph_paint(|glyph| paint_coverage(glyph.source(), input.paint_runs()))?;
            let region_transcript = preparation.region_transcript.clone();
            let cache = self
                .cache
                .get_mut(&preparation_id)
                .ok_or_else(PreparationError::invalid_output)?;
            cache.prepared = Some(repainted.clone());
            cache.prepared_paint_runs = input.paint_runs().to_vec();
            return match region_transcript {
                Some(transcript) => Ok(ParagraphFormationOutput::in_regions(
                    repainted, work, transcript,
                )),
                None => Ok(ParagraphFormationOutput::new(repainted, work)),
            };
        }
        let mut prepared_lines = Vec::with_capacity(preparation.formed_lines.len());
        for formed in &preparation.formed_lines {
            let plan = &formed.plan;
            let shaped_text = &formed.shaped_text;
            if shaped_text.runs().len() != formed.scripts.len() {
                return Err(PreparationError::invalid_output());
            }
            let mut pieces = line_run_pieces(shaped_text, plan.clusters.clone())?;
            reorder_visual_pieces(shaped_text, &mut pieces);
            let prepared_units = lower_visual_units(
                input.text(),
                shaped_text,
                &formed.scripts,
                &pieces,
                &preparation.interaction_units,
                &plan.source,
                plan.reason == LineBreakReason::Mandatory,
            )?;
            let mut prepared_runs = Vec::with_capacity(pieces.len());
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
                let prepared_glyphs = lower_glyphs(
                    input.text(),
                    &preparation.analysis,
                    &preparation.char_starts,
                    shaped_text,
                    run,
                    piece.clusters.clone(),
                    input.paint_runs(),
                )?;
                let unrendered_source = unrendered_source(
                    input.text(),
                    &preparation.analysis,
                    &preparation.char_starts,
                    source.clone(),
                    &prepared_glyphs,
                )?;
                prepared_runs.push(PreparedRun::try_new(
                    checked_source_range(&source)?,
                    run.bidi_level,
                    *script,
                    font.font.clone(),
                    run.font_size,
                    synthesis,
                    normalized_coords.iter().map(|coord| coord.to_bits()),
                    unrendered_source,
                    prepared_glyphs,
                )?);
            }
            prepared_lines.push(PreparedLine::try_new_in_slot(
                plan.slot,
                checked_source_range(&plan.source)?,
                plan.reason,
                plan.advance,
                plan.baseline,
                plan.height,
                plan.content_ascent,
                plan.content_descent,
                prepared_units,
                prepared_runs,
            )?);
        }
        let movements = prepared_cursor_movements(&prepared_lines, text_len)?;
        let resolved_direction = if preparation.analysis.is_rtl() {
            ResolvedDirection::Rtl
        } else {
            ResolvedDirection::Ltr
        };
        let paragraph = PreparedParagraph::try_new(
            input.paragraph(),
            text_len,
            resolved_direction,
            prepared_lines,
            movements,
        )?;
        let region_transcript = preparation.region_transcript.clone();
        let cache = self
            .cache
            .get_mut(&preparation_id)
            .ok_or_else(PreparationError::invalid_output)?;
        cache.prepared = Some(paragraph.clone());
        cache.prepared_paint_runs = input.paint_runs().to_vec();
        match region_transcript {
            Some(transcript) => Ok(ParagraphFormationOutput::in_regions(
                paragraph, work, transcript,
            )),
            None => Ok(ParagraphFormationOutput::new(paragraph, work)),
        }
    }

    fn release(&mut self, preparation: ParagraphPreparationId) {
        self.cache.remove(&preparation);
        self.outputs.remove(&preparation);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.outputs.clear();
    }

    fn retained_entries(&self) -> Option<usize> {
        Some(self.cache.len().saturating_add(self.outputs.len()))
    }
}

#[derive(Debug)]
struct PreparationCache {
    text: Arc<str>,
    paragraph_style: ParagraphStyle,
    analysis_styles: Vec<AnalysisStyle>,
    analysis_runs: Vec<AnalysisRun>,
    analysis: Analysis,
    interaction_units: Vec<Range<usize>>,
    joining_units: Vec<bool>,
    char_starts: Vec<u32>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    style_indices: Vec<u16>,
    inline_flow_indices: Vec<u16>,
    shaped_text: ShapedText,
    scripts: Vec<[u8; 4]>,
    base_cluster_advances: Vec<f32>,
    base_glyph_advances: Vec<f32>,
    logical_clusters: Vec<LogicalCluster>,
    shaped_glyphs: u32,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    constraints: Option<ParagraphConstraints>,
    formed_lines: Vec<FormedLine>,
    region_transcript: Option<RegionTranscript>,
    prepared: Option<PreparedParagraph>,
    prepared_paint_runs: Vec<underwood::adapter::PaintRun>,
}

#[derive(Clone, Debug)]
struct RetainedOutput {
    prepared: PreparedParagraph,
    region_transcript: Option<RegionTranscript>,
}

impl RetainedOutput {
    fn into_formation_output(self) -> ParagraphFormationOutput {
        match self.region_transcript {
            Some(transcript) => ParagraphFormationOutput::in_regions(
                self.prepared,
                FormationWork::default(),
                transcript,
            ),
            None => ParagraphFormationOutput::new(self.prepared, FormationWork::default()),
        }
    }
}
