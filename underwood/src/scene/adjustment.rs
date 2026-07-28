// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Accepted-slot alignment, Western expansion, and caret-coordinate remapping.
//!
//! This module consumes fixed line boundaries and exact slot sizes. It
//! explicitly does not own shaping, line formation, script-specific
//! justification, or scene-record construction.

use super::*;

pub(super) fn constrained_inline_size(constraint: TextConstraint) -> Option<f64> {
    match constraint {
        TextConstraint::Wrap(width) => Some(width.get()),
        TextConstraint::MinContent | TextConstraint::MaxContent => None,
    }
}

pub(super) fn resolve_line_adjustment(
    alignment: TextAlignment,
    direction: ResolvedDirection,
    break_reason: LineBreakReason,
    advance: f64,
    trailing_whitespace: f64,
    opportunities: usize,
    slot_size: Option<f64>,
) -> Result<LineAdjustment, SceneError> {
    let Some(slot_size) = slot_size else {
        return Ok(LineAdjustment::new(
            alignment,
            direction,
            0.0,
            trailing_whitespace,
            0.0,
            0,
        ));
    };
    let content_advance = (advance - trailing_whitespace).max(0.0);
    let free_space = slot_size - content_advance;
    let logical_start = || match direction {
        ResolvedDirection::Ltr => 0.0,
        ResolvedDirection::Rtl => slot_size - advance,
    };
    if free_space <= 0.0 {
        return Ok(LineAdjustment::new(
            alignment,
            direction,
            logical_start(),
            trailing_whitespace,
            0.0,
            0,
        ));
    }
    let left = || match direction {
        ResolvedDirection::Ltr => 0.0,
        ResolvedDirection::Rtl => -trailing_whitespace,
    };
    let right = || match direction {
        ResolvedDirection::Ltr => free_space,
        ResolvedDirection::Rtl => slot_size - advance,
    };
    let offset = match alignment {
        TextAlignment::Start => match direction {
            ResolvedDirection::Ltr => left(),
            ResolvedDirection::Rtl => right(),
        },
        TextAlignment::End => match direction {
            ResolvedDirection::Ltr => right(),
            ResolvedDirection::Rtl => left(),
        },
        TextAlignment::Left => left(),
        TextAlignment::Center => {
            free_space * 0.5
                - if direction == ResolvedDirection::Rtl {
                    trailing_whitespace
                } else {
                    0.0
                }
        }
        TextAlignment::Right => right(),
        TextAlignment::Justify if break_reason == LineBreakReason::Regular && opportunities > 0 => {
            let opportunities = u32::try_from(opportunities)
                .map_err(|_| SceneError::new(SceneErrorKind::SourceCoverage))?;
            let expansion = free_space / f64::from(opportunities);
            return Ok(LineAdjustment::new(
                alignment,
                direction,
                match direction {
                    ResolvedDirection::Ltr => 0.0,
                    ResolvedDirection::Rtl => -trailing_whitespace,
                },
                trailing_whitespace,
                expansion,
                opportunities,
            ));
        }
        TextAlignment::Justify => logical_start(),
    };
    Ok(LineAdjustment::new(
        alignment,
        direction,
        offset,
        trailing_whitespace,
        0.0,
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_edges_resolve_from_direction_and_ignore_trailing_space() {
        let ltr = resolve_line_adjustment(
            TextAlignment::End,
            ResolvedDirection::Ltr,
            LineBreakReason::End,
            50.0,
            10.0,
            0,
            Some(100.0),
        )
        .expect("finite adjustment is valid");
        let rtl = resolve_line_adjustment(
            TextAlignment::End,
            ResolvedDirection::Rtl,
            LineBreakReason::End,
            50.0,
            10.0,
            0,
            Some(100.0),
        )
        .expect("finite adjustment is valid");
        assert_eq!(ltr.inline_offset, 60.0);
        assert_eq!(rtl.inline_offset, -10.0);
    }

    #[test]
    fn overflow_falls_back_to_logical_start() {
        let ltr = resolve_line_adjustment(
            TextAlignment::Center,
            ResolvedDirection::Ltr,
            LineBreakReason::End,
            120.0,
            0.0,
            0,
            Some(100.0),
        )
        .expect("overflow adjustment is valid");
        let rtl = resolve_line_adjustment(
            TextAlignment::Center,
            ResolvedDirection::Rtl,
            LineBreakReason::End,
            120.0,
            0.0,
            0,
            Some(100.0),
        )
        .expect("overflow adjustment is valid");
        assert_eq!(ltr.inline_offset, 0.0);
        assert_eq!(rtl.inline_offset, -20.0);
    }
}
