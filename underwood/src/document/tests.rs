// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
fn same_leaf_range_edits_reuse_one_staged_buffer_and_freeze_at_commit() {
    let mut document = Document::new(DocumentId::from_bytes(*b"document-cow-001"));
    let mut fixture = document.edit();
    let paragraph = fixture
        .append_paragraph(ParagraphRole::BODY)
        .expect("paragraph must append");
    let text = fixture
        .append_text(paragraph, InlineRole::TEXT, "abcdef")
        .expect("text must append");
    let publication = fixture.commit().expect("fixture must publish");
    let old = publication.snapshot().clone();

    let mut edit = document.edit();
    edit.replace_text_range(text, 1..2, "X")
        .expect("first range must stage");
    let first_buffer = edit.staged.paragraphs[0].leaves[0]
        .staged_buffer_ptr()
        .expect("the first range converts shared text to one staged buffer");
    edit.replace_text_range(text, 3..4, "Y")
        .expect("second range must stage");
    let second_buffer = edit.staged.paragraphs[0].leaves[0]
        .staged_buffer_ptr()
        .expect("the second range retains the staged buffer");
    assert_eq!(
        first_buffer, second_buffer,
        "same-length replacements in one transaction must mutate one buffer"
    );

    let committed = edit.commit().expect("ranges must publish atomically");
    assert_eq!(old.text(text), Some("abcdef"));
    assert_eq!(committed.snapshot().text(text), Some("aXcYef"));
    assert!(
        committed.snapshot().paragraphs()[0].leaves[0]
            .staged_buffer_ptr()
            .is_none(),
        "published text must freeze back to shared immutable storage"
    );
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
fn selection_replacement_crosses_leaves_but_not_paragraphs() {
    let mut document = Document::new(DocumentId::from_bytes(*b"document-test-04"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("paragraph must append");
    let first = edit
        .append_text(paragraph, InlineRole::TEXT, "é")
        .expect("first text must append");
    let second = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "z")
        .expect("second text must append");
    let other_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("other paragraph must append");
    let other = edit
        .append_text(other_paragraph, InlineRole::TEXT, "q")
        .expect("other text must append");
    let publication = edit.commit().expect("fixture must publish");
    let revision = publication.snapshot().revision();

    let empty = SnapshotTextSelectionSet::new(document.snapshot().id(), revision, alloc::vec![]);
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

    let anchor = SnapshotTextPosition::new(revision, first, 0, TextAffinity::Downstream);
    let extent = SnapshotTextPosition::new(revision, second, 1, TextAffinity::Upstream);
    let cross_leaf = SnapshotTextSelection::new(
        anchor,
        extent,
        TextSelectionMode::Logical,
        alloc::vec![
            SnapshotTextRange::new(revision, first, 0..2),
            SnapshotTextRange::new(revision, second, 0..1),
        ],
    );
    let cross_leaf =
        SnapshotTextSelectionSet::new(document.snapshot().id(), revision, alloc::vec![cross_leaf]);
    let replaced = document
        .replace_selections(&cross_leaf, "x")
        .expect("one same-paragraph insertion point may cross semantic leaves");
    assert_eq!(replaced.publication().snapshot().text(first), Some("x"));
    assert_eq!(replaced.publication().snapshot().text(second), Some(""));
    assert_eq!(
        replaced.publication().changes().paragraphs(),
        [paragraph],
        "the unrelated paragraph must retain its revision-local work"
    );
    let caret = replaced
        .selections()
        .primary()
        .expect("resulting caret survives")
        .extent();
    assert_eq!((caret.text(), caret.byte()), (first, 1));
    let first_leaf = &replaced.publication().snapshot().paragraphs()[0].leaves[0];
    let second_leaf = &replaced.publication().snapshot().paragraphs()[0].leaves[1];
    assert_eq!(first_leaf.id, first);
    assert_eq!(first_leaf.role, InlineRole::TEXT);
    assert_eq!(second_leaf.id, second);
    assert_eq!(second_leaf.role, InlineRole::EMPHASIS);

    let revision = replaced.publication().snapshot().revision();
    let cross_paragraph = SnapshotTextSelection::new(
        SnapshotTextPosition::new(revision, first, 0, TextAffinity::Downstream),
        SnapshotTextPosition::new(revision, other, 1, TextAffinity::Upstream),
        TextSelectionMode::Logical,
        alloc::vec![
            SnapshotTextRange::new(revision, first, 0..1),
            SnapshotTextRange::new(revision, other, 0..1),
        ],
    );
    let cross_paragraph = SnapshotTextSelectionSet::new(
        document.snapshot().id(),
        revision,
        alloc::vec![cross_paragraph],
    );
    assert_eq!(
        document
            .replace_selections(&cross_paragraph, "x")
            .expect_err("paragraph joining is a structural edit")
            .kind(),
        EditErrorKind::CrossParagraphSelection
    );
    assert_eq!(
        document.snapshot().revision(),
        revision,
        "failed structural replacement must publish nothing"
    );
}

#[test]
fn multi_leaf_replacement_rejects_noncanonical_ranges() {
    let mut document = Document::new(DocumentId::from_bytes(*b"document-test-06"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("paragraph must append");
    let first = edit
        .append_text(paragraph, InlineRole::TEXT, "a")
        .expect("first text must append");
    let second = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "b")
        .expect("second text must append");
    let publication = edit.commit().expect("fixture must publish");
    let revision = publication.snapshot().revision();
    let selection = SnapshotTextSelection::new(
        SnapshotTextPosition::new(revision, first, 0, TextAffinity::Downstream),
        SnapshotTextPosition::new(revision, second, 1, TextAffinity::Upstream),
        TextSelectionMode::Logical,
        alloc::vec![
            SnapshotTextRange::new(revision, second, 0..1),
            SnapshotTextRange::new(revision, first, 0..1),
        ],
    );
    let selections =
        SnapshotTextSelectionSet::new(document.snapshot().id(), revision, alloc::vec![selection]);
    assert_eq!(
        document
            .replace_selections(&selections, "x")
            .expect_err("callers must supply ranges in document order")
            .kind(),
        EditErrorKind::InvalidTextRange
    );
    assert_eq!(document.snapshot().revision(), revision);
}

#[test]
fn multi_leaf_and_shared_leaf_multicaret_rebases_from_original_revision() {
    let mut document = Document::new(DocumentId::from_bytes(*b"document-test-07"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("paragraph must append");
    let first = edit
        .append_text(paragraph, InlineRole::TEXT, "abc")
        .expect("first text must append");
    let second = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "def")
        .expect("second text must append");
    let publication = edit.commit().expect("fixture must publish");
    let revision = publication.snapshot().revision();
    let spanning = SnapshotTextSelection::new(
        SnapshotTextPosition::new(revision, first, 1, TextAffinity::Downstream),
        SnapshotTextPosition::new(revision, second, 1, TextAffinity::Upstream),
        TextSelectionMode::Logical,
        alloc::vec![
            SnapshotTextRange::new(revision, first, 1..3),
            SnapshotTextRange::new(revision, second, 0..1),
        ],
    );
    let shared_leaf = SnapshotTextSelection::new(
        SnapshotTextPosition::new(revision, second, 2, TextAffinity::Downstream),
        SnapshotTextPosition::new(revision, second, 3, TextAffinity::Upstream),
        TextSelectionMode::Logical,
        alloc::vec![SnapshotTextRange::new(revision, second, 2..3)],
    );
    let selections = SnapshotTextSelectionSet::new(
        document.snapshot().id(),
        revision,
        alloc::vec![shared_leaf, spanning],
    );

    let replacement = document
        .replace_selections(&selections, "X")
        .expect("all original-revision operations must publish once");
    assert_eq!(replacement.publication().snapshot().text(first), Some("aX"));
    assert_eq!(
        replacement.publication().snapshot().text(second),
        Some("eX")
    );
    assert_eq!(
        replacement.publication().changes().paragraphs(),
        [paragraph]
    );
    assert_eq!(
        replacement
            .selections()
            .selections()
            .iter()
            .map(|selection| (selection.extent().text(), selection.extent().byte()))
            .collect::<alloc::vec::Vec<_>>(),
        [(second, 2), (first, 2)],
        "each caret must rebase through all earlier operations on its insertion leaf"
    );
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
