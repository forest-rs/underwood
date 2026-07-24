// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn big_endian_readers_reject_short_input() {
    assert_eq!(read_u16(&[0x12, 0x34], 0), Some(0x1234));
    assert_eq!(read_u16(&[0x12], 0), None);
    assert_eq!(read_u32(&[0x12, 0x34, 0x56, 0x78], 0), Some(0x1234_5678));
    assert_eq!(read_u32(&[0x12, 0x34], 0), None);
}

#[test]
fn analysis_units_lock_extended_grapheme_trap_corpus() {
    for (name, text, expected) in [
        (
            "decomposed",
            "e\u{301}",
            core::iter::once(0..3).collect::<Vec<_>>(),
        ),
        (
            "precomposed",
            "é",
            core::iter::once(0..2).collect::<Vec<_>>(),
        ),
        ("crlf", "\r\n", core::iter::once(0..2).collect::<Vec<_>>()),
        (
            "emoji-zwj",
            "👩\u{200d}💻",
            core::iter::once(0..11).collect::<Vec<_>>(),
        ),
        (
            "regional-indicator",
            "🇺🇳",
            core::iter::once(0..8).collect::<Vec<_>>(),
        ),
        (
            "spacing-mark",
            "क\u{93e}",
            core::iter::once(0..6).collect::<Vec<_>>(),
        ),
    ] {
        let analysis = analyze_text(&mut parley_core::Analyzer::new(), text);
        assert_eq!(
            collect_analysis_units(text, &analysis)
                .expect("Parley analysis must expose complete grapheme units"),
            expected,
            "{name} must remain one interaction unit"
        );
    }
}

#[test]
fn unbundled_grapheme_corpus_drives_complete_movements_and_transactions() {
    for (name, text) in [
        ("emoji-zwj", "👩\u{200d}💻"),
        ("regional-indicator", "🇺🇳"),
        ("spacing-mark", "क\u{93e}"),
    ] {
        let analysis = analyze_text(&mut parley_core::Analyzer::new(), text);
        let units = collect_analysis_units(text, &analysis)
            .expect("Parley analysis must expose complete grapheme units");
        assert_eq!(units.len(), 1, "{name} must remain one interaction unit");
        let mut document = Document::new(DocumentId::from_bytes(*b"unbundled-egc-01"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("the proof paragraph is valid");
        let leaf = edit
            .append_text(paragraph, InlineRole::TEXT, text)
            .expect("the proof source is valid");
        edit.commit().expect("the proof document is valid");
        let style = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("proof"), 16.0).expect("the proof style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let styles = StyleMap::new(style);
        let paints = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let request = SceneRequest::new(
            TextConstraint::Wrap(FiniteWidth::new(100.0).expect("the proof width is valid")),
            &styles,
            &paints,
        );
        let output = LayoutEngine::new(AnalysisCursorProof, CacheBudget::new(32))
            .prepare(&document.snapshot(), &request)
            .expect("Parley analysis boundaries must prepare through the public scene path");
        let scene = output.scene();
        let y = scene.lines()[0].bounds().center().y;
        let start = *scene
            .hit_test_closest(Point::new(-100.0, y))
            .expect("the unit start must resolve")
            .position();
        let end = *scene
            .hit_test_closest(Point::new(100.0, y))
            .expect("the unit end must resolve")
            .position();
        let forward = scene
            .selection_set([scene
                .collapsed_selection(&start)
                .expect("the unit start must be a caret")])
            .and_then(|selection| {
                scene.move_selections(&selection, TextMovement::NextLogical, true)
            })
            .expect("the unit must expose one forward logical selection");
        let backward = scene
            .selection_set([scene
                .collapsed_selection(&end)
                .expect("the unit end must be a caret")])
            .and_then(|selection| {
                scene.move_selections(&selection, TextMovement::PreviousLogical, true)
            })
            .expect("the unit must expose one backward logical selection");
        for selection in [&forward, &backward] {
            let ranges = selection
                .primary()
                .expect("the primary selection exists")
                .ranges();
            assert_eq!(ranges.len(), 1, "{name}");
            assert_eq!(ranges[0].text(), leaf, "{name}");
            assert_eq!(
                ranges[0].bytes(),
                0..u32::try_from(text.len()).expect("the focused corpus fits portable offsets"),
                "{name}"
            );
        }
        let replaced = document
            .replace_selections(&forward, "")
            .expect("one complete unit must delete in one transaction");
        assert_eq!(
            replaced.publication().snapshot().text(leaf),
            Some(""),
            "{name}"
        );
    }
}

#[test]
fn catalog_configuration_rejects_unknown_and_untracked_families() {
    let unknown = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("fixture font is valid")
    ])
    .expect("fixture catalog is valid")
    .with_generic_families(GenericFamily::SansSerif, ["Absent Family"])
    .expect_err("generic mappings must not silently omit absent families");
    assert_eq!(
        unknown.kind(),
        AdapterErrorKind::UnknownFamily,
        "unknown family configuration must retain a stable category"
    );

    let arabic = Language::parse("ar").expect("test language is valid");
    let unsupported = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("fixture font is valid")
    ])
    .expect("fixture catalog is valid")
    .with_fallbacks(Script::from_bytes(*b"Latn"), Some(arabic), ["Roboto Flex"])
    .expect_err("untracked script-language pairs must not disappear");
    assert_eq!(
        unsupported.kind(),
        AdapterErrorKind::UnsupportedFallback,
        "unsupported fallback configuration must retain a stable category"
    );
}

#[test]
fn control_only_paragraph_emits_no_phantom_glyph() {
    let mut document = Document::new(DocumentId::from_bytes(*b"shaped-control-1"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, "\n")
        .expect("test control source is valid");
    let published = edit.commit().expect("test edit is valid");

    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 16.0).expect("test style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("fixture font is valid")
    ])
    .expect("fixture catalog is valid");
    let paragraphs = ParleyParagraphEngine::new(fonts);
    let mut layout = LayoutEngine::new(paragraphs, CacheBudget::new(32));
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.0).expect("test width is finite")),
        &styles,
        &paint,
    );
    let output = layout
        .prepare(published.snapshot(), &request)
        .expect("control-only source must prepare without a phantom glyph");
    assert!(
        output.scene().fragments().is_empty(),
        "newline shaping must not manufacture renderable glyphs"
    );
    assert_eq!(
        output.work().shape().records(),
        0,
        "shape work must report the renderable glyph count"
    );
}

#[test]
fn itemization_bounds_shaped_text_relative_offsets() {
    let text = "a".repeat(usize::from(u16::MAX) + 2);
    let analysis = analyze_text(&mut parley_core::Analyzer::new(), &text);
    let style_indices = vec![0; text.chars().count()];
    let items: Vec<_> = analysis
        .itemize(&text, |range| split_item_after(&range, &style_indices))
        .collect();
    assert_eq!(items.len(), 2, "the oversized item must split once");
    assert_eq!(items[0].range.byte_range, 0..usize::from(u16::MAX) + 1);
    assert_eq!(items[1].range.byte_range, text.len() - 1..text.len());
}
