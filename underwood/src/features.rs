// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Explicit prepared-scene capability requests.

use alloc::{sync::Arc, vec::Vec};
use core::fmt;

use crate::ParagraphId;

const DISPLAY: u8 = 1 << 0;
const SOURCES: u8 = 1 << 1;
const SEMANTICS: u8 = 1 << 2;
const HIT_TESTING: u8 = 1 << 3;
const SELECTION: u8 = 1 << 4;
const NAVIGATION: u8 = 1 << 5;
const NATIVE_TEXT_INPUT: u8 = 1 << 6;

/// Normalized capabilities required from one prepared text scene.
///
/// Capabilities form a dependency lattice rather than independent flags.
/// Builder methods include every prerequisite, so an arbitrary invalid bit
/// pattern cannot be constructed through the public API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneFeatures {
    bits: u8,
}

impl SceneFeatures {
    /// Layout metrics and renderer-facing lines, glyphs, and paint topology.
    pub const DISPLAY: Self = Self { bits: DISPLAY };

    /// Display plus source provenance and semantic structure.
    pub const ACCESSIBLE: Self = Self {
        bits: DISPLAY | SOURCES | SEMANTICS,
    };

    /// Display, sources, point hit testing, and selection geometry.
    pub const SELECTABLE: Self = Self {
        bits: DISPLAY | SOURCES | HIT_TESTING | SELECTION,
    };

    /// The complete initial editing profile, including native text queries.
    pub const EDITABLE: Self = Self {
        bits: DISPLAY | SOURCES | HIT_TESTING | SELECTION | NAVIGATION | NATIVE_TEXT_INPUT,
    };

    /// Adds paragraph-local authored-source provenance.
    #[must_use]
    pub const fn with_sources(self) -> Self {
        Self {
            bits: self.bits | DISPLAY | SOURCES,
        }
    }

    /// Adds semantic structure and its source-provenance prerequisite.
    #[must_use]
    pub const fn with_semantics(self) -> Self {
        Self {
            bits: self.bits | DISPLAY | SOURCES | SEMANTICS,
        }
    }

    /// Adds exact point hit testing and its source-provenance prerequisite.
    #[must_use]
    pub const fn with_hit_testing(self) -> Self {
        Self {
            bits: self.bits | DISPLAY | SOURCES | HIT_TESTING,
        }
    }

    /// Adds caret and selection geometry plus exact point hit testing.
    #[must_use]
    pub const fn with_selection(self) -> Self {
        Self {
            bits: self.bits | DISPLAY | SOURCES | HIT_TESTING | SELECTION,
        }
    }

    /// Adds visual and logical navigation plus selection prerequisites.
    #[must_use]
    pub const fn with_navigation(self) -> Self {
        Self {
            bits: self.bits | DISPLAY | SOURCES | HIT_TESTING | SELECTION | NAVIGATION,
        }
    }

    /// Adds native text-input queries and the complete navigation closure.
    #[must_use]
    pub const fn with_native_text_input(self) -> Self {
        Self {
            bits: self.bits
                | DISPLAY
                | SOURCES
                | HIT_TESTING
                | SELECTION
                | NAVIGATION
                | NATIVE_TEXT_INPUT,
        }
    }

    /// Returns whether source provenance is required.
    #[must_use]
    pub const fn has_sources(self) -> bool {
        self.bits & SOURCES != 0
    }

    /// Returns whether semantic structure is required.
    #[must_use]
    pub const fn has_semantics(self) -> bool {
        self.bits & SEMANTICS != 0
    }

    /// Returns whether exact point hit testing is required.
    #[must_use]
    pub const fn has_hit_testing(self) -> bool {
        self.bits & HIT_TESTING != 0
    }

    /// Returns whether caret and selection geometry are required.
    #[must_use]
    pub const fn has_selection(self) -> bool {
        self.bits & SELECTION != 0
    }

    /// Returns whether visual and logical navigation are required.
    #[must_use]
    pub const fn has_navigation(self) -> bool {
        self.bits & NAVIGATION != 0
    }

    /// Returns whether native text-input queries are required.
    #[must_use]
    pub const fn has_native_text_input(self) -> bool {
        self.bits & NATIVE_TEXT_INPUT != 0
    }

    /// Returns whether this capability set can satisfy `required`.
    #[must_use]
    pub const fn contains(self, required: Self) -> bool {
        self.bits & required.bits == required.bits
    }

    pub(crate) const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

/// A uniform scene capability request with optional paragraph overrides.
///
/// Constructing a uniform policy does not allocate. Adding an override creates
/// immutable shared sparse storage, leaving unrelated paragraph requests at
/// the uniform default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneFeaturePolicy {
    default: SceneFeatures,
    overrides: Option<Arc<[ParagraphFeatureOverride]>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParagraphFeatureOverride {
    paragraph: ParagraphId,
    features: SceneFeatures,
}

impl SceneFeaturePolicy {
    /// Creates an allocation-free uniform capability policy.
    #[must_use]
    pub const fn uniform(features: SceneFeatures) -> Self {
        Self {
            default: features,
            overrides: None,
        }
    }

    /// Returns a policy with one sparse paragraph capability override.
    ///
    /// A later override for the same paragraph replaces the earlier value.
    #[must_use]
    pub fn with_paragraph(mut self, paragraph: ParagraphId, features: SceneFeatures) -> Self {
        let mut overrides = self
            .overrides
            .as_deref()
            .map_or_else(Vec::new, <[ParagraphFeatureOverride]>::to_vec);
        match overrides.binary_search_by_key(&paragraph, |entry| entry.paragraph) {
            Ok(index) => overrides[index].features = features,
            Err(index) => overrides.insert(
                index,
                ParagraphFeatureOverride {
                    paragraph,
                    features,
                },
            ),
        }
        self.overrides = Some(overrides.into());
        self
    }

    /// Returns the normalized uniform default.
    #[must_use]
    pub const fn default_features(&self) -> SceneFeatures {
        self.default
    }

    /// Returns the exact capability request for `paragraph`.
    #[must_use]
    pub fn features_for(&self, paragraph: ParagraphId) -> SceneFeatures {
        self.overrides
            .as_deref()
            .and_then(|overrides| {
                overrides
                    .binary_search_by_key(&paragraph, |entry| entry.paragraph)
                    .ok()
                    .map(|index| overrides[index].features)
            })
            .unwrap_or(self.default)
    }

    pub(crate) fn contains_policy(&self, required: &Self) -> bool {
        if !self.default.contains(required.default) {
            return false;
        }
        if let Some(required_overrides) = required.overrides.as_deref()
            && required_overrides
                .iter()
                .any(|entry| !self.features_for(entry.paragraph).contains(entry.features))
        {
            return false;
        }
        if let Some(resident_overrides) = self.overrides.as_deref()
            && resident_overrides.iter().any(|entry| {
                !entry
                    .features
                    .contains(required.features_for(entry.paragraph))
            })
        {
            return false;
        }
        true
    }

    pub(crate) fn overridden_paragraphs(&self) -> impl Iterator<Item = ParagraphId> + '_ {
        self.overrides
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|entry| entry.paragraph)
    }

    pub(crate) fn from_resolved(
        default: SceneFeatures,
        paragraphs: impl IntoIterator<Item = (ParagraphId, SceneFeatures)>,
    ) -> Self {
        let overrides: Vec<_> = paragraphs
            .into_iter()
            .filter_map(|(paragraph, features)| {
                (features != default).then_some(ParagraphFeatureOverride {
                    paragraph,
                    features,
                })
            })
            .collect();
        Self {
            default,
            overrides: (!overrides.is_empty()).then(|| overrides.into()),
        }
    }

    #[cfg(test)]
    fn override_count(&self) -> usize {
        self.overrides
            .as_deref()
            .map_or(0, <[ParagraphFeatureOverride]>::len)
    }
}

impl Default for SceneFeaturePolicy {
    fn default() -> Self {
        Self::uniform(SceneFeatures::DISPLAY)
    }
}

/// Diagnostic returned when a scene query requires an absent capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingSceneCapability {
    paragraph: Option<ParagraphId>,
    required: SceneFeatures,
    requested: SceneFeatures,
    resident: SceneFeatures,
}

impl MissingSceneCapability {
    pub(crate) const fn new(
        paragraph: Option<ParagraphId>,
        required: SceneFeatures,
        requested: SceneFeatures,
        resident: SceneFeatures,
    ) -> Self {
        Self {
            paragraph,
            required,
            requested,
            resident,
        }
    }

    /// Returns the paragraph missing the capability, when one was identified.
    #[must_use]
    pub const fn paragraph(self) -> Option<ParagraphId> {
        self.paragraph
    }

    /// Returns the normalized capability closure required by the query.
    #[must_use]
    pub const fn required(self) -> SceneFeatures {
        self.required
    }

    /// Returns the normalized capabilities requested for the paragraph.
    #[must_use]
    pub const fn requested(self) -> SceneFeatures {
        self.requested
    }

    /// Returns the capabilities actually resident for the paragraph.
    #[must_use]
    pub const fn resident(self) -> SceneFeatures {
        self.resident
    }
}

impl fmt::Display for MissingSceneCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "scene capability is missing: required {:?}, requested {:?}, resident {:?}",
            self.required, self.requested, self.resident
        )?;
        if let Some(paragraph) = self.paragraph {
            write!(formatter, " in paragraph {paragraph:?}")?;
        }
        Ok(())
    }
}

impl core::error::Error for MissingSceneCapability {}

#[cfg(test)]
mod tests {
    use super::{SceneFeaturePolicy, SceneFeatures};
    use crate::{DocumentId, ParagraphId};

    #[test]
    fn capability_builders_include_prerequisites() {
        let editable = SceneFeatures::DISPLAY.with_native_text_input();
        assert_eq!(editable, SceneFeatures::EDITABLE);
        assert!(editable.contains(SceneFeatures::SELECTABLE));
        assert!(SceneFeatures::ACCESSIBLE.has_semantics());
        assert!(!SceneFeatures::ACCESSIBLE.has_hit_testing());
    }

    #[test]
    fn sparse_policy_replaces_only_named_paragraph() {
        let document = DocumentId::from_bytes(*b"feature-policy!!");
        let first = ParagraphId { document, index: 0 };
        let second = ParagraphId { document, index: 1 };
        let policy = SceneFeaturePolicy::uniform(SceneFeatures::DISPLAY)
            .with_paragraph(second, SceneFeatures::EDITABLE)
            .with_paragraph(second, SceneFeatures::SELECTABLE);
        assert_eq!(policy.features_for(first), SceneFeatures::DISPLAY);
        assert_eq!(policy.features_for(second), SceneFeatures::SELECTABLE);
        assert_eq!(policy.override_count(), 1);
    }
}
