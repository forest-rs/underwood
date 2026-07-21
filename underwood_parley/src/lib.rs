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
    Analysis, AnalysisDataSources, AnalysisOptions, Analyzer, FontInstance, ShapeOptions,
    ShapedText, Shaper,
    shape::{CharCluster, ClusterData, Status},
};
use underwood::adapter::{
    FontSynthesis, GlyphPaintCoverage, GlyphPaintSegment, ParagraphInput, ParagraphPreparation,
    ParagraphPreparationOutput, PreparationError, PreparationWork, PreparedGlyph,
    PreparedParagraph, PreparedRun, ShapingRun,
};
use underwood::{
    FontData, FontFamilyName, FontVariation, GenericFamily, Language, ParagraphId, Rect, Script,
    ShapingStyle, Tag, Vec2,
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

impl ParagraphPreparation for ParleyParagraphEngine {
    fn prepare(
        &mut self,
        input: ParagraphInput<'_>,
    ) -> Result<ParagraphPreparationOutput, PreparationError> {
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
                self.cache[index].scripts.clear();
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
                scripts: Vec::new(),
                selected_clusters: 0,
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
        }

        let physics = &self.cache[cache_index];
        if physics.shaped_text.runs().len() != physics.scripts.len() {
            return Err(PreparationError::invalid_output());
        }
        let mut prepared_runs = Vec::with_capacity(physics.shaped_text.runs().len());
        let mut glyph_count = 0_u32;
        for (run, script) in physics.shaped_text.runs().iter().zip(&physics.scripts) {
            let prepared_glyphs =
                lower_glyphs(input.text(), &physics.shaped_text, run, input.paint_runs())?;
            glyph_count = glyph_count
                .saturating_add(u32::try_from(prepared_glyphs.len()).unwrap_or(u32::MAX));
            let font = physics
                .shaped_text
                .fonts()
                .get(run.font_index)
                .ok_or_else(PreparationError::invalid_output)?;
            let normalized_coords = physics
                .shaped_text
                .normalized_coords()
                .get(run.normalized_coords_range.clone())
                .ok_or_else(PreparationError::invalid_output)?;
            prepared_runs.push(PreparedRun::try_new(
                checked_source_range(&run.range.byte_range)?,
                run.bidi_level,
                *script,
                font.font.clone(),
                run.font_size,
                portable_synthesis(font.synthesis)?,
                normalized_coords.iter().map(|coord| coord.to_bits()),
                prepared_glyphs,
            )?);
        }
        let text_len =
            u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
        let paragraph =
            PreparedParagraph::try_from_runs(input.paragraph(), text_len, prepared_runs)?;
        let work = if !analyzed && !shaped {
            PreparationWork::new(false, false, 0, 0, 0)
        } else {
            PreparationWork::new(
                analyzed,
                shaped,
                if shaped { physics.selected_clusters } else { 0 },
                if shaped {
                    u32::try_from(physics.shaped_text.runs().len()).unwrap_or(u32::MAX)
                } else {
                    0
                },
                if shaped { glyph_count } else { 0 },
            )
        };
        Ok(ParagraphPreparationOutput::new(paragraph, work))
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
    scripts: Vec<[u8; 4]>,
    selected_clusters: u32,
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

    let split_after = |range: parley_core::itemize::TextRange| {
        style_indices[range.char_range.start] != style_indices[range.char_range.end]
    };
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
    shaped_text: &ShapedText,
    run: &parley_core::ShapedRun,
    paint_runs: &[underwood::adapter::PaintRun],
) -> Result<Vec<PreparedGlyph>, PreparationError> {
    let clusters = shaped_text
        .clusters()
        .get(run.clusters_range.clone())
        .ok_or_else(PreparationError::invalid_output)?;
    let mut prepared = Vec::with_capacity(run.glyphs_range.len());
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
        for index in (0..clusters.len()).rev() {
            lower_cluster(index)?;
        }
    } else {
        for index in 0..clusters.len() {
            lower_cluster(index)?;
        }
    }
    Ok(prepared)
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
    use underwood::{
        Brush, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth, FontFamily,
        GenericFamily, InlineRole, LayoutEngine, PaintSlot, PaintTable, ParagraphRole,
        SceneRequest, ShapingStyle, StyleMap,
    };
    use underwood::{Language, Script};

    use super::{
        AdapterErrorKind, Font, FontSet, ParleyParagraphEngine, TextData, read_u16, read_u32,
    };

    const LATIN_FONT: &[u8] =
        include_bytes!("../../examples/headless/fonts/RobotoFlex-VariableFont.ttf");

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
            underwood::InlineFlowStyle::default(),
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
}
