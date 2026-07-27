// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renderer-neutral lines, glyph fragments, semantics, and geometry results.
//!
//! This module owns immutable scene observations; it explicitly does not own
//! hit-testing policy or retained preparation.

use super::*;

/// Immutable accepted-line translation and Western space expansion.
///
/// This is post-formation evidence: it never changes the line's source
/// boundary or canonical shaping. Scene glyphs, hit geometry, carets,
/// selections, semantics, and export adapters all consume the resulting
/// adjusted coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineAdjustment {
    alignment: TextAlignment,
    direction: ResolvedDirection,
    inline_offset: f64,
    trailing_whitespace_advance: f64,
    opportunity_expansion: f64,
    expanded_opportunities: u32,
}

impl LineAdjustment {
    pub(super) const fn new(
        alignment: TextAlignment,
        direction: ResolvedDirection,
        inline_offset: f64,
        trailing_whitespace_advance: f64,
        opportunity_expansion: f64,
        expanded_opportunities: u32,
    ) -> Self {
        Self {
            alignment,
            direction,
            inline_offset,
            trailing_whitespace_advance,
            opportunity_expansion,
            expanded_opportunities,
        }
    }

    /// Returns the authored paragraph alignment.
    #[must_use]
    pub const fn alignment(self) -> TextAlignment {
        self.alignment
    }

    /// Returns the paragraph direction consumed to resolve logical alignment.
    #[must_use]
    pub const fn direction(self) -> ResolvedDirection {
        self.direction
    }

    /// Returns the translation from the exact slot's inline start.
    #[must_use]
    pub const fn inline_offset(self) -> f64 {
        self.inline_offset
    }

    /// Returns source-complete trailing whitespace hanging from the aligned
    /// content edge.
    #[must_use]
    pub const fn trailing_whitespace_advance(self) -> f64 {
        self.trailing_whitespace_advance
    }

    /// Returns expansion added to each eligible Western space.
    #[must_use]
    pub const fn opportunity_expansion(self) -> f64 {
        self.opportunity_expansion
    }

    /// Returns how many Western spaces received expansion.
    #[must_use]
    pub const fn expanded_opportunities(self) -> usize {
        self.expanded_opportunities as usize
    }
}

/// Opaque identity of a fragment within the current retained engine context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneFragmentId(pub(super) u64);

/// Opaque identity of one shaped glyph instance in a prepared scene.
///
/// A glyph can occur in more than one paint fragment when style boundaries
/// divide its visible area. Those observations retain one shared identity.
/// Structurally shared paragraph geometry retains its glyph identities across
/// corresponding scene preparations. Compare identities only while at least
/// one such scene remains alive; they are not persistent document identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneGlyphInstanceId {
    pub(super) geometry: usize,
    pub(super) glyph: usize,
}

/// Result of scene-space hit testing.
#[derive(Clone, Debug)]
pub struct TextHit<Source = SnapshotTextUnit, Position = SnapshotTextPosition> {
    pub(super) source: Source,
    pub(super) position: Position,
    pub(super) semantic_id: SemanticId,
    pub(super) bidi_level: u8,
}

impl<Source, Position> TextHit<Source, Position> {
    /// Returns the exact source-complete interaction unit under the point.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Returns the collapsed position selected by the interaction-unit side.
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    /// Returns the semantic text-node identity under the point.
    #[must_use]
    pub const fn semantic_id(&self) -> SemanticId {
        self.semantic_id
    }

    /// Returns the resolved bidi level of the hit interaction unit.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }
}

/// Exact scene-space caret for one snapshot position.
#[derive(Clone, Copy, Debug)]
pub struct SceneCaret<Position = SnapshotTextPosition> {
    pub(super) position: Position,
    pub(super) bounds: Rect,
}

impl<Position> SceneCaret<Position> {
    /// Returns the revision- or epoch-bound position represented by the caret.
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    /// Returns scene-space caret bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// One visual highlight rectangle owned by a selection and logical range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneSelectionRect {
    pub(super) selection: usize,
    pub(super) range: usize,
    pub(super) line: usize,
    pub(super) bounds: Rect,
    pub(super) bidi_level: u8,
}

/// One visual highlight rectangle for selected generated preedit text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCompositionRect {
    pub(super) line: usize,
    pub(super) bounds: Rect,
    pub(super) bidi_level: u8,
}

impl SceneCompositionRect {
    /// Returns the visual line index within the transient scene.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns scene-space highlight bounds.
    #[must_use]
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns the bidi level of the covered visual run.
    #[must_use]
    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}

impl SceneSelectionRect {
    /// Returns the selection index within the requested selection set.
    #[must_use]
    pub const fn selection(self) -> usize {
        self.selection
    }

    /// Returns the logical-range index within the owning selection.
    #[must_use]
    pub const fn range(self) -> usize {
        self.range
    }

    /// Returns the visual line index within the scene.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns the scene-space highlight bounds.
    #[must_use]
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns the bidi level of the covered visual run.
    #[must_use]
    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}
