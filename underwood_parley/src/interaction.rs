// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Source-complete interaction units.
//!
//! This module owns analysis-derived grapheme units and visual slices; it
//! explicitly does not own cursor graphs, line selection, glyph shaping, or
//! document-edit policy.

use alloc::vec::Vec;
use core::{mem::size_of, ops::Range};

use parley_engine::{Analysis, Boundary, ShapedText, shape::Whitespace};
use underwood::TextAffinity;
use underwood::adapter::{
    ClusterBoundary, ClusterWhitespace, PreparationError, PreparedClusterSide,
    PreparedInteractionUnit, PreparedParagraphData,
};

use crate::line_break::RunPiece;
use crate::lowering::source_range;

#[cfg(test)]
pub(crate) fn collect_analysis_units(text: &str, analysis: &Analysis) -> Vec<Range<usize>> {
    let mut units = Vec::new();
    collect_analysis_units_into(text, analysis, &mut units);
    units
}

pub(crate) fn collect_analysis_units_into(
    text: &str,
    analysis: &Analysis,
    units: &mut Vec<Range<usize>>,
) {
    units.clear();
    let mut start = None;
    for ((byte, _), info) in text.char_indices().zip(analysis.char_info()) {
        if info.is_grapheme_start()
            && let Some(previous) = start.replace(byte)
        {
            units.push(previous..byte);
        }
    }
    if let Some(start) = start {
        units.push(start..text.len());
    }
}

#[derive(Debug, Default)]
pub(crate) struct InteractionScratch {
    visual_slices: Vec<VisualInteractionSlice>,
    unit_parts: Vec<(Range<u32>, f64)>,
}

impl InteractionScratch {
    pub(crate) fn accounted_owned_bytes(&self) -> usize {
        size_of::<VisualInteractionSlice>()
            .saturating_mul(self.visual_slices.capacity())
            .saturating_add(
                size_of::<(Range<u32>, f64)>().saturating_mul(self.unit_parts.capacity()),
            )
    }
}

#[derive(Debug)]
struct VisualInteractionSlice {
    source: Range<u32>,
    advance: f64,
    bidi_level: u8,
    script: [u8; 4],
    boundary: Boundary,
    whitespace: Whitespace,
}

pub(crate) fn lower_visual_units(
    text: &str,
    analysis: &Analysis,
    char_starts: &[u32],
    shaped_text: &ShapedText,
    pieces: &[RunPiece],
    interaction_units: &[Range<usize>],
    line_source: &Range<usize>,
    mandatory_line_end: bool,
    scratch: &mut InteractionScratch,
    output: &mut PreparedParagraphData,
) -> Result<(), PreparationError> {
    let slice_count = pieces.iter().map(|piece| piece.clusters.len()).sum();
    let InteractionScratch {
        visual_slices,
        unit_parts,
    } = scratch;
    visual_slices.clear();
    visual_slices.reserve(slice_count);
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
    let mut current_owner = None;
    let mut current_start = 0;
    for (index, slice) in visual_slices.iter().enumerate() {
        let owner = interaction_units
            .partition_point(|unit| unit.start <= slice.source.start as usize)
            .checked_sub(1)
            .filter(|&index| slice.source.end as usize <= interaction_units[index].end)
            .ok_or_else(PreparationError::invalid_output)?;
        if !expected.contains(&owner) {
            return Err(PreparationError::invalid_output());
        }
        if current_owner == Some(owner) {
            continue;
        }
        if let Some(previous) = current_owner {
            lower_prepared_unit(
                text,
                analysis,
                char_starts,
                &interaction_units[previous],
                &visual_slices[current_start..index],
                mandatory_line_end && interaction_units[previous].end == line_source.end,
                unit_parts,
                output,
            )?;
        }
        current_owner = Some(owner);
        current_start = index;
    }
    if let Some(owner) = current_owner {
        lower_prepared_unit(
            text,
            analysis,
            char_starts,
            &interaction_units[owner],
            &visual_slices[current_start..],
            mandatory_line_end && interaction_units[owner].end == line_source.end,
            unit_parts,
            output,
        )?;
    }
    Ok(())
}

fn lower_visual_slice(
    shaped_text: &ShapedText,
    run: &parley_engine::ShapedRun,
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
        source: source_range(&(start..end)),
        advance: f64::from(cluster.advance),
        bidi_level: run.bidi_level,
        script: run.script.to_bytes(),
        boundary: cluster.info.boundary(),
        whitespace: cluster.info.whitespace(),
    })
}

fn lower_prepared_unit(
    text: &str,
    analysis: &Analysis,
    char_starts: &[u32],
    source: &Range<usize>,
    slices: &[VisualInteractionSlice],
    mandatory_line_end: bool,
    unit_parts: &mut Vec<(Range<u32>, f64)>,
    output: &mut PreparedParagraphData,
) -> Result<(), PreparationError> {
    let logical_first = slices
        .iter()
        .min_by_key(|slice| slice.source.start)
        .ok_or_else(PreparationError::invalid_output)?;
    let bidi_level =
        interaction_unit_bidi_level(analysis, char_starts, source, slices, logical_first)?;
    let boundary = logical_first.boundary;
    let mut whitespace = Whitespace::None;
    for slice in slices {
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
    let source_text = text
        .get(source.clone())
        .ok_or_else(PreparationError::invalid_output)?;
    let western_justification_opportunity =
        source_text == " " && matches!(&logical_first.script, b"Latn" | b"Grek" | b"Cyrl");
    let source = source_range(source);
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
    let unit = PreparedInteractionUnit::try_new_with_justification(
        source,
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
        western_justification_opportunity,
        left,
        right,
    )?;
    unit_parts.clear();
    unit_parts.extend(
        slices
            .iter()
            .map(|slice| (slice.source.clone(), slice.advance)),
    );
    let source = unit.source();
    let mut cursor = source.start;
    while cursor < source.end {
        if let Some(slice) = slices
            .iter()
            .find(|slice| slice.source.start <= cursor && cursor < slice.source.end)
        {
            cursor = slice.source.end;
            continue;
        }
        let next = slices
            .iter()
            .filter_map(|slice| (slice.source.start > cursor).then_some(slice.source.start))
            .min()
            .unwrap_or(source.end);
        if next <= cursor || next > source.end {
            return Err(PreparationError::invalid_output());
        }
        unit_parts.push((cursor..next, 0.0));
        cursor = next;
    }
    output.push_unit(unit, unit_parts.drain(..))
}

/// Resolves one atomic grapheme's caret direction without requiring all shaped
/// slices to share a bidi level.
///
/// Leading format characters can resolve differently from the scalar that
/// actually shapes the visible unit. Prefer the first shaping contributor and
/// fall back to the logical-leading scalar for an all-control unit.
fn interaction_unit_bidi_level(
    analysis: &Analysis,
    char_starts: &[u32],
    source: &Range<usize>,
    slices: &[VisualInteractionSlice],
    logical_first: &VisualInteractionSlice,
) -> Result<u8, PreparationError> {
    if slices
        .iter()
        .all(|slice| slice.bidi_level == logical_first.bidi_level)
    {
        return Ok(logical_first.bidi_level);
    }

    let start = u32::try_from(source.start).map_err(|_| PreparationError::invalid_output())?;
    let end = u32::try_from(source.end).map_err(|_| PreparationError::invalid_output())?;
    let first_character = char_starts
        .binary_search(&start)
        .map_err(|_| PreparationError::invalid_output())?;
    let after_last_character = char_starts
        .binary_search(&end)
        .map_err(|_| PreparationError::invalid_output())?;
    let info = analysis
        .char_info()
        .get(first_character..after_last_character)
        .filter(|info| !info.is_empty())
        .ok_or_else(PreparationError::invalid_output)?;
    let levels = analysis.bidi_levels();
    if !levels.is_empty() && levels.len() != analysis.char_info().len() {
        return Err(PreparationError::invalid_output());
    }
    let level_at = |index: usize| {
        levels
            .get(index)
            .copied()
            .unwrap_or_else(|| analysis.paragraph_level())
    };
    let primary = info
        .iter()
        .position(|info| info.contributes_to_shaping())
        .map_or(first_character, |offset| first_character + offset);
    let bidi_level = level_at(primary);
    if !slices.iter().any(|slice| slice.bidi_level == bidi_level) {
        return Err(PreparationError::invalid_output());
    }
    Ok(bidi_level)
}
