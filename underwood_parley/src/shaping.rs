// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Paragraph analysis, itemization, font selection, and shaping.
//!
//! This module owns projection into public Parley shaping operations,
//! including line-final reuse of canonical fonts; it explicitly does not own
//! line-breaking policy or portable scene records.

use alloc::vec::Vec;
use core::cell::Cell;
use core::ops::Range;

use fontique::{Attributes, FallbackKey, QueryFamily, QueryStatus};
use parley_engine::{
    Analysis, AnalysisDataSources, AnalysisOptions, Analyzer, FontInstance, ShapeOptions,
    ShapedText, Shaper,
    itemize::TextRange,
    shape::{CharCluster, Status},
};
use underwood::adapter::{PreparationError, ShapingRun};
use underwood::{BaseDirection, FontData, FontFamilyName, ShapingStyle};

use crate::font::FontSet;
use crate::line_break::LineShapeOutput;

pub(crate) fn analyze_text(
    analyzer: &mut Analyzer,
    text: &str,
    base_direction: BaseDirection,
) -> Analysis {
    let mut analysis = Analysis::new();
    analyzer.analyze(
        text,
        &AnalysisOptions {
            base_direction,
            ..AnalysisOptions::default()
        },
        &mut analysis,
    );
    analysis
}

pub(crate) fn shape_paragraph(
    shaper: &mut Shaper,
    analysis: &Analysis,
    fonts: &mut FontSet,
    text: &str,
    shaping_styles: &[ShapingStyle],
    shaping_runs: &[ShapingRun],
    shaped_text: &mut ShapedText,
    scripts: &mut Vec<[u8; 4]>,
    style_indices: &mut Vec<u16>,
) -> Result<u32, PreparationError> {
    shaped_text.clear();
    scripts.clear();
    style_indices.clear();
    style_indices.reserve(text.chars().count());
    for run in shaping_runs {
        let index =
            u16::try_from(run.style().index()).map_err(|_| PreparationError::invalid_output())?;
        let range = run.bytes();
        let run_text = text
            .get(range.start as usize..range.end as usize)
            .ok_or_else(PreparationError::invalid_output)?;
        style_indices.extend(core::iter::repeat_n(index, run_text.chars().count()));
    }
    shape_range(
        shaper,
        analysis,
        FontSource::Query(fonts),
        text,
        shaping_styles,
        style_indices,
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
        let script = item.script.to_bytes();
        let missing_font = Cell::new(false);
        let options = ShapeOptions {
            font_size: style.font_size(),
            language: style.language(),
            features: style.features(),
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
