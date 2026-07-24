// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained paragraph-engine orchestration and cache ownership.
//!
//! This module owns formation invalidation and retained Parley physics; it
//! explicitly does not own line-breaking, shaping, lowering, or interaction
//! algorithms.

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::ops::Range;

use parley_core::{Analysis, Analyzer, ShapedText, Shaper};
use underwood::adapter::{
    FormationWork, InlineFlowRun, LineBreakReason, ParagraphConstraints, ParagraphFormation,
    ParagraphFormationOutput, ParagraphInput, PreparationError, PreparedLine, PreparedParagraph,
    PreparedRun, ShapingRun,
};
use underwood::{InlineFlowStyle, ParagraphId, ShapingStyle};

use crate::font::FontSet;
use crate::interaction::{collect_analysis_units, lower_visual_units, prepared_cursor_movements};
use crate::line_break::{
    LinePlan, LogicalCluster, collect_logical_clusters, form_lines, line_run_pieces,
    reorder_visual_pieces,
};
use crate::lowering::{checked_source_range, lower_glyphs, portable_synthesis, unrendered_source};
use crate::shaping::{analyze_text, shape_paragraph};
use crate::validation::validate_input_runs;

/// Retained Parley Core paragraph adapter.
#[derive(Debug)]
pub struct ParleyParagraphEngine {
    fonts: FontSet,
    analyzer: Analyzer,
    shaper: Shaper,
    cache: BTreeMap<ParagraphId, PhysicsCache>,
}

impl ParleyParagraphEngine {
    /// Creates a retained adapter from an immutable font snapshot.
    #[must_use]
    pub fn new(fonts: FontSet) -> Self {
        Self {
            fonts,
            analyzer: Analyzer::new(),
            shaper: Shaper::default(),
            cache: BTreeMap::new(),
        }
    }
}

impl ParagraphFormation for ParleyParagraphEngine {
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError> {
        validate_input_runs(&input)?;
        let paragraph = input.paragraph();
        let analyzed = self
            .cache
            .get(&paragraph)
            .is_none_or(|entry| entry.text.as_ref() != input.text());
        if analyzed {
            let analysis = analyze_text(&mut self.analyzer, input.text());
            let interaction_units = collect_analysis_units(input.text(), &analysis)?;
            if let Some(cache) = self.cache.get_mut(&paragraph) {
                cache.text = Arc::from(input.text());
                cache.analysis = analysis;
                cache.interaction_units = interaction_units;
                cache.shaping_styles.clear();
                cache.shaping_runs.clear();
                cache.shaped_text.clear();
                cache.formed_text.clear();
                cache.scripts.clear();
                cache.logical_clusters.clear();
                cache.formed_clusters.clear();
                cache.inline_flow_styles.clear();
                cache.inline_flow_runs.clear();
                cache.constraints = None;
                cache.line_plans.clear();
                cache.break_reshapes = 0;
            } else {
                self.cache.insert(
                    paragraph,
                    PhysicsCache {
                        text: Arc::from(input.text()),
                        analysis,
                        interaction_units,
                        shaping_styles: Vec::new(),
                        shaping_runs: Vec::new(),
                        shaped_text: ShapedText::new(),
                        formed_text: ShapedText::new(),
                        scripts: Vec::new(),
                        logical_clusters: Vec::new(),
                        formed_clusters: Vec::new(),
                        selected_clusters: 0,
                        inline_flow_styles: Vec::new(),
                        inline_flow_runs: Vec::new(),
                        constraints: None,
                        line_plans: Vec::new(),
                        break_reshapes: 0,
                    },
                );
            }
        }

        let physics = self
            .cache
            .get(&paragraph)
            .ok_or_else(PreparationError::invalid_output)?;
        let shaped = physics.shaping_styles != input.shaping_styles()
            || physics.shaping_runs != input.shaping_runs();
        if shaped {
            let cache = self
                .cache
                .get_mut(&paragraph)
                .ok_or_else(PreparationError::invalid_output)?;
            cache.shaping_styles.clear();
            cache.shaping_runs.clear();
            let selected_clusters = shape_paragraph(
                &mut self.shaper,
                &cache.analysis,
                &mut self.fonts,
                input.text(),
                input.shaping_styles(),
                input.shaping_runs(),
                &mut cache.shaped_text,
                &mut cache.scripts,
            )?;
            cache.shaping_styles = input.shaping_styles().to_vec();
            cache.shaping_runs = input.shaping_runs().to_vec();
            cache.selected_clusters = selected_clusters;
            cache.logical_clusters = collect_logical_clusters(input.text(), &cache.shaped_text)?;
            cache.formed_text.clear();
            cache.formed_clusters.clear();
            cache.line_plans.clear();
        }

        let physics = self
            .cache
            .get(&paragraph)
            .ok_or_else(PreparationError::invalid_output)?;
        let needs_formation = shaped
            || physics.inline_flow_styles != input.inline_flow_styles()
            || physics.inline_flow_runs != input.inline_flow_runs()
            || physics.constraints != Some(constraints);
        if needs_formation {
            let cache = self
                .cache
                .get_mut(&paragraph)
                .ok_or_else(PreparationError::invalid_output)?;
            cache.formed_text.clone_from(&cache.shaped_text);
            cache.formed_clusters.clone_from(&cache.logical_clusters);
            cache.break_reshapes = form_lines(
                &mut self.shaper,
                &cache.analysis,
                input.text(),
                &mut cache.formed_text,
                &mut cache.formed_clusters,
                input.inline_flow_styles(),
                input.inline_flow_runs(),
                constraints,
                &mut cache.line_plans,
            )?;
            cache.inline_flow_styles = input.inline_flow_styles().to_vec();
            cache.inline_flow_runs = input.inline_flow_runs().to_vec();
            cache.constraints = Some(constraints);
        }

        let physics = self
            .cache
            .get(&paragraph)
            .ok_or_else(PreparationError::invalid_output)?;
        if physics.formed_text.runs().len() != physics.scripts.len() {
            return Err(PreparationError::invalid_output());
        }
        let mut prepared_lines = Vec::with_capacity(physics.line_plans.len());
        let mut glyph_count = 0_u32;
        for plan in &physics.line_plans {
            let mut pieces = line_run_pieces(&physics.formed_text, plan.clusters.clone())?;
            reorder_visual_pieces(&physics.formed_text, &mut pieces);
            let prepared_units = lower_visual_units(
                input.text(),
                &physics.formed_text,
                &pieces,
                &physics.interaction_units,
                &plan.source,
                plan.reason == LineBreakReason::Mandatory,
            )?;
            let mut prepared_runs = Vec::with_capacity(pieces.len());
            for piece in pieces {
                let run = physics
                    .formed_text
                    .runs()
                    .get(piece.run)
                    .ok_or_else(PreparationError::invalid_output)?;
                let script = physics
                    .scripts
                    .get(piece.run)
                    .ok_or_else(PreparationError::invalid_output)?;
                let font = physics
                    .formed_text
                    .fonts()
                    .get(run.font_index)
                    .ok_or_else(PreparationError::invalid_output)?;
                let normalized_coords = physics
                    .formed_text
                    .normalized_coords()
                    .get(run.normalized_coords_range.clone())
                    .ok_or_else(PreparationError::invalid_output)?;
                let clusters = physics
                    .formed_text
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
                    &physics.analysis,
                    &physics.formed_text,
                    run,
                    piece.clusters.clone(),
                    input.paint_runs(),
                )?;
                glyph_count = glyph_count
                    .saturating_add(u32::try_from(prepared_glyphs.len()).unwrap_or(u32::MAX));
                let unrendered_source = unrendered_source(
                    input.text(),
                    &physics.analysis,
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
            prepared_lines.push(PreparedLine::try_new(
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
        let text_len =
            u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
        let movements = prepared_cursor_movements(&prepared_lines, text_len)?;
        let paragraph =
            PreparedParagraph::try_new(input.paragraph(), text_len, prepared_lines, movements)?;
        let work = FormationWork::new(
            analyzed,
            shaped,
            if shaped { physics.selected_clusters } else { 0 },
            if shaped {
                u32::try_from(physics.formed_text.runs().len()).unwrap_or(u32::MAX)
            } else {
                0
            },
            if shaped { glyph_count } else { 0 },
            if needs_formation {
                u32::try_from(physics.line_plans.len()).unwrap_or(u32::MAX)
            } else {
                0
            },
            if needs_formation {
                physics.break_reshapes
            } else {
                0
            },
        );
        Ok(ParagraphFormationOutput::new(paragraph, work))
    }

    fn release(&mut self, paragraph: ParagraphId) {
        self.cache.remove(&paragraph);
    }

    fn clear(&mut self) {
        self.cache.clear();
    }

    fn retained_entries(&self) -> Option<usize> {
        Some(self.cache.len())
    }
}

#[derive(Debug)]
struct PhysicsCache {
    text: Arc<str>,
    analysis: Analysis,
    interaction_units: Vec<Range<usize>>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    shaped_text: ShapedText,
    formed_text: ShapedText,
    scripts: Vec<[u8; 4]>,
    logical_clusters: Vec<LogicalCluster>,
    formed_clusters: Vec<LogicalCluster>,
    selected_clusters: u32,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    constraints: Option<ParagraphConstraints>,
    line_plans: Vec<LinePlan>,
    break_reshapes: u32,
}
