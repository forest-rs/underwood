// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Staged edits and atomic multi-selection replacement.
//!
//! This module owns mutation validation and publication; it explicitly does
//! not own the immutable document model or scene interaction.

use super::*;

#[derive(Clone, Debug)]
pub(super) struct ReplacementPlan {
    pub(super) selection: usize,
    pub(super) insertion: ReplacementPosition,
    pub(super) ranges: Vec<ReplacementRange>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ReplacementPosition {
    pub(super) text: TextId,
    pub(super) byte: u32,
}

#[derive(Clone, Debug)]
pub(super) struct ReplacementRange {
    pub(super) text: TextId,
    pub(super) bytes: Range<u32>,
}

#[derive(Clone, Debug)]
pub(super) struct ReplacementOperation {
    pub(super) selection: usize,
    pub(super) text: TextId,
    pub(super) bytes: Range<u32>,
    pub(super) inserts: bool,
}

pub(super) fn validate_replacement_plans(
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
        let anchor = validate_selection_position(state, selection.anchor())?;
        let extent = validate_selection_position(state, selection.extent())?;
        if anchor != extent {
            return Err(EditError::for_paragraph(
                EditErrorKind::CrossParagraphSelection,
                anchor,
            ));
        }
        let Some(first) = selection.ranges().first() else {
            return Err(EditError::for_document(
                EditErrorKind::InvalidTextRange,
                state.id,
            ));
        };
        let insertion = ReplacementPosition {
            text: first.text(),
            byte: first.bytes().start,
        };
        let mut previous: Option<(TextId, Range<u32>)> = None;
        let mut ranges = Vec::with_capacity(selection.ranges().len());
        for range in selection.ranges() {
            if range.revision() != state.revision {
                return Err(EditError::for_text(
                    EditErrorKind::RevisionConflict,
                    range.text(),
                ));
            }
            let text = range.text();
            if text.document != state.id {
                return Err(EditError::for_document(
                    EditErrorKind::WrongDocument,
                    state.id,
                ));
            }
            if text.paragraph != anchor.index {
                return Err(EditError::for_paragraph(
                    EditErrorKind::CrossParagraphSelection,
                    anchor,
                ));
            }
            let leaf = state
                .paragraphs
                .get(text.paragraph as usize)
                .and_then(|paragraph| paragraph.leaves.get(text.index as usize))
                .filter(|leaf| leaf.id == text)
                .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, text))?;
            let bytes = range.bytes();
            let out_of_order = previous
                .as_ref()
                .is_some_and(|(previous_text, previous_bytes)| {
                    text.index < previous_text.index
                        || text.index == previous_text.index
                            && (bytes.start < previous_bytes.start
                                || edit_ranges_conflict(previous_bytes, &bytes))
                });
            if bytes.start > bytes.end
                || out_of_order
                || leaf
                    .text
                    .get(bytes.start as usize..bytes.end as usize)
                    .is_none()
            {
                return Err(EditError::for_text(EditErrorKind::InvalidTextRange, text));
            }
            previous = Some((text, bytes.clone()));
            ranges.push(ReplacementRange { text, bytes });
        }
        plans.push(ReplacementPlan {
            selection: selection_index,
            insertion,
            ranges,
        });
    }
    for (index, plan) in plans.iter().enumerate() {
        for other in &plans[..index] {
            for range in &plan.ranges {
                for other_range in &other.ranges {
                    if range.text == other_range.text
                        && edit_ranges_conflict(&range.bytes, &other_range.bytes)
                    {
                        return Err(EditError::for_text(
                            EditErrorKind::OverlappingSelections,
                            range.text,
                        ));
                    }
                }
            }
        }
    }
    let mut affected = Vec::new();
    for plan in &plans {
        for range in &plan.ranges {
            if !affected.contains(&range.text) {
                affected.push(range.text);
            }
        }
    }
    for text in affected {
        let original = state
            .paragraphs
            .get(text.paragraph as usize)
            .and_then(|paragraph| paragraph.leaves.get(text.index as usize))
            .filter(|leaf| leaf.id == text)
            .map(|leaf| leaf.text.len() as u64)
            .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, text))?;
        let removed = plans
            .iter()
            .flat_map(|candidate| &candidate.ranges)
            .filter(|range| range.text == text)
            .try_fold(0_u64, |total, range| {
                total.checked_add(u64::from(range.bytes.end - range.bytes.start))
            })
            .ok_or_else(|| EditError::for_text(EditErrorKind::OversizedText, text))?;
        let selection_count = u64::try_from(
            plans
                .iter()
                .filter(|candidate| candidate.insertion.text == text)
                .count(),
        )
        .map_err(|_| EditError::for_text(EditErrorKind::OversizedText, text))?;
        let inserted = u64::from(replacement_len)
            .checked_mul(selection_count)
            .ok_or_else(|| EditError::for_text(EditErrorKind::OversizedText, text))?;
        let resulting = original
            .checked_sub(removed)
            .and_then(|length| length.checked_add(inserted))
            .ok_or_else(|| EditError::for_text(EditErrorKind::OversizedText, text))?;
        if resulting > u64::from(u32::MAX) {
            return Err(EditError::for_text(EditErrorKind::OversizedText, text));
        }
    }
    Ok(plans)
}

fn validate_selection_position(
    state: &DocumentState,
    position: &SnapshotTextPosition,
) -> Result<ParagraphId, EditError> {
    let text = position.text();
    let paragraph = state
        .paragraphs
        .get(text.paragraph as usize)
        .filter(|paragraph| paragraph.id.index == text.paragraph)
        .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, text))?;
    let leaf = paragraph
        .leaves
        .get(text.index as usize)
        .filter(|leaf| leaf.id == text)
        .ok_or_else(|| EditError::for_text(EditErrorKind::InvalidStructure, text))?;
    if !leaf.text.is_char_boundary(position.byte() as usize) {
        return Err(EditError::for_text(EditErrorKind::InvalidTextRange, text));
    }
    Ok(paragraph.id)
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
/// Staged document transaction. Dropping it publishes nothing.
#[derive(Debug)]
pub struct Edit<'document> {
    pub(super) document: &'document mut Document,
    pub(super) base_revision: DocumentRevision,
    pub(super) staged: DocumentState,
    pub(super) changed: Vec<ParagraphId>,
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

    pub(super) fn replace_text_range(
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
