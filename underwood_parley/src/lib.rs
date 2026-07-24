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
    ClusterBoundary, ClusterWhitespace, FontSynthesis, FormationWork, GlyphPaintCoverage,
    InlineFlowRun, LineBreakReason, ParagraphConstraints, ParagraphFormation,
    ParagraphFormationOutput, ParagraphInput, PreparationError, PreparedCaret, PreparedClusterSide,
    PreparedCursorMovement, PreparedCursorStep, PreparedGlyph, PreparedInteractionSlice,
    PreparedInteractionUnit, PreparedLine, PreparedParagraph, PreparedRun, ShapingRun,
    TextAffinity,
};
use underwood::{
    FontData, FontFamilyName, FontVariation, GenericFamily, InlineFlowStyle, Language, ParagraphId,
    Script, ShapingStyle, Tag, Vec2,
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

    /// Returns a catalog with a fixed snapshot of platform fonts available for fallback.
    ///
    /// This operation is available only with the `system-fonts` feature. It is
    /// intended for native hosts whose users can enter scripts not covered by
    /// an application's bundled resources. Deterministic proofs and benchmarks
    /// should continue to use [`Self::try_from_fonts`] alone.
    ///
    /// The snapshot is loaded before the catalog enters a paragraph engine;
    /// this adapter does not observe later platform font-database changes.
    #[cfg(feature = "system-fonts")]
    #[must_use]
    pub fn with_system_fonts(mut self) -> Self {
        self.collection.load_system_fonts();
        self
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
                let analysis = analyze_text(&mut self.analyzer, input.text());
                let interaction_units = collect_analysis_units(input.text(), &analysis)?;
                self.cache[index].text = Arc::from(input.text());
                self.cache[index].analysis = analysis;
                self.cache[index].interaction_units = interaction_units;
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
            let analysis = analyze_text(&mut self.analyzer, input.text());
            let interaction_units = collect_analysis_units(input.text(), &analysis)?;
            self.cache.push(PhysicsCache {
                paragraph: input.paragraph(),
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
}

#[derive(Debug)]
struct PhysicsCache {
    paragraph: ParagraphId,
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

fn collect_analysis_units(
    text: &str,
    analysis: &Analysis,
) -> Result<Vec<Range<usize>>, PreparationError> {
    let mut starts = Vec::new();
    let mut characters = 0_usize;
    for ((byte, _), info) in text.char_indices().zip(analysis.char_info()) {
        characters += 1;
        if info.is_grapheme_start() {
            starts.push(byte);
        }
    }
    if characters != text.chars().count()
        || characters != analysis.char_info().len()
        || (!text.is_empty() && starts.first() != Some(&0))
    {
        return Err(PreparationError::invalid_output());
    }
    let mut units = Vec::with_capacity(starts.len());
    for (index, start) in starts.iter().copied().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(text.len());
        if start >= end || text.get(start..end).is_none() {
            return Err(PreparationError::invalid_output());
        }
        units.push(start..end);
    }
    Ok(units)
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

#[derive(Clone, Debug)]
struct VisualInteractionSlice {
    source: Range<usize>,
    advance: f64,
    bidi_level: u8,
    boundary: Boundary,
    whitespace: Whitespace,
}

fn lower_visual_units(
    text: &str,
    shaped_text: &ShapedText,
    pieces: &[RunPiece],
    interaction_units: &[Range<usize>],
    line_source: &Range<usize>,
    mandatory_line_end: bool,
) -> Result<Vec<PreparedInteractionUnit>, PreparationError> {
    let slice_count = pieces.iter().map(|piece| piece.clusters.len()).sum();
    let mut visual_slices = Vec::with_capacity(slice_count);
    for piece in pieces {
        let run = shaped_text
            .runs()
            .get(piece.run)
            .ok_or_else(PreparationError::invalid_output)?;
        if run.bidi_level & 1 == 1 {
            for index in piece.clusters.clone().rev() {
                visual_slices.push(lower_visual_slice(shaped_text, run, index)?);
            }
        } else {
            for index in piece.clusters.clone() {
                visual_slices.push(lower_visual_slice(shaped_text, run, index)?);
            }
        }
    }

    let expected_start = interaction_units.partition_point(|unit| unit.end <= line_source.start);
    let expected_end = interaction_units.partition_point(|unit| unit.start < line_source.end);
    let expected = expected_start..expected_end;
    if interaction_units[expected.clone()]
        .iter()
        .any(|source| line_source.start > source.start || source.end > line_source.end)
    {
        return Err(PreparationError::invalid_output());
    }
    let mut seen = alloc::vec![false; expected.len()];
    let mut prepared = Vec::with_capacity(expected.len());
    let mut current_owner = None;
    let mut current_slices = Vec::new();
    for slice in visual_slices {
        let owner = interaction_units
            .partition_point(|unit| unit.start <= slice.source.start)
            .checked_sub(1)
            .filter(|&index| slice.source.end <= interaction_units[index].end)
            .ok_or_else(PreparationError::invalid_output)?;
        if !expected.contains(&owner) {
            return Err(PreparationError::invalid_output());
        }
        if current_owner == Some(owner) {
            current_slices.push(slice);
            continue;
        }
        if let Some(previous) = current_owner {
            prepared.push(lower_prepared_unit(
                text,
                &interaction_units[previous],
                core::mem::take(&mut current_slices),
                mandatory_line_end && interaction_units[previous].end == line_source.end,
            )?);
        }
        if seen[owner - expected.start] {
            return Err(PreparationError::invalid_output());
        }
        seen[owner - expected.start] = true;
        current_owner = Some(owner);
        current_slices.push(slice);
    }
    if let Some(owner) = current_owner {
        prepared.push(lower_prepared_unit(
            text,
            &interaction_units[owner],
            current_slices,
            mandatory_line_end && interaction_units[owner].end == line_source.end,
        )?);
    }
    if seen.iter().any(|seen| !seen) {
        return Err(PreparationError::invalid_output());
    }
    Ok(prepared)
}

fn lower_visual_slice(
    shaped_text: &ShapedText,
    run: &parley_core::ShapedRun,
    index: usize,
) -> Result<VisualInteractionSlice, PreparationError> {
    let cluster = shaped_text
        .clusters()
        .get(index)
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
    Ok(VisualInteractionSlice {
        source: start..end,
        advance: f64::from(cluster.advance),
        bidi_level: run.bidi_level,
        boundary: cluster.info.boundary(),
        whitespace: cluster.info.whitespace(),
    })
}

fn lower_prepared_unit(
    text: &str,
    source: &Range<usize>,
    slices: Vec<VisualInteractionSlice>,
    mandatory_line_end: bool,
) -> Result<PreparedInteractionUnit, PreparationError> {
    let first = slices
        .iter()
        .min_by_key(|slice| slice.source.start)
        .ok_or_else(PreparationError::invalid_output)?;
    if first.source.start != source.start
        || slices
            .iter()
            .any(|slice| slice.bidi_level != first.bidi_level)
    {
        return Err(PreparationError::invalid_output());
    }
    let bidi_level = first.bidi_level;
    let boundary = first.boundary;
    let mut whitespace = Whitespace::None;
    for slice in &slices {
        if slice.whitespace == Whitespace::None {
            continue;
        }
        if whitespace != Whitespace::None && whitespace != slice.whitespace {
            return Err(PreparationError::invalid_output());
        }
        whitespace = slice.whitespace;
    }
    if mandatory_line_end
        && text
            .get(source.clone())
            .is_some_and(|unit| unit == "\r" || unit == "\n" || unit == "\r\n")
    {
        whitespace = Whitespace::Newline;
    }
    let source = checked_source_range(source)?;
    let (left, right) = if bidi_level & 1 == 1 {
        (
            PreparedClusterSide::new(source.end, TextAffinity::Upstream),
            PreparedClusterSide::new(source.start, TextAffinity::Downstream),
        )
    } else {
        (
            PreparedClusterSide::new(source.start, TextAffinity::Downstream),
            PreparedClusterSide::new(source.end, TextAffinity::Upstream),
        )
    };
    let slices = slices
        .into_iter()
        .map(|slice| {
            PreparedInteractionSlice::try_new(checked_source_range(&slice.source)?, slice.advance)
        })
        .collect::<Result<Vec<_>, PreparationError>>()?;
    PreparedInteractionUnit::try_new(
        source,
        slices,
        bidi_level,
        match boundary {
            Boundary::None => ClusterBoundary::None,
            Boundary::Word => ClusterBoundary::Word,
            Boundary::Line => ClusterBoundary::Line,
            Boundary::Mandatory => ClusterBoundary::Mandatory,
        },
        match whitespace {
            Whitespace::None => ClusterWhitespace::None,
            Whitespace::Space => ClusterWhitespace::Space,
            Whitespace::NoBreakSpace => ClusterWhitespace::NoBreakSpace,
            Whitespace::Tab => ClusterWhitespace::Tab,
            Whitespace::Newline => ClusterWhitespace::Newline,
        },
        left,
        right,
    )
}

#[derive(Clone, Debug)]
struct CursorCluster {
    source: Range<u32>,
    rtl: bool,
    line: usize,
    visual_offset: f64,
    advance: f64,
    end_of_line: bool,
    hard_line_end: bool,
    soft_line_end: bool,
}

fn prepared_cursor_movements(
    lines: &[PreparedLine],
    text_len: u32,
) -> Result<Vec<PreparedCursorMovement>, PreparationError> {
    let mut clusters = Vec::new();
    let mut positions = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let first = clusters.len();
        let mut visual_offset = 0.0;
        for (unit_index, unit) in line.units().iter().enumerate() {
            push_cursor_position(&mut positions, unit.left());
            push_cursor_position(&mut positions, unit.right());
            clusters.push(CursorCluster {
                source: unit.source(),
                rtl: unit.bidi_level() & 1 == 1,
                line: line_index,
                visual_offset,
                advance: unit.advance(),
                end_of_line: unit_index + 1 == line.units().len(),
                hard_line_end: unit_index + 1 == line.units().len()
                    && line.break_reason() == LineBreakReason::Mandatory,
                soft_line_end: false,
            });
            visual_offset += unit.advance();
        }
        if clusters.len() > first
            && line.break_reason() == LineBreakReason::Regular
            && let Some(last) = clusters.last_mut()
        {
            last.soft_line_end = true;
        }
        if line.units().is_empty() {
            let source = line.source();
            push_cursor_position(
                &mut positions,
                PreparedClusterSide::new(
                    source.start,
                    if source.start == 0 {
                        TextAffinity::Downstream
                    } else {
                        TextAffinity::Upstream
                    },
                ),
            );
        }
    }
    if positions.is_empty() && text_len == 0 {
        positions.push(PreparedClusterSide::new(0, TextAffinity::Downstream));
    }
    let mut movements = Vec::new();
    let mut index = 0;
    while index < positions.len() {
        let position = positions[index];
        let movement = PreparedCursorMovement::new(
            position,
            prepared_cursor_caret(lines, &clusters, position)?,
            previous_visual_cursor(&clusters, text_len, position)?,
            next_visual_cursor(&clusters, text_len, position)?,
            previous_logical_cursor(&clusters, text_len, position)?,
            next_logical_cursor(&clusters, text_len, position)?,
        );
        for step in [
            movement.previous_visual(),
            movement.next_visual(),
            movement.previous_logical(),
            movement.next_logical(),
        ]
        .into_iter()
        .flatten()
        {
            push_cursor_position(&mut positions, step.target());
        }
        movements.push(movement);
        index += 1;
    }
    Ok(movements)
}

fn prepared_cursor_caret(
    lines: &[PreparedLine],
    clusters: &[CursorCluster],
    position: PreparedClusterSide,
) -> Result<PreparedCaret, PreparationError> {
    let [left, right] = visual_cursor_clusters(clusters, position);
    let placement = match (left, right) {
        (Some(left), Some(right)) => {
            let left_cluster = &clusters[left];
            if left_cluster.end_of_line {
                if left_cluster.soft_line_end {
                    if left_cluster.rtl && position.affinity() == TextAffinity::Downstream
                        || !left_cluster.rtl && position.affinity() == TextAffinity::Upstream
                    {
                        cursor_cluster_placement(left_cluster, true)
                    } else {
                        cursor_cluster_placement(&clusters[right], false)
                    }
                } else if left_cluster.hard_line_end {
                    cursor_cluster_placement(&clusters[right], false)
                } else {
                    cursor_cluster_placement(left_cluster, true)
                }
            } else {
                cursor_cluster_placement(left_cluster, true)
            }
        }
        (Some(left), None) if clusters[left].hard_line_end => last_line_placement(lines),
        (Some(left), _) => cursor_cluster_placement(&clusters[left], true),
        (_, Some(right)) => cursor_cluster_placement(&clusters[right], false),
        _ => last_line_placement(lines),
    };
    PreparedCaret::try_new(
        u32::try_from(placement.0).map_err(|_| PreparationError::invalid_output())?,
        placement.1,
    )
}

fn cursor_cluster_placement(cluster: &CursorCluster, at_end: bool) -> (usize, f64) {
    (
        cluster.line,
        cluster.visual_offset + if at_end { cluster.advance } else { 0.0 },
    )
}

fn last_line_placement(lines: &[PreparedLine]) -> (usize, f64) {
    (lines.len().saturating_sub(1), 0.0)
}

fn push_cursor_position(positions: &mut Vec<PreparedClusterSide>, position: PreparedClusterSide) {
    if !positions.contains(&position) {
        positions.push(position);
    }
}

fn previous_visual_cursor(
    clusters: &[CursorCluster],
    text_len: u32,
    position: PreparedClusterSide,
) -> Result<Option<PreparedCursorStep>, PreparationError> {
    let [left, right] = visual_cursor_clusters(clusters, position);
    if let (Some(left), Some(right)) = (left, right)
        && clusters[left].soft_line_end
    {
        if clusters[left].rtl && position.affinity() == TextAffinity::Upstream {
            let index = if clusters[right].rtl {
                clusters[left].source.start
            } else {
                clusters[left].source.end
            };
            return normalize_cursor(clusters, text_len, index, TextAffinity::Downstream)
                .map(|target| Some(PreparedCursorStep::new(target, None)));
        } else if !clusters[left].rtl && position.affinity() == TextAffinity::Downstream {
            let index = if clusters[right].rtl {
                clusters[right].source.end
            } else {
                clusters[right].source.start
            };
            return normalize_cursor(clusters, text_len, index, TextAffinity::Upstream)
                .map(|target| Some(PreparedCursorStep::new(target, None)));
        }
    }
    let Some(left) = left else {
        return Ok(None);
    };
    let cluster = &clusters[left];
    let index = if cluster.rtl {
        cluster.source.end
    } else {
        cluster.source.start
    };
    let source = cluster.source.clone();
    normalize_cursor(
        clusters,
        text_len,
        index,
        affinity_for_visual_direction(cluster.rtl, false),
    )
    .map(|target| Some(PreparedCursorStep::new(target, Some(source))))
}

fn next_visual_cursor(
    clusters: &[CursorCluster],
    text_len: u32,
    position: PreparedClusterSide,
) -> Result<Option<PreparedCursorStep>, PreparationError> {
    let [left, right] = visual_cursor_clusters(clusters, position);
    if let (Some(left), Some(right)) = (left, right) {
        if clusters[left].soft_line_end {
            if clusters[left].rtl && position.affinity() == TextAffinity::Downstream {
                let index = if clusters[right].rtl {
                    clusters[right].source.end
                } else {
                    clusters[right].source.start
                };
                return normalize_cursor(clusters, text_len, index, TextAffinity::Upstream)
                    .map(|target| Some(PreparedCursorStep::new(target, None)));
            } else if !clusters[left].rtl && position.affinity() == TextAffinity::Upstream {
                let index = if clusters[right].rtl {
                    clusters[right].source.end
                } else {
                    clusters[right].source.start
                };
                return normalize_cursor(clusters, text_len, index, TextAffinity::Downstream)
                    .map(|target| Some(PreparedCursorStep::new(target, None)));
            }
        }
        let source = clusters[right].source.clone();
        return cursor_after_visual_cluster(clusters, text_len, right)
            .map(|target| Some(PreparedCursorStep::new(target, Some(source))));
    }
    right.map_or(Ok(None), |right| {
        let source = clusters[right].source.clone();
        cursor_after_visual_cluster(clusters, text_len, right)
            .map(Some)
            .map(|target| target.map(|target| PreparedCursorStep::new(target, Some(source))))
    })
}

fn cursor_after_visual_cluster(
    clusters: &[CursorCluster],
    text_len: u32,
    index: usize,
) -> Result<PreparedClusterSide, PreparationError> {
    let cluster = &clusters[index];
    let offset = if cluster.rtl {
        cluster.source.start
    } else {
        cluster.source.end
    };
    normalize_cursor(
        clusters,
        text_len,
        offset,
        affinity_for_visual_direction(cluster.rtl, true),
    )
}

fn previous_logical_cursor(
    clusters: &[CursorCluster],
    text_len: u32,
    position: PreparedClusterSide,
) -> Result<Option<PreparedCursorStep>, PreparationError> {
    upstream_cursor_cluster(clusters, position.offset()).map_or(Ok(None), |index| {
        let source = clusters[index].source.clone();
        normalize_cursor(
            clusters,
            text_len,
            clusters[index].source.start,
            TextAffinity::Downstream,
        )
        .map(|target| Some(PreparedCursorStep::new(target, Some(source))))
    })
}

fn next_logical_cursor(
    clusters: &[CursorCluster],
    text_len: u32,
    position: PreparedClusterSide,
) -> Result<Option<PreparedCursorStep>, PreparationError> {
    downstream_cursor_cluster(clusters, position.offset()).map_or(Ok(None), |index| {
        let source = clusters[index].source.clone();
        normalize_cursor(
            clusters,
            text_len,
            clusters[index].source.end,
            TextAffinity::Upstream,
        )
        .map(|target| Some(PreparedCursorStep::new(target, Some(source))))
    })
}

fn normalize_cursor(
    clusters: &[CursorCluster],
    text_len: u32,
    index: u32,
    affinity: TextAffinity,
) -> Result<PreparedClusterSide, PreparationError> {
    if index > text_len {
        return Err(PreparationError::invalid_output());
    }
    if let Some(cluster) = downstream_cursor_cluster(clusters, index) {
        let index = clusters[cluster].source.start;
        Ok(PreparedClusterSide::new(
            index,
            if index == 0 {
                TextAffinity::Downstream
            } else {
                affinity
            },
        ))
    } else {
        Ok(PreparedClusterSide::new(text_len, TextAffinity::Upstream))
    }
}

fn visual_cursor_clusters(
    clusters: &[CursorCluster],
    position: PreparedClusterSide,
) -> [Option<usize>; 2] {
    let upstream = upstream_cursor_cluster(clusters, position.offset());
    let downstream = downstream_cursor_cluster(clusters, position.offset());
    if position.affinity() == TextAffinity::Upstream {
        if let Some(cluster) = upstream {
            if clusters[cluster].rtl {
                [cluster.checked_sub(1), Some(cluster)]
            } else {
                [Some(cluster), next_visual_cluster(clusters, cluster)]
            }
        } else if let Some(cluster) = downstream {
            if clusters[cluster].rtl {
                [None, Some(cluster)]
            } else {
                [Some(cluster), None]
            }
        } else {
            [None, None]
        }
    } else if let Some(cluster) = downstream {
        if clusters[cluster].rtl {
            [Some(cluster), next_visual_cluster(clusters, cluster)]
        } else {
            [cluster.checked_sub(1), Some(cluster)]
        }
    } else if let Some(cluster) = upstream {
        if clusters[cluster].rtl {
            [None, Some(cluster)]
        } else {
            [Some(cluster), None]
        }
    } else {
        [None, None]
    }
}

fn next_visual_cluster(clusters: &[CursorCluster], index: usize) -> Option<usize> {
    index.checked_add(1).filter(|next| *next < clusters.len())
}

fn upstream_cursor_cluster(clusters: &[CursorCluster], offset: u32) -> Option<usize> {
    clusters
        .iter()
        .position(|cluster| cluster.source.start < offset && offset <= cluster.source.end)
}

fn downstream_cursor_cluster(clusters: &[CursorCluster], offset: u32) -> Option<usize> {
    clusters
        .iter()
        .position(|cluster| cluster.source.start <= offset && offset < cluster.source.end)
}

const fn affinity_for_visual_direction(rtl: bool, moving_right: bool) -> TextAffinity {
    match (rtl, moving_right) {
        (true, true) | (false, false) => TextAffinity::Downstream,
        _ => TextAffinity::Upstream,
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
            let paint = paint_coverage(source.clone(), paint_runs)?;
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
    source: Range<u32>,
    paint_runs: &[underwood::adapter::PaintRun],
) -> Result<GlyphPaintCoverage, PreparationError> {
    let mut matching = paint_runs.iter().filter(|paint| {
        let bytes = paint.bytes();
        bytes.start < source.end && bytes.end > source.start
    });
    let paint = matching
        .next()
        .ok_or_else(PreparationError::unsupported_paint_coverage)?;
    if matching.next().is_some()
        || paint.bytes().start > source.start
        || paint.bytes().end < source.end
    {
        return Err(PreparationError::unsupported_paint_coverage());
    }
    GlyphPaintCoverage::whole(source, paint.slot())
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
    use parley_core::{FontInstance, ShapeOptions, ShapedText, Shaper};

    use underwood::adapter::{
        ClusterBoundary, ClusterWhitespace, FontSynthesis, FormationWork, GlyphPaintCoverage,
        LineBreakReason as TestLineBreakReason, ParagraphConstraints, ParagraphFormation,
        ParagraphFormationOutput, PreparationErrorKind, PreparedClusterSide, PreparedGlyph,
        PreparedInteractionSlice, PreparedInteractionUnit, PreparedLine, PreparedParagraph,
        PreparedRun,
    };
    use underwood::{
        Brush, Color, CompositionId, CompositionUpdate, ComputedInlineStyle, Document, DocumentId,
        EditErrorKind, EditableSurface, EditableSurfaceElement, FiniteWidth, FontData, FontFamily,
        FontWeight, GenericFamily, InlineFlowStyle, InlineRole, LayoutEngine, LineHeight,
        PaintSlot, PaintTable, ParagraphRole, Point, ProjectedTextPosition, ProjectedTextSource,
        SceneRequest, SelectionErrorKind, ShapingStyle, SnapshotTextUnit, StyleMap,
        SurfaceErrorKind, SurfaceTextEncoding, TextAffinity, TextMovement, TextScene,
        TextSelectionMode, Vec2,
    };
    use underwood::{Language, Script};

    use super::{
        AdapterErrorKind, Font, FontSet, ParleyParagraphEngine, TextData, analyze_text,
        choose_line, collect_analysis_units, collect_logical_clusters, commit_regular_break,
        read_u16, read_u32, split_item_after,
    };

    const LATIN_FONT: &[u8] =
        include_bytes!("../../examples/headless/fonts/RobotoFlex-VariableFont.ttf");
    const ARABIC_FONT: &[u8] =
        include_bytes!("../../examples/headless/fonts/NotoKufiArabic-Regular.otf");

    #[derive(Debug)]
    struct AnalysisCursorProof;

    impl ParagraphFormation for AnalysisCursorProof {
        fn form(
            &mut self,
            input: underwood::adapter::ParagraphInput<'_>,
            _constraints: ParagraphConstraints,
        ) -> Result<ParagraphFormationOutput, underwood::adapter::PreparationError> {
            let analysis = analyze_text(&mut parley_core::Analyzer::new(), input.text());
            let units = collect_analysis_units(input.text(), &analysis)?;
            let mut prepared_units = Vec::with_capacity(units.len());
            let mut glyphs = Vec::with_capacity(units.len());
            for (id, source) in units.into_iter().enumerate() {
                let source = super::checked_source_range(&source)?;
                prepared_units.push(PreparedInteractionUnit::try_new(
                    source.clone(),
                    [PreparedInteractionSlice::try_new(source.clone(), 1.0)?],
                    0,
                    ClusterBoundary::None,
                    ClusterWhitespace::None,
                    PreparedClusterSide::new(source.start, TextAffinity::Downstream),
                    PreparedClusterSide::new(source.end, TextAffinity::Upstream),
                )?);
                let slot = input
                    .paint_runs()
                    .iter()
                    .find(|run| {
                        let bytes = run.bytes();
                        bytes.start <= source.start && source.end <= bytes.end
                    })
                    .ok_or_else(underwood::adapter::PreparationError::invalid_output)?
                    .slot();
                let paint = GlyphPaintCoverage::whole(source.clone(), slot)?;
                glyphs.push(PreparedGlyph::try_new(
                    u32::try_from(id).unwrap_or(u32::MAX),
                    source,
                    Vec2::new(1.0, 0.0),
                    Vec2::ZERO,
                    paint,
                )?);
            }
            let source = 0..u32::try_from(input.text().len())
                .map_err(|_| underwood::adapter::PreparationError::invalid_output())?;
            let unit_count = u32::try_from(prepared_units.len())
                .map_err(|_| underwood::adapter::PreparationError::invalid_output())?;
            let advance = prepared_units.len() as f64;
            let run = PreparedRun::try_new(
                source.clone(),
                0,
                *b"Zyyy",
                FontData::new(Blob::from(vec![0_u8]), 0),
                16.0,
                FontSynthesis::default(),
                [],
                [],
                glyphs,
            )?;
            let line = PreparedLine::try_new(
                source.clone(),
                TestLineBreakReason::End,
                advance,
                0.8,
                1.0,
                0.8,
                0.2,
                prepared_units,
                [run],
            )?;
            let movements =
                super::prepared_cursor_movements(core::slice::from_ref(&line), source.end)?;
            let paragraph =
                PreparedParagraph::try_new(input.paragraph(), source.end, [line], movements)?;
            Ok(ParagraphFormationOutput::new(
                paragraph,
                FormationWork::new(true, false, unit_count, 1, unit_count, 1, 0),
            ))
        }
    }

    fn shape_arabic(text: &str) -> (parley_core::Analysis, Shaper, ShapedText) {
        let analysis = analyze_text(&mut parley_core::Analyzer::new(), text);
        let font = FontInstance {
            font: FontData::new(Blob::from(ARABIC_FONT.to_vec()), 0),
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
    fn analysis_units_lock_extended_grapheme_trap_corpus() {
        for (name, text, expected) in [
            (
                "decomposed",
                "e\u{301}",
                core::iter::once(0..3).collect::<Vec<_>>(),
            ),
            (
                "precomposed",
                "é",
                core::iter::once(0..2).collect::<Vec<_>>(),
            ),
            ("crlf", "\r\n", core::iter::once(0..2).collect::<Vec<_>>()),
            (
                "emoji-zwj",
                "👩\u{200d}💻",
                core::iter::once(0..11).collect::<Vec<_>>(),
            ),
            (
                "regional-indicator",
                "🇺🇳",
                core::iter::once(0..8).collect::<Vec<_>>(),
            ),
            (
                "spacing-mark",
                "क\u{93e}",
                core::iter::once(0..6).collect::<Vec<_>>(),
            ),
        ] {
            let analysis = analyze_text(&mut parley_core::Analyzer::new(), text);
            assert_eq!(
                collect_analysis_units(text, &analysis)
                    .expect("Parley analysis must expose complete grapheme units"),
                expected,
                "{name} must remain one interaction unit"
            );
        }
    }

    #[test]
    fn unbundled_grapheme_corpus_drives_complete_movements_and_transactions() {
        for (name, text) in [
            ("emoji-zwj", "👩\u{200d}💻"),
            ("regional-indicator", "🇺🇳"),
            ("spacing-mark", "क\u{93e}"),
        ] {
            let analysis = analyze_text(&mut parley_core::Analyzer::new(), text);
            let units = collect_analysis_units(text, &analysis)
                .expect("Parley analysis must expose complete grapheme units");
            assert_eq!(units.len(), 1, "{name} must remain one interaction unit");
            let mut document = Document::new(DocumentId::from_bytes(*b"unbundled-egc-01"));
            let mut edit = document.edit();
            let paragraph = edit
                .append_paragraph(ParagraphRole::BODY)
                .expect("the proof paragraph is valid");
            let leaf = edit
                .append_text(paragraph, InlineRole::TEXT, text)
                .expect("the proof source is valid");
            edit.commit().expect("the proof document is valid");
            let style = ComputedInlineStyle::new(
                ShapingStyle::new(FontFamily::named("proof"), 16.0)
                    .expect("the proof style is valid"),
                InlineFlowStyle::default(),
                PaintSlot::new(0),
            );
            let styles = StyleMap::new(style);
            let paints = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
            let request = SceneRequest::new(
                FiniteWidth::new(100.0).expect("the proof width is valid"),
                &styles,
                &paints,
            );
            let output = LayoutEngine::new(AnalysisCursorProof)
                .prepare(&document.snapshot(), &request)
                .expect("Parley analysis boundaries must prepare through the public scene path");
            let scene = output.scene();
            let y = scene.lines()[0].bounds().center().y;
            let start = *scene
                .hit_test_closest(Point::new(-100.0, y))
                .expect("the unit start must resolve")
                .position();
            let end = *scene
                .hit_test_closest(Point::new(100.0, y))
                .expect("the unit end must resolve")
                .position();
            let forward = scene
                .selection_set([scene
                    .collapsed_selection(&start)
                    .expect("the unit start must be a caret")])
                .and_then(|selection| {
                    scene.move_selections(&selection, TextMovement::NextLogical, true)
                })
                .expect("the unit must expose one forward logical selection");
            let backward = scene
                .selection_set([scene
                    .collapsed_selection(&end)
                    .expect("the unit end must be a caret")])
                .and_then(|selection| {
                    scene.move_selections(&selection, TextMovement::PreviousLogical, true)
                })
                .expect("the unit must expose one backward logical selection");
            for selection in [&forward, &backward] {
                let ranges = selection
                    .primary()
                    .expect("the primary selection exists")
                    .ranges();
                assert_eq!(ranges.len(), 1, "{name}");
                assert_eq!(ranges[0].text(), leaf, "{name}");
                assert_eq!(
                    ranges[0].bytes(),
                    0..u32::try_from(text.len()).expect("the focused corpus fits portable offsets"),
                    "{name}"
                );
            }
            let replaced = document
                .replace_selections(&forward, "")
                .expect("one complete unit must delete in one transaction");
            assert_eq!(
                replaced.publication().snapshot().text(leaf),
                Some(""),
                "{name}"
            );
        }
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
    fn visual_bidi_selection_retains_disjoint_ranges_and_set_ownership() {
        let text = "abc مرحبا XYZ";
        let arabic_start =
            u32::try_from(text.find('م').expect("Arabic run exists")).expect("fixture offset fits");
        let arabic_end = u32::try_from(text.find(" X").expect("trailing run exists"))
            .expect("fixture offset fits");
        let trailing_start = arabic_end + 1;
        let (mut document, styles, paint) = fixture_document(text, 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("mixed-bidi interaction must prepare");
        let scene = output.scene();
        let hits = scan_line_hits(scene, 0);
        let arabic: Vec<_> = hits
            .iter()
            .filter(|hit| hit.source.start >= arabic_start && hit.source.end <= arabic_end)
            .collect();
        assert!(arabic.len() >= 4, "fixture needs several Arabic clusters");
        let anchor_hit = arabic[arabic.len() / 2];
        let extent_hit = hits
            .iter()
            .find(|hit| hit.source.start == trailing_start)
            .expect("trailing Latin cluster is visible");
        let prefix_hit = hits.first().expect("prefix cluster is visible");
        let y = scene.lines()[0].bounds().center().y;
        let anchor = *scene
            .hit_test(Point::new(anchor_hit.min_x + 0.01, y))
            .expect("Arabic anchor must hit")
            .position();
        let extent = *scene
            .hit_test(Point::new(extent_hit.max_x - 0.01, y))
            .expect("Latin extent must hit")
            .position();
        let visual = scene
            .selection(&anchor, &extent, TextSelectionMode::Visual)
            .expect("visual caret path must select");
        let reverse = scene
            .selection(&extent, &anchor, TextSelectionMode::Visual)
            .expect("the reverse visual caret path must select");
        assert_eq!(
            visual.ranges(),
            reverse.ranges(),
            "visual selection source is independent of drag direction"
        );
        assert_eq!(
            visual
                .ranges()
                .iter()
                .map(|range| range.bytes())
                .collect::<Vec<_>>(),
            [4..10, 14..16],
            "the pinned mixed-bidi fixture must preserve its exact logical gap"
        );
        assert!(
            visual.ranges().windows(2).any(|ranges| {
                ranges[0].text() == ranges[1].text()
                    && ranges[0].bytes().end < ranges[1].bytes().start
            }),
            "a visually contiguous bidi gesture must retain its logical gap: {:?}",
            visual.ranges()
        );

        let prefix_start = *scene
            .hit_test(Point::new(prefix_hit.min_x + 0.01, y))
            .expect("prefix start must hit")
            .position();
        let prefix_end = *scene
            .hit_test(Point::new(prefix_hit.max_x - 0.01, y))
            .expect("prefix end must hit")
            .position();
        let prefix = scene
            .selection(&prefix_start, &prefix_end, TextSelectionMode::Logical)
            .expect("prefix selection must form");
        let selections = scene
            .selection_set([visual, prefix])
            .expect("nonoverlapping insertion points form one set");
        assert_eq!(selections.selections().len(), 2);
        let geometry = scene
            .selection_geometry(&selections)
            .expect("the complete selection set has geometry");
        assert!(
            geometry.iter().any(|rect| rect.selection() == 0)
                && geometry.iter().any(|rect| rect.selection() == 1),
            "geometry must preserve independent selection ownership: {geometry:?}"
        );
        assert!(
            geometry.iter().any(|rect| rect.range() > 0),
            "visual selection geometry must preserve disjoint-range ownership"
        );

        let carets = scene
            .selection_set([
                scene
                    .collapsed_selection(&prefix_start)
                    .expect("prefix caret is valid"),
                scene
                    .collapsed_selection(&anchor)
                    .expect("Arabic caret is valid"),
            ])
            .expect("two independent carets form one set");
        let duplicate = scene
            .collapsed_selection(&prefix_start)
            .expect("prefix caret is valid");
        assert_eq!(
            scene
                .selection_set([duplicate.clone(), duplicate])
                .expect_err("duplicate insertion points must fail as one set")
                .kind(),
            SelectionErrorKind::OverlappingSelections
        );
        let moved = scene
            .move_selections(&carets, TextMovement::NextVisual, false)
            .expect("the whole set moves through adapter transitions");
        assert_eq!(
            document.snapshot().revision(),
            selections.revision(),
            "selection and movement must not publish document work"
        );
        assert_eq!(moved.selections().len(), 2);
        assert!(
            moved
                .selections()
                .iter()
                .all(|selection| selection.is_collapsed())
        );
        assert_ne!(
            moved.selections()[0].extent(),
            moved.selections()[1].extent(),
            "independent carets must remain independent after movement"
        );

        let text_id = selections.selections()[0].ranges()[0].text();
        let replacement = document
            .replace_selections(&selections, "§")
            .expect("the complete set must publish atomically");
        assert_eq!(replacement.publication().changes().paragraphs().len(), 1);
        assert_eq!(replacement.selections().selections().len(), 2);
        assert!(
            replacement
                .selections()
                .selections()
                .iter()
                .all(|selection| selection.is_collapsed()),
            "every input insertion point must receive one post-edit caret"
        );
        assert_eq!(
            replacement
                .publication()
                .snapshot()
                .text(text_id)
                .expect("edited leaf survives")
                .matches('§')
                .count(),
            2,
            "one multi-range visual selection plus one prefix selection inserts twice, not once per range"
        );
        let error = document
            .replace_selections(&selections, "stale")
            .expect_err("old scene selections must not migrate across publication");
        assert_eq!(error.kind(), EditErrorKind::RevisionConflict);
    }

    #[test]
    fn logical_delete_and_backspace_remove_one_extended_grapheme() {
        for (source, movement, at_end, expected_range, expected_text) in [
            ("aé", TextMovement::PreviousLogical, true, 1..3, "a"),
            ("ae\u{301}", TextMovement::PreviousLogical, true, 1..4, "a"),
            ("éa", TextMovement::NextLogical, false, 0..2, "a"),
            ("a\r\n", TextMovement::PreviousLogical, true, 1..3, "a"),
            ("ب\u{64e}", TextMovement::NextLogical, true, 0..4, ""),
        ] {
            let (mut document, styles, paint) = fixture_document(source, 1.2);
            let mut engine = fixture_engine();
            let request = SceneRequest::new(
                FiniteWidth::new(1_000.0).expect("test width is valid"),
                &styles,
                &paint,
            );
            let output = engine
                .prepare(&document.snapshot(), &request)
                .expect("grapheme interaction must prepare");
            let scene = output.scene();
            let line = if at_end {
                scene.lines().last()
            } else {
                scene.lines().first()
            }
            .expect("the fixture must expose a line");
            let position = *scene
                .hit_test_closest(Point::new(
                    if at_end { 10_000.0 } else { -10_000.0 },
                    line.bounds().center().y,
                ))
                .expect("line edge must resolve")
                .position();
            let carets = scene
                .selection_set([scene
                    .collapsed_selection(&position)
                    .expect("line-edge caret is valid")])
                .expect("one caret forms a set");
            let deletion = scene
                .move_selections(&carets, movement, true)
                .expect("deletion range follows the adapter grapheme transition");
            assert_eq!(
                deletion.primary().expect("primary survives").ranges()[0].bytes(),
                expected_range,
                "deletion must select the complete extended grapheme for {source:?}"
            );
            let text = position.text();
            let replacement = document
                .replace_selections(&deletion, "")
                .expect("grapheme deletion must publish once");
            assert_eq!(
                replacement.publication().snapshot().text(text),
                Some(expected_text)
            );
        }
    }

    #[test]
    fn multi_paragraph_selection_edit_reshapes_only_affected_paragraphs() {
        let mut document = Document::new(DocumentId::from_bytes(*b"selection-cache1"));
        let mut edit = document.edit();
        let first_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("first paragraph is valid");
        edit.append_text(first_paragraph, InlineRole::TEXT, "alpha")
            .expect("first text is valid");
        let middle_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("middle paragraph is valid");
        edit.append_text(middle_paragraph, InlineRole::TEXT, "bravo")
            .expect("middle text is valid");
        let last_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("last paragraph is valid");
        edit.append_text(last_paragraph, InlineRole::TEXT, "charlie")
            .expect("last text is valid");
        edit.commit().expect("fixture edit is valid");

        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("fixture shaping style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let mut engine = fixture_engine();
        let initial = engine
            .prepare(&document.snapshot(), &request)
            .expect("initial document must prepare");
        assert_eq!(initial.work().shape().paragraphs(), 3);
        let scene = initial.scene();
        let first = *scene
            .hit_test_closest(Point::new(10_000.0, scene.lines()[0].bounds().center().y))
            .expect("first paragraph end resolves")
            .position();
        let last = *scene
            .hit_test_closest(Point::new(10_000.0, scene.lines()[2].bounds().center().y))
            .expect("last paragraph end resolves")
            .position();
        let selections = scene
            .selection_set([
                scene
                    .collapsed_selection(&first)
                    .expect("first caret is valid"),
                scene
                    .collapsed_selection(&last)
                    .expect("last caret is valid"),
            ])
            .expect("two paragraph-local carets form one set");
        let replacement = document
            .replace_selections(&selections, "!")
            .expect("both insertions publish atomically");
        assert_eq!(
            replacement.publication().changes().paragraphs(),
            [first_paragraph, last_paragraph]
        );

        let updated = engine
            .prepare(replacement.publication().snapshot(), &request)
            .expect("updated document must prepare");
        assert_eq!(updated.work().shape().paragraphs(), 2);
        assert_eq!(updated.work().reused_paragraphs(), 1);
        assert_eq!(
            updated.work().analysis().paragraphs(),
            2,
            "only changed paragraphs return to Unicode analysis"
        );
        assert_eq!(
            updated.work().geometry().paragraphs(),
            2,
            "the unchanged middle paragraph must retain its geometry"
        );
    }

    #[test]
    fn event_feed_composition_normalizes_multi_selection_and_retains_committed_work() {
        let mut document = Document::new(DocumentId::from_bytes(*b"ime-feed-cache01"));
        let mut edit = document.edit();
        let first_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("first paragraph is valid");
        let first_text = edit
            .append_text(first_paragraph, InlineRole::TEXT, "alpha")
            .expect("first text is valid");
        let middle_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("middle paragraph is valid");
        edit.append_text(middle_paragraph, InlineRole::TEXT, "bravo")
            .expect("middle text is valid");
        let last_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("last paragraph is valid");
        edit.append_text(last_paragraph, InlineRole::TEXT, "charlie")
            .expect("last text is valid");
        edit.commit().expect("fixture document must publish");
        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("fixture style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let snapshot = document.snapshot();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let mut engine = fixture_engine();
        let committed = engine
            .prepare(&snapshot, &request)
            .expect("committed scene must prepare");
        let scene = committed.scene();
        let first_y = scene.lines()[0].bounds().center().y;
        let last_y = scene.lines()[2].bounds().center().y;
        let start = *scene
            .hit_test_closest(Point::new(-100.0, first_y))
            .expect("paragraph start must resolve")
            .position();
        let end = *scene
            .hit_test_closest(Point::new(10_000.0, last_y))
            .expect("paragraph end must resolve")
            .position();
        let selections = scene
            .selection_set([
                scene
                    .collapsed_selection(&start)
                    .expect("primary caret must be valid"),
                scene
                    .collapsed_selection(&end)
                    .expect("secondary caret must be valid"),
            ])
            .expect("two independent carets must form one set");
        let started = scene
            .begin_composition(&selections, CompositionId::from_bytes(*b"feed-composition"))
            .expect("event-feed composition must start");
        assert!(started.selection_changed());
        assert_eq!(selections.selections().len(), 2);
        assert_eq!(started.selections().selections().len(), 1);
        assert!(
            started
                .selections()
                .primary()
                .expect("normalized primary exists")
                .is_collapsed()
        );

        let mut session = started.into_session();
        let expected = session.epoch();
        session
            .update(
                expected,
                CompositionUpdate::new("مرحبا").with_selection(10..10),
            )
            .expect("Arabic event-feed snapshot must update one epoch");
        let transient = engine
            .prepare_composition(&snapshot, &request, &session)
            .expect("Arabic preedit must shape through Parley");
        assert_eq!(transient.work().shape().paragraphs(), 1);
        assert_eq!(transient.work().reused_paragraphs(), 2);
        assert!(transient.scene().fragments().iter().any(|fragment| {
            fragment.script() == *b"Arab"
                && fragment.source().is_some_and(|source| {
                    source.sources().iter().any(|segment| {
                        matches!(segment, ProjectedTextSource::Composition(range)
                            if range.id() == session.id() && range.epoch() == session.epoch())
                    })
                })
        }));
        assert_eq!(snapshot.text(start.text()), Some("alpha"));

        let cancelled = engine
            .prepare(&snapshot, &request)
            .expect("ending the feed without commit must reveal committed scene");
        assert_eq!(cancelled.work().shape().paragraphs(), 0);
        assert_eq!(cancelled.work().geometry().paragraphs(), 0);
        assert_eq!(cancelled.work().reused_paragraphs(), 3);

        let replacement = session
            .commit(&mut document, "مرحبا")
            .expect("feed commit must publish exactly once");
        assert_eq!(replacement.publication().changes().paragraphs().len(), 1);
        assert_eq!(
            replacement.publication().changes().paragraphs(),
            [first_paragraph]
        );
        assert_eq!(snapshot.text(start.text()), Some("alpha"));
        assert_eq!(
            replacement.publication().snapshot().text(first_text),
            Some("مرحباalpha")
        );
        let committed_update = engine
            .prepare(replacement.publication().snapshot(), &request)
            .expect("one IME commit must retain unaffected siblings");
        assert_eq!(
            committed_update.work().shape().paragraphs(),
            0,
            "the committed publication can reuse physics already formed for the identical preedit"
        );
        assert_eq!(committed_update.work().geometry().paragraphs(), 1);
        assert_eq!(committed_update.work().reused_paragraphs(), 2);
    }

    #[test]
    fn host_driven_queries_share_the_exact_parley_composition_epoch() {
        let (document, styles, paint) = fixture_document("Aé office", 1.2);
        let snapshot = document.snapshot();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let mut engine = fixture_engine();
        let committed = engine
            .prepare(&snapshot, &request)
            .expect("committed host surface must prepare");
        let scene = committed.scene();
        let y = scene.lines()[0].bounds().center().y;
        let end = *scene
            .hit_test_closest(Point::new(10_000.0, y))
            .expect("host insertion point must resolve")
            .position();
        let selections = scene
            .selection_set([scene
                .collapsed_selection(&end)
                .expect("host caret must be valid")])
            .expect("host selection set must validate");
        let surface = EditableSurface::new(&snapshot, [EditableSurfaceElement::text(end.text())])
            .expect("host chooses one explicit semantic surface");
        let committed_host = surface
            .bind(scene, &selections)
            .expect("host selection and committed geometry must bind atomically");
        let replacement = committed_host
            .replacement_selection(1..3)
            .expect("the native byte range for precomposed é must map through the surface");
        let mut session = scene
            .begin_composition(
                &replacement,
                CompositionId::from_bytes(*b"host-composition"),
            )
            .expect("host-driven composition must replace its explicit authored range")
            .into_session();
        let expected = session.epoch();
        session
            .update(
                expected,
                CompositionUpdate::new("écho").with_selection(5..5),
            )
            .expect("host marked text must update");
        let transient = engine
            .prepare_composition(&snapshot, &request, &session)
            .expect("host marked text must shape through Parley");
        let host = surface
            .bind_composition(transient.scene(), &session)
            .expect("text and geometry must bind to the same epoch");

        assert_eq!(host.composition(), Some((session.id(), session.epoch())));
        assert_eq!(host.text(), "Aécho office");
        assert_eq!(host.marked_range(), Some(1..6));
        assert_eq!(host.host_selection(), Some(6..6));
        assert_eq!(
            host.range_in_encoding(0..13, SurfaceTextEncoding::Utf16)
                .expect("surface must answer UTF-16 conversion"),
            0..12
        );
        assert_eq!(
            host.range_from_encoding(0..12, SurfaceTextEncoding::Utf16)
                .expect("UTF-16 conversion must round trip"),
            0..13
        );
        assert_eq!(
            host.text_for_range(1..6)
                .expect("arbitrary marked-text query must resolve"),
            "écho"
        );
        assert_eq!(
            host.snapshot_range(1..6)
                .expect_err("generated marked text cannot become authored source")
                .kind(),
            SurfaceErrorKind::UnmappedRange
        );
        assert_eq!(
            host.replacement_selection(1..6)
                .expect_err("a transient marked range cannot become authored replacement source")
                .kind(),
            SurfaceErrorKind::UnsupportedSelection
        );
        assert!(host.caret_rect().is_some());
        let marked_rect = host
            .first_rect_for_range(1..6)
            .expect("marked range geometry must answer synchronously")
            .expect("marked text has visible geometry");
        let hit_offset = host
            .offset_for_point(marked_rect.center())
            .expect("point hit must map back to the same surface");
        assert!((1..=6).contains(&hit_offset));
        let line = transient.scene().lines()[0].bounds();
        let mut x = line.x0;
        let mut generated_step = None;
        while x <= line.x1 && generated_step.is_none() {
            if let Some(hit) = transient.scene().hit_test(Point::new(x, line.center().y)) {
                let position = *hit.position();
                if matches!(position, ProjectedTextPosition::Composition(_)) {
                    generated_step = [TextMovement::PreviousLogical, TextMovement::NextLogical]
                        .into_iter()
                        .filter_map(|movement| transient.scene().move_position(&position, movement))
                        .find(|moved| matches!(moved, ProjectedTextPosition::Composition(_)))
                        .map(|moved| (position, moved));
                }
            }
            x += 0.05;
        }
        let (position, moved) =
            generated_step.expect("preedit movement must stay in the prepared cursor graph");
        assert_ne!(moved, position);
    }

    #[test]
    fn generated_combining_mark_shapes_identically_without_authored_provenance() {
        let (document, styles, paint) = fixture_document("eX", 1.2);
        let snapshot = document.snapshot();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let mut engine = fixture_engine();
        let committed = engine
            .prepare(&snapshot, &request)
            .expect("committed base must prepare");
        let scene = committed.scene();
        let y = scene.lines()[0].bounds().center().y;
        let start = *scene
            .hit_test_closest(Point::new(-100.0, y))
            .expect("base start must resolve")
            .position();
        let caret = scene
            .selection_set([scene
                .collapsed_selection(&start)
                .expect("base caret must be valid")])
            .expect("one caret forms a set");
        let after_base = scene
            .move_selections(&caret, TextMovement::NextLogical, false)
            .expect("logical movement must cross the base cluster");
        assert_eq!(
            after_base
                .primary()
                .expect("moved primary exists")
                .extent()
                .byte(),
            1
        );
        let mut session = scene
            .begin_composition(&after_base, CompositionId::from_bytes(*b"combining-preedt"))
            .expect("combining composition must start")
            .into_session();
        session
            .update(
                session.epoch(),
                CompositionUpdate::new("\u{301}").with_selection(2..2),
            )
            .expect("combining mark is a valid preedit snapshot");
        let transient = engine
            .prepare_composition(&snapshot, &request, &session)
            .expect("generated combining mark must shape through Parley");

        let (authored, authored_styles, authored_paint) = fixture_document("e\u{301}X", 1.2);
        let authored_request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &authored_styles,
            &authored_paint,
        );
        let authored_scene = fixture_engine()
            .prepare(&authored.snapshot(), &authored_request)
            .expect("authored comparison must shape")
            .scene()
            .fragments()
            .iter()
            .flat_map(|fragment| fragment.glyphs())
            .map(|glyph| (glyph.id(), glyph.position(), glyph.advance()))
            .collect::<Vec<_>>();
        let projected_scene = transient
            .scene()
            .fragments()
            .iter()
            .flat_map(|fragment| fragment.glyphs())
            .map(|glyph| (glyph.id(), glyph.position(), glyph.advance()))
            .collect::<Vec<_>>();
        assert_eq!(
            projected_scene, authored_scene,
            "generated provenance must not split the shaping run or change glyph geometry"
        );
        assert!(transient.scene().fragments().iter().any(|fragment| {
            fragment.source().is_some_and(|source| {
                source
                    .sources()
                    .iter()
                    .any(|segment| matches!(segment, ProjectedTextSource::Composition(_)))
            })
        }));
        let line = transient.scene().lines()[0].bounds();
        let mut unit_source = None;
        let mut unit_positions = Vec::new();
        let mut x = line.x0;
        while x <= line.x1 {
            if let Some(hit) = transient.scene().hit_test(Point::new(x, line.center().y)) {
                let has_snapshot = hit
                    .source()
                    .sources()
                    .iter()
                    .any(|source| matches!(source, ProjectedTextSource::Snapshot(_)));
                let has_generated = hit
                    .source()
                    .sources()
                    .iter()
                    .any(|source| matches!(source, ProjectedTextSource::Composition(_)));
                if has_snapshot && has_generated {
                    unit_source.get_or_insert_with(|| hit.source().clone());
                    if !unit_positions.contains(hit.position()) {
                        unit_positions.push(*hit.position());
                    }
                }
            }
            x += 0.05;
        }
        let unit_source = unit_source.expect("the composed grapheme must expose one mixed source");
        assert_eq!(
            unit_source.sources().len(),
            2,
            "authored base and generated mark must both remain in the hit unit"
        );
        assert_eq!(
            unit_positions.len(),
            2,
            "the provenance boundary inside one grapheme must not become a caret stop"
        );
        assert!(unit_positions.iter().enumerate().any(|(index, position)| {
            unit_positions
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .any(|(_, other)| {
                    transient
                        .scene()
                        .move_position(position, TextMovement::PreviousLogical)
                        .is_some_and(|moved| moved == *other)
                        || transient
                            .scene()
                            .move_position(position, TextMovement::NextLogical)
                            .is_some_and(|moved| moved == *other)
                })
        }));
        assert_eq!(snapshot.text(start.text()), Some("eX"));
    }

    #[test]
    fn scene_movement_crosses_semantic_paragraph_boundaries() {
        let mut document = Document::new(DocumentId::from_bytes(*b"paragraph-move01"));
        let mut edit = document.edit();
        let first_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("first paragraph is valid");
        let first = edit
            .append_text(first_paragraph, InlineRole::TEXT, "one")
            .expect("first text is valid");
        let second_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("second paragraph is valid");
        let second = edit
            .append_text(second_paragraph, InlineRole::TEXT, "two")
            .expect("second text is valid");
        edit.commit().expect("fixture edit is valid");
        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("fixture shaping style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = fixture_engine()
            .prepare(&document.snapshot(), &request)
            .expect("multi-paragraph interaction must prepare");
        let scene = output.scene();
        let end = *scene
            .hit_test_closest(Point::new(10_000.0, scene.lines()[0].bounds().center().y))
            .expect("first paragraph end must resolve")
            .position();
        assert_eq!(end.text(), first);
        let carets = scene
            .selection_set([scene
                .collapsed_selection(&end)
                .expect("first paragraph caret is valid")])
            .expect("one caret forms a set");
        for movement in [TextMovement::NextVisual, TextMovement::NextLogical] {
            let moved = scene
                .move_selections(&carets, movement, false)
                .expect("movement must compose across paragraph boundaries");
            assert_eq!(
                moved.primary().expect("primary survives").extent().text(),
                second
            );
        }
    }

    #[test]
    fn exact_interaction_uses_ligature_components_not_glyph_ink() {
        let (document, styles, paint) = fixture_document("office", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("ligature interaction must prepare");
        let scene = output.scene();
        assert!(
            scene.fragments().len() < 6,
            "the fixture must contain a substituted multi-source glyph"
        );

        let hits = scan_line_hits(scene, 0);
        let sources: Vec<_> = hits.iter().map(|hit| hit.source.clone()).collect();
        assert_eq!(
            sources,
            vec![0..1, 1..2, 2..3, 3..4, 4..5, 5..6],
            "each ligature component must retain its own hit interval: {hits:?}"
        );

        let y = scene.lines()[0].bounds().center().y;
        let first = scene
            .hit_test(Point::new(0.1, y))
            .expect("the first cluster must be hittable");
        let second = scene
            .hit_test(Point::new(0.5, y))
            .expect("a second point in the same cluster must be hittable");
        assert_eq!(first.position(), second.position());
        assert_eq!(
            scene
                .caret(first.position())
                .expect("first hit caret must resolve")
                .bounds(),
            scene
                .caret(second.position())
                .expect("second hit caret must resolve")
                .bounds(),
            "caret geometry must come from the prepared stop, not the query x coordinate"
        );
    }

    #[test]
    fn interaction_map_groups_combining_source_and_keeps_whitespace() {
        for (text, expected) in [
            ("e\u{301}", core::iter::once(0..3).collect::<Vec<_>>()),
            ("a b", vec![0..1, 1..2, 2..3]),
        ] {
            let (document, styles, paint) = fixture_document(text, 1.2);
            let mut engine = fixture_engine();
            let request = SceneRequest::new(
                FiniteWidth::new(1_000.0).expect("test width is valid"),
                &styles,
                &paint,
            );
            let output = engine
                .prepare(&document.snapshot(), &request)
                .expect("cluster interaction must prepare");
            let hits = scan_line_hits(output.scene(), 0);
            assert_eq!(
                hits.iter()
                    .map(|hit| hit.source.clone())
                    .collect::<Vec<_>>(),
                expected,
                "source-complete graphemes and whitespace must remain hittable for {text:?}: {hits:?}"
            );
        }
    }

    #[test]
    fn split_leaf_grapheme_is_one_hit_movement_and_atomic_replacement_unit() {
        let mut document = Document::new(DocumentId::from_bytes(*b"split-grapheme-1"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph is valid");
        let base = edit
            .append_text(paragraph, InlineRole::TEXT, "e")
            .expect("base leaf is valid");
        let mark = edit
            .append_text(paragraph, InlineRole::EMPHASIS, "\u{301}")
            .expect("mark leaf is valid");
        edit.commit().expect("fixture edit is valid");
        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("fixture shaping style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let mut engine = fixture_engine();
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("a grapheme crossing semantic leaves must still prepare");

        let semantic_texts: Vec<_> = output
            .scene()
            .semantics()
            .filter_map(|semantic| semantic.source().map(|source| source.text()))
            .collect();
        assert!(semantic_texts.contains(&base));
        assert!(semantic_texts.contains(&mark));
        assert!(output.scene().fragments().iter().any(|fragment| {
            let texts: Vec<_> = fragment.sources().map(|source| source.text()).collect();
            texts.contains(&base) && texts.contains(&mark)
        }));
        let scene = output.scene();
        let y = scene.lines()[0].bounds().center().y;
        let hit = scene
            .hit_test(Point::new(scene.lines()[0].bounds().x0, y))
            .expect("the source-complete grapheme must be hittable");
        assert_eq!(hit.source().sources().len(), 2);
        assert_eq!(hit.source().sources()[0].text(), base);
        assert_eq!(hit.source().sources()[0].bytes(), 0..1);
        assert_eq!(hit.source().sources()[1].text(), mark);
        assert_eq!(hit.source().sources()[1].bytes(), 0..2);
        let base_semantic = scene
            .semantics()
            .find(|semantic| {
                semantic
                    .source()
                    .is_some_and(|source| source.text() == base)
            })
            .expect("base semantics must survive")
            .semantic_id();
        assert_eq!(
            hit.semantic_id(),
            base_semantic,
            "a zero-advance mark has no fabricated pointer interior"
        );

        let end = *scene
            .hit_test_closest(Point::new(10_000.0, y))
            .expect("the trailing grapheme side must resolve")
            .position();
        let carets = scene
            .selection_set([scene
                .collapsed_selection(&end)
                .expect("the trailing position is valid")])
            .expect("one caret forms a selection set");
        let deletion = scene
            .move_selections(&carets, TextMovement::PreviousLogical, true)
            .expect("backspace must cross the complete grapheme");
        let ranges = deletion
            .primary()
            .expect("the primary selection survives")
            .ranges();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].text(), base);
        assert_eq!(ranges[0].bytes(), 0..1);
        assert_eq!(ranges[1].text(), mark);
        assert_eq!(ranges[1].bytes(), 0..2);
        let geometry = scene
            .selection_geometry(&deletion)
            .expect("source-complete selection geometry must resolve");
        assert_eq!(
            geometry.len(),
            1,
            "one grapheme crossing two leaves must paint one selection rectangle"
        );

        let replacement = document
            .replace_selections(&deletion, "")
            .expect("one multi-leaf grapheme must publish atomically");
        assert_eq!(replacement.publication().snapshot().text(base), Some(""));
        assert_eq!(replacement.publication().snapshot().text(mark), Some(""));
        assert_eq!(
            replacement.publication().changes().paragraphs(),
            [paragraph]
        );
    }

    #[test]
    fn rtl_visual_hits_retain_reversed_logical_sides() {
        let (document, styles, paint) = fixture_document("مرحبا", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("RTL interaction must prepare");
        let hits = scan_line_hits(output.scene(), 0);
        assert!(
            hits.len() >= 5,
            "Arabic source must expose real clusters: {hits:?}"
        );
        assert!(
            hits.windows(2)
                .all(|pair| pair[0].source.start > pair[1].source.start),
            "visual left-to-right traversal must retain descending RTL source: {hits:?}"
        );
        assert!(
            hits.iter().all(|hit| {
                hit.position == hit.source.end && hit.affinity == TextAffinity::Upstream
            }),
            "the visual left side of every RTL cluster must resolve to its logical end: {hits:?}"
        );
    }

    #[test]
    fn soft_wrap_exposes_both_affinities_for_one_logical_boundary() {
        let (document, styles, paint) = fixture_document("alpha beta gamma", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(72.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("wrapped interaction must prepare");
        let first = scan_line_hits(output.scene(), 0);
        let second = scan_line_hits(output.scene(), 1);
        let at_end = first.last().expect("first line has a final cluster");
        let at_start = second.first().expect("second line has an initial cluster");
        let end_hit = output
            .scene()
            .hit_test(Point::new(
                at_end.max_x,
                output.scene().lines()[0].bounds().center().y,
            ))
            .expect("line-end cluster must be hittable");
        let start_hit = output
            .scene()
            .hit_test(Point::new(
                at_start.min_x,
                output.scene().lines()[1].bounds().center().y,
            ))
            .expect("next-line cluster must be hittable");
        assert_eq!(end_hit.position().byte(), start_hit.position().byte());
        assert_eq!(end_hit.position().affinity(), TextAffinity::Upstream);
        assert_eq!(start_hit.position().affinity(), TextAffinity::Downstream);
        assert_ne!(
            output
                .scene()
                .caret(end_hit.position())
                .expect("upstream caret must resolve")
                .bounds()
                .y0,
            output
                .scene()
                .caret(start_hit.position())
                .expect("downstream caret must resolve")
                .bounds()
                .y0,
            "affinity must select the correct side of the soft wrap"
        );
    }

    #[test]
    fn empty_editable_leaf_has_a_closest_hit_and_exact_caret() {
        let (document, styles, paint) = fixture_document("", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("empty editable text must prepare");
        let hit = output
            .scene()
            .hit_test_closest(Point::new(200.0, 80.0))
            .expect("empty semantic text must expose a clamped position");
        assert_eq!(sole_unit_source(hit.source()).bytes(), 0..0);
        assert_eq!(hit.position().byte(), 0);
        assert_eq!(hit.position().affinity(), TextAffinity::Downstream);
        let caret = output
            .scene()
            .caret(hit.position())
            .expect("empty position must have caret geometry");
        assert_eq!(caret.bounds().x0, 0.0);
        assert!(caret.bounds().height() > 0.0);
    }

    #[test]
    fn structurally_leafless_paragraph_is_not_editable() {
        let mut document = Document::new(DocumentId::from_bytes(*b"leafless-hit-001"));
        let mut edit = document.edit();
        edit.append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph is valid");
        edit.commit().expect("fixture edit is valid");
        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("fixture shaping style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = fixture_engine()
            .prepare(&document.snapshot(), &request)
            .expect("a leafless paragraph must still prepare");
        assert!(
            output
                .scene()
                .hit_test_closest(Point::new(0.0, 0.0))
                .is_none(),
            "structure without a semantic text leaf must not manufacture an editable position"
        );
    }

    #[test]
    fn semantic_leaf_boundary_ownership_follows_affinity() {
        let mut document = Document::new(DocumentId::from_bytes(*b"leaf-boundary-01"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph is valid");
        let first_text = edit
            .append_text(paragraph, InlineRole::TEXT, "ab")
            .expect("first leaf is valid");
        let second_text = edit
            .append_text(paragraph, InlineRole::EMPHASIS, "cd")
            .expect("second leaf is valid");
        edit.commit().expect("fixture edit is valid");
        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
                .expect("fixture shaping style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let mut engine = fixture_engine();
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("multi-leaf interaction must prepare");
        let scene = output.scene();
        let y = scene.lines()[0].bounds().center().y;
        let mut first_right = None;
        let mut second_left = None;
        let mut x = scene.lines()[0].bounds().x0;
        while x <= scene.lines()[0].bounds().x1 {
            if let Some(hit) = scene.hit_test(Point::new(x, y)) {
                let source = sole_unit_source(hit.source());
                if source.text() == first_text {
                    first_right = Some((x, hit.semantic_id()));
                } else if source.text() == second_text && second_left.is_none() {
                    second_left = Some((x, hit.semantic_id()));
                }
            }
            x += 0.05;
        }
        let (first_x, first_semantic) = first_right.expect("first leaf must be hittable");
        let (second_x, second_semantic) = second_left.expect("second leaf must be hittable");
        let first_hit = scene
            .hit_test(Point::new(first_x, y))
            .expect("first leaf trailing side must resolve");
        let second_hit = scene
            .hit_test(Point::new(second_x, y))
            .expect("second leaf leading side must resolve");
        assert_eq!(first_hit.position().text(), first_text);
        assert_eq!(first_hit.position().byte(), 2);
        assert_eq!(first_hit.position().affinity(), TextAffinity::Upstream);
        assert_eq!(second_hit.position().text(), second_text);
        assert_eq!(second_hit.position().byte(), 0);
        assert_eq!(second_hit.position().affinity(), TextAffinity::Downstream);
        assert_ne!(first_semantic, second_semantic);
        assert_eq!(first_hit.semantic_id(), first_semantic);
        assert_eq!(second_hit.semantic_id(), second_semantic);
    }

    #[test]
    fn caret_rejects_a_position_from_another_revision() {
        let (mut document, styles, paint) = fixture_document("abc", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let old_output = engine
            .prepare(&document.snapshot(), &request)
            .expect("old interaction must prepare");
        let old_hit = old_output
            .scene()
            .hit_test(Point::new(
                0.0,
                old_output.scene().lines()[0].bounds().center().y,
            ))
            .expect("old scene must be hittable");
        let old_position = *old_hit.position();
        let mut edit = document.edit();
        edit.replace_text(old_position.text(), "abcd")
            .expect("replacement is valid");
        edit.commit().expect("replacement must publish");
        let new_output = engine
            .prepare(&document.snapshot(), &request)
            .expect("new interaction must prepare");
        assert!(
            new_output.scene().caret(&old_position).is_none(),
            "a snapshot position must not silently migrate to a newer revision"
        );
    }

    #[test]
    fn closest_hit_selects_the_nearest_line_before_its_inline_edge() {
        let text = "a\nsupercalifragilisticexpialidocious";
        let (document, styles, paint) = fixture_document(text, 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("explicitly broken interaction must prepare");
        assert_eq!(output.scene().lines().len(), 2);
        let hit = output
            .scene()
            .hit_test_closest(Point::new(
                10_000.0,
                output.scene().lines()[0].bounds().center().y,
            ))
            .expect("first line must clamp despite a much wider later line");
        assert!(
            sole_unit_source(hit.source()).bytes().end <= 2,
            "block-axis selection must happen before inline clamping: {hit:?}"
        );
    }

    #[test]
    fn mandatory_break_keeps_before_and_after_carets_on_distinct_lines() {
        let (document, styles, paint) = fixture_document("a\n", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("mandatory-break interaction must prepare");
        assert_eq!(output.scene().lines().len(), 2);
        let before = output
            .scene()
            .hit_test_closest(Point::new(
                10_000.0,
                output.scene().lines()[0].bounds().center().y,
            ))
            .expect("the broken line must clamp before the control");
        let after = output
            .scene()
            .hit_test_closest(Point::new(
                10_000.0,
                output.scene().lines()[1].bounds().center().y,
            ))
            .expect("the final empty line must expose the post-break caret");
        assert_eq!(before.position().byte(), 1);
        assert_eq!(after.position().byte(), 2);
        assert_ne!(
            output
                .scene()
                .caret(before.position())
                .expect("pre-break caret must resolve")
                .bounds()
                .y0,
            output
                .scene()
                .caret(after.position())
                .expect("post-break caret must resolve")
                .bounds()
                .y0
        );
    }

    #[derive(Clone, Debug)]
    struct ScannedHit {
        source: core::ops::Range<u32>,
        position: u32,
        affinity: TextAffinity,
        min_x: f64,
        max_x: f64,
    }

    fn scan_line_hits(scene: &TextScene, line_index: usize) -> Vec<ScannedHit> {
        let bounds = scene.lines()[line_index].bounds();
        let y = bounds.center().y;
        let mut hits: Vec<ScannedHit> = Vec::new();
        let mut x = bounds.x0;
        while x <= bounds.x1 {
            if let Some(hit) = scene.hit_test(Point::new(x, y)) {
                let source = sole_unit_source(hit.source()).bytes();
                if let Some(existing) = hits.iter_mut().find(|existing| existing.source == source) {
                    existing.max_x = x;
                } else {
                    hits.push(ScannedHit {
                        source,
                        position: hit.position().byte(),
                        affinity: hit.position().affinity(),
                        min_x: x,
                        max_x: x,
                    });
                }
            }
            x += 0.05;
        }
        hits
    }

    fn sole_unit_source(unit: &SnapshotTextUnit) -> &underwood::SnapshotTextRange {
        let [source] = unit.sources() else {
            panic!("fixture interaction unit must remain within one semantic leaf");
        };
        source
    }

    #[test]
    fn zero_advance_arabic_mark_uses_unclipped_whole_glyph_paint() {
        let (document, styles, paint) = fixture_document("ب", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("Arabic mark shaping must form a scene");
        let mark = output
            .scene()
            .fragments()
            .iter()
            .find(|fragment| fragment.glyphs()[0].advance().x == 0.0)
            .expect("Noto Kufi beh must expose its zero-advance dot glyph");
        assert_eq!(mark.paint(), PaintSlot::new(0));
        assert_eq!(
            mark.paint_clip(),
            None,
            "ordinary zero-advance marks must let the font rasterizer paint the complete glyph"
        );
    }

    #[test]
    fn ordinary_glyphs_do_not_require_outline_metrics_or_paint_clips() {
        let (document, styles, paint) = fixture_document("j office ب", 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("ordinary glyph shaping must not require outline metrics");
        assert!(
            !output.scene().fragments().is_empty(),
            "the mixed fixture must produce renderable glyphs"
        );
        assert!(
            output
                .scene()
                .fragments()
                .iter()
                .all(|fragment| fragment.paint_clip().is_none()),
            "single-paint glyphs must be complete unclipped draws"
        );
    }

    #[test]
    fn synthetic_embolden_prepares_without_outline_metrics() {
        let mut document = Document::new(DocumentId::from_bytes(*b"embolden-test-01"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("test paragraph is valid");
        edit.append_text(paragraph, InlineRole::TEXT, "مرحبا")
            .expect("test source is valid");
        edit.commit().expect("test edit is valid");

        let shaping = ShapingStyle::new(FontFamily::named("Noto Kufi Arabic"), 20.0)
            .expect("test style is valid")
            .with_font_weight(FontWeight::BOLD)
            .expect("bold request is valid");
        let styles = StyleMap::new(ComputedInlineStyle::new(
            shaping,
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        ));
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let fonts = FontSet::try_from_fonts([
            Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid")
        ])
        .expect("fixture catalog is valid");
        let mut engine = LayoutEngine::new(
            ParleyParagraphEngine::new(TextData::compiled_minimal(), fonts)
                .expect("fixture adapter is valid"),
        );
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("synthetic emboldening must not require outline bounds to prepare");

        assert!(!output.scene().fragments().is_empty());
        assert!(output.scene().fragments().iter().all(|fragment| {
            fragment.synthesis().embolden() && fragment.paint_clip().is_none()
        }));
    }

    #[cfg(all(feature = "system-fonts", target_vendor = "apple"))]
    #[test]
    fn system_font_fallback_prepares_han_without_outline_metrics() {
        let mut document = Document::new(DocumentId::from_bytes(*b"system-han-test1"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("test paragraph is valid");
        edit.append_text(paragraph, InlineRole::TEXT, "漢字")
            .expect("test source is valid");
        edit.commit().expect("test edit is valid");

        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0).expect("test style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let fonts = FontSet::try_from_fonts([
            Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid")
        ])
        .expect("fixture catalog is valid")
        .with_system_fonts();
        let mut engine = LayoutEngine::new(
            ParleyParagraphEngine::new(TextData::compiled_minimal(), fonts)
                .expect("system-font adapter is valid"),
        );
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("Han source must prepare through the native fallback catalog");

        assert!(output.scene().fragments().iter().any(|fragment| {
            fragment.script() == *b"Hani"
                && fragment.font().data.as_ref() != LATIN_FONT
                && fragment.paint_clip().is_none()
        }));
    }

    #[test]
    fn split_paint_ligature_without_component_geometry_fails_explicitly() {
        let mut document = Document::new(DocumentId::from_bytes(*b"paint-ligature01"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("test paragraph is valid");
        let prefix = edit
            .append_text(paragraph, InlineRole::TEXT, "of")
            .expect("prefix is valid");
        let suffix = edit
            .append_text(paragraph, InlineRole::EMPHASIS, "fice")
            .expect("suffix is valid");
        edit.commit().expect("test edit is valid");

        let base = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Roboto Flex"), 40.0).expect("test style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let mut styles = StyleMap::new(base.clone());
        styles.set(prefix, base.clone());
        styles.set(suffix, base.with_paint(PaintSlot::new(1)));
        let paint = PaintTable::from_brushes([
            Brush::Solid(Color::BLACK),
            Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)),
        ]);
        let request = SceneRequest::new(
            FiniteWidth::new(1_000.0).expect("test width is valid"),
            &styles,
            &paint,
        );
        let error = fixture_engine()
            .prepare(&document.snapshot(), &request)
            .expect_err("Roboto Flex has no GDEF ligature carets for an exact paint split");
        assert_eq!(
            error.preparation(),
            Some(PreparationErrorKind::UnsupportedPaintCoverage)
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
