// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::Cell;
use core::fmt;
use core::ops::Range;

use fontique::{
    Attributes, Blob, Collection, CollectionOptions, FallbackKey, FontInfo, QueryFamily,
    QueryStatus, SourceCache, SourceId, SourceInfo, SourceKind, Synthesis,
};
use parley_core::{
    Analysis, AnalysisDataSources, AnalysisOptions, Analyzer, Boundary, FontInstance, ShapeOptions,
    ShapedText, Shaper,
    shape::{CharCluster, ClusterData, Status, Whitespace},
};
use underwood::adapter::{
    FontSynthesis, FormationWork, GlyphPaintCoverage, GlyphPaintSegment, InlineFlowRun,
    LineBreakReason, ParagraphConstraints, ParagraphFormation, ParagraphFormationOutput,
    ParagraphInput, PreparationError, PreparedGlyph, PreparedLine, PreparedParagraph, PreparedRun,
    ShapingRun,
};
use underwood::{
    FontData, FontFamilyName, FontVariation, GenericFamily, InlineFlowStyle, Language, ParagraphId,
    Rect, Script, ShapingStyle, Tag, Vec2,
};

/// Owned validated font bytes and a face within them.
#[derive(Clone)]
pub struct Font {
    diagnostic_name: Arc<str>,
    digest: u64,
    blob: Blob<u8>,
    index: u32,
    units_per_em: u16,
}

impl fmt::Debug for Font {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Font")
            .field("diagnostic_name", &self.diagnostic_name)
            .field("digest", &self.digest)
            .field("index", &self.index)
            .field("units_per_em", &self.units_per_em)
            .finish_non_exhaustive()
    }
}

impl Font {
    /// Copies the bytes and validates face zero in a font file or collection.
    pub fn from_bytes(diagnostic_name: &str, bytes: &[u8]) -> Result<Self, AdapterError> {
        let index = 0;
        let units_per_em = units_per_em(bytes, index)
            .ok_or_else(|| AdapterError::new(AdapterErrorKind::InvalidFont))?;
        let blob = Blob::from(bytes.to_vec());
        let source = SourceInfo::new(SourceId::new(), SourceKind::Memory(blob.clone()));
        FontInfo::from_source(source, index)
            .ok_or_else(|| AdapterError::new(AdapterErrorKind::InvalidFont))?;
        Ok(Self {
            diagnostic_name: Arc::from(diagnostic_name),
            digest: digest_bytes(bytes),
            blob,
            index,
            units_per_em,
        })
    }
}

/// Deterministic caller-supplied Fontique catalog for the headless adapter.
#[derive(Clone)]
pub struct FontSet {
    collection: Collection,
    source_cache: SourceCache,
    font_count: usize,
}

impl fmt::Debug for FontSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontSet")
            .field("font_count", &self.font_count)
            .finish_non_exhaustive()
    }
}

impl FontSet {
    /// Registers a non-empty set of memory fonts with system discovery disabled.
    pub fn try_from_fonts(fonts: impl IntoIterator<Item = Font>) -> Result<Self, AdapterError> {
        let fonts: Vec<_> = fonts.into_iter().collect();
        if fonts.is_empty() {
            return Err(AdapterError::new(AdapterErrorKind::EmptyFontSet));
        }
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let mut font_count = 0_usize;
        for font in fonts {
            let blob = font.blob;
            let registered = collection.register_fonts(blob.clone(), None);
            if registered.is_empty()
                || registered.iter().any(|(_, fonts)| {
                    fonts
                        .iter()
                        .any(|font| units_per_em(blob.as_ref(), font.index()).is_none())
                })
            {
                return Err(AdapterError::new(AdapterErrorKind::InvalidFont));
            }
            font_count = font_count.saturating_add(
                registered
                    .iter()
                    .map(|(_, fonts)| fonts.len())
                    .sum::<usize>(),
            );
        }
        Ok(Self {
            collection,
            source_cache: SourceCache::default(),
            font_count,
        })
    }

    /// Returns a copy whose generic family resolves to the supplied named families.
    pub fn with_generic_families(
        mut self,
        generic: GenericFamily,
        family_names: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, AdapterError> {
        let families = resolve_family_ids(&mut self.collection, family_names)?;
        self.collection
            .set_generic_families(generic, families.into_iter());
        Ok(self)
    }

    /// Returns a copy with named fallback families for one script and optional language.
    pub fn with_fallbacks(
        mut self,
        script: Script,
        language: Option<Language>,
        family_names: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, AdapterError> {
        let families = resolve_family_ids(&mut self.collection, family_names)?;
        if !self.collection.set_fallbacks(
            FallbackKey::new(script, language.as_ref()),
            families.into_iter(),
        ) {
            return Err(AdapterError::new(AdapterErrorKind::UnsupportedFallback));
        }
        Ok(self)
    }
}

fn resolve_family_ids(
    collection: &mut Collection,
    family_names: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<Vec<fontique::FamilyId>, AdapterError> {
    family_names
        .into_iter()
        .map(|name| {
            collection
                .family_id(name.as_ref())
                .ok_or_else(|| AdapterError::new(AdapterErrorKind::UnknownFamily))
        })
        .collect()
}

/// Immutable Unicode-data configuration for the compiled minimal path.
#[derive(Clone, Debug, Default)]
pub struct TextData {
    _private: (),
}

impl TextData {
    /// Returns the first-slice compiled minimal configuration.
    #[must_use]
    pub fn compiled_minimal() -> Self {
        Self::default()
    }
}

/// Retained Parley Core paragraph adapter.
#[derive(Debug)]
pub struct ParleyParagraphEngine {
    _data: TextData,
    fonts: FontSet,
    analyzer: Analyzer,
    shaper: Shaper,
    cache: Vec<PhysicsCache>,
}

impl ParleyParagraphEngine {
    /// Creates a retained adapter from immutable text data and fonts.
    pub fn new(data: TextData, fonts: FontSet) -> Result<Self, AdapterError> {
        Ok(Self {
            _data: data,
            fonts,
            analyzer: Analyzer::new(),
            shaper: Shaper::default(),
            cache: Vec::new(),
        })
    }
}

impl ParagraphFormation for ParleyParagraphEngine {
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError> {
        validate_input_runs(&input)?;
        let existing_index = self
            .cache
            .iter()
            .position(|entry| entry.paragraph == input.paragraph());
        let (cache_index, analyzed) = if let Some(index) = existing_index {
            if self.cache[index].text.as_ref() == input.text() {
                (index, false)
            } else {
                self.cache[index].text = Arc::from(input.text());
                self.cache[index].analysis = analyze_text(&mut self.analyzer, input.text());
                self.cache[index].shaping_styles.clear();
                self.cache[index].shaping_runs.clear();
                self.cache[index].shaped_text.clear();
                self.cache[index].formed_text.clear();
                self.cache[index].scripts.clear();
                self.cache[index].logical_clusters.clear();
                self.cache[index].formed_clusters.clear();
                self.cache[index].line_plans.clear();
                (index, true)
            }
        } else {
            self.cache.push(PhysicsCache {
                paragraph: input.paragraph(),
                text: Arc::from(input.text()),
                analysis: analyze_text(&mut self.analyzer, input.text()),
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
                max_inline_advance: 0,
                line_plans: Vec::new(),
                break_reshapes: 0,
            });
            (self.cache.len() - 1, true)
        };

        let shaped = self.cache[cache_index].shaping_styles != input.shaping_styles()
            || self.cache[cache_index].shaping_runs != input.shaping_runs();
        if shaped {
            self.cache[cache_index].shaping_styles.clear();
            self.cache[cache_index].shaping_runs.clear();
            let cache = &mut self.cache[cache_index];
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

        let needs_formation = shaped
            || self.cache[cache_index].inline_flow_styles != input.inline_flow_styles()
            || self.cache[cache_index].inline_flow_runs != input.inline_flow_runs()
            || self.cache[cache_index].max_inline_advance
                != constraints.max_inline_advance().to_bits();
        if needs_formation {
            let cache = &mut self.cache[cache_index];
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
                constraints.max_inline_advance(),
                &mut cache.line_plans,
            )?;
            cache.inline_flow_styles = input.inline_flow_styles().to_vec();
            cache.inline_flow_runs = input.inline_flow_runs().to_vec();
            cache.max_inline_advance = constraints.max_inline_advance().to_bits();
        }

        let physics = &self.cache[cache_index];
        if physics.formed_text.runs().len() != physics.scripts.len() {
            return Err(PreparationError::invalid_output());
        }
        let mut prepared_lines = Vec::with_capacity(physics.line_plans.len());
        let mut glyph_count = 0_u32;
        for plan in &physics.line_plans {
            let mut pieces = line_run_pieces(&physics.formed_text, plan.clusters.clone())?;
            reorder_visual_pieces(&physics.formed_text, &mut pieces);
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
                    .shaped_text
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
                    portable_synthesis(font.synthesis)?,
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
                prepared_runs,
            )?);
        }
        let text_len =
            u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
        let paragraph =
            PreparedParagraph::try_from_lines(input.paragraph(), text_len, prepared_lines)?;
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
}

#[derive(Debug)]
struct PhysicsCache {
    paragraph: ParagraphId,
    text: Arc<str>,
    analysis: Analysis,
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
    max_inline_advance: u64,
    line_plans: Vec<LinePlan>,
    break_reshapes: u32,
}

#[derive(Clone, Debug)]
struct LinePlan {
    clusters: Range<usize>,
    source: Range<usize>,
    reason: LineBreakReason,
    advance: f64,
    baseline: f64,
    height: f64,
    content_ascent: f64,
    content_descent: f64,
}

#[derive(Clone, Debug)]
struct LogicalCluster {
    run: usize,
    index: usize,
    source: Range<usize>,
    boundary: Boundary,
    source_char: char,
    whitespace: Whitespace,
    ligature_component: bool,
    advance: f64,
}

#[derive(Clone, Debug)]
struct RunPiece {
    run: usize,
    clusters: Range<usize>,
}

fn form_lines(
    shaper: &mut Shaper,
    analysis: &Analysis,
    text: &str,
    shaped_text: &mut ShapedText,
    clusters: &mut Vec<LogicalCluster>,
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
    max_inline_advance: f64,
    plans: &mut Vec<LinePlan>,
) -> Result<u32, PreparationError> {
    plans.clear();
    if text.is_empty() {
        return Ok(0);
    }
    if clusters.is_empty() {
        return Err(PreparationError::invalid_output());
    }

    let mut break_reshapes = 0_u32;
    let mut start = 0_usize;
    while start < clusters.len() {
        let choice = choose_line(clusters, start, max_inline_advance)?;
        let (end, advance, reshaped) = if choice.reason == LineBreakReason::Regular {
            commit_regular_break(
                shaper,
                analysis,
                text,
                shaped_text,
                clusters,
                start,
                choice.end,
                max_inline_advance,
            )?
        } else {
            (choice.end, choice.advance, false)
        };
        if reshaped {
            break_reshapes = break_reshapes.saturating_add(1);
        }
        plans.push(make_line_plan(
            shaped_text,
            clusters,
            inline_flow_styles,
            inline_flow_runs,
            start..end,
            choice.reason,
            advance,
            None,
        )?);
        start = end;
    }

    if plans
        .last()
        .is_some_and(|plan| plan.reason == LineBreakReason::Mandatory)
    {
        let previous = plans
            .last()
            .cloned()
            .ok_or_else(PreparationError::invalid_output)?;
        plans.push(make_line_plan(
            shaped_text,
            clusters,
            inline_flow_styles,
            inline_flow_runs,
            clusters.len()..clusters.len(),
            LineBreakReason::End,
            0.0,
            Some(&previous),
        )?);
    }
    Ok(break_reshapes)
}

#[derive(Clone, Copy, Debug)]
struct LineChoice {
    end: usize,
    reason: LineBreakReason,
    advance: f64,
}

fn choose_line(
    clusters: &[LogicalCluster],
    start: usize,
    max_inline_advance: f64,
) -> Result<LineChoice, PreparationError> {
    let mut index = start;
    let mut advance = 0.0_f64;
    let mut last_opportunity: Option<(usize, f64)> = None;
    while index < clusters.len() {
        let cluster = &clusters[index];
        if cluster.boundary == Boundary::Line && !cluster.ligature_component && index > start {
            last_opportunity = Some((index, advance));
        }

        let next_advance = advance + cluster.advance;
        if cluster.whitespace == Whitespace::Newline {
            advance = next_advance;
            index += 1;
            let cr_before_lf = cluster.source_char == '\r'
                && clusters
                    .get(index)
                    .is_some_and(|next| next.source_char == '\n');
            if cr_before_lf {
                continue;
            }
            return Ok(LineChoice {
                end: index,
                reason: LineBreakReason::Mandatory,
                advance,
            });
        }

        if next_advance > max_inline_advance
            && let Some((end, opportunity_advance)) = last_opportunity
        {
            return Ok(LineChoice {
                end,
                reason: LineBreakReason::Regular,
                advance: opportunity_advance,
            });
        }
        advance = next_advance;
        index += 1;
    }
    Ok(LineChoice {
        end: clusters.len(),
        reason: LineBreakReason::End,
        advance,
    })
}

fn commit_regular_break(
    shaper: &mut Shaper,
    analysis: &Analysis,
    text: &str,
    shaped_text: &mut ShapedText,
    clusters: &mut Vec<LogicalCluster>,
    start: usize,
    mut end: usize,
    max_inline_advance: f64,
) -> Result<(usize, f64, bool), PreparationError> {
    loop {
        let pos = clusters
            .get(end)
            .ok_or_else(PreparationError::invalid_output)?
            .source
            .start;
        let reshaped = !shaped_text.unsafe_break_region(pos).is_empty();
        if reshaped {
            shaper.apply_break(text, analysis, shaped_text, pos);
            *clusters = collect_logical_clusters(text, shaped_text)?;
        }
        let advance = clusters[start..end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum();
        if advance <= max_inline_advance {
            return Ok((end, advance, reshaped));
        }

        let previous = (start + 1..end).rev().find(|&index| {
            let cluster = &clusters[index];
            cluster.boundary == Boundary::Line && !cluster.ligature_component
        });
        let Some(previous) = previous else {
            return Ok((end, advance, reshaped));
        };
        if reshaped {
            shaper.apply_concat(text, analysis, shaped_text, pos);
            *clusters = collect_logical_clusters(text, shaped_text)?;
        }
        end = previous;
    }
}

fn collect_logical_clusters(
    text: &str,
    shaped_text: &ShapedText,
) -> Result<Vec<LogicalCluster>, PreparationError> {
    let mut clusters = Vec::with_capacity(shaped_text.clusters().len());
    for (run_index, run) in shaped_text.runs().iter().enumerate() {
        for cluster_index in run.clusters_range.clone() {
            let cluster = shaped_text
                .clusters()
                .get(cluster_index)
                .ok_or_else(PreparationError::invalid_output)?;
            let start = run
                .range
                .byte_range
                .start
                .checked_add(usize::from(cluster.text_offset))
                .ok_or_else(PreparationError::invalid_output)?;
            let end = start
                .checked_add(usize::from(cluster.text_len))
                .ok_or_else(PreparationError::invalid_output)?;
            if text.get(start..end).is_none() {
                return Err(PreparationError::invalid_output());
            }
            clusters.push(LogicalCluster {
                run: run_index,
                index: cluster_index,
                source: start..end,
                boundary: cluster.info.boundary(),
                source_char: cluster.info.source_char(),
                whitespace: cluster.info.whitespace(),
                ligature_component: cluster.is_ligature_component(),
                advance: f64::from(cluster.advance),
            });
        }
    }
    if !text.is_empty() && clusters.is_empty() {
        return Err(PreparationError::invalid_output());
    }
    Ok(clusters)
}

fn make_line_plan(
    shaped_text: &ShapedText,
    clusters: &[LogicalCluster],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
    logical_range: Range<usize>,
    reason: LineBreakReason,
    advance: f64,
    empty_metrics: Option<&LinePlan>,
) -> Result<LinePlan, PreparationError> {
    if logical_range.is_empty() {
        let metrics = empty_metrics.ok_or_else(PreparationError::invalid_output)?;
        let at = clusters.last().map_or(0, |cluster| cluster.source.end);
        return Ok(LinePlan {
            clusters: shaped_text.clusters().len()..shaped_text.clusters().len(),
            source: at..at,
            reason,
            advance,
            baseline: metrics.baseline,
            height: metrics.height,
            content_ascent: metrics.content_ascent,
            content_descent: metrics.content_descent,
        });
    }

    let first = clusters
        .get(logical_range.start)
        .ok_or_else(PreparationError::invalid_output)?;
    let last = clusters
        .get(logical_range.end - 1)
        .ok_or_else(PreparationError::invalid_output)?;
    let mut above = 0.0_f64;
    let mut below = 0.0_f64;
    let mut content_ascent = 0.0_f64;
    let mut content_descent = 0.0_f64;
    for cluster in &clusters[logical_range.clone()] {
        let run = shaped_text
            .runs()
            .get(cluster.run)
            .ok_or_else(PreparationError::invalid_output)?;
        let multiplier =
            inline_flow_multiplier(&cluster.source, inline_flow_styles, inline_flow_runs)?;
        let requested_height = f64::from(run.font_size) * f64::from(multiplier);
        let ascent = f64::from(run.font_metrics.ascent);
        let descent = f64::from(run.font_metrics.descent);
        let half_leading = (requested_height - (ascent + descent)) / 2.0;
        let run_above = ascent + half_leading;
        above = above.max(run_above);
        below = below.max(requested_height - run_above);
        content_ascent = content_ascent.max(ascent);
        content_descent = content_descent.max(descent);
    }
    Ok(LinePlan {
        clusters: first.index..last.index + 1,
        source: first.source.start..last.source.end,
        reason,
        advance,
        baseline: above,
        height: above + below,
        content_ascent,
        content_descent,
    })
}

fn inline_flow_multiplier(
    source: &Range<usize>,
    styles: &[InlineFlowStyle],
    runs: &[InlineFlowRun],
) -> Result<f32, PreparationError> {
    let mut multiplier = 0.0_f32;
    for run in runs {
        let bytes = run.bytes();
        if bytes.start as usize >= source.end || bytes.end as usize <= source.start {
            continue;
        }
        let style = styles
            .get(run.style().index())
            .ok_or_else(PreparationError::invalid_output)?;
        multiplier = multiplier.max(style.line_height().multiplier());
    }
    if multiplier <= 0.0 {
        return Err(PreparationError::invalid_output());
    }
    Ok(multiplier)
}

fn line_run_pieces(
    shaped_text: &ShapedText,
    clusters: Range<usize>,
) -> Result<Vec<RunPiece>, PreparationError> {
    let mut pieces = Vec::new();
    for (run_index, run) in shaped_text.runs().iter().enumerate() {
        let start = run.clusters_range.start.max(clusters.start);
        let end = run.clusters_range.end.min(clusters.end);
        if start < end {
            pieces.push(RunPiece {
                run: run_index,
                clusters: start..end,
            });
        }
    }
    if !clusters.is_empty()
        && pieces
            .iter()
            .map(|piece| piece.clusters.len())
            .sum::<usize>()
            != clusters.len()
    {
        return Err(PreparationError::invalid_output());
    }
    Ok(pieces)
}

fn reorder_visual_pieces(shaped_text: &ShapedText, pieces: &mut [RunPiece]) {
    let mut max_level = 0_u8;
    let mut lowest_odd_level = u8::MAX;
    for piece in pieces.iter() {
        let level = shaped_text.runs()[piece.run].bidi_level;
        max_level = max_level.max(level);
        if level & 1 != 0 {
            lowest_odd_level = lowest_odd_level.min(level);
        }
    }
    if lowest_odd_level == u8::MAX {
        return;
    }
    for level in (lowest_odd_level..=max_level).rev() {
        let mut start = 0_usize;
        while start < pieces.len() {
            if shaped_text.runs()[pieces[start].run].bidi_level < level {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < pieces.len() && shaped_text.runs()[pieces[end].run].bidi_level >= level {
                end += 1;
            }
            pieces[start..end].reverse();
            start = end;
        }
    }
}

fn analyze_text(analyzer: &mut Analyzer, text: &str) -> Analysis {
    let mut analysis = Analysis::new();
    analyzer.analyze(
        text,
        &AnalysisOptions {
            word_break: &[],
            line_break_override: None,
        },
        &mut analysis,
    );
    analysis
}

fn shape_paragraph(
    shaper: &mut Shaper,
    analysis: &Analysis,
    fonts: &mut FontSet,
    text: &str,
    shaping_styles: &[ShapingStyle],
    shaping_runs: &[ShapingRun],
    shaped_text: &mut ShapedText,
    scripts: &mut Vec<[u8; 4]>,
) -> Result<u32, PreparationError> {
    let analysis_data = AnalysisDataSources::new();
    shaped_text.clear();
    scripts.clear();
    let mut style_indices = Vec::with_capacity(text.chars().count());
    for run in shaping_runs {
        let index =
            u16::try_from(run.style().index()).map_err(|_| PreparationError::invalid_output())?;
        let range = run.bytes();
        let run_text = text
            .get(range.start as usize..range.end as usize)
            .ok_or_else(PreparationError::invalid_output)?;
        style_indices.extend(core::iter::repeat_n(index, run_text.chars().count()));
    }
    let selected_clusters = Cell::new(0_u32);

    let split_after =
        |range: parley_core::itemize::TextRange| split_item_after(&range, &style_indices);
    for item in analysis.itemize(text, split_after) {
        let style = &shaping_styles[usize::from(style_indices[item.range.char_range.start])];
        let script = item.script.to_bytes();
        let missing_font = Cell::new(false);
        let mut query = fonts.collection.query(&mut fonts.source_cache);
        query.set_families(query_families(style.font_families()));
        query.set_attributes(Attributes::new(
            style.font_width(),
            style.font_style(),
            style.font_weight(),
        ));
        let language = style.language();
        query.set_fallbacks(FallbackKey::new(item.script, language.as_ref()));
        let appended = shaper.shape_item(
            text,
            analysis,
            &item,
            &ShapeOptions {
                font_size: style.font_size(),
                language: style.language(),
                features: style.features(),
                variations: style.variations(),
                char_style_indices: &style_indices,
            },
            |cluster| match select_font(&mut query, cluster, &analysis_data) {
                Some(font) => {
                    selected_clusters.set(selected_clusters.get().saturating_add(1));
                    Some(FontInstance {
                        font: FontData::new(font.blob, font.index),
                        synthesis: font.synthesis,
                    })
                }
                None => {
                    missing_font.set(true);
                    None
                }
            },
            &analysis_data,
            shaped_text,
        );
        if missing_font.get() {
            shaped_text.clear();
            scripts.clear();
            return Err(PreparationError::missing_font());
        }
        scripts.extend(core::iter::repeat_n(script, appended.len()));
    }
    if !text.is_empty() && shaped_text.runs().is_empty() {
        scripts.clear();
        return Err(PreparationError::missing_font());
    }
    Ok(selected_clusters.get())
}

fn split_item_after(range: &parley_core::itemize::TextRange, style_indices: &[u16]) -> bool {
    style_indices[range.char_range.start] != style_indices[range.char_range.end]
        || range.byte_range.len() > usize::from(u16::MAX)
}

fn query_families<'a>(names: &'a [FontFamilyName<'static>]) -> Vec<QueryFamily<'a>> {
    names
        .iter()
        .map(|name| match name {
            FontFamilyName::Named(name) => QueryFamily::Named(name.as_ref()),
            FontFamilyName::Generic(generic) => QueryFamily::Generic(*generic),
        })
        .collect()
}

fn select_font(
    query: &mut fontique::Query<'_>,
    cluster: &mut CharCluster,
    data: &AnalysisDataSources,
) -> Option<fontique::QueryFont> {
    let mut selected = None;
    query.matches_with(|font| {
        let Some(charmap) = font.charmap() else {
            return QueryStatus::Continue;
        };
        let status = cluster.map(
            |character| charmap.map(character).is_some_and(|glyph| glyph != 0),
            data,
        );
        match status {
            Status::Complete => {
                selected = Some(font.clone());
                QueryStatus::Stop
            }
            Status::Keep => {
                selected = Some(font.clone());
                QueryStatus::Continue
            }
            Status::Discard => QueryStatus::Continue,
        }
    });
    selected
}

fn lower_glyphs(
    text: &str,
    analysis: &Analysis,
    shaped_text: &ShapedText,
    run: &parley_core::ShapedRun,
    cluster_range: Range<usize>,
    paint_runs: &[underwood::adapter::PaintRun],
) -> Result<Vec<PreparedGlyph>, PreparationError> {
    let clusters = shaped_text
        .clusters()
        .get(run.clusters_range.clone())
        .ok_or_else(PreparationError::invalid_output)?;
    let start = cluster_range
        .start
        .checked_sub(run.clusters_range.start)
        .ok_or_else(PreparationError::invalid_output)?;
    let end = cluster_range
        .end
        .checked_sub(run.clusters_range.start)
        .ok_or_else(PreparationError::invalid_output)?;
    if start >= end || end > clusters.len() {
        return Err(PreparationError::invalid_output());
    }
    let mut prepared = Vec::with_capacity(cluster_range.len());
    let mut lower_cluster = |index: usize| -> Result<(), PreparationError> {
        let cluster = clusters
            .get(index)
            .ok_or_else(PreparationError::invalid_output)?;
        if cluster.is_ligature_component() {
            return Ok(());
        }
        let source = cluster_source(run, clusters, index)?;
        if text
            .get(source.start as usize..source.end as usize)
            .is_none()
        {
            return Err(PreparationError::invalid_output());
        }
        if !source_contributes_to_shaping(text, analysis, &source)? {
            return Ok(());
        }
        lower_cluster_glyphs(shaped_text, run, cluster, |glyph| {
            let advance = Vec2::new(f64::from(glyph.advance), 0.0);
            let paint = paint_coverage(
                text,
                source.clone(),
                advance,
                run.font_size,
                paint_runs,
                run.bidi_level & 1 == 1,
            )?;
            prepared.push(PreparedGlyph::try_new(
                glyph.id,
                source.clone(),
                advance,
                Vec2::new(f64::from(glyph.x), -f64::from(glyph.y)),
                paint,
            )?);
            Ok(())
        })
    };
    if run.bidi_level & 1 == 1 {
        for index in (start..end).rev() {
            lower_cluster(index)?;
        }
    } else {
        for index in start..end {
            lower_cluster(index)?;
        }
    }
    Ok(prepared)
}

fn source_contributes_to_shaping(
    text: &str,
    analysis: &Analysis,
    source: &Range<u32>,
) -> Result<bool, PreparationError> {
    let start = source.start as usize;
    let end = source.end as usize;
    let before = text
        .get(..start)
        .ok_or_else(PreparationError::invalid_output)?;
    let source_text = text
        .get(start..end)
        .ok_or_else(PreparationError::invalid_output)?;
    let char_start = before.chars().count();
    let char_end = char_start
        .checked_add(source_text.chars().count())
        .ok_or_else(PreparationError::invalid_output)?;
    Ok(analysis
        .char_info()
        .get(char_start..char_end)
        .ok_or_else(PreparationError::invalid_output)?
        .iter()
        .any(|info| info.contributes_to_shaping()))
}

fn unrendered_source(
    text: &str,
    analysis: &Analysis,
    source: Range<usize>,
    glyphs: &[PreparedGlyph],
) -> Result<Vec<Range<u32>>, PreparationError> {
    let before = text
        .get(..source.start)
        .ok_or_else(PreparationError::invalid_output)?;
    let source_text = text
        .get(source.clone())
        .ok_or_else(PreparationError::invalid_output)?;
    let char_start = before.chars().count();
    let mut unrendered: Vec<Range<u32>> = Vec::new();
    for (index, (offset, character)) in source_text.char_indices().enumerate() {
        let start = source
            .start
            .checked_add(offset)
            .ok_or_else(PreparationError::invalid_output)?;
        let end = start
            .checked_add(character.len_utf8())
            .ok_or_else(PreparationError::invalid_output)?;
        let range = checked_source_range(&(start..end))?;
        if glyphs.iter().any(|glyph| {
            let glyph_source = glyph.source();
            glyph_source.start <= range.start && glyph_source.end >= range.end
        }) {
            continue;
        }
        let info = analysis
            .char_info()
            .get(char_start + index)
            .ok_or_else(PreparationError::invalid_output)?;
        if info.contributes_to_shaping() {
            return Err(PreparationError::invalid_output());
        }
        if let Some(previous) = unrendered.last_mut()
            && previous.end == range.start
        {
            previous.end = range.end;
        } else {
            unrendered.push(range);
        }
    }
    Ok(unrendered)
}

fn cluster_source(
    run: &parley_core::ShapedRun,
    clusters: &[ClusterData],
    index: usize,
) -> Result<Range<u32>, PreparationError> {
    let cluster = clusters
        .get(index)
        .ok_or_else(PreparationError::invalid_output)?;
    let run_start = run.range.byte_range.start;
    let mut start = run_start
        .checked_add(usize::from(cluster.text_offset))
        .ok_or_else(PreparationError::invalid_output)?;
    let mut end = start
        .checked_add(usize::from(cluster.text_len))
        .ok_or_else(PreparationError::invalid_output)?;
    if cluster.is_ligature_start() {
        if run.bidi_level & 1 == 1 {
            for component in clusters[..index].iter().rev() {
                if !component.is_ligature_component() {
                    break;
                }
                let component_start = run_start
                    .checked_add(usize::from(component.text_offset))
                    .ok_or_else(PreparationError::invalid_output)?;
                let component_end = component_start
                    .checked_add(usize::from(component.text_len))
                    .ok_or_else(PreparationError::invalid_output)?;
                if component_end != start {
                    return Err(PreparationError::invalid_output());
                }
                start = component_start;
            }
        } else {
            for component in clusters.iter().skip(index + 1) {
                if !component.is_ligature_component() {
                    break;
                }
                let component_start = run_start
                    .checked_add(usize::from(component.text_offset))
                    .ok_or_else(PreparationError::invalid_output)?;
                if component_start != end {
                    return Err(PreparationError::invalid_output());
                }
                end = end
                    .checked_add(usize::from(component.text_len))
                    .ok_or_else(PreparationError::invalid_output)?;
            }
        }
    }
    checked_source_range(&(start..end))
}

fn lower_cluster_glyphs(
    shaped_text: &ShapedText,
    run: &parley_core::ShapedRun,
    cluster: &ClusterData,
    mut lower: impl FnMut(parley_core::Glyph) -> Result<(), PreparationError>,
) -> Result<(), PreparationError> {
    if cluster.glyph_len == u8::MAX {
        return lower(parley_core::Glyph {
            id: cluster.glyph_offset,
            x: 0.0,
            y: 0.0,
            advance: cluster.advance,
        });
    }
    let start = run
        .glyphs_range
        .start
        .checked_add(cluster.glyph_offset as usize)
        .ok_or_else(PreparationError::invalid_output)?;
    let end = start
        .checked_add(usize::from(cluster.glyph_len))
        .ok_or_else(PreparationError::invalid_output)?;
    for glyph in shaped_text
        .glyphs()
        .get(start..end)
        .ok_or_else(PreparationError::invalid_output)?
    {
        lower(*glyph)?;
    }
    Ok(())
}

fn checked_source_range(range: &Range<usize>) -> Result<Range<u32>, PreparationError> {
    let start = u32::try_from(range.start).map_err(|_| PreparationError::invalid_output())?;
    let end = u32::try_from(range.end).map_err(|_| PreparationError::invalid_output())?;
    Ok(start..end)
}

fn portable_synthesis(synthesis: Synthesis) -> Result<FontSynthesis, PreparationError> {
    FontSynthesis::try_new(
        synthesis
            .variation_settings()
            .iter()
            .map(|(tag, value)| FontVariation::new(Tag::from_bytes(tag.to_be_bytes()), *value)),
        synthesis.embolden(),
        synthesis.skew(),
    )
}

fn paint_coverage(
    text: &str,
    source: Range<u32>,
    advance: Vec2,
    font_size: f32,
    paint_runs: &[underwood::adapter::PaintRun],
    rtl: bool,
) -> Result<GlyphPaintCoverage, PreparationError> {
    let source_start = source.start as usize;
    let source_end = source.end as usize;
    let source_text = text
        .get(source_start..source_end)
        .ok_or_else(PreparationError::unsupported_paint_coverage)?;
    let total_chars = source_text.chars().count();
    if total_chars == 0 {
        return Err(PreparationError::unsupported_paint_coverage());
    }
    let intersecting_runs = paint_runs
        .iter()
        .filter(|paint| {
            let bytes = paint.bytes();
            bytes.start < source.end && bytes.end > source.start
        })
        .count();
    if intersecting_runs > 1
        && (advance.x == 0.0
            || !source_text
                .chars()
                .all(|character| character.is_ascii_alphabetic()))
    {
        return Err(PreparationError::unsupported_paint_coverage());
    }
    let mut segments = Vec::new();
    let mut covered = source.start;
    let total_width = advance.x.abs();
    let mut prior_chars = 0_usize;
    for paint in paint_runs {
        let bytes = paint.bytes();
        let start = bytes.start.max(source.start);
        let end = bytes.end.min(source.end);
        if start >= end {
            continue;
        }
        if start != covered {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        let segment_text = text
            .get(start as usize..end as usize)
            .ok_or_else(PreparationError::unsupported_paint_coverage)?;
        let segment_chars = segment_text.chars().count();
        let next_chars = prior_chars + segment_chars;
        let first_fraction = prior_chars as f64 / total_chars as f64;
        let next_fraction = next_chars as f64 / total_chars as f64;
        let (x0, x1) = if rtl {
            (
                total_width * (1.0 - next_fraction),
                total_width * (1.0 - first_fraction),
            )
        } else {
            (total_width * first_fraction, total_width * next_fraction)
        };
        segments.push(GlyphPaintSegment::new(
            start..end,
            paint.slot(),
            Rect::new(x0, -f64::from(font_size), x1, f64::from(font_size) * 0.25),
        )?);
        covered = end;
        prior_chars = next_chars;
    }
    if covered != source.end || prior_chars != total_chars {
        return Err(PreparationError::unsupported_paint_coverage());
    }
    GlyphPaintCoverage::try_from_segments(segments)
}

fn validate_input_runs(input: &ParagraphInput<'_>) -> Result<(), PreparationError> {
    let text_len =
        u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
    validate_run_coverage(
        input,
        input.shaping_runs().iter().map(ShapingRun::bytes),
        text_len,
        PreparationError::invalid_output,
    )?;
    if input.shaping_styles().len() > usize::from(u16::MAX) + 1
        || input
            .shaping_runs()
            .iter()
            .any(|run| run.style().index() >= input.shaping_styles().len())
    {
        return Err(PreparationError::invalid_output());
    }
    validate_run_coverage(
        input,
        input.inline_flow_runs().iter().map(InlineFlowRun::bytes),
        text_len,
        PreparationError::invalid_output,
    )?;
    if input.inline_flow_styles().len() > usize::from(u16::MAX) + 1
        || input
            .inline_flow_runs()
            .iter()
            .any(|run| run.style().index() >= input.inline_flow_styles().len())
    {
        return Err(PreparationError::invalid_output());
    }
    validate_run_coverage(
        input,
        input.paint_runs().iter().map(|run| run.bytes()),
        text_len,
        PreparationError::unsupported_paint_coverage,
    )
}

fn validate_run_coverage(
    input: &ParagraphInput<'_>,
    ranges: impl IntoIterator<Item = Range<u32>>,
    text_len: u32,
    error: fn() -> PreparationError,
) -> Result<(), PreparationError> {
    let mut end = 0_u32;
    for range in ranges {
        if range.start != end
            || range.start >= range.end
            || range.end > text_len
            || input
                .text()
                .get(range.start as usize..range.end as usize)
                .is_none()
        {
            return Err(error());
        }
        end = range.end;
    }
    if end != text_len {
        return Err(error());
    }
    Ok(())
}

fn units_per_em(bytes: &[u8], face_index: u32) -> Option<u16> {
    let font_offset = if bytes.get(0..4)? == b"ttcf" {
        let index = usize::try_from(face_index).ok()?;
        read_u32(bytes, 12_usize.checked_add(index.checked_mul(4)?)?)? as usize
    } else if face_index == 0 {
        0
    } else {
        return None;
    };
    let table_count = usize::from(read_u16(bytes, font_offset.checked_add(4)?)?);
    let records = font_offset.checked_add(12)?;
    for index in 0..table_count {
        let record = records.checked_add(index.checked_mul(16)?)?;
        if bytes.get(record..record.checked_add(4)?)? == b"head" {
            let offset = read_u32(bytes, record.checked_add(8)?)? as usize;
            let units = read_u16(bytes, offset.checked_add(18)?)?;
            return (units != 0).then_some(units);
        }
    }
    None
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let data: [u8; 2] = bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_be_bytes(data))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let data: [u8; 4] = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(data))
}

fn digest_bytes(bytes: &[u8]) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        digest = (digest ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    digest
}

/// Stable category for adapter construction failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdapterErrorKind {
    /// Supplied bytes contain no usable first face or font metrics.
    InvalidFont,
    /// A font set contains no fonts.
    EmptyFontSet,
    /// A configured generic or fallback family is absent from the catalog.
    UnknownFamily,
    /// Fontique does not track the requested script and language fallback key.
    UnsupportedFallback,
}

/// Concrete adapter construction error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AdapterError {
    kind: AdapterErrorKind,
}

impl AdapterError {
    const fn new(kind: AdapterErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> AdapterErrorKind {
        self.kind
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Parley adapter construction failed: {:?}",
            self.kind
        )
    }
}

impl core::error::Error for AdapterError {}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use fontique::{Blob, Synthesis};
    use parley_core::{AnalysisDataSources, FontInstance, ShapeOptions, ShapedText, Shaper};

    use underwood::adapter::LineBreakReason as TestLineBreakReason;
    use underwood::{
        Brush, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth, FontFamily,
        GenericFamily, InlineFlowStyle, InlineRole, LayoutEngine, LineHeight, PaintSlot,
        PaintTable, ParagraphRole, SceneRequest, ShapingStyle, StyleMap,
    };
    use underwood::{Language, Script};

    use super::{
        AdapterErrorKind, Font, FontSet, ParleyParagraphEngine, TextData, analyze_text,
        choose_line, collect_logical_clusters, commit_regular_break, read_u16, read_u32,
        split_item_after,
    };

    const LATIN_FONT: &[u8] =
        include_bytes!("../../examples/headless/fonts/RobotoFlex-VariableFont.ttf");
    const ARABIC_FONT: &[u8] =
        include_bytes!("../../examples/headless/fonts/NotoKufiArabic-Regular.otf");

    fn shape_arabic(text: &str) -> (parley_core::Analysis, Shaper, ShapedText) {
        let analysis = analyze_text(&mut parley_core::Analyzer::new(), text);
        let data_sources = AnalysisDataSources::new();
        let font = FontInstance {
            font: underwood::FontData::new(Blob::from(ARABIC_FONT.to_vec()), 0),
            synthesis: Synthesis::default(),
        };
        let style_indices = vec![0; text.chars().count()];
        let mut shaper = Shaper::default();
        let mut shaped = ShapedText::new();
        for item in analysis.itemize(text, |_| false) {
            shaper.shape_item(
                text,
                &analysis,
                &item,
                &ShapeOptions {
                    font_size: 20.0,
                    language: None,
                    features: &[],
                    variations: &[],
                    char_style_indices: &style_indices,
                },
                |_| Some(font.clone()),
                &data_sources,
                &mut shaped,
            );
        }
        (analysis, shaper, shaped)
    }

    #[test]
    fn big_endian_readers_reject_short_input() {
        assert_eq!(read_u16(&[0x12, 0x34], 0), Some(0x1234));
        assert_eq!(read_u16(&[0x12], 0), None);
        assert_eq!(read_u32(&[0x12, 0x34, 0x56, 0x78], 0), Some(0x1234_5678));
        assert_eq!(read_u32(&[0x12, 0x34], 0), None);
    }

    #[test]
    fn catalog_configuration_rejects_unknown_and_untracked_families() {
        let unknown = FontSet::try_from_fonts([
            Font::from_bytes("latin", LATIN_FONT).expect("fixture font is valid")
        ])
        .expect("fixture catalog is valid")
        .with_generic_families(GenericFamily::SansSerif, ["Absent Family"])
        .expect_err("generic mappings must not silently omit absent families");
        assert_eq!(
            unknown.kind(),
            AdapterErrorKind::UnknownFamily,
            "unknown family configuration must retain a stable category"
        );

        let arabic = Language::parse("ar").expect("test language is valid");
        let unsupported = FontSet::try_from_fonts([
            Font::from_bytes("latin", LATIN_FONT).expect("fixture font is valid")
        ])
        .expect("fixture catalog is valid")
        .with_fallbacks(Script::from_bytes(*b"Latn"), Some(arabic), ["Roboto Flex"])
        .expect_err("untracked script-language pairs must not disappear");
        assert_eq!(
            unsupported.kind(),
            AdapterErrorKind::UnsupportedFallback,
            "unsupported fallback configuration must retain a stable category"
        );
    }

    #[test]
    fn control_only_paragraph_emits_no_phantom_glyph() {
        let mut document = Document::new(DocumentId::from_bytes(*b"shaped-control-1"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("test paragraph is valid");
        edit.append_text(paragraph, InlineRole::TEXT, "\n")
            .expect("test control source is valid");
        let published = edit.commit().expect("test edit is valid");

        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 16.0).expect("test style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let fonts = FontSet::try_from_fonts([
            Font::from_bytes("latin", LATIN_FONT).expect("fixture font is valid")
        ])
        .expect("fixture catalog is valid");
        let paragraphs = ParleyParagraphEngine::new(TextData::compiled_minimal(), fonts)
            .expect("test adapter is valid");
        let mut layout = LayoutEngine::new(paragraphs);
        let request = SceneRequest::new(
            FiniteWidth::new(100.0).expect("test width is finite"),
            &styles,
            &paint,
        );
        let output = layout
            .prepare(published.snapshot(), &request)
            .expect("control-only source must prepare without a phantom glyph");
        assert!(
            output.scene().fragments().is_empty(),
            "newline shaping must not manufacture renderable glyphs"
        );
        assert_eq!(
            output.work().shape().records(),
            0,
            "shape work must report the renderable glyph count"
        );
    }

    #[test]
    fn itemization_bounds_shaped_text_relative_offsets() {
        let text = "a".repeat(usize::from(u16::MAX) + 2);
        let analysis = analyze_text(&mut parley_core::Analyzer::new(), &text);
        let style_indices = vec![0; text.chars().count()];
        let items: Vec<_> = analysis
            .itemize(&text, |range| split_item_after(&range, &style_indices))
            .collect();
        assert_eq!(items.len(), 2, "the oversized item must split once");
        assert_eq!(items[0].range.byte_range, 0..usize::from(u16::MAX) + 1);
        assert_eq!(items[1].range.byte_range, text.len() - 1..text.len());
    }

    #[test]
    fn product_path_wraps_only_at_parley_line_boundaries() {
        let text = "alpha beta gamma";
        let (document, styles, paint) = fixture_document(text, 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(72.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("legal wrapping must form a scene");
        let lines = output.scene().lines();
        assert_eq!(lines.len(), 3, "legal opportunities must form three lines");
        assert_eq!(
            lines[0].break_reason(),
            underwood::adapter::LineBreakReason::Regular
        );
        assert_eq!(
            lines[1].break_reason(),
            underwood::adapter::LineBreakReason::Regular
        );
        assert_eq!(
            lines[2].break_reason(),
            underwood::adapter::LineBreakReason::End
        );
        assert_eq!(lines[0].sources()[0].bytes(), 0..6);
        assert_eq!(
            lines[1].sources()[0].bytes().start,
            u32::try_from(text.find("beta").expect("beta is present")).expect("fixture range fits")
        );
        assert_eq!(
            lines[2].sources()[0].bytes().start,
            u32::try_from(text.find("gamma").expect("gamma is present"))
                .expect("fixture range fits")
        );
    }

    #[test]
    fn product_path_coalesces_crlf_and_honors_mandatory_breaks() {
        let text = "a\r\nb\nc\u{2028}d\u{2029}e";
        let (document, styles, paint) = fixture_document(text, 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("mandatory breaks must form a scene");
        let lines = output.scene().lines();
        let ranges: Vec<_> = lines.iter().map(|line| line.sources()[0].bytes()).collect();
        assert_eq!(
            lines.len(),
            5,
            "CRLF, LF, LS, and PS form four breaks: {ranges:?}"
        );
        assert_eq!(lines[0].sources()[0].bytes(), 0..3, "CRLF stays together");
        assert!(
            lines[..4]
                .iter()
                .all(|line| line.break_reason() == underwood::adapter::LineBreakReason::Mandatory)
        );
        assert_eq!(
            lines[4].break_reason(),
            underwood::adapter::LineBreakReason::End
        );
        assert_eq!(
            lines.last().expect("final line exists").sources()[0]
                .bytes()
                .end,
            u32::try_from(text.len()).expect("fixture length fits")
        );
    }

    #[test]
    fn product_path_uses_font_metrics_for_the_baseline() {
        let (document, styles, paint) = fixture_document("Ag", 1.5);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("metric-backed formation must succeed");
        let line = &output.scene().lines()[0];
        assert_eq!(line.bounds().height(), 30.0);
        assert!(line.baseline() > line.bounds().y0 && line.baseline() < line.bounds().y1);
        assert_eq!(
            line.baseline(),
            output.scene().fragments()[0].glyphs()[0].position().y
        );
        assert_ne!(
            line.baseline() - line.bounds().y0,
            24.0,
            "the 80/20 split is gone"
        );
        assert!(line.content_ascent() > line.content_descent());
    }

    #[test]
    fn line_metrics_and_source_slices_span_mixed_semantic_leaves() {
        let mut document = Document::new(DocumentId::from_bytes(*b"mixed-leaf-test1"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph is valid");
        let small = edit
            .append_text(paragraph, InlineRole::TEXT, "small ")
            .expect("first fixture leaf is valid");
        let large = edit
            .append_text(paragraph, InlineRole::EMPHASIS, "BIG")
            .expect("second fixture leaf is valid");
        edit.commit().expect("fixture edit is valid");

        let small_style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("small shaping style is valid"),
            InlineFlowStyle::new(
                LineHeight::from_multiplier(1.2).expect("small line height is valid"),
            ),
            PaintSlot::new(0),
        );
        let large_style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 40.0)
                .expect("large shaping style is valid"),
            InlineFlowStyle::new(
                LineHeight::from_multiplier(1.5).expect("large line height is valid"),
            ),
            PaintSlot::new(0),
        );
        let mut styles = StyleMap::new(small_style);
        styles.set(large, large_style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("mixed leaf formation succeeds");
        let line = &output.scene().lines()[0];
        assert_eq!(line.sources().len(), 2);
        assert_eq!(line.sources()[0].text(), small);
        assert_eq!(line.sources()[0].bytes(), 0..6);
        assert_eq!(line.sources()[1].text(), large);
        assert_eq!(line.sources()[1].bytes(), 0..3);
        assert_eq!(line.bounds().height(), 60.0);
        assert!(
            output
                .scene()
                .fragments()
                .iter()
                .any(|fragment| fragment.font_size() == 20.0)
        );
        assert!(
            output
                .scene()
                .fragments()
                .iter()
                .any(|fragment| fragment.font_size() == 40.0)
        );
    }

    #[test]
    fn non_breaking_space_and_unbreakable_words_overflow_honestly() {
        for text in ["alpha\u{a0}beta", "supercalifragilisticexpialidocious"] {
            let (document, styles, paint) = fixture_document(text, 1.2);
            let mut engine = fixture_engine();
            let request = SceneRequest::new(
                FiniteWidth::new(10.0).expect("test width is valid"),
                &styles,
                &paint,
            );
            let output = engine
                .prepare(&document.snapshot(), &request)
                .expect("an unbreakable unit may overflow");
            assert_eq!(
                output.scene().lines().len(),
                1,
                "unbreakable source must not be split: {text:?}"
            );
            assert_eq!(
                output.scene().lines()[0].break_reason(),
                underwood::adapter::LineBreakReason::End
            );
            assert!(
                output.scene().lines()[0].bounds().width() > 10.0,
                "overflow must remain visible rather than report a false fit: {text:?}"
            );
        }
    }

    #[test]
    fn width_and_line_height_reform_without_reshaping() {
        let text = "alpha beta gamma";
        let (document, compact_styles, paint) = fixture_document(text, 1.2);
        let (_, spacious_styles, _) = fixture_document(text, 1.8);
        let mut engine = fixture_engine();
        let wide = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &compact_styles,
            &paint,
        );
        engine
            .prepare(&document.snapshot(), &wide)
            .expect("initial formation succeeds");

        let narrow = SceneRequest::new(
            FiniteWidth::new(72.0).expect("test width is valid"),
            &compact_styles,
            &paint,
        );
        let narrowed = engine
            .prepare(&document.snapshot(), &narrow)
            .expect("width-only formation succeeds");
        assert_eq!(narrowed.work().analysis().paragraphs(), 0);
        assert_eq!(narrowed.work().itemization().paragraphs(), 0);
        assert_eq!(narrowed.work().font_selection().paragraphs(), 0);
        assert_eq!(narrowed.work().shape().paragraphs(), 0);
        assert_eq!(narrowed.work().flow().paragraphs(), 1);

        let spacious = SceneRequest::new(
            FiniteWidth::new(72.0).expect("test width is valid"),
            &spacious_styles,
            &paint,
        );
        let respaced = engine
            .prepare(&document.snapshot(), &spacious)
            .expect("line-height-only formation succeeds");
        assert_eq!(respaced.work().analysis().paragraphs(), 0);
        assert_eq!(respaced.work().shape().paragraphs(), 0);
        assert_eq!(respaced.work().flow().paragraphs(), 1);
        assert!(
            respaced.scene().lines()[0].bounds().height()
                > narrowed.scene().lines()[0].bounds().height()
        );
    }

    #[test]
    fn legal_zero_width_break_reshapes_an_arabic_join() {
        let text = "سل\u{200b}ام";
        let break_at = u32::try_from(text.find("ام").expect("break suffix is present"))
            .expect("fixture range fits");
        let (document, styles, paint) = fixture_document(text, 1.2);
        let fonts = FontSet::try_from_fonts([
            Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid")
        ])
        .expect("fixture catalog is valid")
        .with_fallbacks(Script::from_bytes(*b"Arab"), None, ["Noto Kufi Arabic"])
        .expect("Arabic fallback is valid");
        let mut engine = LayoutEngine::new(
            ParleyParagraphEngine::new(TextData::compiled_minimal(), fonts)
                .expect("fixture adapter is valid"),
        );
        let wide = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let unbroken = engine
            .prepare(&document.snapshot(), &wide)
            .expect("unbroken shaping succeeds");
        let unbroken_glyphs: Vec<_> = unbroken
            .scene()
            .fragments()
            .iter()
            .flat_map(|fragment| fragment.glyphs())
            .map(|glyph| (glyph.id(), glyph.source().bytes()))
            .collect();

        let narrow = SceneRequest::new(
            FiniteWidth::new(25.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &narrow)
            .expect("the legal break reshapes its bounded cursive context");
        assert_eq!(output.work().analysis().paragraphs(), 0);
        assert_eq!(output.work().shape().paragraphs(), 0);
        assert_eq!(output.work().break_reshapes(), 1);
        let broken_glyphs: Vec<_> = output
            .scene()
            .fragments()
            .iter()
            .flat_map(|fragment| fragment.glyphs())
            .map(|glyph| (glyph.id(), glyph.source().bytes()))
            .collect();
        assert_ne!(
            broken_glyphs, unbroken_glyphs,
            "committing the break must change real Arabic glyph output"
        );
        assert_eq!(output.scene().lines().len(), 2);
        assert_eq!(output.scene().lines()[0].sources()[0].bytes(), 0..break_at);
        assert_eq!(
            output.scene().lines()[1].sources()[0].bytes(),
            break_at..u32::try_from(text.len()).expect("fixture range fits")
        );
        assert!(output.scene().fragments().iter().all(|fragment| {
            fragment.glyphs().iter().all(|glyph| {
                let source = glyph.source().bytes();
                source.end <= break_at || source.start >= break_at
            })
        }));
    }

    #[test]
    fn reshape_overflow_backs_up_and_restores_the_rejected_seam() {
        let text = "س سل\u{200b}ام";
        let pos = text.find("ام").expect("unsafe suffix exists");
        let (analysis, mut shaper, canonical) = shape_arabic(text);
        let mut formed = canonical.clone();
        let mut clusters =
            collect_logical_clusters(text, &formed).expect("canonical clusters are valid");
        let unsafe_end = clusters
            .iter()
            .position(|cluster| cluster.source.start == pos)
            .expect("unsafe break cluster exists");
        let prior_safe = (1..unsafe_end)
            .rev()
            .find(|&index| {
                let cluster = &clusters[index];
                cluster.boundary == parley_core::Boundary::Line && !cluster.ligature_component
            })
            .expect("fixture has an earlier legal break");
        let unbroken_advance: f64 = clusters[..unsafe_end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum();
        shaper.apply_break(text, &analysis, &mut formed, pos);
        let broken_clusters =
            collect_logical_clusters(text, &formed).expect("broken clusters are valid");
        let broken_advance: f64 = broken_clusters[..unsafe_end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum();
        assert!(
            broken_advance > unbroken_advance,
            "the fixture must make break shaping change fit"
        );
        shaper.apply_concat(text, &analysis, &mut formed, pos);
        clusters = collect_logical_clusters(text, &formed).expect("restored clusters are valid");

        let width = (unbroken_advance + broken_advance) * 0.5;
        let initial = choose_line(&clusters, 0, width).expect("initial selection succeeds");
        assert_eq!(initial.reason, TestLineBreakReason::Regular);
        assert_eq!(initial.end, unsafe_end, "clusters: {clusters:#?}");
        let (committed_end, committed_advance, committed_reshape) = commit_regular_break(
            &mut shaper,
            &analysis,
            text,
            &mut formed,
            &mut clusters,
            0,
            unsafe_end,
            width,
        )
        .expect("overflowing reshaped break backs up");
        assert_eq!(committed_end, prior_safe);
        assert!(committed_advance <= width);
        assert!(!committed_reshape, "the committed earlier seam is safe");
        assert_eq!(
            formed, canonical,
            "rejecting the unsafe seam must concat it before backing up"
        );
    }

    #[test]
    fn mixed_bidi_glyphs_are_visual_inside_a_logical_line() {
        let (document, styles, paint) = fixture_document("office مرحبا world", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("mixed bidi formation succeeds");
        let arabic: Vec<_> = output
            .scene()
            .fragments()
            .iter()
            .filter(|fragment| fragment.bidi_level() & 1 == 1)
            .map(|fragment| {
                let glyph = &fragment.glyphs()[0];
                (glyph.position().x, glyph.source().bytes().start)
            })
            .collect();
        assert!(arabic.len() > 1, "Arabic run must expose multiple glyphs");
        assert!(
            arabic.windows(2).all(|pair| pair[0].1 >= pair[1].1)
                && arabic.windows(2).any(|pair| pair[0].1 > pair[1].1),
            "RTL glyph records run in visual order opposite logical source: {arabic:?}"
        );
    }

    #[test]
    fn bidi_format_controls_remain_source_complete_without_phantom_glyphs() {
        let text = "office \u{2067}مرحبا\u{2069} world";
        let (document, styles, paint) = fixture_document(text, 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("bidi format controls must not become phantom glyphs or source gaps");
        assert_eq!(output.scene().lines().len(), 1);
        assert_eq!(
            output.scene().lines()[0].sources()[0].bytes(),
            0..u32::try_from(text.len()).expect("fixture length fits")
        );
        let isolate = u32::try_from(text.find('\u{2067}').expect("isolate exists"))
            .expect("fixture range fits");
        let pop = u32::try_from(text.find('\u{2069}').expect("pop isolate exists"))
            .expect("fixture range fits");
        assert!(output.scene().fragments().iter().all(|fragment| {
            fragment.glyphs().iter().all(|glyph| {
                let source = glyph.source().bytes();
                !((source.start <= isolate && source.end >= isolate + 3)
                    || (source.start <= pop && source.end >= pop + 3))
            })
        }));
    }

    fn fixture_engine() -> LayoutEngine {
        let fonts = FontSet::try_from_fonts([
            Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid"),
            Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid"),
        ])
        .expect("fixture catalog is valid")
        .with_fallbacks(Script::from_bytes(*b"Arab"), None, ["Noto Kufi Arabic"])
        .expect("Arabic fallback is valid");
        LayoutEngine::new(
            ParleyParagraphEngine::new(TextData::compiled_minimal(), fonts)
                .expect("fixture adapter is valid"),
        )
    }

    fn fixture_document(text: &str, line_height: f32) -> (Document, StyleMap, PaintTable) {
        let mut document = Document::new(DocumentId::from_bytes(*b"breaking-test-01"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph is valid");
        edit.append_text(paragraph, InlineRole::TEXT, text)
            .expect("fixture text is valid");
        edit.commit().expect("fixture edit is valid");
        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("fixture shaping style is valid"),
            InlineFlowStyle::new(
                LineHeight::from_multiplier(line_height).expect("fixture line height is valid"),
            ),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        (document, styles, paint)
    }
}
