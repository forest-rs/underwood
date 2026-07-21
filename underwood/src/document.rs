// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{EditError, EditErrorKind};

/// Opaque identity of one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocumentId([u8; 16]);

impl DocumentId {
    /// Creates a document identity from caller-owned opaque bytes.
    #[must_use]
    pub const fn from_bytes(value: [u8; 16]) -> Self {
        Self(value)
    }

    pub(crate) const fn opaque_bytes(self) -> [u8; 16] {
        self.0
    }
}

/// Monotonic revision within one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentRevision(pub(crate) u64);

/// Opaque identity of a paragraph within one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParagraphId {
    pub(crate) document: DocumentId,
    pub(crate) index: u32,
}

/// Opaque identity of a text leaf within one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextId {
    pub(crate) document: DocumentId,
    pub(crate) paragraph: u32,
    pub(crate) index: u32,
}

/// Opaque identity of a semantic node within one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemanticId {
    document: DocumentId,
    paragraph: u32,
    text: Option<u32>,
}

/// Semantic role of a block paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParagraphRole(u8);

impl ParagraphRole {
    /// Ordinary body paragraph.
    pub const BODY: Self = Self(0);
}

/// Semantic role of an inline text leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InlineRole(u8);

impl InlineRole {
    /// Ordinary text.
    pub const TEXT: Self = Self(0);

    /// Emphasized text.
    pub const EMPHASIS: Self = Self(1);
}

#[derive(Clone, Debug)]
pub(crate) struct TextLeaf {
    pub(crate) id: TextId,
    pub(crate) role: InlineRole,
    pub(crate) text: Arc<str>,
}

#[derive(Clone, Debug)]
pub(crate) struct Paragraph {
    pub(crate) id: ParagraphId,
    pub(crate) role: ParagraphRole,
    pub(crate) version: u64,
    pub(crate) leaves: Vec<TextLeaf>,
}

#[derive(Clone, Debug)]
pub(crate) struct DocumentState {
    pub(crate) id: DocumentId,
    pub(crate) revision: DocumentRevision,
    pub(crate) paragraphs: Vec<Paragraph>,
}

/// Mutable owner of the current immutable document snapshot.
#[derive(Debug)]
pub struct Document {
    state: Arc<DocumentState>,
}

impl Document {
    /// Creates an empty document at revision zero.
    #[must_use]
    pub fn new(id: DocumentId) -> Self {
        Self {
            state: Arc::new(DocumentState {
                id,
                revision: DocumentRevision(0),
                paragraphs: Vec::new(),
            }),
        }
    }

    /// Returns a cheap immutable snapshot of the current revision.
    #[must_use]
    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            state: Arc::clone(&self.state),
        }
    }

    /// Starts an atomic staged edit.
    pub fn edit(&mut self) -> Edit<'_> {
        Edit {
            base_revision: self.state.revision,
            staged: (*self.state).clone(),
            changed: Vec::new(),
            document: self,
        }
    }
}

/// Immutable, cheaply cloneable view of one exact document revision.
#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    pub(crate) state: Arc<DocumentState>,
}

impl DocumentSnapshot {
    /// Returns the owning document identity.
    #[must_use]
    pub fn id(&self) -> DocumentId {
        self.state.id
    }

    /// Returns this snapshot's exact revision.
    #[must_use]
    pub fn revision(&self) -> DocumentRevision {
        self.state.revision
    }

    /// Returns a text leaf when the identity belongs to this document and exists in this revision.
    #[must_use]
    pub fn text(&self, id: TextId) -> Option<&str> {
        self.leaf(id).map(|leaf| leaf.text.as_ref())
    }

    pub(crate) fn paragraphs(&self) -> &[Paragraph] {
        &self.state.paragraphs
    }

    pub(crate) fn leaf(&self, id: TextId) -> Option<&TextLeaf> {
        if id.document != self.state.id {
            return None;
        }
        self.state
            .paragraphs
            .get(id.paragraph as usize)?
            .leaves
            .get(id.index as usize)
            .filter(|leaf| leaf.id == id)
    }
}

/// Staged document transaction. Dropping it publishes nothing.
#[derive(Debug)]
pub struct Edit<'document> {
    document: &'document mut Document,
    base_revision: DocumentRevision,
    staged: DocumentState,
    changed: Vec<ParagraphId>,
}

impl Edit<'_> {
    /// Appends an empty paragraph and returns its document-scoped identity.
    pub fn append_paragraph(&mut self, role: ParagraphRole) -> Result<ParagraphId, EditError> {
        let index = u32::try_from(self.staged.paragraphs.len()).map_err(|_| {
            EditError::for_document(EditErrorKind::InvalidStructure, self.staged.id)
        })?;
        let id = ParagraphId {
            document: self.staged.id,
            index,
        };
        self.staged.paragraphs.push(Paragraph {
            id,
            role,
            version: 1,
            leaves: Vec::new(),
        });
        self.mark_changed(id);
        Ok(id)
    }

    /// Appends an immutable text leaf to a paragraph.
    pub fn append_text(
        &mut self,
        paragraph: ParagraphId,
        role: InlineRole,
        text: &str,
    ) -> Result<TextId, EditError> {
        let document_id = self.staged.id;
        let record = self.paragraph_mut(paragraph)?;
        let index = u32::try_from(record.leaves.len())
            .map_err(|_| EditError::for_paragraph(EditErrorKind::OversizedText, paragraph))?;
        let id = TextId {
            document: document_id,
            paragraph: paragraph.index,
            index,
        };
        record.leaves.push(TextLeaf {
            id,
            role,
            text: Arc::from(text),
        });
        record.version = record.version.saturating_add(1);
        self.mark_changed(paragraph);
        Ok(id)
    }

    /// Replaces the complete contents of one text leaf.
    pub fn replace_text(&mut self, text: TextId, replacement: &str) -> Result<(), EditError> {
        if text.document != self.staged.id {
            return Err(EditError::for_text(EditErrorKind::WrongDocument, text));
        }
        let paragraph = self
            .staged
            .paragraphs
            .get_mut(text.paragraph as usize)
            .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, text))?;
        let leaf = paragraph
            .leaves
            .get_mut(text.index as usize)
            .filter(|leaf| leaf.id == text)
            .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, text))?;
        leaf.text = Arc::from(replacement);
        paragraph.version = paragraph.version.saturating_add(1);
        let paragraph_id = paragraph.id;
        self.mark_changed(paragraph_id);
        Ok(())
    }

    /// Atomically publishes the staged revision.
    pub fn commit(mut self) -> Result<Publication, EditError> {
        if self.document.state.revision != self.base_revision {
            return Err(EditError::for_document(
                EditErrorKind::RevisionConflict,
                self.staged.id,
            ));
        }
        self.staged.revision =
            DocumentRevision(self.base_revision.0.checked_add(1).ok_or_else(|| {
                EditError::for_document(EditErrorKind::RevisionConflict, self.staged.id)
            })?);
        let state = Arc::new(self.staged);
        self.document.state = Arc::clone(&state);
        Ok(Publication {
            snapshot: DocumentSnapshot { state },
            changes: ChangeSet {
                paragraphs: self.changed.into(),
            },
        })
    }

    fn paragraph_mut(&mut self, id: ParagraphId) -> Result<&mut Paragraph, EditError> {
        if id.document != self.staged.id {
            return Err(EditError::for_paragraph(EditErrorKind::WrongDocument, id));
        }
        self.staged
            .paragraphs
            .get_mut(id.index as usize)
            .filter(|paragraph| paragraph.id == id)
            .ok_or_else(|| EditError::for_paragraph(EditErrorKind::InvalidStructure, id))
    }

    fn mark_changed(&mut self, paragraph: ParagraphId) {
        if !self.changed.contains(&paragraph) {
            self.changed.push(paragraph);
        }
    }
}

/// Result of one committed document edit.
#[derive(Clone, Debug)]
pub struct Publication {
    snapshot: DocumentSnapshot,
    changes: ChangeSet,
}

impl Publication {
    /// Returns the newly published snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &DocumentSnapshot {
        &self.snapshot
    }

    /// Returns the paragraph-level change summary.
    #[must_use]
    pub fn changes(&self) -> &ChangeSet {
        &self.changes
    }
}

/// Paragraph-level summary of a committed edit.
#[derive(Clone, Debug)]
pub struct ChangeSet {
    paragraphs: Arc<[ParagraphId]>,
}

impl ChangeSet {
    /// Returns paragraphs touched by the transaction in document order of first mutation.
    #[must_use]
    pub fn paragraphs(&self) -> &[ParagraphId] {
        &self.paragraphs
    }
}

impl Paragraph {
    pub(crate) fn projected_text(&self) -> String {
        let mut text = String::new();
        for leaf in &self.leaves {
            text.push_str(&leaf.text);
        }
        text
    }

    pub(crate) fn semantic_id(&self) -> SemanticId {
        SemanticId {
            document: self.id.document,
            paragraph: self.id.index,
            text: None,
        }
    }
}

impl TextLeaf {
    pub(crate) fn semantic_id(&self) -> SemanticId {
        SemanticId {
            document: self.id.document,
            paragraph: self.id.paragraph,
            text: Some(self.id.index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, DocumentId, InlineRole, ParagraphRole};

    #[test]
    fn dropped_edit_publishes_nothing_and_old_snapshot_survives() {
        let mut document = Document::new(DocumentId::from_bytes(*b"document-test-01"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("paragraph must append");
        let leaf = edit
            .append_text(paragraph, InlineRole::TEXT, "old")
            .expect("text must append");
        let first = edit.commit().expect("first edit must commit");
        let old = first.snapshot().clone();

        let mut edit = document.edit();
        edit.replace_text(leaf, "not published")
            .expect("replacement must stage");
        drop(edit);
        assert_eq!(document.snapshot().text(leaf), Some("old"));

        let mut edit = document.edit();
        edit.replace_text(leaf, "new")
            .expect("replacement must stage");
        edit.commit().expect("replacement must commit");
        assert_eq!(old.text(leaf), Some("old"));
        assert_eq!(document.snapshot().text(leaf), Some("new"));
    }
}
