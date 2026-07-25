// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Document identities, immutable state, snapshots, and publications.
//!
//! This module owns persistent semantic document state; it explicitly does not
//! own edit validation or staged transaction algorithms.

use super::*;

/// Opaque identity of one document.
///
/// Independent live documents used with the same layout engine must have
/// distinct identities. Revisions and retained paragraph caches are scoped by
/// this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// Top-level document heading.
    pub const HEADING_1: Self = Self(1);

    /// Second-level section heading.
    pub const HEADING_2: Self = Self(2);
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
    pub(super) state: Arc<DocumentState>,
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

    pub(crate) fn text(&self, id: TextId) -> Option<&str> {
        if id.document != self.state.id {
            return None;
        }
        self.state
            .paragraphs
            .get(id.paragraph as usize)?
            .leaves
            .get(id.index as usize)
            .filter(|leaf| leaf.id == id)
            .map(|leaf| leaf.text.as_ref())
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

    /// Atomically replaces every independent snapshot selection.
    ///
    /// Each selection is one insertion point: all of its logical ranges are
    /// removed, and `replacement` is inserted once at its first logical
    /// boundary. Independent selections each receive one insertion. The whole
    /// set is validated before any edit is staged.
    pub fn replace_selections(
        &mut self,
        selections: &SnapshotTextSelectionSet,
        replacement: &str,
    ) -> Result<SelectionReplacement, EditError> {
        let replacement_len = u32::try_from(replacement.len())
            .map_err(|_| EditError::for_document(EditErrorKind::OversizedText, self.state.id))?;
        let plans = validate_replacement_plans(&self.state, selections, replacement_len)?;
        let mut operations = Vec::new();
        for plan in &plans {
            for (range_index, range) in plan.ranges.iter().enumerate() {
                operations.push(ReplacementOperation {
                    selection: plan.selection,
                    text: range.text,
                    bytes: range.bytes.clone(),
                    inserts: range_index == 0,
                });
            }
        }
        operations.sort_unstable_by(|first, second| {
            (
                second.text.paragraph,
                second.text.index,
                second.bytes.start,
                second.bytes.end,
            )
                .cmp(&(
                    first.text.paragraph,
                    first.text.index,
                    first.bytes.start,
                    first.bytes.end,
                ))
        });

        let mut edit = self.edit();
        for operation in &operations {
            edit.replace_text_range(
                operation.text,
                operation.bytes.clone(),
                if operation.inserts { replacement } else { "" },
            )?;
        }
        let publication = edit.commit()?;
        let revision = publication.snapshot().revision();
        let mut resulting = Vec::with_capacity(plans.len());
        for plan in &plans {
            let mut byte = i64::from(plan.insertion.byte);
            for operation in &operations {
                if operation.text != plan.insertion.text {
                    continue;
                }
                if !operation.bytes.is_empty() && operation.bytes.end <= plan.insertion.byte {
                    byte -= i64::from(operation.bytes.end - operation.bytes.start);
                }
                if operation.inserts
                    && (operation.bytes.start < plan.insertion.byte
                        || operation.selection == plan.selection)
                {
                    byte += i64::from(replacement_len);
                }
            }
            let byte = u32::try_from(byte).map_err(|_| {
                EditError::for_text(EditErrorKind::OversizedText, plan.insertion.text)
            })?;
            let position = SnapshotTextPosition::new(
                revision,
                plan.insertion.text,
                byte,
                if byte == 0 {
                    TextAffinity::Downstream
                } else {
                    TextAffinity::Upstream
                },
            );
            resulting.push(SnapshotTextSelection::new(
                position,
                position,
                TextSelectionMode::Logical,
                alloc::vec![SnapshotTextRange::new(
                    revision,
                    plan.insertion.text,
                    byte..byte,
                )],
            ));
        }
        Ok(SelectionReplacement {
            publication,
            selections: SnapshotTextSelectionSet::new(self.state.id, revision, resulting),
        })
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
/// Result of one committed document edit.
#[derive(Clone, Debug)]
pub struct Publication {
    pub(super) snapshot: DocumentSnapshot,
    pub(super) changes: ChangeSet,
}

/// Result of one atomic selection-set replacement.
#[derive(Clone, Debug)]
pub struct SelectionReplacement {
    publication: Publication,
    selections: SnapshotTextSelectionSet,
}

impl SelectionReplacement {
    /// Returns the newly published document revision and change summary.
    #[must_use]
    pub const fn publication(&self) -> &Publication {
        &self.publication
    }

    /// Returns collapsed post-edit selections in input order.
    #[must_use]
    pub const fn selections(&self) -> &SnapshotTextSelectionSet {
        &self.selections
    }

    /// Consumes the result into its publication and post-edit selections.
    #[must_use]
    pub fn into_parts(self) -> (Publication, SnapshotTextSelectionSet) {
        (self.publication, self.selections)
    }
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
    pub(super) paragraphs: Arc<[ParagraphId]>,
}

impl ChangeSet {
    /// Returns paragraphs touched by the transaction in document order.
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
