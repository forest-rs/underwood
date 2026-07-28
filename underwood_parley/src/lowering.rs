// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Portable glyph lowering and paint/source coverage.
//!
//! This module owns conversion from Parley shaped records to backend-neutral
//! prepared glyphs; it explicitly does not own shaping, line-breaking, or
//! renderer policy.

use alloc::vec::Vec;
use core::ops::Range;

use fontique::Synthesis;
use parley_engine::{Analysis, ShapedText, shape::ClusterData};
use underwood::adapter::{FontSynthesis, PreparationError, PreparedParagraphData};
use underwood::{FontVariation, Tag, Vec2};

pub(crate) fn lower_glyphs_into(
    text: &str,
    analysis: &Analysis,
    char_starts: &[u32],
    shaped_text: &ShapedText,
    run: &parley_engine::ShapedRun,
    cluster_range: Range<usize>,
    output: &mut PreparedParagraphData,
) -> Result<(), PreparationError> {
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
        if !source_contributes_to_shaping(analysis, char_starts, &source) {
            return Ok(());
        }
        lower_cluster_glyphs(shaped_text, run, cluster, |glyph| {
            let advance = Vec2::new(f64::from(glyph.advance), 0.0);
            output.push_glyph(
                glyph.id,
                source.clone(),
                advance,
                Vec2::new(f64::from(glyph.x), -f64::from(glyph.y)),
            )
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
    Ok(())
}

fn source_contributes_to_shaping(
    analysis: &Analysis,
    char_starts: &[u32],
    source: &Range<u32>,
) -> bool {
    let char_start = char_index(char_starts, source.start);
    let char_end = char_index(char_starts, source.end);
    analysis.char_info()[char_start..char_end]
        .iter()
        .any(|info| info.contributes_to_shaping())
}

pub(crate) fn append_unrendered_source(
    text: &str,
    analysis: &Analysis,
    char_starts: &[u32],
    source: Range<usize>,
    glyphs: Range<usize>,
    output: &mut PreparedParagraphData,
) -> Result<(), PreparationError> {
    let source_text = text
        .get(source.clone())
        .ok_or_else(PreparationError::invalid_output)?;
    let char_start = char_index(
        char_starts,
        u32::try_from(source.start).map_err(|_| PreparationError::invalid_output())?,
    );
    let mut pending: Option<Range<u32>> = None;
    for (index, (offset, character)) in source_text.char_indices().enumerate() {
        let start = source
            .start
            .checked_add(offset)
            .ok_or_else(PreparationError::invalid_output)?;
        let end = start
            .checked_add(character.len_utf8())
            .ok_or_else(PreparationError::invalid_output)?;
        let range = source_range(&(start..end));
        if output.renders(glyphs.clone(), range.clone())? {
            continue;
        }
        let info = analysis
            .char_info()
            .get(char_start + index)
            .ok_or_else(PreparationError::invalid_output)?;
        if info.contributes_to_shaping() {
            return Err(PreparationError::invalid_output());
        }
        if let Some(previous) = pending.as_mut()
            && previous.end == range.start
        {
            previous.end = range.end;
        } else {
            if let Some(previous) = pending.replace(range) {
                output.push_unrendered_source(previous)?;
            }
        }
    }
    if let Some(pending) = pending {
        output.push_unrendered_source(pending)?;
    }
    Ok(())
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "ParagraphFormation::form rejects text longer than u32 before indexing"
)]
pub(crate) fn index_char_starts(text: &str, output: &mut Vec<u32>) {
    output.clear();
    output.reserve(text.chars().count().saturating_add(1));
    for (byte, _) in text.char_indices() {
        output.push(byte as u32);
    }
    output.push(text.len() as u32);
}

fn char_index(char_starts: &[u32], byte: u32) -> usize {
    char_starts
        .binary_search(&byte)
        .expect("Parley cluster sources end on scalar boundaries")
}

fn cluster_source(
    run: &parley_engine::ShapedRun,
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
    Ok(source_range(&(start..end)))
}

fn lower_cluster_glyphs(
    shaped_text: &ShapedText,
    run: &parley_engine::ShapedRun,
    cluster: &ClusterData,
    mut lower: impl FnMut(parley_engine::Glyph) -> Result<(), PreparationError>,
) -> Result<(), PreparationError> {
    if cluster.glyph_len == u8::MAX {
        return lower(parley_engine::Glyph {
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

#[expect(
    clippy::cast_possible_truncation,
    reason = "ParagraphFormation::form rejects text longer than u32 before lowering"
)]
pub(crate) fn source_range(range: &Range<usize>) -> Range<u32> {
    range.start as u32..range.end as u32
}

pub(crate) fn portable_synthesis(synthesis: Synthesis) -> Result<FontSynthesis, PreparationError> {
    FontSynthesis::try_new(
        synthesis
            .variation_settings()
            .iter()
            .map(|(tag, value)| FontVariation::new(Tag::from_bytes(tag.to_be_bytes()), *value)),
        synthesis.embolden(),
        synthesis.skew(),
    )
}
