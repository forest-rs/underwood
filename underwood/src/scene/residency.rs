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
    structure: usize,
    layout: usize,
    paint: usize,
    sources: usize,
    semantics: usize,
    hit_testing: usize,
    selection: usize,
    navigation: usize,
    native_text_input: usize,
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

    /// Returns persistent scene-spine and publication structure bytes.
    #[must_use]
    pub const fn structure(self) -> usize {
        self.structure
    }

    /// Returns source-independent line, glyph, and flow-layout bytes.
    #[must_use]
    pub const fn layout(self) -> usize {
        self.layout
    }

    /// Returns renderer-facing paint topology bytes.
    #[must_use]
    pub const fn paint(self) -> usize {
        self.paint
    }

    /// Returns paragraph-local authored and generated provenance bytes.
    #[must_use]
    pub const fn sources(self) -> usize {
        self.sources
    }

    /// Returns semantic structure and geometry bytes.
    #[must_use]
    pub const fn semantics(self) -> usize {
        self.semantics
    }

    /// Returns point-hit cluster and visual-slice bytes.
    #[must_use]
    pub const fn hit_testing(self) -> usize {
        self.hit_testing
    }

    /// Returns caret and selection-geometry bytes.
    #[must_use]
    pub const fn selection(self) -> usize {
        self.selection
    }

    /// Returns logical and visual movement-graph bytes.
    #[must_use]
    pub const fn navigation(self) -> usize {
        self.navigation
    }

    /// Returns bytes unique to native text-input queries.
    ///
    /// A zero charge is valid when the native profile is completely served by
    /// its source, selection, and navigation prerequisites.
    #[must_use]
    pub const fn native_text_input(self) -> usize {
        self.native_text_input
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
    paragraphs: usize,
    bytes: SceneResidencyBytes,
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

    /// Returns the number of paragraph segments in this observation.
    #[must_use]
    pub const fn paragraphs(self) -> usize {
        self.paragraphs
    }

    /// Returns the aggregate category charges.
    #[must_use]
    pub const fn bytes(self) -> SceneResidencyBytes {
        self.bytes
    }
}

/// Capability and residency observation for one paragraph scene segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParagraphSceneResidency {
    paragraph: ParagraphId,
    requested: SceneFeatures,
    resident: SceneFeatures,
    bytes: SceneResidencyBytes,
}

impl ParagraphSceneResidency {
    /// Returns the stable paragraph identity.
    #[must_use]
    pub const fn paragraph(self) -> ParagraphId {
        self.paragraph
    }

    /// Returns the normalized capabilities requested by this scene handle.
    #[must_use]
    pub const fn requested(self) -> SceneFeatures {
        self.requested
    }

    /// Returns the capabilities physically resident in this segment.
    #[must_use]
    pub const fn resident(self) -> SceneFeatures {
        self.resident
    }

    /// Returns this segment's deterministic category charges.
    #[must_use]
    pub const fn bytes(self) -> SceneResidencyBytes {
        self.bytes
    }
}

/// Allocation-free paragraph residency traversal for one prepared scene.
#[derive(Clone, Debug)]
pub struct SceneParagraphResidencies<'a> {
    requested: &'a SceneFeaturePolicy,
    segments: SpineSegments<'a>,
}

impl<'a> SceneParagraphResidencies<'a> {
    pub(super) fn new(requested: &'a SceneFeaturePolicy, spine: &'a SceneSpine) -> Self {
        Self {
            requested,
            segments: spine.segments(),
        }
    }
}

impl Iterator for SceneParagraphResidencies<'_> {
    type Item = ParagraphSceneResidency;

    fn next(&mut self) -> Option<Self::Item> {
        let positioned = self.segments.next()?;
        let segment = positioned.segment;
        Some(ParagraphSceneResidency {
            paragraph: segment.paragraph,
            requested: self.requested.features_for(segment.paragraph),
            resident: segment.geometry.features,
            bytes: paragraph_bytes(segment),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.segments.size_hint()
    }
}

impl ExactSizeIterator for SceneParagraphResidencies<'_> {}

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
