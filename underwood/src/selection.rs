// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::{DocumentId, DocumentRevision, TextAffinity, TextId};

/// Whether a selection extension follows document order or visual caret order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextSelectionMode {
    /// Select one contiguous document interval.
    Logical,
    /// Select the interaction units crossed in visual caret order.
    Visual,
}

/// One interaction-unit cursor movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextMovement {
    /// Move to the preceding caret stop in visual order.
    PreviousVisual,
    /// Move to the following caret stop in visual order.
    NextVisual,
    /// Move across the preceding interaction unit in logical order.
    PreviousLogical,
    /// Move across the following interaction unit in logical order.
    NextLogical,
}

/// Collapsed text position valid only for one immutable snapshot revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SnapshotTextPosition {
    revision: DocumentRevision,
    text: TextId,
    byte: u32,
    affinity: TextAffinity,
}

impl SnapshotTextPosition {
    pub(crate) const fn new(
        revision: DocumentRevision,
        text: TextId,
        byte: u32,
        affinity: TextAffinity,
    ) -> Self {
        Self {
            revision,
            text,
            byte,
            affinity,
        }
    }

    /// Returns the exact snapshot revision.
    #[must_use]
    pub const fn revision(self) -> DocumentRevision {
        self.revision
    }

    /// Returns the semantic text leaf containing the position.
    #[must_use]
    pub const fn text(self) -> TextId {
        self.text
    }

    /// Returns the UTF-8 byte boundary within the text leaf.
    #[must_use]
    pub const fn byte(self) -> u32 {
        self.byte
    }

    /// Returns which logical side owns the position.
    #[must_use]
    pub const fn affinity(self) -> TextAffinity {
        self.affinity
    }
}

/// Dense source range valid only for one exact immutable snapshot revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotTextRange {
    revision: DocumentRevision,
    text: TextId,
    bytes: Range<u32>,
}

impl SnapshotTextRange {
    pub(crate) const fn new(revision: DocumentRevision, text: TextId, bytes: Range<u32>) -> Self {
        Self {
            revision,
            text,
            bytes,
        }
    }

    /// Returns the exact snapshot revision.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Returns the text leaf identity.
    #[must_use]
    pub const fn text(&self) -> TextId {
        self.text
    }

    /// Returns the UTF-8 byte range within the text leaf.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Source-complete interaction unit in one immutable snapshot revision.
///
/// One extended grapheme can cross semantic text-leaf boundaries. Each member
/// retains its exact [`TextId`] and leaf-local UTF-8 bytes while the ordered
/// collection remains one atomic movement, selection, and replacement unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotTextUnit {
    sources: Arc<[SnapshotTextRange]>,
}

impl SnapshotTextUnit {
    pub(crate) fn new(sources: Vec<SnapshotTextRange>) -> Self {
        Self {
            sources: sources.into(),
        }
    }

    /// Returns the ordered leaf-local ranges comprising this interaction unit.
    #[must_use]
    pub fn sources(&self) -> &[SnapshotTextRange] {
        &self.sources
    }
}

/// One revision-bound insertion point and its selected logical source ranges.
///
/// A visual selection can contain several noncontiguous logical ranges across
/// bidi boundaries. Those ranges still form one insertion point. Independent
/// carets are represented by separate members of [`SnapshotTextSelectionSet`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotTextSelection {
    anchor: SnapshotTextPosition,
    extent: SnapshotTextPosition,
    mode: TextSelectionMode,
    ranges: Arc<[SnapshotTextRange]>,
}

impl SnapshotTextSelection {
    pub(crate) fn new(
        anchor: SnapshotTextPosition,
        extent: SnapshotTextPosition,
        mode: TextSelectionMode,
        ranges: Vec<SnapshotTextRange>,
    ) -> Self {
        Self {
            anchor,
            extent,
            mode,
            ranges: ranges.into(),
        }
    }

    /// Returns the fixed edge retained while extending the selection.
    #[must_use]
    pub const fn anchor(&self) -> &SnapshotTextPosition {
        &self.anchor
    }

    /// Returns the moving edge of the selection.
    #[must_use]
    pub const fn extent(&self) -> &SnapshotTextPosition {
        &self.extent
    }

    /// Returns how the anchor-to-extent interval was interpreted.
    #[must_use]
    pub const fn mode(&self) -> TextSelectionMode {
        self.mode
    }

    /// Returns logically ordered leaf-local ranges for this insertion point.
    #[must_use]
    pub fn ranges(&self) -> &[SnapshotTextRange] {
        &self.ranges
    }

    /// Returns whether this selection is a collapsed caret.
    #[must_use]
    pub fn is_collapsed(&self) -> bool {
        self.ranges.iter().all(SnapshotTextRange::is_empty)
    }
}

/// A revision-bound ordered set of independent text selections.
///
/// The first member is the primary selection. An empty set has no primary
/// selection but still names the scene revision it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotTextSelectionSet {
    document: DocumentId,
    revision: DocumentRevision,
    selections: Arc<[SnapshotTextSelection]>,
}

impl SnapshotTextSelectionSet {
    pub(crate) fn new(
        document: DocumentId,
        revision: DocumentRevision,
        selections: Vec<SnapshotTextSelection>,
    ) -> Self {
        Self {
            document,
            revision,
            selections: selections.into(),
        }
    }

    /// Returns the owning document identity.
    #[must_use]
    pub const fn document(&self) -> DocumentId {
        self.document
    }

    /// Returns the exact snapshot revision.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Returns independent selections in stable application order.
    #[must_use]
    pub fn selections(&self) -> &[SnapshotTextSelection] {
        &self.selections
    }

    /// Returns the primary selection when the set is nonempty.
    #[must_use]
    pub fn primary(&self) -> Option<&SnapshotTextSelection> {
        self.selections.first()
    }

    /// Returns whether the set contains no selections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }
}
