// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;
use core::ops::Range;

use crate::adapter::PreparationErrorKind;
use crate::document::{DocumentId, ParagraphId, TextId};
use crate::style::PaintSlot;

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
        write!(formatter, "scene preparation failed: {:?}", self.kind)
    }
}

impl core::error::Error for SceneError {}
