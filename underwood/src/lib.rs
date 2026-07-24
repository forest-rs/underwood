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
mod projection;
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
pub use kurbo::{Affine, Point, Rect, Size, Vec2};
pub use parlance::{
    BaseDirection, FontFamily, FontFamilyName, FontFeature, FontStyle, FontVariation, FontWeight,
    FontWidth, GenericFamily, Language, Script, Tag,
};
pub use peniko::{Brush, Color, FontData};
pub use projection::{
    ProjectedText, ProjectionBuilder, ProjectionError, ProjectionErrorKind, ProjectionKind,
    ProjectionSegment, ProjectionSegments, WhitespaceCollapse,
};
pub use scene::{
    CacheBudget, CacheDiagnostics, CompositionScene, CompositionSceneOutput, LayoutEngine,
    ProjectedTextPosition, ProjectedTextRange, ProjectedTextSource, SceneCaret,
    SceneCompositionRect, SceneFragment, SceneFragmentId, SceneGlyph, SceneGlyphInstanceId,
    SceneLine, SceneOutput, SceneSelectionRect, SemanticFragment, StageWork, TextHit, TextMetrics,
    TextScene, WorkReport,
};
pub use selection::{
    SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection, SnapshotTextSelectionSet,
    SnapshotTextUnit, TextMovement, TextSelectionMode,
};
pub use style::{
    ComputedInlineStyle, FiniteWidth, InlineFlowStyle, LineHeight, PaintSlot, PaintTable,
    ParagraphStyle, SceneRequest, ShapingStyle, StyleMap, TextConstraint,
};
