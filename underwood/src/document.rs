// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::{
    EditError, EditErrorKind, SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection,
    SnapshotTextSelectionSet, TextAffinity, TextSelectionMode,
};

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
                    text: plan.text,
                    bytes: range.clone(),
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
            let mut byte = i64::from(plan.insertion);
            for operation in &operations {
                if operation.text != plan.text {
                    continue;
                }
                if !operation.bytes.is_empty() && operation.bytes.end <= plan.insertion {
                    byte -= i64::from(operation.bytes.end - operation.bytes.start);
                }
                if operation.inserts
                    && (operation.bytes.start < plan.insertion
                        || operation.selection == plan.selection)
                {
                    byte += i64::from(replacement_len);
                }
            }
            let byte = u32::try_from(byte)
                .map_err(|_| EditError::for_text(EditErrorKind::OversizedText, plan.text))?;
            let position = SnapshotTextPosition::new(
                revision,
                plan.text,
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
                alloc::vec![SnapshotTextRange::new(revision, plan.text, byte..byte,)],
            ));
        }
        Ok(SelectionReplacement {
            publication,
            selections: SnapshotTextSelectionSet::new(self.state.id, revision, resulting),
        })
    }
}

#[derive(Clone, Debug)]
struct ReplacementPlan {
    selection: usize,
    text: TextId,
    insertion: u32,
    ranges: Vec<Range<u32>>,
}

#[derive(Clone, Debug)]
struct ReplacementOperation {
    selection: usize,
    text: TextId,
    bytes: Range<u32>,
    inserts: bool,
}

fn validate_replacement_plans(
    state: &DocumentState,
    selections: &SnapshotTextSelectionSet,
    replacement_len: u32,
) -> Result<Vec<ReplacementPlan>, EditError> {
    if selections.document() != state.id {
        return Err(EditError::for_document(
            EditErrorKind::WrongDocument,
            state.id,
        ));
    }
    if selections.revision() != state.revision {
        return Err(EditError::for_document(
            EditErrorKind::RevisionConflict,
            state.id,
        ));
    }
    if selections.is_empty() {
        return Err(EditError::for_document(
            EditErrorKind::EmptySelectionSet,
            state.id,
        ));
    }
    let mut plans = Vec::with_capacity(selections.selections().len());
    for (selection_index, selection) in selections.selections().iter().enumerate() {
        if selection.anchor().revision() != state.revision
            || selection.extent().revision() != state.revision
        {
            return Err(EditError::for_document(
                EditErrorKind::RevisionConflict,
                state.id,
            ));
        }
        if selection.anchor().text().document != state.id
            || selection.extent().text().document != state.id
        {
            return Err(EditError::for_document(
                EditErrorKind::WrongDocument,
                state.id,
            ));
        }
        let Some(first) = selection.ranges().first() else {
            return Err(EditError::for_document(
                EditErrorKind::InvalidTextRange,
                state.id,
            ));
        };
        let text = first.text();
        let leaf = state
            .paragraphs
            .get(text.paragraph as usize)
            .and_then(|paragraph| paragraph.leaves.get(text.index as usize))
            .filter(|leaf| leaf.id == text)
            .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, text))?;
        let mut previous_end = None;
        let mut ranges = Vec::with_capacity(selection.ranges().len());
        for range in selection.ranges() {
            if range.revision() != state.revision {
                return Err(EditError::for_text(
                    EditErrorKind::RevisionConflict,
                    range.text(),
                ));
            }
            if range.text() != text {
                return Err(EditError::for_text(
                    EditErrorKind::CrossLeafSelection,
                    range.text(),
                ));
            }
            let bytes = range.bytes();
            if bytes.start > bytes.end
                || previous_end.is_some_and(|end| end > bytes.start)
                || leaf
                    .text
                    .get(bytes.start as usize..bytes.end as usize)
                    .is_none()
            {
                return Err(EditError::for_text(EditErrorKind::InvalidTextRange, text));
            }
            previous_end = Some(bytes.end);
            ranges.push(bytes);
        }
        plans.push(ReplacementPlan {
            selection: selection_index,
            text,
            insertion: first.bytes().start,
            ranges,
        });
    }
    for (index, plan) in plans.iter().enumerate() {
        for other in &plans[..index] {
            if plan.text != other.text {
                continue;
            }
            for range in &plan.ranges {
                for other_range in &other.ranges {
                    if edit_ranges_conflict(range, other_range) {
                        return Err(EditError::for_text(
                            EditErrorKind::OverlappingSelections,
                            plan.text,
                        ));
                    }
                }
            }
        }
    }
    for (index, plan) in plans.iter().enumerate() {
        if plans[..index]
            .iter()
            .any(|previous| previous.text == plan.text)
        {
            continue;
        }
        let original = state
            .paragraphs
            .get(plan.text.paragraph as usize)
            .and_then(|paragraph| paragraph.leaves.get(plan.text.index as usize))
            .filter(|leaf| leaf.id == plan.text)
            .map(|leaf| leaf.text.len() as u64)
            .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, plan.text))?;
        let removed = plans
            .iter()
            .filter(|candidate| candidate.text == plan.text)
            .flat_map(|candidate| &candidate.ranges)
            .try_fold(0_u64, |total, range| {
                total.checked_add(u64::from(range.end - range.start))
            })
            .ok_or_else(|| EditError::for_text(EditErrorKind::OversizedText, plan.text))?;
        let selection_count = u64::try_from(
            plans
                .iter()
                .filter(|candidate| candidate.text == plan.text)
                .count(),
        )
        .map_err(|_| EditError::for_text(EditErrorKind::OversizedText, plan.text))?;
        let inserted = u64::from(replacement_len)
            .checked_mul(selection_count)
            .ok_or_else(|| EditError::for_text(EditErrorKind::OversizedText, plan.text))?;
        let resulting = original
            .checked_sub(removed)
            .and_then(|length| length.checked_add(inserted))
            .ok_or_else(|| EditError::for_text(EditErrorKind::OversizedText, plan.text))?;
        if resulting > u64::from(u32::MAX) {
            return Err(EditError::for_text(EditErrorKind::OversizedText, plan.text));
        }
    }
    Ok(plans)
}

fn edit_ranges_conflict(first: &Range<u32>, second: &Range<u32>) -> bool {
    if first.is_empty() && second.is_empty() {
        first.start == second.start
    } else if first.is_empty() {
        second.start <= first.start && first.start <= second.end
    } else if second.is_empty() {
        first.start <= second.start && second.start <= first.end
    } else {
        first.start < second.end && second.start < first.end
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

    fn replace_text_range(
        &mut self,
        text: TextId,
        bytes: Range<u32>,
        replacement: &str,
    ) -> Result<(), EditError> {
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
        let mut value = String::from(leaf.text.as_ref());
        if value
            .get(bytes.start as usize..bytes.end as usize)
            .is_none()
        {
            return Err(EditError::for_text(EditErrorKind::InvalidTextRange, text));
        }
        value.replace_range(bytes.start as usize..bytes.end as usize, replacement);
        leaf.text = Arc::from(value);
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
        self.changed
            .sort_unstable_by_key(|paragraph| paragraph.index);
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
    paragraphs: Arc<[ParagraphId]>,
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

#[cfg(test)]
mod tests {
    use super::{Document, DocumentId, InlineRole, ParagraphRole};
    use crate::{
        EditErrorKind, SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection,
        SnapshotTextSelectionSet, TextAffinity, TextSelectionMode,
    };

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

    #[test]
    fn paragraph_heading_roles_survive_snapshot_publication() {
        let mut document = Document::new(DocumentId::from_bytes(*b"document-test-02"));
        let mut edit = document.edit();
        edit.append_paragraph(ParagraphRole::HEADING_1)
            .expect("level-one heading must append");
        edit.append_paragraph(ParagraphRole::HEADING_2)
            .expect("level-two heading must append");
        edit.append_paragraph(ParagraphRole::BODY)
            .expect("body paragraph must append");
        let published = edit.commit().expect("document must publish");

        let roles: alloc::vec::Vec<_> = published
            .snapshot()
            .paragraphs()
            .iter()
            .map(|paragraph| paragraph.role)
            .collect();
        assert_eq!(
            roles,
            [
                ParagraphRole::HEADING_1,
                ParagraphRole::HEADING_2,
                ParagraphRole::BODY
            ]
        );
    }

    #[test]
    fn selection_replacement_handles_interleaved_ranges_and_multiple_paragraphs() {
        let mut document = Document::new(DocumentId::from_bytes(*b"document-test-03"));
        let mut edit = document.edit();
        let first_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("first paragraph must append");
        let first = edit
            .append_text(first_paragraph, InlineRole::TEXT, "abcdefghij")
            .expect("first text must append");
        let second_paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("second paragraph must append");
        let second = edit
            .append_text(second_paragraph, InlineRole::TEXT, "klm")
            .expect("second text must append");
        let initial = edit.commit().expect("fixture must publish");
        let revision = initial.snapshot().revision();

        let visual = selection(revision, first, 1, TextSelectionMode::Visual, [1..2, 5..6]);
        let middle = selection(
            revision,
            first,
            3,
            TextSelectionMode::Logical,
            core::iter::once(3..4),
        );
        let other_paragraph = selection(
            revision,
            second,
            1,
            TextSelectionMode::Logical,
            core::iter::once(1..1),
        );
        let selections = SnapshotTextSelectionSet::new(
            document.snapshot().id(),
            revision,
            alloc::vec![other_paragraph, middle, visual],
        );
        let result = document
            .replace_selections(&selections, "X")
            .expect("the whole set must publish");
        assert_eq!(
            result.publication().snapshot().text(first),
            Some("aXcXeghij")
        );
        assert_eq!(result.publication().snapshot().text(second), Some("kXlm"));
        assert_eq!(
            result.publication().changes().paragraphs(),
            [first_paragraph, second_paragraph]
        );
        assert_eq!(
            result
                .selections()
                .selections()
                .iter()
                .map(|selection| (selection.extent().text(), selection.extent().byte()))
                .collect::<alloc::vec::Vec<_>>(),
            [(second, 2), (first, 4), (first, 2)],
            "post-edit carets must preserve reverse-document input order"
        );
        assert_eq!(
            document
                .replace_selections(&selections, "stale")
                .expect_err("old selections must be rejected")
                .kind(),
            EditErrorKind::RevisionConflict
        );
    }

    #[test]
    fn selection_replacement_rejects_cross_leaf_and_utf8_interior_ranges() {
        let mut document = Document::new(DocumentId::from_bytes(*b"document-test-04"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("paragraph must append");
        let first = edit
            .append_text(paragraph, InlineRole::TEXT, "é")
            .expect("first text must append");
        let second = edit
            .append_text(paragraph, InlineRole::TEXT, "z")
            .expect("second text must append");
        let publication = edit.commit().expect("fixture must publish");
        let revision = publication.snapshot().revision();

        let empty =
            SnapshotTextSelectionSet::new(document.snapshot().id(), revision, alloc::vec![]);
        assert_eq!(
            document
                .replace_selections(&empty, "x")
                .expect_err("an empty set must not publish a phantom edit")
                .kind(),
            EditErrorKind::EmptySelectionSet
        );

        let foreign = SnapshotTextSelectionSet::new(
            DocumentId::from_bytes(*b"document-test-05"),
            revision,
            alloc::vec![selection(
                revision,
                first,
                0,
                TextSelectionMode::Logical,
                core::iter::once(0..0),
            )],
        );
        assert_eq!(
            document
                .replace_selections(&foreign, "x")
                .expect_err("a selection set from another document must fail as a unit")
                .kind(),
            EditErrorKind::WrongDocument
        );

        let duplicate = selection(
            revision,
            first,
            0,
            TextSelectionMode::Logical,
            core::iter::once(0..0),
        );
        let duplicates = SnapshotTextSelectionSet::new(
            document.snapshot().id(),
            revision,
            alloc::vec![duplicate.clone(), duplicate],
        );
        assert_eq!(
            document
                .replace_selections(&duplicates, "x")
                .expect_err("duplicate insertion points must not apply in arbitrary order")
                .kind(),
            EditErrorKind::OverlappingSelections
        );

        let position = SnapshotTextPosition::new(revision, first, 0, TextAffinity::Downstream);
        let cross_leaf = SnapshotTextSelection::new(
            position,
            position,
            TextSelectionMode::Logical,
            alloc::vec![
                SnapshotTextRange::new(revision, first, 0..2),
                SnapshotTextRange::new(revision, second, 0..1),
            ],
        );
        let cross_leaf = SnapshotTextSelectionSet::new(
            document.snapshot().id(),
            revision,
            alloc::vec![cross_leaf],
        );
        assert_eq!(
            document
                .replace_selections(&cross_leaf, "x")
                .expect_err("one insertion point cannot cross leaves yet")
                .kind(),
            EditErrorKind::CrossLeafSelection
        );

        let invalid = selection(
            revision,
            first,
            1,
            TextSelectionMode::Logical,
            core::iter::once(1..1),
        );
        let invalid =
            SnapshotTextSelectionSet::new(document.snapshot().id(), revision, alloc::vec![invalid]);
        assert_eq!(
            document
                .replace_selections(&invalid, "x")
                .expect_err("a UTF-8 interior offset must fail before publication")
                .kind(),
            EditErrorKind::InvalidTextRange
        );
        assert_eq!(document.snapshot().revision(), revision);
    }

    fn selection(
        revision: crate::DocumentRevision,
        text: crate::TextId,
        byte: u32,
        mode: TextSelectionMode,
        ranges: impl IntoIterator<Item = core::ops::Range<u32>>,
    ) -> SnapshotTextSelection {
        let position = SnapshotTextPosition::new(
            revision,
            text,
            byte,
            if byte == 0 {
                TextAffinity::Downstream
            } else {
                TextAffinity::Upstream
            },
        );
        SnapshotTextSelection::new(
            position,
            position,
            mode,
            ranges
                .into_iter()
                .map(|range| SnapshotTextRange::new(revision, text, range))
                .collect(),
        )
    }
}
