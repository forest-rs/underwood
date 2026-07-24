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
use parley_core::{Analysis, ShapedText, shape::ClusterData};
use underwood::adapter::{FontSynthesis, GlyphPaintCoverage, PreparationError, PreparedGlyph};
use underwood::{FontVariation, Tag, Vec2};

pub(crate) fn lower_glyphs(
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

pub(crate) fn unrendered_source(
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

pub(crate) fn checked_source_range(range: &Range<usize>) -> Result<Range<u32>, PreparationError> {
    let start = u32::try_from(range.start).map_err(|_| PreparationError::invalid_output())?;
    let end = u32::try_from(range.end).map_err(|_| PreparationError::invalid_output())?;
    Ok(start..end)
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
