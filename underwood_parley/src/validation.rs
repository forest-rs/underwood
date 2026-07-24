// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Paragraph-adapter input invariants.
//!
//! This module owns fail-closed range and coverage validation; it explicitly
//! does not own shaping, layout policy, or error presentation.

use core::ops::Range;

use underwood::adapter::{InlineFlowRun, ParagraphInput, PreparationError, ShapingRun};

pub(crate) fn validate_input_runs(input: &ParagraphInput<'_>) -> Result<(), PreparationError> {
    let text_len =
        u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
    validate_run_coverage(
        input,
        input.shaping_runs().iter().map(ShapingRun::bytes),
        text_len,
        PreparationError::invalid_output,
    )?;
    if input.shaping_styles().len() > usize::from(u16::MAX) + 1
        || input
            .shaping_runs()
            .iter()
            .any(|run| run.style().index() >= input.shaping_styles().len())
    {
        return Err(PreparationError::invalid_output());
    }
    validate_run_coverage(
        input,
        input.inline_flow_runs().iter().map(InlineFlowRun::bytes),
        text_len,
        PreparationError::invalid_output,
    )?;
    if input.inline_flow_styles().len() > usize::from(u16::MAX) + 1
        || input
            .inline_flow_runs()
            .iter()
            .any(|run| run.style().index() >= input.inline_flow_styles().len())
    {
        return Err(PreparationError::invalid_output());
    }
    validate_run_coverage(
        input,
        input.paint_runs().iter().map(|run| run.bytes()),
        text_len,
        PreparationError::unsupported_paint_coverage,
    )
}

fn validate_run_coverage(
    input: &ParagraphInput<'_>,
    ranges: impl IntoIterator<Item = Range<u32>>,
    text_len: u32,
    error: fn() -> PreparationError,
) -> Result<(), PreparationError> {
    let mut end = 0_u32;
    for range in ranges {
        if range.start != end
            || range.start >= range.end
            || range.end > text_len
            || input
                .text()
                .get(range.start as usize..range.end as usize)
                .is_none()
        {
            return Err(error());
        }
        end = range.end;
    }
    if end != text_len {
        return Err(error());
    }
    Ok(())
}
