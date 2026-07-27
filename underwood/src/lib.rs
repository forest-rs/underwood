// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;

pub mod adapter;

pub use adapter::TextAffinity;

mod block;
mod composition;
mod document;
mod editable;
mod error;
mod features;
mod projection;
mod region;
mod scene;
mod selection;
mod style;

pub use block::{BlockRequest, TextBlock, TextBlockSnapshot};
pub use composition::{
    CompositionClause, CompositionClauseKind, CompositionEpoch, CompositionId, CompositionSession,
    CompositionStart, CompositionTextPosition, CompositionTextRange, CompositionUpdate,
};
pub use document::{
    ChangeSet, Document, DocumentId, DocumentRevision, DocumentSnapshot, Edit, InlineRole,
    ParagraphId, ParagraphRole, Publication, SelectionReplacement, SemanticId, TextId,
};
pub use editable::{
    EditableSurface, EditableSurfaceElement, EditableSurfaceSelection, EditableSurfaceSnapshot,
    SurfaceTextEncoding,
};
pub use error::{
    CompositionError, CompositionErrorKind, EditError, EditErrorKind, SceneError, SceneErrorKind,
    SelectionError, SelectionErrorKind, StyleError, StyleErrorKind, SurfaceError, SurfaceErrorKind,
};
pub use features::{MissingSceneCapability, SceneFeaturePolicy, SceneFeatures};
pub use kurbo::{Affine, Point, Rect, Size, Vec2};
pub use parlance::{
    BaseDirection, FontFamily, FontFamilyName, FontFeature, FontStyle, FontVariation, FontWeight,
    FontWidth, GenericFamily, Language, OverflowWrap, Script, Tag, TextWrapMode, WordBreak,
};
pub use peniko::{Brush, Color, FontData};
pub use projection::{
    ProjectedText, ProjectionBuilder, ProjectionError, ProjectionErrorKind, ProjectionKind,
    ProjectionSegment, ProjectionSegments, WhitespaceCollapse,
};
pub use region::{
    FloatSide, FlowRegion, LineSlot, RegionAttempt, RegionAttemptOutcome, RegionCursor,
    RegionFloat, RegionFlow, RegionTranscript,
};
pub use scene::{
    CacheBudget, CacheDiagnostics, CompositionScene, CompositionSceneOutput, LayoutEngine,
    LineAdjustment, ParagraphSceneResidency, PreparationMemory, PreparationReuse, PreparationTrace,
    ProjectedSceneEditing, ProjectedSceneFragmentView, ProjectedSceneFragments,
    ProjectedSceneGlyphView, ProjectedSceneGlyphs, ProjectedSceneInteraction,
    ProjectedSceneLineView, ProjectedSceneLines, ProjectedSources, ProjectedTextPosition,
    ProjectedTextRange, ProjectedTextSource, ProjectedTextUnitView, SceneCaret,
    SceneCompositionRect, SceneEditing, SceneFragmentId, SceneFragmentView, SceneFragments,
    SceneGlyphInstanceId, SceneGlyphView, SceneGlyphs, SceneInteraction, SceneLineView, SceneLines,
    SceneOutput, SceneParagraphResidencies, SceneRegionAttempts, SceneRegionTranscript,
    SceneResidency, SceneResidencyBytes, SceneSelection, SceneSelectionRect, SceneSemantics,
    SemanticFragmentView, SnapshotSources, SnapshotTextUnitView, StageWork, TextHit, TextMetrics,
    TextScene, WorkReport,
};
pub use selection::{
    SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection, SnapshotTextSelectionSet,
    SnapshotTextUnit, TextMovement, TextSelectionMode,
};
pub use style::{
    AnalysisStyle, ComputedInlineStyle, FiniteWidth, InlineFlowStyle, LineHeight, LineHeightBasis,
    PaintSlot, PaintTable, ParagraphStyle, ResolvedDirection, SceneRequest, ShapingStyle, StyleMap,
    TextAlignment, TextConstraint, TextSpacing,
};
