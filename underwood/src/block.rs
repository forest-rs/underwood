// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained single-paragraph text over the document and scene foundation.

use crate::{
    ComputedInlineStyle, Document, DocumentId, DocumentSnapshot, EditError, InlineRole, PaintTable,
    ParagraphRole, ParagraphStyle, TextConstraint, TextId,
};

/// Mutable retained single-paragraph text content.
///
/// A block is one ordinary paragraph and one plain semantic text leaf in the
/// existing document model. It exists to hide construction and publication
/// ceremony, not to create a separate source or shaping representation.
#[derive(Debug)]
pub struct TextBlock {
    document: Document,
    text: TextId,
}

impl TextBlock {
    /// Creates one retained plain-text block and publishes its initial revision.
    pub fn plain(id: DocumentId, text: &str) -> Result<Self, EditError> {
        let mut document = Document::new(id);
        let mut edit = document.edit();
        let paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
        let text = edit.append_text(paragraph, InlineRole::TEXT, text)?;
        edit.commit()?;
        Ok(Self { document, text })
    }

    /// Returns the stable identity shared with prepared document scenes.
    #[must_use]
    pub fn id(&self) -> DocumentId {
        self.document.snapshot().id()
    }

    /// Returns a cheap immutable view of the current exact revision.
    #[must_use]
    pub fn snapshot(&self) -> TextBlockSnapshot {
        TextBlockSnapshot {
            document: self.document.snapshot(),
            text: self.text,
        }
    }

    /// Replaces the complete plain text and atomically publishes one revision.
    ///
    /// Setting the current value again performs no publication.
    pub fn set_text(&mut self, text: &str) -> Result<(), EditError> {
        if self.document.snapshot().text(self.text) == Some(text) {
            return Ok(());
        }
        let mut edit = self.document.edit();
        edit.replace_text(self.text, text)?;
        edit.commit()?;
        Ok(())
    }
}

/// Immutable, cheaply cloneable view of one exact text-block revision.
#[derive(Clone, Debug)]
pub struct TextBlockSnapshot {
    document: DocumentSnapshot,
    text: TextId,
}

impl TextBlockSnapshot {
    /// Returns the block's stable identity.
    #[must_use]
    pub fn id(&self) -> DocumentId {
        self.document.id()
    }

    /// Returns the complete plain text at this revision.
    #[must_use]
    pub fn text(&self) -> &str {
        self.document
            .text(self.text)
            .expect("TextBlock snapshots retain their single text leaf")
    }

    pub(crate) const fn document(&self) -> &DocumentSnapshot {
        &self.document
    }
}

/// Borrowed style, paint, and intrinsic constraint for one block preparation.
#[derive(Clone, Copy, Debug)]
pub struct BlockRequest<'a> {
    pub(crate) constraint: TextConstraint,
    pub(crate) style: &'a ComputedInlineStyle,
    pub(crate) paint: &'a PaintTable,
    pub(crate) paragraph_style: ParagraphStyle,
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
        }
    }

    /// Returns a copy with paragraph-level analysis and flow values.
    #[must_use]
    pub const fn with_paragraph_style(mut self, style: ParagraphStyle) -> Self {
        self.paragraph_style = style;
        self
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
        assert_eq!(before.document.revision(), after.document.revision());
        assert_eq!(after.text(), "Save");
    }
}
