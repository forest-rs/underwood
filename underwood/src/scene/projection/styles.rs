// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Projection of authored style partitions onto presentation text.

use super::*;

pub(super) fn project_style_runs(
    paragraph: ParagraphId,
    mapping: &TextProjection,
    analysis: &mut Vec<AnalysisRun>,
    shaping: &mut Vec<ShapingRun>,
    inline_flow: &mut Vec<InlineFlowRun>,
    paint: &mut Vec<PaintRun>,
) -> Result<(), SceneError> {
    if mapping.is_identity() {
        return Ok(());
    }
    *analysis = project_analysis_runs(paragraph, mapping, analysis)?;
    *shaping = project_shaping_runs(paragraph, mapping, shaping)?;
    *inline_flow = project_inline_flow_runs(paragraph, mapping, inline_flow)?;
    *paint = project_paint_runs(paragraph, mapping, paint)?;
    Ok(())
}

fn project_analysis_runs(
    paragraph: ParagraphId,
    mapping: &TextProjection,
    source_runs: &[AnalysisRun],
) -> Result<Vec<AnalysisRun>, SceneError> {
    let mut projected = Vec::new();
    for segment in mapping.segments() {
        match segment.kind() {
            ProjectionKind::Identity => {
                project_identity_runs(&segment, source_runs, AnalysisRun::bytes, |range, run| {
                    append_analysis_id_run(&mut projected, range, run.style());
                });
            }
            ProjectionKind::Replacement | ProjectionKind::Collapsed | ProjectionKind::Inserted => {
                let owner = segment.source().start;
                let run =
                    run_for_position(source_runs, owner, AnalysisRun::bytes).ok_or_else(|| {
                        SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            paragraph,
                            segment.source(),
                        )
                    })?;
                append_analysis_id_run(&mut projected, segment.projected(), run.style());
            }
            ProjectionKind::Omitted => {}
        }
    }
    Ok(projected)
}

fn project_shaping_runs(
    paragraph: ParagraphId,
    mapping: &TextProjection,
    source_runs: &[ShapingRun],
) -> Result<Vec<ShapingRun>, SceneError> {
    let mut projected = Vec::new();
    for segment in mapping.segments() {
        match segment.kind() {
            ProjectionKind::Identity => {
                project_identity_runs(
                    &segment,
                    source_runs,
                    |run| run.bytes(),
                    |range, run| append_shaping_id_run(&mut projected, range, run.style()),
                );
            }
            ProjectionKind::Replacement | ProjectionKind::Collapsed | ProjectionKind::Inserted => {
                let owner = segment.source().start;
                let run =
                    run_for_position(source_runs, owner, ShapingRun::bytes).ok_or_else(|| {
                        SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            paragraph,
                            segment.source(),
                        )
                    })?;
                append_shaping_id_run(&mut projected, segment.projected(), run.style());
            }
            ProjectionKind::Omitted => {}
        }
    }
    Ok(projected)
}

fn project_inline_flow_runs(
    paragraph: ParagraphId,
    mapping: &TextProjection,
    source_runs: &[InlineFlowRun],
) -> Result<Vec<InlineFlowRun>, SceneError> {
    let mut projected = Vec::new();
    for segment in mapping.segments() {
        match segment.kind() {
            ProjectionKind::Identity => {
                project_identity_runs(
                    &segment,
                    source_runs,
                    |run| run.bytes(),
                    |range, run| append_inline_flow_id_run(&mut projected, range, run.style()),
                );
            }
            ProjectionKind::Replacement | ProjectionKind::Collapsed | ProjectionKind::Inserted => {
                let owner = segment.source().start;
                let run = run_for_position(source_runs, owner, InlineFlowRun::bytes).ok_or_else(
                    || {
                        SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            paragraph,
                            segment.source(),
                        )
                    },
                )?;
                append_inline_flow_id_run(&mut projected, segment.projected(), run.style());
            }
            ProjectionKind::Omitted => {}
        }
    }
    Ok(projected)
}

fn project_paint_runs(
    paragraph: ParagraphId,
    mapping: &TextProjection,
    source_runs: &[PaintRun],
) -> Result<Vec<PaintRun>, SceneError> {
    let mut projected = Vec::new();
    for segment in mapping.segments() {
        match segment.kind() {
            ProjectionKind::Identity => {
                project_identity_runs(
                    &segment,
                    source_runs,
                    |run| run.bytes(),
                    |range, run| append_paint_run(&mut projected, range, run.slot()),
                );
            }
            ProjectionKind::Replacement | ProjectionKind::Collapsed | ProjectionKind::Inserted => {
                let owner = segment.source().start;
                let run =
                    run_for_position(source_runs, owner, PaintRun::bytes).ok_or_else(|| {
                        SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            paragraph,
                            segment.source(),
                        )
                    })?;
                append_paint_run(&mut projected, segment.projected(), run.slot());
            }
            ProjectionKind::Omitted => {}
        }
    }
    Ok(projected)
}

fn project_identity_runs<T>(
    segment: &ProjectionSegment,
    source_runs: &[T],
    range_of: impl Fn(&T) -> Range<u32>,
    mut append: impl FnMut(Range<u32>, &T),
) {
    let source = segment.source();
    let projected = segment.projected();
    for run in source_runs {
        let run_range = range_of(run);
        let start = source.start.max(run_range.start);
        let end = source.end.min(run_range.end);
        if start < end {
            let projected_start = projected.start + (start - source.start);
            append(projected_start..projected_start + (end - start), run);
        }
    }
}

fn run_for_position<T>(
    runs: &[T],
    position: u32,
    range_of: impl Fn(&T) -> Range<u32>,
) -> Option<&T> {
    runs.iter()
        .find(|run| {
            let range = range_of(run);
            range.start <= position && position < range.end
        })
        .or_else(|| {
            runs.iter().rev().find(|run| {
                let range = range_of(run);
                range.end == position
            })
        })
}

fn append_shaping_id_run(runs: &mut Vec<ShapingRun>, bytes: Range<u32>, style: ShapingStyleId) {
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.style() == style
    {
        let start = last.bytes().start;
        *last = ShapingRun::new(start..bytes.end, style);
    } else {
        runs.push(ShapingRun::new(bytes, style));
    }
}

fn append_analysis_id_run(runs: &mut Vec<AnalysisRun>, bytes: Range<u32>, style: AnalysisStyleId) {
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.style() == style
    {
        let start = last.bytes().start;
        *last = AnalysisRun::new(start..bytes.end, style);
    } else {
        runs.push(AnalysisRun::new(bytes, style));
    }
}

fn append_inline_flow_id_run(
    runs: &mut Vec<InlineFlowRun>,
    bytes: Range<u32>,
    style: InlineFlowStyleId,
) {
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.style() == style
    {
        let start = last.bytes().start;
        *last = InlineFlowRun::new(start..bytes.end, style);
    } else {
        runs.push(InlineFlowRun::new(bytes, style));
    }
}
