// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;
use core::ops::Range;

use crate::adapter::PreparationErrorKind;
use crate::document::{DocumentId, ParagraphId, TextId};
use crate::style::PaintSlot;

/// Stable category for composition-state failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompositionErrorKind {
    /// The composition or selection belongs to another document revision.
    WrongSnapshot,
    /// Composition requires at least one insertion point.
    EmptySelectionSet,
    /// The replacement target cannot be represented by the first composition slice.
    UnsupportedSelection,
    /// A preedit selection is reversed, out of bounds, or not on UTF-8 boundaries.
    InvalidPreeditRange,
    /// An IME clause is invalid, reversed, overlapping, or not on UTF-8 boundaries.
    InvalidClauseRange,
    /// A delayed update named an epoch older or newer than the current state.
    StaleEpoch,
    /// The session exhausted its monotonic epoch space.
    EpochExhausted,
}

/// Concrete composition-state error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CompositionError {
    kind: CompositionErrorKind,
}

impl CompositionError {
    pub(crate) const fn new(kind: CompositionErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> CompositionErrorKind {
        self.kind
    }
}

impl fmt::Display for CompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "composition operation failed: {:?}", self.kind)
    }
}

impl core::error::Error for CompositionError {}

/// Stable category for revisioned editable-surface failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SurfaceErrorKind {
    /// The scene, selection, or composition belongs to another snapshot.
    WrongSnapshot,
    /// A requested semantic text leaf is absent from the snapshot.
    UnknownText,
    /// A text leaf appears more than once in one flattened surface.
    DuplicateText,
    /// A range is reversed, out of bounds, or not on a valid encoding boundary.
    InvalidRange,
    /// Surface-only separator bytes cannot be mapped back to document text.
    UnmappedRange,
    /// A native query cannot represent the requested multi-range selection.
    UnsupportedSelection,
}

/// Concrete editable-surface error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SurfaceError {
    kind: SurfaceErrorKind,
}

impl SurfaceError {
    pub(crate) const fn new(kind: SurfaceErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> SurfaceErrorKind {
        self.kind
    }
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "editable surface operation failed: {:?}",
            self.kind
        )
    }
}

impl core::error::Error for SurfaceError {}

/// Stable category for document-edit failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditErrorKind {
    /// An identity belongs to another document.
    WrongDocument,
    /// The requested structural target does not exist.
    InvalidStructure,
    /// Text or a collection cannot be represented by the first-slice indices.
    OversizedText,
    /// The transaction's base revision is no longer current.
    RevisionConflict,
    /// A snapshot selection contains an invalid or non-boundary text range.
    InvalidTextRange,
    /// One text replacement would cross a paragraph structure boundary.
    CrossParagraphSelection,
    /// Independent selections overlap or duplicate an insertion point.
    OverlappingSelections,
    /// A replacement was requested for a set with no insertion points.
    EmptySelectionSet,
}

/// Concrete document-edit error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct EditError {
    kind: EditErrorKind,
    document: Option<DocumentId>,
    paragraph: Option<ParagraphId>,
    text: Option<TextId>,
}

impl EditError {
    pub(crate) const fn for_document(kind: EditErrorKind, document: DocumentId) -> Self {
        Self {
            kind,
            document: Some(document),
            paragraph: None,
            text: None,
        }
    }

    pub(crate) const fn for_paragraph(kind: EditErrorKind, paragraph: ParagraphId) -> Self {
        Self {
            kind,
            document: Some(paragraph.document),
            paragraph: Some(paragraph),
            text: None,
        }
    }

    pub(crate) const fn for_text(kind: EditErrorKind, text: TextId) -> Self {
        Self {
            kind,
            document: Some(text.document),
            paragraph: None,
            text: Some(text),
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> EditErrorKind {
        self.kind
    }

    /// Returns the affected document identity when available.
    #[must_use]
    pub const fn document(&self) -> Option<DocumentId> {
        self.document
    }

    /// Returns the affected paragraph identity when available.
    #[must_use]
    pub const fn paragraph(&self) -> Option<ParagraphId> {
        self.paragraph
    }

    /// Returns the affected text identity when available.
    #[must_use]
    pub const fn text(&self) -> Option<TextId> {
        self.text
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "document edit failed: {:?}", self.kind)
    }
}

impl core::error::Error for EditError {}

/// Stable category for style failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StyleErrorKind {
    /// A font-family request is empty, duplicated, or syntactically invalid.
    InvalidFontFamily,
    /// A numeric value is non-finite or not strictly positive when required.
    InvalidNumber,
    /// A text identity is unknown to the relevant snapshot.
    UnknownText,
    /// A paint slot has no value in the table.
    AbsentPaintSlot,
}

/// Concrete style error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct StyleError {
    kind: StyleErrorKind,
    text: Option<TextId>,
    paint: Option<PaintSlot>,
}

impl StyleError {
    pub(crate) const fn new(kind: StyleErrorKind) -> Self {
        Self {
            kind,
            text: None,
            paint: None,
        }
    }

    pub(crate) const fn for_paint(kind: StyleErrorKind, paint: PaintSlot) -> Self {
        Self {
            kind,
            text: None,
            paint: Some(paint),
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> StyleErrorKind {
        self.kind
    }

    /// Returns the affected text identity when available.
    #[must_use]
    pub const fn text(&self) -> Option<TextId> {
        self.text
    }

    /// Returns the affected paint slot when available.
    #[must_use]
    pub const fn paint(&self) -> Option<PaintSlot> {
        self.paint
    }
}

impl fmt::Display for StyleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "style operation failed: {:?}", self.kind)
    }
}

impl core::error::Error for StyleError {}

/// Stable category for scene-preparation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SceneErrorKind {
    /// The finite width is invalid.
    InvalidWidth,
    /// Paragraph preparation failed.
    Preparation,
    /// Prepared source ranges do not cover valid snapshot text.
    SourceCoverage,
    /// The first-slice flow algorithm could not produce finite geometry.
    Flow,
    /// A style references an absent paint slot or text leaf.
    InvalidStyle,
    /// A composition does not belong to the requested snapshot or has an invalid target.
    InvalidComposition,
}

/// Concrete scene-preparation error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SceneError {
    kind: SceneErrorKind,
    document: Option<DocumentId>,
    paragraph: Option<ParagraphId>,
    source: Option<Range<u32>>,
    preparation: Option<PreparationErrorKind>,
}

impl SceneError {
    pub(crate) const fn new(kind: SceneErrorKind) -> Self {
        Self {
            kind,
            document: None,
            paragraph: None,
            source: None,
            preparation: None,
        }
    }

    pub(crate) const fn for_document(kind: SceneErrorKind, document: DocumentId) -> Self {
        Self {
            kind,
            document: Some(document),
            paragraph: None,
            source: None,
            preparation: None,
        }
    }

    pub(crate) const fn for_paragraph(kind: SceneErrorKind, paragraph: ParagraphId) -> Self {
        Self {
            kind,
            document: Some(paragraph.document),
            paragraph: Some(paragraph),
            source: None,
            preparation: None,
        }
    }

    pub(crate) fn for_source(
        kind: SceneErrorKind,
        paragraph: ParagraphId,
        source: Range<u32>,
    ) -> Self {
        Self {
            kind,
            document: Some(paragraph.document),
            paragraph: Some(paragraph),
            source: Some(source),
            preparation: None,
        }
    }

    pub(crate) const fn from_preparation(
        paragraph: ParagraphId,
        preparation: PreparationErrorKind,
    ) -> Self {
        Self {
            kind: SceneErrorKind::Preparation,
            document: Some(paragraph.document),
            paragraph: Some(paragraph),
            source: None,
            preparation: Some(preparation),
        }
    }

    pub(crate) fn from_preparation_source(
        paragraph: ParagraphId,
        source: Range<u32>,
        preparation: PreparationErrorKind,
    ) -> Self {
        Self {
            kind: SceneErrorKind::Preparation,
            document: Some(paragraph.document),
            paragraph: Some(paragraph),
            source: Some(source),
            preparation: Some(preparation),
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> SceneErrorKind {
        self.kind
    }

    /// Returns the affected document identity when available.
    #[must_use]
    pub const fn document(&self) -> Option<DocumentId> {
        self.document
    }

    /// Returns the affected paragraph identity when available.
    #[must_use]
    pub const fn paragraph(&self) -> Option<ParagraphId> {
        self.paragraph
    }

    /// Returns the paragraph-local source range when validation identified one.
    #[must_use]
    pub fn source(&self) -> Option<Range<u32>> {
        self.source.clone()
    }

    /// Returns the underlying preparation category without exposing a backend error.
    #[must_use]
    pub const fn preparation(&self) -> Option<PreparationErrorKind> {
        self.preparation
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scene preparation failed: {:?}", self.kind)?;
        if let Some(preparation) = self.preparation {
            write!(formatter, " ({preparation:?})")?;
        }
        Ok(())
    }
}

impl core::error::Error for SceneError {}

/// Stable category for snapshot-selection failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionErrorKind {
    /// A position or selection belongs to another document or scene revision.
    WrongSnapshot,
    /// A position is not present in the scene interaction map.
    UnknownPosition,
    /// Cursor transitions do not connect the requested visual selection.
    DisconnectedMovement,
    /// Independent selections overlap or duplicate an insertion point.
    OverlappingSelections,
}

/// Concrete snapshot-selection error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SelectionError {
    kind: SelectionErrorKind,
}

impl SelectionError {
    pub(crate) const fn new(kind: SelectionErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> SelectionErrorKind {
        self.kind
    }
}

impl fmt::Display for SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "snapshot selection failed: {:?}", self.kind)
    }
}

impl core::error::Error for SelectionError {}

#[cfg(test)]
mod tests {
    use alloc::format;

    use super::{PreparationErrorKind, SceneError};
    use crate::{DocumentId, ParagraphId};

    #[test]
    fn scene_error_display_includes_the_preparation_category() {
        let error = SceneError::from_preparation(
            ParagraphId {
                document: DocumentId::from_bytes(*b"scene-error-doc1"),
                index: 4,
            },
            PreparationErrorKind::UnsupportedPaintCoverage,
        );
        assert_eq!(
            format!("{error}"),
            "scene preparation failed: Preparation (UnsupportedPaintCoverage)"
        );
    }
}
