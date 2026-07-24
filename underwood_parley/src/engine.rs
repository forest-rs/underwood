// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained paragraph-engine orchestration and cache ownership.
//!
//! This module owns formation invalidation and retained Parley physics; it
//! explicitly does not own line-breaking, shaping, lowering, or interaction
//! algorithms.

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::ops::Range;

use parley_engine::{Analysis, Analyzer, ShapedText, Shaper};
use underwood::adapter::{
    FormationWork, InlineFlowRun, LineBreakReason, LineShapingWork, ParagraphConstraints,
    ParagraphFormation, ParagraphFormationOutput, ParagraphInput, PreparationError, PreparedLine,
    PreparedParagraph, PreparedRun, ShapingRun,
};
use underwood::{InlineFlowStyle, ParagraphId, ParagraphStyle, ShapingStyle};

use crate::font::FontSet;
use crate::interaction::{collect_analysis_units, lower_visual_units, prepared_cursor_movements};
use crate::line_break::{
    FormedLine, LineFormationWork, LogicalCluster, collect_logical_clusters, form_lines,
    line_run_pieces, reorder_visual_pieces, update_line_metrics,
};
use crate::lowering::{checked_source_range, lower_glyphs, portable_synthesis, unrendered_source};
use crate::shaping::{analyze_text, shape_line, shape_paragraph, shaped_glyph_count};
use crate::validation::validate_input_runs;

/// Retained Parley Engine paragraph adapter.
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
        let analyzed = self.cache.get(&paragraph).is_none_or(|entry| {
            entry.text.as_ref() != input.text() || entry.paragraph_style != input.paragraph_style()
        });
        if analyzed {
            let analysis = analyze_text(
                &mut self.analyzer,
                input.text(),
                input.paragraph_style().base_direction(),
            );
            let interaction_units = collect_analysis_units(input.text(), &analysis)?;
            if let Some(cache) = self.cache.get_mut(&paragraph) {
                cache.text = Arc::from(input.text());
                cache.paragraph_style = input.paragraph_style();
                cache.analysis = analysis;
                cache.interaction_units = interaction_units;
                cache.shaping_styles.clear();
                cache.shaping_runs.clear();
                cache.style_indices.clear();
                cache.shaped_text.clear();
                cache.scripts.clear();
                cache.logical_clusters.clear();
                cache.shaped_glyphs = 0;
                cache.inline_flow_styles.clear();
                cache.inline_flow_runs.clear();
                cache.constraints = None;
                cache.formed_lines.clear();
                cache.line_work = LineFormationWork::default();
            } else {
                self.cache.insert(
                    paragraph,
                    PhysicsCache {
                        text: Arc::from(input.text()),
                        paragraph_style: input.paragraph_style(),
                        analysis,
                        interaction_units,
                        shaping_styles: Vec::new(),
                        shaping_runs: Vec::new(),
                        style_indices: Vec::new(),
                        shaped_text: ShapedText::new(),
                        scripts: Vec::new(),
                        logical_clusters: Vec::new(),
                        selected_clusters: 0,
                        shaped_glyphs: 0,
                        inline_flow_styles: Vec::new(),
                        inline_flow_runs: Vec::new(),
                        constraints: None,
                        formed_lines: Vec::new(),
                        line_work: LineFormationWork::default(),
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
                &mut cache.style_indices,
            )?;
            cache.shaping_styles = input.shaping_styles().to_vec();
            cache.shaping_runs = input.shaping_runs().to_vec();
            cache.selected_clusters = selected_clusters;
            cache.shaped_glyphs = shaped_glyph_count(&cache.shaped_text);
            cache.logical_clusters = collect_logical_clusters(input.text(), &cache.shaped_text)?;
            cache.formed_lines.clear();
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
            if !shaped && cache.constraints == Some(constraints) {
                update_line_metrics(
                    input.text(),
                    &mut cache.formed_lines,
                    input.inline_flow_styles(),
                    input.inline_flow_runs(),
                )?;
                cache.line_work = LineFormationWork::default();
            } else {
                let analysis = &cache.analysis;
                let shaped_text = &cache.shaped_text;
                let scripts = &cache.scripts;
                let logical_clusters = &cache.logical_clusters;
                let style_indices = &cache.style_indices;
                let formed_lines = &mut cache.formed_lines;
                cache.line_work = form_lines(
                    input.text(),
                    shaped_text,
                    scripts,
                    logical_clusters,
                    input.inline_flow_styles(),
                    input.inline_flow_runs(),
                    constraints,
                    formed_lines,
                    |source| {
                        shape_line(
                            &mut self.shaper,
                            analysis,
                            shaped_text,
                            input.text(),
                            input.shaping_styles(),
                            style_indices,
                            source,
                        )
                    },
                )?;
            }
            cache.inline_flow_styles = input.inline_flow_styles().to_vec();
            cache.inline_flow_runs = input.inline_flow_runs().to_vec();
            cache.constraints = Some(constraints);
        }

        let physics = self
            .cache
            .get(&paragraph)
            .ok_or_else(PreparationError::invalid_output)?;
        let mut prepared_lines = Vec::with_capacity(physics.formed_lines.len());
        for formed in &physics.formed_lines {
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
                &pieces,
                &physics.interaction_units,
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
                    &physics.analysis,
                    shaped_text,
                    run,
                    piece.clusters.clone(),
                    input.paint_runs(),
                )?;
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
                u32::try_from(physics.shaped_text.runs().len()).unwrap_or(u32::MAX)
            } else {
                0
            },
            if shaped { physics.shaped_glyphs } else { 0 },
            if needs_formation {
                u32::try_from(physics.formed_lines.len()).unwrap_or(u32::MAX)
            } else {
                0
            },
            if needs_formation {
                LineShapingWork::new(
                    physics.line_work.reshapes,
                    physics.line_work.resolved_clusters,
                    physics.line_work.shaped_runs,
                    physics.line_work.shaped_glyphs,
                )
            } else {
                LineShapingWork::default()
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
    paragraph_style: ParagraphStyle,
    analysis: Analysis,
    interaction_units: Vec<Range<usize>>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    style_indices: Vec<u16>,
    shaped_text: ShapedText,
    scripts: Vec<[u8; 4]>,
    logical_clusters: Vec<LogicalCluster>,
    selected_clusters: u32,
    shaped_glyphs: u32,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    constraints: Option<ParagraphConstraints>,
    formed_lines: Vec<FormedLine>,
    line_work: LineFormationWork,
}
