// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained paragraph preparation and immutable renderer-neutral scenes.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::adapter::{
    FontSynthesis, FormationWork, InlineFlowRun, InlineFlowStyleId, LineBreakReason, PaintRun,
    ParagraphConstraints, ParagraphFormation, ParagraphInput, PreparationErrorKind,
    PreparedParagraph, ShapingRun, ShapingStyleId, TextAffinity,
};
use crate::document::Paragraph;
use crate::{
    Affine, BlockRequest, CompositionError, CompositionErrorKind, CompositionId,
    CompositionSession, CompositionStart, DocumentRevision, DocumentSnapshot, FontData,
    InlineFlowStyle, InlineRole, PaintSlot, PaintTable, ParagraphId, ParagraphRole, Point, Rect,
    SceneError, SceneErrorKind, SceneRequest, SelectionError, SelectionErrorKind, SemanticId,
    ShapingStyle, Size, SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection,
    SnapshotTextSelectionSet, SnapshotTextUnit, StyleMap, TextBlockSnapshot, TextConstraint,
    TextId, TextMovement, TextSelectionMode, Vec2,
};

mod engine;
mod geometry;
mod interaction;
mod output;
mod projection;
mod records;

pub use engine::{CacheBudget, CacheDiagnostics, LayoutEngine};
pub use interaction::{CompositionScene, TextScene};
pub use output::{
    CompositionSceneOutput, ProjectedTextPosition, ProjectedTextRange, ProjectedTextSource,
    SceneOutput, StageWork, TextMetrics, WorkReport,
};
pub use records::{
    SceneCaret, SceneCompositionRect, SceneFragment, SceneFragmentId, SceneGlyph,
    SceneGlyphInstanceId, SceneLine, SceneSelectionRect, SemanticFragment, TextHit,
};

use geometry::*;
use interaction::{
    SceneCaretStop, SceneCluster, SceneCursorMovement, SceneCursorStep, SceneHitSlice,
};
use projection::*;

#[cfg(test)]
use projection::{append_inline_flow_run, append_shaping_run};

#[cfg(test)]
mod tests;
