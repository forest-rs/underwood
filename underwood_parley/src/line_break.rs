// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private line-formation policy over retained Parley facts.
//!
//! This module owns constraint handling, legal-break selection, line metrics,
//! and line-local bidi ordering; it explicitly does not own shaping, font
//! selection, portable lowering, or scene construction.

use alloc::vec::Vec;
use core::ops::Range;

use parley_core::{Analysis, Boundary, ShapedText, Shaper, shape::Whitespace};
use underwood::adapter::{InlineFlowRun, LineBreakReason, ParagraphConstraints, PreparationError};
use underwood::{InlineFlowStyle, TextConstraint};

#[derive(Clone, Debug)]
pub(crate) struct LinePlan {
    pub(crate) clusters: Range<usize>,
    pub(crate) source: Range<usize>,
    pub(crate) reason: LineBreakReason,
    pub(crate) advance: f64,
    pub(crate) baseline: f64,
    pub(crate) height: f64,
    pub(crate) content_ascent: f64,
    pub(crate) content_descent: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct LogicalCluster {
    pub(crate) run: usize,
    pub(crate) index: usize,
    pub(crate) source: Range<usize>,
    pub(crate) boundary: Boundary,
    pub(crate) source_char: char,
    pub(crate) whitespace: Whitespace,
    pub(crate) ligature_component: bool,
    pub(crate) advance: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct RunPiece {
    pub(crate) run: usize,
    pub(crate) clusters: Range<usize>,
}

pub(crate) fn form_lines(
    shaper: &mut Shaper,
    analysis: &Analysis,
    text: &str,
    shaped_text: &mut ShapedText,
    clusters: &mut Vec<LogicalCluster>,
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
    constraints: ParagraphConstraints,
    plans: &mut Vec<LinePlan>,
) -> Result<u32, PreparationError> {
    plans.clear();
    if text.is_empty() {
        return Ok(0);
    }
    if clusters.is_empty() {
        return Err(PreparationError::invalid_output());
    }

    let mut break_reshapes = 0_u32;
    let mut start = 0_usize;
    while start < clusters.len() {
        let choice = choose_line(clusters, start, constraints.text())?;
        let (end, advance, reshaped) = if choice.reason == LineBreakReason::Regular {
            commit_regular_break(
                shaper,
                analysis,
                text,
                shaped_text,
                clusters,
                start,
                choice.end,
                constraints.text(),
            )?
        } else {
            (choice.end, choice.advance, false)
        };
        if reshaped {
            break_reshapes = break_reshapes.saturating_add(1);
        }
        plans.push(make_line_plan(
            shaped_text,
            clusters,
            inline_flow_styles,
            inline_flow_runs,
            start..end,
            choice.reason,
            advance,
            None,
        )?);
        start = end;
    }

    if plans
        .last()
        .is_some_and(|plan| plan.reason == LineBreakReason::Mandatory)
    {
        let previous = plans
            .last()
            .cloned()
            .ok_or_else(PreparationError::invalid_output)?;
        plans.push(make_line_plan(
            shaped_text,
            clusters,
            inline_flow_styles,
            inline_flow_runs,
            clusters.len()..clusters.len(),
            LineBreakReason::End,
            0.0,
            Some(&previous),
        )?);
    }
    Ok(break_reshapes)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineChoice {
    pub(crate) end: usize,
    pub(crate) reason: LineBreakReason,
    pub(crate) advance: f64,
}

pub(crate) fn choose_line(
    clusters: &[LogicalCluster],
    start: usize,
    constraint: TextConstraint,
) -> Result<LineChoice, PreparationError> {
    let mut index = start;
    let mut advance = 0.0_f64;
    let mut last_opportunity: Option<(usize, f64)> = None;
    while index < clusters.len() {
        let cluster = &clusters[index];
        if cluster.boundary == Boundary::Line && !cluster.ligature_component && index > start {
            if constraint == TextConstraint::MinContent {
                return Ok(LineChoice {
                    end: index,
                    reason: LineBreakReason::Regular,
                    advance,
                });
            }
            if matches!(constraint, TextConstraint::Wrap(_)) {
                last_opportunity = Some((index, advance));
            }
        }

        let next_advance = advance + cluster.advance;
        if cluster.whitespace == Whitespace::Newline {
            advance = next_advance;
            index += 1;
            let cr_before_lf = cluster.source_char == '\r'
                && clusters
                    .get(index)
                    .is_some_and(|next| next.source_char == '\n');
            if cr_before_lf {
                continue;
            }
            return Ok(LineChoice {
                end: index,
                reason: LineBreakReason::Mandatory,
                advance,
            });
        }

        if matches!(constraint, TextConstraint::Wrap(width) if next_advance > width.get())
            && let Some((end, opportunity_advance)) = last_opportunity
        {
            return Ok(LineChoice {
                end,
                reason: LineBreakReason::Regular,
                advance: opportunity_advance,
            });
        }
        advance = next_advance;
        index += 1;
    }
    Ok(LineChoice {
        end: clusters.len(),
        reason: LineBreakReason::End,
        advance,
    })
}

pub(crate) fn commit_regular_break(
    shaper: &mut Shaper,
    analysis: &Analysis,
    text: &str,
    shaped_text: &mut ShapedText,
    clusters: &mut Vec<LogicalCluster>,
    start: usize,
    mut end: usize,
    constraint: TextConstraint,
) -> Result<(usize, f64, bool), PreparationError> {
    loop {
        let pos = clusters
            .get(end)
            .ok_or_else(PreparationError::invalid_output)?
            .source
            .start;
        let reshaped = !shaped_text.unsafe_break_region(pos).is_empty();
        if reshaped {
            shaper.apply_break(text, analysis, shaped_text, pos);
            *clusters = collect_logical_clusters(text, shaped_text)?;
        }
        let advance = clusters[start..end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum();
        if match constraint {
            TextConstraint::MinContent => true,
            TextConstraint::MaxContent => false,
            TextConstraint::Wrap(width) => advance <= width.get(),
        } {
            return Ok((end, advance, reshaped));
        }

        let previous = (start + 1..end).rev().find(|&index| {
            let cluster = &clusters[index];
            cluster.boundary == Boundary::Line && !cluster.ligature_component
        });
        let Some(previous) = previous else {
            return Ok((end, advance, reshaped));
        };
        if reshaped {
            shaper.apply_concat(text, analysis, shaped_text, pos);
            *clusters = collect_logical_clusters(text, shaped_text)?;
        }
        end = previous;
    }
}

pub(crate) fn collect_logical_clusters(
    text: &str,
    shaped_text: &ShapedText,
) -> Result<Vec<LogicalCluster>, PreparationError> {
    let mut clusters = Vec::with_capacity(shaped_text.clusters().len());
    for (run_index, run) in shaped_text.runs().iter().enumerate() {
        for cluster_index in run.clusters_range.clone() {
            let cluster = shaped_text
                .clusters()
                .get(cluster_index)
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
            if text.get(start..end).is_none() {
                return Err(PreparationError::invalid_output());
            }
            clusters.push(LogicalCluster {
                run: run_index,
                index: cluster_index,
                source: start..end,
                boundary: cluster.info.boundary(),
                source_char: cluster.info.source_char(),
                whitespace: cluster.info.whitespace(),
                ligature_component: cluster.is_ligature_component(),
                advance: f64::from(cluster.advance),
            });
        }
    }
    if !text.is_empty() && clusters.is_empty() {
        return Err(PreparationError::invalid_output());
    }
    Ok(clusters)
}

fn make_line_plan(
    shaped_text: &ShapedText,
    clusters: &[LogicalCluster],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
    logical_range: Range<usize>,
    reason: LineBreakReason,
    advance: f64,
    empty_metrics: Option<&LinePlan>,
) -> Result<LinePlan, PreparationError> {
    if logical_range.is_empty() {
        let metrics = empty_metrics.ok_or_else(PreparationError::invalid_output)?;
        let at = clusters.last().map_or(0, |cluster| cluster.source.end);
        return Ok(LinePlan {
            clusters: shaped_text.clusters().len()..shaped_text.clusters().len(),
            source: at..at,
            reason,
            advance,
            baseline: metrics.baseline,
            height: metrics.height,
            content_ascent: metrics.content_ascent,
            content_descent: metrics.content_descent,
        });
    }

    let first = clusters
        .get(logical_range.start)
        .ok_or_else(PreparationError::invalid_output)?;
    let last = clusters
        .get(logical_range.end - 1)
        .ok_or_else(PreparationError::invalid_output)?;
    let mut above = 0.0_f64;
    let mut below = 0.0_f64;
    let mut content_ascent = 0.0_f64;
    let mut content_descent = 0.0_f64;
    for cluster in &clusters[logical_range.clone()] {
        let run = shaped_text
            .runs()
            .get(cluster.run)
            .ok_or_else(PreparationError::invalid_output)?;
        let multiplier =
            inline_flow_multiplier(&cluster.source, inline_flow_styles, inline_flow_runs)?;
        let requested_height = f64::from(run.font_size) * f64::from(multiplier);
        let ascent = f64::from(run.font_metrics.ascent);
        let descent = f64::from(run.font_metrics.descent);
        let half_leading = (requested_height - (ascent + descent)) / 2.0;
        let run_above = ascent + half_leading;
        above = above.max(run_above);
        below = below.max(requested_height - run_above);
        content_ascent = content_ascent.max(ascent);
        content_descent = content_descent.max(descent);
    }
    Ok(LinePlan {
        clusters: first.index..last.index + 1,
        source: first.source.start..last.source.end,
        reason,
        advance,
        baseline: above,
        height: above + below,
        content_ascent,
        content_descent,
    })
}

fn inline_flow_multiplier(
    source: &Range<usize>,
    styles: &[InlineFlowStyle],
    runs: &[InlineFlowRun],
) -> Result<f32, PreparationError> {
    let mut multiplier = 0.0_f32;
    for run in runs {
        let bytes = run.bytes();
        if bytes.start as usize >= source.end || bytes.end as usize <= source.start {
            continue;
        }
        let style = styles
            .get(run.style().index())
            .ok_or_else(PreparationError::invalid_output)?;
        multiplier = multiplier.max(style.line_height().multiplier());
    }
    if multiplier <= 0.0 {
        return Err(PreparationError::invalid_output());
    }
    Ok(multiplier)
}

pub(crate) fn line_run_pieces(
    shaped_text: &ShapedText,
    clusters: Range<usize>,
) -> Result<Vec<RunPiece>, PreparationError> {
    let mut pieces = Vec::new();
    for (run_index, run) in shaped_text.runs().iter().enumerate() {
        let start = run.clusters_range.start.max(clusters.start);
        let end = run.clusters_range.end.min(clusters.end);
        if start < end {
            pieces.push(RunPiece {
                run: run_index,
                clusters: start..end,
            });
        }
    }
    if !clusters.is_empty()
        && pieces
            .iter()
            .map(|piece| piece.clusters.len())
            .sum::<usize>()
            != clusters.len()
    {
        return Err(PreparationError::invalid_output());
    }
    Ok(pieces)
}

pub(crate) fn reorder_visual_pieces(shaped_text: &ShapedText, pieces: &mut [RunPiece]) {
    let mut max_level = 0_u8;
    let mut lowest_odd_level = u8::MAX;
    for piece in pieces.iter() {
        let level = shaped_text.runs()[piece.run].bidi_level;
        max_level = max_level.max(level);
        if level & 1 != 0 {
            lowest_odd_level = lowest_odd_level.min(level);
        }
    }
    if lowest_odd_level == u8::MAX {
        return;
    }
    for level in (lowest_odd_level..=max_level).rev() {
        let mut start = 0_usize;
        while start < pieces.len() {
            if shaped_text.runs()[pieces[start].run].bidi_level < level {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < pieces.len() && shaped_text.runs()[pieces[end].run].bidi_level >= level {
                end += 1;
            }
            pieces[start..end].reverse();
            start = end;
        }
    }
}
