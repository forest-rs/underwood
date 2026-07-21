// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;

pub mod adapter;

mod document;
mod error;
mod scene;
mod style;

pub use document::{
    ChangeSet, Document, DocumentId, DocumentRevision, DocumentSnapshot, Edit, InlineRole,
    ParagraphId, ParagraphRole, Publication, SemanticId, TextId,
};
pub use error::{EditError, EditErrorKind, SceneError, SceneErrorKind, StyleError, StyleErrorKind};
pub use kurbo::{Affine, Point, Rect, Size, Vec2};
pub use parlance::{FontFeature, FontVariation, Language, Tag};
pub use peniko::{Brush, Color, FontData};
pub use scene::{
    LayoutEngine, SceneCaret, SceneFragment, SceneFragmentId, SceneGlyph, SceneLine, SceneOutput,
    SemanticFragment, SnapshotTextRange, StageWork, TextHit, TextScene, WorkReport,
};
pub use style::{
    ComputedInlineStyle, FiniteWidth, InlineFlowStyle, LineHeight, PaintSlot, PaintTable,
    SceneRequest, ShapingStyle, StyleMap,
};
