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

use fontique::{Blob, CharmapIndex, FontInfo, SourceId, SourceInfo, SourceKind, Synthesis};
use parley_core::{
    Analysis, AnalysisDataSources, AnalysisOptions, Analyzer, FontInstance, ShapeOptions, Shaper,
    shape::{CharCluster, Status},
};
use underwood::adapter::{
    GlyphPaintCoverage, GlyphPaintSegment, ParagraphInput, ParagraphPreparation,
    ParagraphPreparationOutput, PreparationError, PreparationWork, PreparedGlyph,
    PreparedParagraph, PreparedRun, ShapingRun,
};
use underwood::{FontData, ParagraphId, Rect, ShapingStyle, Vec2};

/// Owned validated font bytes and a face within them.
#[derive(Clone)]
pub struct Font {
    diagnostic_name: Arc<str>,
    digest: u64,
    blob: Blob<u8>,
    index: u32,
    units_per_em: u16,
    charmap: CharmapIndex,
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
        let info = FontInfo::from_source(source, index)
            .ok_or_else(|| AdapterError::new(AdapterErrorKind::InvalidFont))?;
        Ok(Self {
            diagnostic_name: Arc::from(diagnostic_name),
            digest: digest_bytes(bytes),
            blob,
            index,
            units_per_em,
            charmap: info.charmap_index(),
        })
    }

    fn instance(&self) -> FontInstance {
        FontInstance {
            blob: self.blob.clone(),
            index: self.index,
            synthesis: Synthesis::default(),
        }
    }

    fn data(&self) -> FontData {
        FontData::new(self.blob.clone(), self.index)
    }

    fn covers(&self, character: char) -> bool {
        self.charmap
            .charmap(self.blob.as_ref())
            .and_then(|charmap| charmap.map(character))
            .is_some()
    }
}

/// Immutable ordered fallback set for the headless adapter.
#[derive(Clone, Debug)]
pub struct FontSet {
    fonts: Arc<[Font]>,
}

impl FontSet {
    /// Collects a non-empty ordered font set.
    pub fn try_from_fonts(fonts: impl IntoIterator<Item = Font>) -> Result<Self, AdapterError> {
        let fonts: Vec<_> = fonts.into_iter().collect();
        if fonts.is_empty() {
            return Err(AdapterError::new(AdapterErrorKind::EmptyFontSet));
        }
        Ok(Self {
            fonts: fonts.into(),
        })
    }

    fn select<'a>(
        &'a self,
        cluster: &mut CharCluster,
        data: &AnalysisDataSources,
    ) -> Option<&'a Font> {
        let mut best = None;
        for font in self.fonts.iter() {
            match cluster.map(|character| font.covers(character), data) {
                Status::Complete => return Some(font),
                Status::Keep => best = Some(font),
                Status::Discard => {}
            }
        }
        best
    }

    fn by_instance(&self, instance: &FontInstance) -> Option<&Font> {
        self.fonts
            .iter()
            .find(|font| font.blob.id() == instance.blob.id() && font.index == instance.index)
    }
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
                self.cache[index].runs.clear();
                (index, true)
            }
        } else {
            self.cache.push(PhysicsCache {
                paragraph: input.paragraph(),
                text: Arc::from(input.text()),
                analysis: analyze_text(&mut self.analyzer, input.text()),
                shaping_styles: Vec::new(),
                shaping_runs: Vec::new(),
                runs: Vec::new(),
            });
            (self.cache.len() - 1, true)
        };

        let shaped = self.cache[cache_index].shaping_styles != input.shaping_styles()
            || self.cache[cache_index].shaping_runs != input.shaping_runs();
        if shaped {
            let runs = shape_paragraph(
                &mut self.shaper,
                &self.cache[cache_index].analysis,
                &self.fonts,
                input.text(),
                input.shaping_styles(),
                input.shaping_runs(),
            )?;
            self.cache[cache_index].shaping_styles = input.shaping_styles().to_vec();
            self.cache[cache_index].shaping_runs = input.shaping_runs().to_vec();
            self.cache[cache_index].runs = runs;
        }

        let physics = &self.cache[cache_index];
        let mut prepared_runs = Vec::with_capacity(physics.runs.len());
        let mut glyph_count = 0_u32;
        for run in &physics.runs {
            let mut prepared_glyphs = Vec::with_capacity(run.glyphs.len());
            for glyph in &run.glyphs {
                let paint = paint_coverage(
                    input.text(),
                    glyph.source.clone(),
                    glyph.advance,
                    run.font_size,
                    input.paint_runs(),
                    run.bidi_level & 1 == 1,
                )?;
                prepared_glyphs.push(PreparedGlyph::try_new(
                    glyph.id,
                    glyph.source.clone(),
                    glyph.advance,
                    glyph.offset,
                    paint,
                )?);
                glyph_count = glyph_count.saturating_add(1);
            }
            prepared_runs.push(PreparedRun::try_new(
                run.source.clone(),
                run.bidi_level,
                run.script,
                run.font.clone(),
                run.font_size,
                run.normalized_coords.iter().copied(),
                prepared_glyphs,
            )?);
        }
        let text_len =
            u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
        let paragraph =
            PreparedParagraph::try_from_runs(input.paragraph(), text_len, prepared_runs)?;
        let work = if !analyzed && !shaped {
            PreparationWork::new(false, false, 0, 0)
        } else {
            PreparationWork::new(
                analyzed,
                shaped,
                if shaped {
                    u32::try_from(physics.runs.len()).unwrap_or(u32::MAX)
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
    runs: Vec<PhysicsRun>,
}

#[derive(Clone, Debug)]
struct PhysicsRun {
    source: Range<u32>,
    bidi_level: u8,
    script: [u8; 4],
    font: FontData,
    font_size: f32,
    normalized_coords: Vec<i16>,
    glyphs: Vec<PhysicsGlyph>,
}

#[derive(Clone, Debug)]
struct PhysicsGlyph {
    id: u32,
    source: Range<u32>,
    advance: Vec2,
    offset: Vec2,
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
    fonts: &FontSet,
    text: &str,
    shaping_styles: &[ShapingStyle],
    shaping_runs: &[ShapingRun],
) -> Result<Vec<PhysicsRun>, PreparationError> {
    let analysis_data = AnalysisDataSources::new();
    let char_offsets = char_byte_offsets(text);
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
    let mut runs = Vec::new();

    let split_after = |range: parley_core::itemize::TextRange| {
        style_indices[range.char_range.start] != style_indices[range.char_range.end]
    };
    for item in analysis.itemize(text, split_after) {
        let style = &shaping_styles[usize::from(style_indices[item.range.char_range.start])];
        let script = item.script.to_bytes();
        let missing_font = Cell::new(false);
        shaper.shape_item(
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
            |cluster| match fonts.select(cluster, &analysis_data) {
                Some(font) => Some(font.instance()),
                None => {
                    missing_font.set(true);
                    None
                }
            },
            &analysis_data,
            |shaped| {
                let font = fonts
                    .by_instance(&shaped.font)
                    .expect("Parley must return the selected immutable font instance");
                runs.push(copy_run(
                    shaped,
                    item.bidi_level,
                    script,
                    font,
                    style.font_size(),
                    &char_offsets,
                ));
            },
        );
        if missing_font.get() {
            return Err(PreparationError::missing_font());
        }
    }
    if !text.is_empty() && runs.is_empty() {
        return Err(PreparationError::missing_font());
    }
    Ok(runs)
}

fn copy_run(
    run: parley_core::ShapedRun<'_>,
    bidi_level: u8,
    script: [u8; 4],
    font: &Font,
    font_size: f32,
    char_offsets: &[usize],
) -> PhysicsRun {
    let infos = run.glyph_buffer.glyph_infos();
    let positions = run.glyph_buffer.glyph_positions();
    let mut cluster_starts: Vec<usize> = infos
        .iter()
        .map(|info| {
            run.range.char_range.start
                + usize::try_from(info.cluster).expect("glyph cluster must fit usize")
        })
        .collect();
    cluster_starts.sort_unstable();
    cluster_starts.dedup();
    let scale = font_size / f32::from(font.units_per_em);
    let glyphs = infos
        .iter()
        .zip(positions)
        .map(|(info, position)| {
            let cluster = run.range.char_range.start
                + usize::try_from(info.cluster).expect("glyph cluster must fit usize");
            let next = cluster_starts
                .iter()
                .copied()
                .find(|candidate| *candidate > cluster)
                .unwrap_or(run.range.char_range.end);
            PhysicsGlyph {
                id: info.glyph_id,
                source: u32::try_from(char_offsets[cluster]).expect("text length was validated")
                    ..u32::try_from(char_offsets[next]).expect("text length was validated"),
                advance: Vec2::new(
                    f64::from(position.x_advance) * f64::from(scale),
                    f64::from(position.y_advance) * f64::from(scale),
                ),
                offset: Vec2::new(
                    f64::from(position.x_offset) * f64::from(scale),
                    f64::from(position.y_offset) * f64::from(scale),
                ),
            }
        })
        .collect();
    PhysicsRun {
        source: u32::try_from(run.range.byte_range.start).expect("text length was validated")
            ..u32::try_from(run.range.byte_range.end).expect("text length was validated"),
        bidi_level,
        script,
        font: font.data(),
        font_size,
        normalized_coords: run.coords.iter().map(|coord| coord.to_bits()).collect(),
        glyphs,
    }
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

fn char_byte_offsets(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(offset, _)| offset)
        .chain(core::iter::once(text.len()))
        .collect()
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
    use super::{read_u16, read_u32};

    #[test]
    fn big_endian_readers_reject_short_input() {
        assert_eq!(read_u16(&[0x12, 0x34], 0), Some(0x1234));
        assert_eq!(read_u16(&[0x12], 0), None);
        assert_eq!(read_u32(&[0x12, 0x34, 0x56, 0x78], 0), Some(0x1234_5678));
        assert_eq!(read_u32(&[0x12, 0x34], 0), None);
    }
}
