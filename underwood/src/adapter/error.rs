// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Stable adapter failure vocabulary.
//!
//! This module owns paragraph-preparation error categories; it explicitly does
//! not own backend-specific error payloads.

use super::*;

/// Stable category for adapter and prepared-output failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreparationErrorKind {
    /// Required Unicode data or another capability is unavailable.
    MissingCapability,
    /// No usable font is available for the source.
    MissingFont,
    /// Faithful source-to-paint coverage cannot be represented.
    UnsupportedPaintCoverage,
    /// Adapter output violates the owned preparation contract.
    InvalidOutput,
    /// Work was cancelled before publication.
    Cancelled,
}

/// Concrete paragraph-preparation error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PreparationError {
    kind: PreparationErrorKind,
}

impl PreparationError {
    /// Creates an error for unavailable Unicode or shaping capabilities.
    #[must_use]
    pub const fn missing_capability() -> Self {
        Self {
            kind: PreparationErrorKind::MissingCapability,
        }
    }

    /// Creates an error for missing usable fonts.
    #[must_use]
    pub const fn missing_font() -> Self {
        Self {
            kind: PreparationErrorKind::MissingFont,
        }
    }

    /// Creates an error for paint coverage that cannot be represented faithfully.
    #[must_use]
    pub const fn unsupported_paint_coverage() -> Self {
        Self {
            kind: PreparationErrorKind::UnsupportedPaintCoverage,
        }
    }

    /// Creates an error for invalid backend output.
    #[must_use]
    pub const fn invalid_output() -> Self {
        Self {
            kind: PreparationErrorKind::InvalidOutput,
        }
    }

    /// Creates an error for cancelled work.
    #[must_use]
    pub const fn cancelled() -> Self {
        Self {
            kind: PreparationErrorKind::Cancelled,
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> PreparationErrorKind {
        self.kind
    }
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "paragraph preparation failed: {:?}", self.kind)
    }
}

impl core::error::Error for PreparationError {}
