// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained paragraph preparation and immutable renderer-neutral scenes.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::adapter::{
    AnalysisRun, AnalysisStyleId, FontSynthesis, FormationWork, InlineFlowRun, InlineFlowStyleId,
    LineBreakReason, PaintRun, ParagraphConstraints, ParagraphFormation,
    ParagraphFormationCacheDiagnostics, ParagraphFormationChange, ParagraphFormationReuse,
    ParagraphInput, ParagraphPreparationId, PreparationErrorKind, PreparedClusterSide,
    PreparedGlyphView, PreparedGlyphs, PreparedInteractionUnitView, PreparedLineView,
    PreparedParagraph, PreparedParagraphFacts, PreparedRunView, ShapingRun, ShapingStyleId,
    TextAffinity,
};
use crate::document::Paragraph;
use crate::{
    Affine, AnalysisStyle, BaseDirection, BlockRequest, CompositionError, CompositionErrorKind,
    CompositionId, CompositionSession, CompositionStart, ComputedInlineStyle, DocumentRevision,
    DocumentSnapshot, FontData, InlineFlowStyle, InlineRole, MissingSceneCapability, PaintSlot,
    PaintTable, ParagraphId, ParagraphRole, ParagraphStyle, Point, ProjectedText as TextProjection,
    ProjectionKind, ProjectionSegment, Rect, RegionAttemptOutcome, RegionCursor, RegionFlow,
    RegionTranscript, ResolvedDirection, SceneError, SceneErrorKind, SceneFeaturePolicy,
    SceneFeatures, SceneRequest, SelectionError, SelectionErrorKind, SemanticId, ShapingStyle,
    Size, SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection, SnapshotTextSelectionSet,
    SnapshotTextUnit, StyleMap, TextAlignment, TextBlockSnapshot, TextConstraint, TextId,
    TextMovement, TextSelectionMode, Vec2,
};

mod adjustment;
mod cursor;
mod engine;
mod facades;
mod geometry;
mod interaction;
mod output;
mod projection;
mod records;
mod residency;
mod shared_cache;
mod source_map;
mod spine;
mod views;

pub use engine::{CacheBudget, CacheDiagnostics, LayoutEngine};
pub use facades::{
    ProjectedSceneDisplay, ProjectedSceneEditing, ProjectedSceneInteraction,
    ProjectedSceneSemanticAccess, ProjectedSceneSourceAccess, SceneDisplay, SceneEditing,
    SceneInteraction, SceneSelection, SceneSemanticAccess, SceneSourceAccess,
};
pub use interaction::{CompositionScene, TextScene};
pub use output::{
    CompositionSceneOutput, PreparationMemory, PreparationReuse, PreparationTrace,
    ProjectedTextPosition, ProjectedTextRange, ProjectedTextSource, SceneOutput,
    SceneRegionAttempts, SceneRegionTranscript, StageWork, TextMetrics, WorkReport,
};
pub use records::{
    LineAdjustment, SceneCaret, SceneCompositionRect, SceneFragmentId, SceneGlyphInstanceId,
    SceneSelectionRect, TextHit,
};
pub use residency::{
    ParagraphSceneResidency, SceneParagraphResidencies, SceneResidency, SceneResidencyBytes,
};
pub use views::{
    ProjectedSceneFragmentView, ProjectedSceneFragments, ProjectedSceneGlyphView,
    ProjectedSceneGlyphs, ProjectedSceneLineView, ProjectedSceneLines, ProjectedSources,
    ProjectedTextUnitView, SceneFragmentView, SceneFragments, SceneGlyphView, SceneGlyphs,
    SceneLineView, SceneLines, SceneSemantics, SemanticFragmentView, SnapshotSources,
    SnapshotTextUnitView,
};

use adjustment::*;
use cursor::*;
use geometry::*;
use interaction::{SceneCore, SceneCursorStep};
use projection::*;
use shared_cache::*;
use source_map::*;
use spine::*;

#[cfg(test)]
use projection::{append_analysis_run, append_inline_flow_run, append_shaping_run};

#[cfg(test)]
mod tests;
