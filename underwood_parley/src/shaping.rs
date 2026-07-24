// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Paragraph analysis, itemization, font selection, and initial shaping.
//!
//! This module owns projection into public Parley shaping operations; it
//! explicitly does not own line-breaking policy or portable scene records.

use alloc::vec::Vec;
use core::cell::Cell;

use fontique::{Attributes, FallbackKey, QueryFamily, QueryStatus};
use parley_core::{
    Analysis, AnalysisDataSources, AnalysisOptions, Analyzer, FontInstance, ShapeOptions,
    ShapedText, Shaper,
    itemize::TextRange,
    shape::{CharCluster, Status},
};
use underwood::adapter::{PreparationError, ShapingRun};
use underwood::{FontData, FontFamilyName, ShapingStyle};

use crate::font::FontSet;

pub(crate) fn analyze_text(analyzer: &mut Analyzer, text: &str) -> Analysis {
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

pub(crate) fn shape_paragraph(
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

    let split_after = |range: TextRange| split_item_after(&range, &style_indices);
    for item in analysis.itemize(text, split_after) {
        let style = &shaping_styles[usize::from(style_indices[item.range.char_range.start])];
        let script = item.script.to_bytes();
        let missing_font = Cell::new(false);
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
