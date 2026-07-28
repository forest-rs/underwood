// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic accounting for capability-scaled prepared scenes.
//!
//! This module owns published-scene residency observations. It explicitly does
//! not own cache eviction, allocator-exact accounting, or renderer resources.

use super::*;
use core::mem::size_of;

/// Deterministic byte charges for one prepared-scene representation.
///
/// Charges cover owned table capacities and immutable packed storage. They
/// deliberately exclude allocator metadata, shared font bytes, paint-table
/// values, and caller or renderer resources.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneResidencyBytes {
    /// Persistent scene-spine and publication structure bytes.
    pub structure: usize,
    /// Source-independent line, glyph, and flow-layout bytes.
    pub layout: usize,
    /// Renderer-facing paint topology bytes.
    pub paint: usize,
    /// Paragraph-local authored and generated provenance bytes.
    pub sources: usize,
    /// Semantic structure and geometry bytes.
    pub semantics: usize,
    /// Point-hit cluster and visual-slice bytes.
    pub hit_testing: usize,
    /// Caret and selection-geometry bytes.
    pub selection: usize,
    /// Logical and visual movement-graph bytes.
    pub navigation: usize,
    /// Bytes unique to native text-input queries.
    pub native_text_input: usize,
}

impl SceneResidencyBytes {
    pub(super) const fn from_categories(
        layout: usize,
        paint: usize,
        sources: usize,
        semantics: usize,
        hit_testing: usize,
        selection: usize,
        navigation: usize,
        native_text_input: usize,
    ) -> Self {
        Self {
            structure: 0,
            layout,
            paint,
            sources,
            semantics,
            hit_testing,
            selection,
            navigation,
            native_text_input,
        }
    }

    pub(super) fn add_structure(&mut self, bytes: usize) {
        self.structure = self.structure.saturating_add(bytes);
    }

    pub(super) fn add_assign(&mut self, other: Self) {
        self.structure = self.structure.saturating_add(other.structure);
        self.layout = self.layout.saturating_add(other.layout);
        self.paint = self.paint.saturating_add(other.paint);
        self.sources = self.sources.saturating_add(other.sources);
        self.semantics = self.semantics.saturating_add(other.semantics);
        self.hit_testing = self.hit_testing.saturating_add(other.hit_testing);
        self.selection = self.selection.saturating_add(other.selection);
        self.navigation = self.navigation.saturating_add(other.navigation);
        self.native_text_input = self
            .native_text_input
            .saturating_add(other.native_text_input);
    }

    /// Returns the saturating sum of every reported category.
    #[must_use]
    pub const fn total(self) -> usize {
        self.structure
            .saturating_add(self.layout)
            .saturating_add(self.paint)
            .saturating_add(self.sources)
            .saturating_add(self.semantics)
            .saturating_add(self.hit_testing)
            .saturating_add(self.selection)
            .saturating_add(self.navigation)
            .saturating_add(self.native_text_input)
    }
}

/// Aggregate deterministic residency for one immutable prepared scene.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneResidency {
    /// Paragraph segments in the scene.
    pub paragraphs: usize,
    /// Aggregate category charges.
    pub bytes: SceneResidencyBytes,
}

impl SceneResidency {
    pub(super) fn from_spine(spine: &SceneSpine) -> Self {
        let mut bytes = SceneResidencyBytes::default();
        bytes.add_structure(spine.accounted_node_bytes());
        for positioned in spine.segments() {
            bytes.add_assign(paragraph_bytes(positioned.segment));
        }
        Self {
            paragraphs: spine.summary().paragraphs,
            bytes,
        }
    }
}

/// Capability and residency observation for one paragraph scene segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParagraphSceneResidency {
    /// Stable paragraph identity.
    pub paragraph: ParagraphId,
    /// Normalized capabilities requested by this scene handle.
    pub requested: SceneFeatures,
    /// Capabilities physically resident in this segment.
    pub resident: SceneFeatures,
    /// This segment's deterministic category charges.
    pub bytes: SceneResidencyBytes,
}

pub(super) fn paragraph_residencies<'a>(
    requested: &'a SceneFeaturePolicy,
    spine: &'a SceneSpine,
) -> impl ExactSizeIterator<Item = ParagraphSceneResidency> + Clone + 'a {
    spine.segments().map(|positioned| {
        let segment = positioned.segment;
        ParagraphSceneResidency {
            paragraph: segment.paragraph,
            requested: requested.features_for(segment.paragraph),
            resident: segment.geometry.features,
            bytes: paragraph_bytes(segment),
        }
    })
}

pub(super) fn paragraph_bytes(segment: &ParagraphSceneSegment) -> SceneResidencyBytes {
    let mut bytes = segment.geometry.residency_bytes();
    bytes.paint = bytes.paint.saturating_add(segment.paint.residency_bytes());
    bytes.layout =
        bytes
            .layout
            .saturating_add(segment.region_transcript.as_ref().map_or(0, |transcript| {
                size_of::<crate::RegionAttempt>().saturating_mul(transcript.attempts().len())
            }));
    bytes
}
