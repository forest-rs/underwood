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
    /// Authored paragraph alignment.
    pub alignment: TextAlignment,
    /// Paragraph direction used to resolve logical alignment.
    pub direction: ResolvedDirection,
    /// Translation from the exact slot's inline start.
    pub inline_offset: f64,
    /// Source-complete trailing whitespace hanging from the content edge.
    pub trailing_whitespace_advance: f64,
    /// Expansion added to each eligible Western space.
    pub opportunity_expansion: f64,
    /// Western spaces that received expansion.
    pub expanded_opportunities: u32,
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
    /// Exact source-complete interaction unit under the point.
    pub source: Source,
    /// Collapsed position selected by the interaction-unit side.
    pub position: Position,
    /// Semantic text-node identity under the point.
    pub semantic_id: SemanticId,
    /// Resolved bidi level of the hit interaction unit.
    pub bidi_level: u8,
}

/// Exact scene-space caret for one snapshot position.
#[derive(Clone, Copy, Debug)]
pub struct SceneCaret<Position = SnapshotTextPosition> {
    /// Revision- or epoch-bound position represented by the caret.
    pub position: Position,
    /// Scene-space caret bounds.
    pub bounds: Rect,
}

/// One visual highlight rectangle owned by a selection and logical range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneSelectionRect {
    /// Selection index within the requested selection set.
    pub selection: usize,
    /// Logical-range index within the owning selection.
    pub range: usize,
    /// Visual line index within the scene.
    pub line: usize,
    /// Scene-space highlight bounds.
    pub bounds: Rect,
    /// Bidi level of the covered visual run.
    pub bidi_level: u8,
}

/// One visual highlight rectangle for selected generated preedit text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCompositionRect {
    /// Visual line index within the transient scene.
    pub line: usize,
    /// Scene-space highlight bounds.
    pub bounds: Rect,
    /// Bidi level of the covered visual run.
    pub bidi_level: u8,
}
