// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Paragraph analysis, itemization, font selection, and shaping.
//!
//! This module owns projection into public Parley shaping operations,
//! including line-final reuse of canonical fonts; it explicitly does not own
//! line-breaking policy or portable scene records.

use alloc::{borrow::Cow, vec::Vec};
use core::cell::Cell;
use core::ops::Range;

use fontique::{Attributes, FallbackKey, QueryFamily, QueryStatus};
use parley_engine::{
    Analysis, AnalysisDataSources, AnalysisOptions, Analyzer, FontInstance, JoiningType,
    ShapeOptions, ShapedText, Shaper,
    itemize::TextRange,
    shape::{CharCluster, Status},
};
use underwood::adapter::{AnalysisRun, InlineFlowRun, PreparationError, ShapingRun};
use underwood::{
    AnalysisStyle, BaseDirection, FontData, FontFamilyName, FontFeature, InlineFlowStyle,
    ShapingStyle, Tag, WordBreak,
};

use crate::font::FontSet;
use crate::line_break::LineShapeOutput;

#[cfg(test)]
pub(crate) fn analyze_text(
    analyzer: &mut Analyzer,
    text: &str,
    base_direction: BaseDirection,
) -> Analysis {
    analyze_text_with_styles(analyzer, text, base_direction, &[], &[])
        .expect("empty analysis-style runs are valid")
}

pub(crate) fn analyze_text_with_styles(
    analyzer: &mut Analyzer,
    text: &str,
    base_direction: BaseDirection,
    styles: &[AnalysisStyle],
    runs: &[AnalysisRun],
) -> Result<Analysis, PreparationError> {
    let mut word_break: Vec<(Range<usize>, WordBreak)> = Vec::new();
    for run in runs {
        let style = styles
            .get(run.style().index())
            .ok_or_else(PreparationError::invalid_output)?;
        if style.word_break() == WordBreak::Normal {
            continue;
        }
        let bytes = run.bytes();
        let range = bytes.start as usize..bytes.end as usize;
        if let Some((previous, previous_style)) = word_break.last_mut()
            && *previous_style == style.word_break()
            && previous.end == range.start
        {
            previous.end = range.end;
        } else {
            word_break.push((range, style.word_break()));
        }
    }
    let mut analysis = Analysis::new();
    analyzer.analyze(
        text,
        &AnalysisOptions {
            base_direction,
            word_break: &word_break,
            ..AnalysisOptions::default()
        },
        &mut analysis,
    );
    Ok(analysis)
}

pub(crate) fn shape_paragraph(
    shaper: &mut Shaper,
    analysis: &Analysis,
    fonts: &mut FontSet,
    text: &str,
    shaping_styles: &[ShapingStyle],
    shaping_runs: &[ShapingRun],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
    shaped_text: &mut ShapedText,
    scripts: &mut Vec<[u8; 4]>,
    style_indices: &mut Vec<u16>,
    inline_flow_indices: &mut Vec<u16>,
) -> Result<u32, PreparationError> {
    shaped_text.clear();
    scripts.clear();
    prepare_style_indices(
        text,
        shaping_runs,
        inline_flow_runs,
        style_indices,
        inline_flow_indices,
    )?;
    shape_range(
        shaper,
        analysis,
        FontSource::Query(fonts),
        text,
        shaping_styles,
        style_indices,
        inline_flow_styles,
        inline_flow_indices,
        0..text.len(),
        shaped_text,
        scripts,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Retained canonical reshaping keeps every cache input explicit"
)]
pub(crate) fn reshape_paragraph_with_retained_fonts(
    shaper: &mut Shaper,
    analysis: &Analysis,
    canonical_text: &ShapedText,
    text: &str,
    shaping_styles: &[ShapingStyle],
    shaping_runs: &[ShapingRun],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
    shaped_text: &mut ShapedText,
    scripts: &mut Vec<[u8; 4]>,
    style_indices: &mut Vec<u16>,
    inline_flow_indices: &mut Vec<u16>,
) -> Result<u32, PreparationError> {
    shaped_text.clear();
    scripts.clear();
    prepare_style_indices(
        text,
        shaping_runs,
        inline_flow_runs,
        style_indices,
        inline_flow_indices,
    )?;
    shape_range(
        shaper,
        analysis,
        FontSource::Retained(canonical_text),
        text,
        shaping_styles,
        style_indices,
        inline_flow_styles,
        inline_flow_indices,
        0..text.len(),
        shaped_text,
        scripts,
    )
}

pub(crate) fn shape_line(
    shaper: &mut Shaper,
    analysis: &Analysis,
    canonical_text: &ShapedText,
    text: &str,
    shaping_styles: &[ShapingStyle],
    style_indices: &[u16],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_indices: &[u16],
    source: Range<usize>,
) -> Result<LineShapeOutput, PreparationError> {
    let mut shaped_text = ShapedText::new();
    let mut scripts = Vec::new();
    let selected_clusters = shape_range(
        shaper,
        analysis,
        FontSource::Retained(canonical_text),
        text,
        shaping_styles,
        style_indices,
        inline_flow_styles,
        inline_flow_indices,
        source,
        &mut shaped_text,
        &mut scripts,
    )?;
    let shaped_glyphs = shaped_glyph_count(&shaped_text);
    Ok(LineShapeOutput {
        shaped_text,
        scripts,
        resolved_clusters: selected_clusters,
        shaped_glyphs,
    })
}

fn prepare_style_indices(
    text: &str,
    shaping_runs: &[ShapingRun],
    inline_flow_runs: &[InlineFlowRun],
    shaping: &mut Vec<u16>,
    inline_flow: &mut Vec<u16>,
) -> Result<(), PreparationError> {
    shaping.clear();
    let characters = text.chars().count();
    shaping.reserve(characters);
    for run in shaping_runs {
        let index =
            u16::try_from(run.style().index()).map_err(|_| PreparationError::invalid_output())?;
        let range = run.bytes();
        let run_text = text
            .get(range.start as usize..range.end as usize)
            .ok_or_else(PreparationError::invalid_output)?;
        shaping.extend(core::iter::repeat_n(index, run_text.chars().count()));
    }
    prepare_inline_flow_indices(text, inline_flow_runs, inline_flow)?;
    if shaping.len() != characters || inline_flow.len() != characters {
        return Err(PreparationError::invalid_output());
    }
    Ok(())
}

pub(crate) fn prepare_inline_flow_indices(
    text: &str,
    runs: &[InlineFlowRun],
    indices: &mut Vec<u16>,
) -> Result<(), PreparationError> {
    indices.clear();
    indices.reserve(text.chars().count());
    for run in runs {
        let index =
            u16::try_from(run.style().index()).map_err(|_| PreparationError::invalid_output())?;
        let range = run.bytes();
        let run_text = text
            .get(range.start as usize..range.end as usize)
            .ok_or_else(PreparationError::invalid_output)?;
        indices.extend(core::iter::repeat_n(index, run_text.chars().count()));
    }
    if indices.len() != text.chars().count() {
        return Err(PreparationError::invalid_output());
    }
    Ok(())
}

fn spacing_features(
    authored: &[FontFeature],
    inline_flow: InlineFlowStyle,
    has_joining_behavior: bool,
) -> Cow<'_, [FontFeature]> {
    if inline_flow.spacing().letter() == 0.0 || has_joining_behavior {
        return Cow::Borrowed(authored);
    }
    let liga = Tag::new(b"liga");
    let clig = Tag::new(b"clig");
    let needs_liga = !authored.iter().any(|feature| feature.tag == liga);
    let needs_clig = !authored.iter().any(|feature| feature.tag == clig);
    if !needs_liga && !needs_clig {
        return Cow::Borrowed(authored);
    }
    let mut features =
        Vec::with_capacity(authored.len() + usize::from(needs_liga) + usize::from(needs_clig));
    features.extend_from_slice(authored);
    if needs_liga {
        features.push(FontFeature::new(liga, 0));
    }
    if needs_clig {
        features.push(FontFeature::new(clig, 0));
    }
    Cow::Owned(features)
}

pub(crate) fn shaped_glyph_count(shaped_text: &ShapedText) -> u32 {
    shaped_text.clusters().iter().fold(0_u32, |count, cluster| {
        count.saturating_add(if cluster.glyph_len == u8::MAX {
            1
        } else {
            u32::from(cluster.glyph_len)
        })
    })
}

enum FontSource<'a> {
    Query(&'a mut FontSet),
    Retained(&'a ShapedText),
}

#[expect(
    clippy::too_many_arguments,
    reason = "The private shaping seam makes every retained input explicit"
)]
fn shape_range(
    shaper: &mut Shaper,
    analysis: &Analysis,
    mut font_source: FontSource<'_>,
    text: &str,
    shaping_styles: &[ShapingStyle],
    style_indices: &[u16],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_indices: &[u16],
    source: Range<usize>,
    shaped_text: &mut ShapedText,
    scripts: &mut Vec<[u8; 4]>,
) -> Result<u32, PreparationError> {
    if source.start > source.end
        || text.get(source.clone()).is_none()
        || !text.is_char_boundary(source.start)
        || !text.is_char_boundary(source.end)
    {
        return Err(PreparationError::invalid_output());
    }
    let analysis_data = AnalysisDataSources::new();
    let selected_clusters = Cell::new(0_u32);

    let split_after = |range: TextRange| {
        split_item_after(&range, style_indices)
            || split_item_after(&range, inline_flow_indices)
            || range.byte_range.end == source.start
            || range.byte_range.end == source.end
    };
    for item in analysis.itemize(text, split_after) {
        if item.range.byte_range.end <= source.start || item.range.byte_range.start >= source.end {
            continue;
        }
        if item.range.byte_range.start < source.start || item.range.byte_range.end > source.end {
            return Err(PreparationError::invalid_output());
        }
        let style = &shaping_styles[usize::from(style_indices[item.range.char_range.start])];
        let inline_flow =
            inline_flow_styles[usize::from(inline_flow_indices[item.range.char_range.start])];
        let script = item.script.to_bytes();
        let missing_font = Cell::new(false);
        let item_info = analysis
            .char_info()
            .get(item.range.char_range.clone())
            .ok_or_else(PreparationError::invalid_output)?;
        let features = spacing_features(
            style.features(),
            inline_flow,
            item_info
                .iter()
                .any(|info| has_joining_behavior(info.joining_type)),
        );
        let options = ShapeOptions {
            font_size: style.font_size(),
            language: style.language(),
            features: features.as_ref(),
            variations: style.variations(),
            char_style_indices: style_indices,
        };
        let appended = match &mut font_source {
            FontSource::Query(fonts) => {
                let (collection, source_cache) = fonts.resources_mut();
                let mut query = collection.query(source_cache);
                query.set_families(query_families(style.font_families()));
                query.set_attributes(Attributes::new(
                    style.font_width(),
                    style.font_style(),
                    style.font_weight(),
                ));
                let language = style.language();
                query.set_fallbacks(FallbackKey::new(item.script, language.as_ref()));
                shaper.shape_item(
                    text,
                    analysis,
                    &item,
                    &options,
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
                )
            }
            FontSource::Retained(canonical_text) => shaper.shape_item(
                text,
                analysis,
                &item,
                &options,
                |cluster| {
                    let font = retained_font(canonical_text, cluster);
                    if font.is_some() {
                        selected_clusters.set(selected_clusters.get().saturating_add(1));
                    } else {
                        missing_font.set(true);
                    }
                    font
                },
                shaped_text,
            ),
        };
        if missing_font.get() {
            shaped_text.clear();
            scripts.clear();
            return Err(PreparationError::missing_font());
        }
        scripts.extend(core::iter::repeat_n(script, appended.len()));
    }
    if !source.is_empty() && shaped_text.runs().is_empty() {
        scripts.clear();
        return Err(PreparationError::missing_font());
    }
    Ok(selected_clusters.get())
}

pub(crate) const fn has_joining_behavior(joining_type: JoiningType) -> bool {
    matches!(
        joining_type,
        JoiningType::JoinCausing
            | JoiningType::DualJoining
            | JoiningType::LeftJoining
            | JoiningType::RightJoining
    )
}

fn retained_font(canonical_text: &ShapedText, cluster: &CharCluster) -> Option<FontInstance> {
    let source = cluster.range();
    let start = source.start as usize;
    let end = source.end as usize;
    let run = canonical_text
        .runs()
        .iter()
        .find(|run| run.range.byte_range.start <= start && end <= run.range.byte_range.end)?;
    canonical_text.fonts().get(run.font_index).cloned()
}

pub(crate) fn split_item_after(range: &TextRange, style_indices: &[u16]) -> bool {
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
