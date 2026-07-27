// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Pre-stable, backend-facing paragraph preparation contract.
//!
//! Successful outputs own every retained font, coordinate, and glyph record.
//! No backend-specific type crosses this boundary.

use alloc::vec::Vec;
use alloc::{boxed::Box, sync::Arc};
use core::fmt;
use core::mem::size_of;
use core::ops::Range;

use crate::{
    Affine, AnalysisStyle, FontData, FontVariation, InlineFlowStyle, PaintSlot, ParagraphId,
    ParagraphStyle, Rect, RegionCursor, RegionFlow, RegionTranscript, ResolvedDirection,
    SceneFeatures, ShapingStyle, TextConstraint, Vec2,
};

mod error;
mod formation;
mod interaction;
mod paint;
mod prepared;

pub use error::{PreparationError, PreparationErrorKind};
pub use formation::{
    AnalysisRun, AnalysisStyleId, FormationWork, InlineFlowRun, InlineFlowStyleId, LineShapingWork,
    PaintRun, ParagraphConstraints, ParagraphFormation, ParagraphFormationCacheDiagnostics,
    ParagraphFormationChange, ParagraphFormationOutput, ParagraphFormationReuse, ParagraphInput,
    ParagraphPreparationId, ShapingRun, ShapingStyleId,
};
pub use interaction::{
    ClusterBoundary, ClusterWhitespace, LineBreakReason, PreparedClusterSide,
    PreparedInteractionSlice, PreparedInteractionUnit, PreparedInteractionUnitView,
    PreparedInteractionUnits, TextAffinity,
};
pub use paint::{GlyphPaintCoverage, GlyphPaintSegment};
pub(crate) use prepared::PreparedParagraphFacts;
pub use prepared::{FontSynthesis, PreparedGlyph, PreparedLine, PreparedParagraph, PreparedRun};

#[cfg(test)]
mod tests;
