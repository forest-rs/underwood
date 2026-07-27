// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Source-complete interaction units.
//!
//! This module owns analysis-derived grapheme units and visual slices; it
//! explicitly does not own cursor graphs, line selection, glyph shaping, or
//! document-edit policy.

use alloc::vec::Vec;
use core::ops::Range;

use parley_engine::{Analysis, Boundary, ShapedText, shape::Whitespace};
use underwood::TextAffinity;
use underwood::adapter::{
    ClusterBoundary, ClusterWhitespace, PreparationError, PreparedClusterSide,
    PreparedInteractionSlice, PreparedInteractionUnit,
};

use crate::line_break::RunPiece;
use crate::lowering::checked_source_range;

#[cfg(test)]
pub(crate) fn collect_analysis_units(
    text: &str,
    analysis: &Analysis,
) -> Result<Vec<Range<usize>>, PreparationError> {
    let mut units = Vec::new();
    collect_analysis_units_into(text, analysis, &mut units)?;
    Ok(units)
}

pub(crate) fn collect_analysis_units_into(
    text: &str,
    analysis: &Analysis,
    units: &mut Vec<Range<usize>>,
) -> Result<(), PreparationError> {
    units.clear();
    let mut characters = 0_usize;
    let mut start = None;
    for ((byte, _), info) in text.char_indices().zip(analysis.char_info()) {
        characters += 1;
        if info.is_grapheme_start() {
            if start.is_none() && byte != 0 {
                return Err(PreparationError::invalid_output());
            }
            if let Some(previous) = start.replace(byte) {
                units.push(previous..byte);
            }
        }
    }
    if characters != text.chars().count()
        || characters != analysis.char_info().len()
        || (!text.is_empty() && start.is_none())
    {
        return Err(PreparationError::invalid_output());
    }
    if let Some(start) = start {
        if start >= text.len() || text.get(start..text.len()).is_none() {
            return Err(PreparationError::invalid_output());
        }
        units.push(start..text.len());
    }
    Ok(())
}

struct VisualInteractionSlice {
    source: Range<usize>,
    advance: f64,
    bidi_level: u8,
    script: [u8; 4],
    boundary: Boundary,
    whitespace: Whitespace,
}

pub(crate) struct PreparedInteractionData {
    pub(crate) slices: Vec<PreparedInteractionSlice>,
    pub(crate) units: Vec<PreparedInteractionUnit>,
}

pub(crate) fn lower_visual_units(
    text: &str,
    shaped_text: &ShapedText,
    scripts: &[[u8; 4]],
    pieces: &[RunPiece],
    interaction_units: &[Range<usize>],
    line_source: &Range<usize>,
    mandatory_line_end: bool,
) -> Result<PreparedInteractionData, PreparationError> {
    let slice_count = pieces.iter().map(|piece| piece.clusters.len()).sum();
    let mut visual_slices = Vec::with_capacity(slice_count);
    for piece in pieces {
        let run = shaped_text
            .runs()
            .get(piece.run)
            .ok_or_else(PreparationError::invalid_output)?;
        let script = *scripts
            .get(piece.run)
            .ok_or_else(PreparationError::invalid_output)?;
        if run.bidi_level & 1 == 1 {
            for index in piece.clusters.clone().rev() {
                visual_slices.push(lower_visual_slice(shaped_text, run, script, index)?);
            }
        } else {
            for index in piece.clusters.clone() {
                visual_slices.push(lower_visual_slice(shaped_text, run, script, index)?);
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
    let mut prepared_slices = Vec::with_capacity(visual_slices.len());
    let mut prepared_units = Vec::with_capacity(expected.len());
    let mut current_owner = None;
    let mut current_start = 0;
    for (index, slice) in visual_slices.iter().enumerate() {
        let owner = interaction_units
            .partition_point(|unit| unit.start <= slice.source.start)
            .checked_sub(1)
            .filter(|&index| slice.source.end <= interaction_units[index].end)
            .ok_or_else(PreparationError::invalid_output)?;
        if !expected.contains(&owner) {
            return Err(PreparationError::invalid_output());
        }
        if current_owner == Some(owner) {
            continue;
        }
        if let Some(previous) = current_owner {
            prepared_units.push(lower_prepared_unit(
                text,
                &interaction_units[previous],
                &visual_slices[current_start..index],
                &mut prepared_slices,
                mandatory_line_end && interaction_units[previous].end == line_source.end,
            )?);
        }
        if seen[owner - expected.start] {
            return Err(PreparationError::invalid_output());
        }
        seen[owner - expected.start] = true;
        current_owner = Some(owner);
        current_start = index;
    }
    if let Some(owner) = current_owner {
        prepared_units.push(lower_prepared_unit(
            text,
            &interaction_units[owner],
            &visual_slices[current_start..],
            &mut prepared_slices,
            mandatory_line_end && interaction_units[owner].end == line_source.end,
        )?);
    }
    if seen.iter().any(|seen| !seen) {
        return Err(PreparationError::invalid_output());
    }
    Ok(PreparedInteractionData {
        slices: prepared_slices,
        units: prepared_units,
    })
}

fn lower_visual_slice(
    shaped_text: &ShapedText,
    run: &parley_engine::ShapedRun,
    script: [u8; 4],
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
        script,
        boundary: cluster.info.boundary(),
        whitespace: cluster.info.whitespace(),
    })
}

fn lower_prepared_unit(
    text: &str,
    source: &Range<usize>,
    slices: &[VisualInteractionSlice],
    prepared_slices: &mut Vec<PreparedInteractionSlice>,
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
        || slices.iter().any(|slice| slice.script != first.script)
    {
        return Err(PreparationError::invalid_output());
    }
    let bidi_level = first.bidi_level;
    let boundary = first.boundary;
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
        source_text == " " && matches!(&first.script, b"Latn" | b"Grek" | b"Cyrl");
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
    let slice_start = prepared_slices.len();
    let mut advance = 0.0;
    for slice in slices {
        advance += slice.advance;
        if !advance.is_finite() {
            return Err(PreparationError::invalid_output());
        }
        prepared_slices.push(PreparedInteractionSlice::try_new(
            checked_source_range(&slice.source)?,
            slice.advance,
        )?);
    }
    let slice_end = prepared_slices.len();
    PreparedInteractionUnit::try_new_with_justification(
        source,
        slice_start..slice_end,
        advance,
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
    )
}
