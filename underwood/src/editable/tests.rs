// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{
    EditableSurface, EditableSurfaceElement, SurfaceTextEncoding, byte_offset_for_encoded,
    encoded_len,
};
use crate::{Document, DocumentId, InlineRole, ParagraphRole, SurfaceErrorKind};

#[test]
fn explicit_scope_flattens_leaves_and_rejects_ambiguous_identity() {
    let mut document = Document::new(DocumentId::from_bytes(*b"surface-scope-01"));
    let mut edit = document.edit();
    let first_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("first paragraph must append");
    let first = edit
        .append_text(first_paragraph, InlineRole::TEXT, "alpha")
        .expect("first text must append");
    let second_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("second paragraph must append");
    let second = edit
        .append_text(second_paragraph, InlineRole::TEXT, "مرحبا")
        .expect("second text must append");
    edit.commit().expect("fixture must publish");
    let snapshot = document.snapshot();
    let surface = EditableSurface::new(
        &snapshot,
        [
            EditableSurfaceElement::text(first),
            EditableSurfaceElement::separator("\n"),
            EditableSurfaceElement::text(second),
        ],
    )
    .expect("explicit semantic scope must flatten");
    assert_eq!(surface.text(), "alpha\nمرحبا");

    let duplicate = EditableSurface::new(
        &snapshot,
        [
            EditableSurfaceElement::text(first),
            EditableSurfaceElement::text(first),
        ],
    )
    .expect_err("duplicated semantic text makes offsets ambiguous");
    assert_eq!(duplicate.kind(), SurfaceErrorKind::DuplicateText);
}

#[test]
fn encoding_conversion_rejects_interior_utf16_units() {
    let text = "A🙂é";
    assert_eq!(
        encoded_len(text, SurfaceTextEncoding::Utf16).expect("fixture length fits"),
        4
    );
    assert_eq!(
        byte_offset_for_encoded(text, 3, SurfaceTextEncoding::Utf16)
            .expect("post-surrogate offset must map"),
        5
    );
    assert_eq!(
        byte_offset_for_encoded(text, 2, SurfaceTextEncoding::Utf16)
            .expect_err("interior surrogate offset must fail")
            .kind(),
        SurfaceErrorKind::InvalidRange
    );
}
