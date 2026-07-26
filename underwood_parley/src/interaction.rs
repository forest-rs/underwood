// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Source-complete interaction units and cursor movement.
//!
//! This module owns analysis-derived grapheme units, visual slices, and cursor
//! transitions; it explicitly does not own line selection, glyph shaping, or
//! document-edit policy.

use alloc::vec::Vec;
use core::ops::Range;

use parley_engine::{Analysis, Boundary, ShapedText, shape::Whitespace};
use underwood::TextAffinity;
use underwood::adapter::{
    ClusterBoundary, ClusterWhitespace, LineBreakReason, PreparationError, PreparedCaret,
    PreparedClusterSide, PreparedCursorMovement, PreparedCursorStep, PreparedInteractionSlice,
    PreparedInteractionUnit, PreparedLine,
};

use crate::line_break::RunPiece;
use crate::lowering::checked_source_range;

pub(crate) fn collect_analysis_units(
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

pub(crate) fn prepared_cursor_movements(
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
