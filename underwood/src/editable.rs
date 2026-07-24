// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Focused native text-input surfaces and exact host mapping.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::{
    CompositionEpoch, CompositionId, CompositionScene, CompositionSession, CompositionTextPosition,
    CompositionTextRange, DocumentId, DocumentRevision, DocumentSnapshot, Point,
    ProjectedTextPosition, ProjectedTextRange, ProjectedTextSource, Rect, SnapshotTextPosition,
    SnapshotTextRange, SnapshotTextSelectionSet, SurfaceError, SurfaceErrorKind, TextAffinity,
    TextId, TextScene, TextSelectionMode,
};

mod snapshot;
mod surface;

pub use snapshot::EditableSurfaceSnapshot;
pub use surface::{
    EditableSurface, EditableSurfaceElement, EditableSurfaceSelection, SurfaceTextEncoding,
};

use snapshot::{
    BoundScene, BoundSurfaceSource, BoundSurfaceSpan, append_bound_span, map_selections, one_range,
    surface_len,
};

#[cfg(test)]
use snapshot::{byte_offset_for_encoded, encoded_len};

#[cfg(test)]
mod tests;
