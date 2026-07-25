// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private line-formation policy over retained Parley facts.
//!
//! This module owns constraint handling, legal-break selection, line metrics,
//! and line-local bidi ordering; it explicitly does not own shaping, font
//! selection, portable lowering, or scene construction.

use alloc::vec::Vec;
use core::ops::Range;

use parley_engine::ShapedText;
use underwood::adapter::{InlineFlowRun, LineBreakReason, ParagraphConstraints, PreparationError};
use underwood::{InlineFlowStyle, OverflowWrap, TextConstraint, TextWrapMode};

use crate::line_former::{
    CandidateBreak, CommitOutcome, FormationConstraint, LineCandidate, LineFormer, LineFormerError,
    LineFormerWork, LineLimits, LineMeasurements,
};

pub(crate) use crate::line_former::LogicalCluster;

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
pub(crate) struct FormedLine {
    pub(crate) plan: LinePlan,
    pub(crate) shaped_text: ShapedText,
    pub(crate) scripts: Vec<[u8; 4]>,
}

#[derive(Clone, Debug)]
pub(crate) struct LineShapeOutput {
    pub(crate) shaped_text: ShapedText,
    pub(crate) scripts: Vec<[u8; 4]>,
    pub(crate) resolved_clusters: u32,
    pub(crate) shaped_glyphs: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LineFormationWork {
    pub(crate) reshapes: u32,
    pub(crate) resolved_clusters: u32,
    pub(crate) shaped_runs: u32,
    pub(crate) shaped_glyphs: u32,
    pub(crate) candidates: u32,
    pub(crate) rejected_candidates: u32,
    pub(crate) checkpoint_restores: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct RunPiece {
    pub(crate) run: usize,
    pub(crate) clusters: Range<usize>,
}

pub(crate) fn form_lines(
    text: &str,
    canonical_text: &ShapedText,
    canonical_scripts: &[[u8; 4]],
    clusters: &[LogicalCluster],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
    constraints: ParagraphConstraints,
    lines: &mut Vec<FormedLine>,
    mut shape_line: impl FnMut(Range<usize>) -> Result<LineShapeOutput, PreparationError>,
) -> Result<LineFormationWork, PreparationError> {
    lines.clear();
    if text.is_empty() {
        return Ok(LineFormationWork::default());
    }
    if clusters.is_empty() {
        return Err(PreparationError::invalid_output());
    }

    let constraint = formation_constraint(constraints.text());
    let limits = line_limits(constraint);
    let mut former =
        LineFormer::new(clusters, constraint).map_err(|_| PreparationError::invalid_output())?;
    let first_candidate = former
        .candidate()
        .map_err(map_former_error)?
        .ok_or_else(PreparationError::invalid_output)?;
    validate_candidate(first_candidate)?;
    if first_candidate.reason() == CandidateBreak::End && first_candidate.end() == clusters.len() {
        let plan = make_line_plan(
            canonical_text,
            clusters,
            inline_flow_styles,
            inline_flow_runs,
            first_candidate.clusters(),
            line_break_reason(first_candidate.reason()),
            first_candidate.canonical_advance(),
            None,
        )?;
        match former
            .commit(
                first_candidate,
                LineMeasurements {
                    advance: plan.advance,
                    height: plan.height,
                },
                limits,
            )
            .map_err(map_former_error)?
        {
            CommitOutcome::Accepted(_) => {}
            CommitOutcome::Retry(_) | CommitOutcome::SlotRejected => {
                return Err(PreparationError::invalid_output());
            }
        }
        lines.push(FormedLine {
            plan,
            shaped_text: canonical_text.clone(),
            scripts: canonical_scripts.to_vec(),
        });
        let mut work = LineFormationWork::default();
        record_former_work(&mut work, former.work());
        return Ok(work);
    }

    let mut work = LineFormationWork::default();
    let mut candidate = first_candidate;
    loop {
        let checkpoint = former.checkpoint(lines.len());
        loop {
            validate_candidate(candidate)?;
            let source = candidate.source();
            let output = shape_line(source.clone())?;
            let LineShapeOutput {
                shaped_text,
                scripts,
                resolved_clusters,
                shaped_glyphs,
            } = output;
            work.reshapes = work.reshapes.saturating_add(1);
            work.resolved_clusters = work.resolved_clusters.saturating_add(resolved_clusters);
            work.shaped_runs = work
                .shaped_runs
                .saturating_add(u32::try_from(shaped_text.runs().len()).unwrap_or(u32::MAX));
            work.shaped_glyphs = work.shaped_glyphs.saturating_add(shaped_glyphs);
            let formed_clusters = collect_logical_clusters(text, &shaped_text)?;
            if formed_clusters.first().map(|cluster| cluster.source.start) != Some(source.start)
                || formed_clusters.last().map(|cluster| cluster.source.end) != Some(source.end)
            {
                return Err(PreparationError::invalid_output());
            }
            let advance = formed_clusters.iter().map(|cluster| cluster.advance).sum();
            let plan = make_line_plan(
                &shaped_text,
                &formed_clusters,
                inline_flow_styles,
                inline_flow_runs,
                0..formed_clusters.len(),
                line_break_reason(candidate.reason()),
                advance,
                None,
            )?;
            if scripts.len() != shaped_text.runs().len() {
                return Err(PreparationError::invalid_output());
            }
            match former
                .commit(
                    candidate,
                    LineMeasurements {
                        advance: plan.advance,
                        height: plan.height,
                    },
                    limits,
                )
                .map_err(map_former_error)?
            {
                CommitOutcome::Accepted(_) => {
                    lines.push(FormedLine {
                        plan,
                        shaped_text,
                        scripts,
                    });
                    break;
                }
                CommitOutcome::Retry(retry) => candidate = retry,
                CommitOutcome::SlotRejected => {
                    former
                        .restore(checkpoint, lines)
                        .map_err(map_former_error)?;
                    return Err(PreparationError::invalid_output());
                }
            }
        }
        let Some(next) = former.candidate().map_err(map_former_error)? else {
            break;
        };
        candidate = next;
    }

    if former.needs_terminal_empty_line() {
        let previous = lines
            .last()
            .map(|line| line.plan.clone())
            .ok_or_else(PreparationError::invalid_output)?;
        lines.push(FormedLine {
            plan: make_line_plan(
                canonical_text,
                clusters,
                inline_flow_styles,
                inline_flow_runs,
                clusters.len()..clusters.len(),
                LineBreakReason::End,
                0.0,
                Some(&previous),
            )?,
            shaped_text: ShapedText::new(),
            scripts: Vec::new(),
        });
    }
    record_former_work(&mut work, former.work());
    Ok(work)
}

fn formation_constraint(constraint: TextConstraint) -> FormationConstraint {
    match constraint {
        TextConstraint::MinContent => FormationConstraint::MinContent,
        TextConstraint::MaxContent => FormationConstraint::MaxContent,
        TextConstraint::Wrap(width) => FormationConstraint::Wrap(width.get()),
    }
}

fn line_limits(constraint: FormationConstraint) -> LineLimits {
    LineLimits {
        max_advance: match constraint {
            FormationConstraint::Wrap(width) => Some(width),
            FormationConstraint::MinContent | FormationConstraint::MaxContent => None,
        },
        max_height: None,
    }
}

fn line_break_reason(reason: CandidateBreak) -> LineBreakReason {
    match reason {
        CandidateBreak::Regular => LineBreakReason::Regular,
        CandidateBreak::Mandatory => LineBreakReason::Mandatory,
        CandidateBreak::End => LineBreakReason::End,
    }
}

fn map_former_error(_error: LineFormerError) -> PreparationError {
    PreparationError::invalid_output()
}

fn validate_candidate(candidate: LineCandidate) -> Result<(), PreparationError> {
    let clusters = candidate.clusters();
    let trailing = candidate.trailing_whitespace_clusters();
    if trailing.start < clusters.start
        || trailing.end != clusters.end
        || candidate.trailing_whitespace_advance() > candidate.canonical_advance()
    {
        return Err(PreparationError::invalid_output());
    }
    Ok(())
}

fn record_former_work(work: &mut LineFormationWork, former: LineFormerWork) {
    work.candidates = former.proposed;
    work.rejected_candidates = former.rejected;
    work.checkpoint_restores = former.restores;
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct LineChoice {
    pub(crate) end: usize,
    pub(crate) reason: LineBreakReason,
}

#[cfg(test)]
pub(crate) fn choose_line(
    clusters: &[LogicalCluster],
    start: usize,
    constraint: TextConstraint,
) -> Result<LineChoice, PreparationError> {
    let mut former = LineFormer::at(clusters, formation_constraint(constraint), start)
        .map_err(map_former_error)?;
    let candidate = former
        .candidate()
        .map_err(map_former_error)?
        .ok_or_else(PreparationError::invalid_output)?;
    Ok(LineChoice {
        end: candidate.end(),
        reason: line_break_reason(candidate.reason()),
    })
}

pub(crate) fn update_line_metrics(
    text: &str,
    lines: &mut [FormedLine],
    inline_flow_styles: &[InlineFlowStyle],
    inline_flow_runs: &[InlineFlowRun],
) -> Result<(), PreparationError> {
    let mut previous = None;
    for line in lines {
        let reason = line.plan.reason;
        let advance = line.plan.advance;
        let clusters = collect_logical_clusters(text, &line.shaped_text)?;
        line.plan = make_line_plan(
            &line.shaped_text,
            &clusters,
            inline_flow_styles,
            inline_flow_runs,
            0..clusters.len(),
            reason,
            advance,
            previous.as_ref(),
        )?;
        previous = Some(line.plan.clone());
    }
    Ok(())
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
                allows_soft_wrap: true,
                allows_emergency_wrap: false,
                emergency_affects_min_content: false,
                advance: f64::from(cluster.advance),
            });
        }
    }
    if !text.is_empty() && clusters.is_empty() {
        return Err(PreparationError::invalid_output());
    }
    Ok(clusters)
}

pub(crate) fn apply_wrap_policy(
    clusters: &mut [LogicalCluster],
    styles: &[InlineFlowStyle],
    runs: &[InlineFlowRun],
) -> Result<(), PreparationError> {
    for cluster in clusters {
        let style = inline_flow_style_at(cluster.source.start, styles, runs)?;
        cluster.allows_soft_wrap = style.text_wrap_mode() == TextWrapMode::Wrap;
        cluster.allows_emergency_wrap = cluster.allows_soft_wrap
            && matches!(
                style.overflow_wrap(),
                OverflowWrap::Anywhere | OverflowWrap::BreakWord
            );
        cluster.emergency_affects_min_content =
            cluster.allows_soft_wrap && style.overflow_wrap() == OverflowWrap::Anywhere;
    }
    Ok(())
}

fn inline_flow_style_at(
    source: usize,
    styles: &[InlineFlowStyle],
    runs: &[InlineFlowRun],
) -> Result<InlineFlowStyle, PreparationError> {
    let index = runs.partition_point(|run| run.bytes().end as usize <= source);
    let run = runs
        .get(index)
        .filter(|run| {
            let bytes = run.bytes();
            bytes.start as usize <= source && source < bytes.end as usize
        })
        .ok_or_else(PreparationError::invalid_output)?;
    styles
        .get(run.style().index())
        .copied()
        .ok_or_else(PreparationError::invalid_output)
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
        let ascent = f64::from(run.font_metrics.ascent);
        let descent = f64::from(run.font_metrics.descent);
        let metrics_height =
            run.font_metrics.ascent + run.font_metrics.descent + run.font_metrics.leading;
        let requested_height = inline_flow_line_height(
            &cluster.source,
            inline_flow_styles,
            inline_flow_runs,
            run.font_size,
            metrics_height,
        )?;
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

fn inline_flow_line_height(
    source: &Range<usize>,
    styles: &[InlineFlowStyle],
    runs: &[InlineFlowRun],
    font_size: f32,
    metrics_height: f32,
) -> Result<f64, PreparationError> {
    let mut height = 0.0_f32;
    for run in runs {
        let bytes = run.bytes();
        if bytes.start as usize >= source.end || bytes.end as usize <= source.start {
            continue;
        }
        let style = styles
            .get(run.style().index())
            .ok_or_else(PreparationError::invalid_output)?;
        height = height.max(style.line_height().resolve(font_size, metrics_height));
    }
    if !height.is_finite() || height <= 0.0 {
        return Err(PreparationError::invalid_output());
    }
    Ok(f64::from(height))
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
