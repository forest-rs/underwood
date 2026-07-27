// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Compact retained single-paragraph text over the shared scene foundation.

use crate::{
    ComputedInlineStyle, Document, DocumentId, DocumentRevision, EditError, EditErrorKind,
    PaintTable, ParagraphId, ParagraphStyle, SnapshotTextSelectionSet, TextConstraint, TextId,
};
use alloc::sync::Arc;

#[derive(Debug)]
struct BlockState {
    id: DocumentId,
    revision: DocumentRevision,
    text: Arc<str>,
}

/// Mutable retained single-paragraph text content.
///
/// A block has document-compatible identities but retains only one immutable
/// text allocation and one compact revision record. Preparation lowers it
/// through the same paragraph, cache, and scene machinery as a document.
#[derive(Debug)]
pub struct TextBlock {
    state: Arc<BlockState>,
}

impl TextBlock {
    /// Creates one retained plain-text block and publishes its initial revision.
    pub fn plain(id: DocumentId, text: &str) -> Result<Self, EditError> {
        u32::try_from(text.len())
            .map_err(|_| EditError::for_document(EditErrorKind::OversizedText, id))?;
        Ok(Self {
            state: Arc::new(BlockState {
                id,
                revision: DocumentRevision(1),
                text: Arc::from(text),
            }),
        })
    }

    /// Returns the stable identity shared with prepared document scenes.
    #[must_use]
    pub fn id(&self) -> DocumentId {
        self.state.id
    }

    /// Returns a cheap immutable view of the current exact revision.
    #[must_use]
    pub fn snapshot(&self) -> TextBlockSnapshot {
        TextBlockSnapshot {
            state: Arc::clone(&self.state),
        }
    }

    /// Returns the complete current plain text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.state.text
    }

    /// Replaces the complete plain text and atomically publishes one revision.
    ///
    /// Setting the current value again performs no publication.
    pub fn set_text(&mut self, text: &str) -> Result<(), EditError> {
        if self.state.text.as_ref() == text {
            return Ok(());
        }
        u32::try_from(text.len())
            .map_err(|_| EditError::for_document(EditErrorKind::OversizedText, self.state.id))?;
        let revision = self
            .state
            .revision
            .0
            .checked_add(1)
            .map(DocumentRevision)
            .ok_or_else(|| {
                EditError::for_document(EditErrorKind::RevisionConflict, self.state.id)
            })?;
        self.state = Arc::new(BlockState {
            id: self.state.id,
            revision,
            text: Arc::from(text),
        });
        Ok(())
    }

    /// Atomically replaces every independent selection and returns rebound carets.
    ///
    /// The selection set must belong to the block's current revision. Each
    /// independent selection receives one insertion even when a visual bidi
    /// selection contains several logical ranges. The returned collapsed
    /// selections belong to the newly published revision.
    pub fn replace_selections(
        &mut self,
        selections: &SnapshotTextSelectionSet,
        replacement: &str,
    ) -> Result<SnapshotTextSelectionSet, EditError> {
        let mut document = self.snapshot().materialize_document();
        let replacement = document.replace_selections(selections, replacement)?;
        let selections = replacement.into_parts().1;
        let text = Arc::from(
            document
                .text(self.text_id())
                .expect("a materialized block retains its plain text leaf"),
        );
        self.state = Arc::new(BlockState {
            id: self.state.id,
            revision: selections.revision(),
            text,
        });
        Ok(selections)
    }

    fn text_id(&self) -> TextId {
        text_id(self.state.id)
    }
}

/// Immutable, cheaply cloneable view of one exact text-block revision.
#[derive(Clone, Debug)]
pub struct TextBlockSnapshot {
    state: Arc<BlockState>,
}

impl TextBlockSnapshot {
    /// Returns the block's stable identity.
    #[must_use]
    pub fn id(&self) -> DocumentId {
        self.state.id
    }

    /// Returns this snapshot's exact monotonic revision.
    #[must_use]
    pub fn revision(&self) -> DocumentRevision {
        self.state.revision
    }

    /// Returns the complete plain text at this revision.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.state.text
    }

    /// Returns the stable text-leaf identity represented by this block.
    #[must_use]
    pub fn text_id(&self) -> TextId {
        text_id(self.state.id)
    }

    pub(crate) fn paragraph_id(&self) -> ParagraphId {
        ParagraphId {
            document: self.state.id,
            index: 0,
        }
    }

    pub(crate) fn shares_state_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    pub(crate) fn materialize_document(&self) -> Document {
        Document::from_plain_block(
            self.state.id,
            self.state.revision,
            Arc::clone(&self.state.text),
        )
    }
}

const fn text_id(document: DocumentId) -> TextId {
    TextId {
        document,
        paragraph: 0,
        index: 0,
    }
}

/// Borrowed style, paint, and intrinsic constraint for one block preparation.
#[derive(Clone, Copy, Debug)]
pub struct BlockRequest<'a> {
    pub(crate) constraint: TextConstraint,
    pub(crate) style: &'a ComputedInlineStyle,
    pub(crate) paint: &'a PaintTable,
    pub(crate) paragraph_style: ParagraphStyle,
    pub(crate) region_flow: Option<&'a crate::RegionFlow>,
    pub(crate) features: crate::SceneFeatures,
    pub(crate) trace: bool,
}

impl<'a> BlockRequest<'a> {
    /// Creates a block request that borrows one reusable computed style.
    #[must_use]
    pub const fn new(
        constraint: TextConstraint,
        style: &'a ComputedInlineStyle,
        paint: &'a PaintTable,
    ) -> Self {
        Self {
            constraint,
            style,
            paint,
            paragraph_style: ParagraphStyle::DEFAULT,
            region_flow: None,
            features: crate::SceneFeatures::DISPLAY,
            trace: false,
        }
    }

    /// Returns a copy with the requested prepared-scene capabilities.
    #[must_use]
    pub const fn with_features(mut self, features: crate::SceneFeatures) -> Self {
        self.features = features;
        self
    }

    /// Returns the requested prepared-scene capabilities.
    #[must_use]
    pub const fn features(self) -> crate::SceneFeatures {
        self.features
    }

    /// Returns a copy with paragraph-level analysis and flow values.
    #[must_use]
    pub const fn with_paragraph_style(mut self, style: ParagraphStyle) -> Self {
        self.paragraph_style = style;
        self
    }

    /// Returns a block request that fills exact slots from a region flow.
    ///
    /// Region slots replace the single wrapping width. Intrinsic block
    /// measurement continues to use [`Self::new`] without regions.
    #[must_use]
    pub fn with_region_flow(mut self, region_flow: &'a crate::RegionFlow) -> Self {
        self.constraint = TextConstraint::Wrap(crate::FiniteWidth(region_flow.max_inline_size()));
        self.region_flow = Some(region_flow);
        self
    }

    /// Returns the exact region policy, when one was requested.
    #[must_use]
    pub const fn region_flow(self) -> Option<&'a crate::RegionFlow> {
        self.region_flow
    }

    /// Returns a request that records deterministic preparation diagnostics.
    #[must_use]
    pub const fn with_preparation_trace(mut self) -> Self {
        self.trace = true;
        self
    }

    /// Returns whether detailed preparation tracing was requested.
    #[must_use]
    pub const fn preparation_trace(self) -> bool {
        self.trace
    }
}

#[cfg(test)]
mod tests {
    use super::TextBlock;
    use crate::DocumentId;

    #[test]
    fn repeated_value_is_not_published_again() {
        let mut block = TextBlock::plain(DocumentId::from_bytes(*b"block-same-value"), "Save")
            .expect("block must initialize");
        let before = block.snapshot();
        block.set_text("Save").expect("same text must be accepted");
        let after = block.snapshot();
        assert_eq!(before.revision(), after.revision());
        assert_eq!(after.text(), "Save");
    }
}
