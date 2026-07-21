// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

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
}

impl EditError {
    pub(crate) const fn new(kind: EditErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> EditErrorKind {
        self.kind
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
    /// A numeric value is negative or not finite.
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
}

impl StyleError {
    pub(crate) const fn new(kind: StyleErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> StyleErrorKind {
        self.kind
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
}

impl SceneError {
    pub(crate) const fn new(kind: SceneErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> SceneErrorKind {
        self.kind
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scene preparation failed: {:?}", self.kind)
    }
}

impl core::error::Error for SceneError {}
