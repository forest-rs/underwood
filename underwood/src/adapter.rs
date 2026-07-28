// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Pre-stable, backend-facing paragraph preparation contract.
//!
//! Successful outputs own every retained font, coordinate, and glyph record.
//! No backend-specific type crosses this boundary.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;
use core::ops::Range;

use crate::{
    Affine, AnalysisStyle, FontData, FontVariation, InlineFlowStyle, ParagraphId, ParagraphStyle,
    RegionCursor, RegionFlow, RegionTranscript, ResolvedDirection, SceneFeatures, ShapingStyle,
    TextConstraint, Vec2,
};

mod error;
mod formation;
mod interaction;
mod prepared;

#[expect(
    clippy::cast_possible_truncation,
    reason = "canonical shaping records deliberately retain native f32 precision"
)]
fn compact_shaping_coordinate(value: f64) -> Option<f32> {
    let compact = value as f32;
    compact.is_finite().then_some(compact)
}

pub use error::{PreparationError, PreparationErrorKind};
pub use formation::{
    AnalysisRun, AnalysisStyleId, FormationWork, InlineFlowRun, InlineFlowStyleId, LineShapingWork,
    ParagraphConstraints, ParagraphFormation, ParagraphFormationCacheDiagnostics,
    ParagraphFormationChange, ParagraphFormationOutput, ParagraphFormationReuse, ParagraphInput,
    ParagraphPreparationId, ShapingRun, ShapingStyleId,
};
pub use interaction::{
    ClusterBoundary, ClusterWhitespace, LineBreakReason, PreparedClusterSide,
    PreparedInteractionSlice, PreparedInteractionUnit, PreparedInteractionUnitView, TextAffinity,
};
pub(crate) use interaction::{PreparedInteractionSliceSpill, prepared_interaction_unit_view};
pub(crate) use prepared::PreparedParagraphFacts;
pub use prepared::{
    FontSynthesis, PreparedGlyphView, PreparedLine, PreparedLineView, PreparedParagraph,
    PreparedParagraphData, PreparedRun, PreparedRunView,
};

#[cfg(test)]
mod tests;
