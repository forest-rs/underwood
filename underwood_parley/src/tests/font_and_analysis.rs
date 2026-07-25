// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn explicit_paragraph_direction_covers_neutral_empty_and_first_strong_traps() {
    let mut analyzer = parley_engine::Analyzer::new();
    let numeric_auto = analyze_text(&mut analyzer, "123 / 456", BaseDirection::Auto);
    let numeric_rtl = analyze_text(&mut analyzer, "123 / 456", BaseDirection::Rtl);
    assert!(!numeric_auto.is_rtl());
    assert!(numeric_rtl.is_rtl());
    assert!(numeric_auto.bidi_levels().is_empty());
    assert!(
        numeric_rtl
            .bidi_levels()
            .iter()
            .any(|level| !level.is_multiple_of(2))
    );

    let empty_rtl = analyze_text(&mut analyzer, "", BaseDirection::Rtl);
    assert!(empty_rtl.is_rtl());
    assert!(empty_rtl.bidi_levels().is_empty());

    let arabic_auto = analyze_text(&mut analyzer, "مرحبا hello", BaseDirection::Auto);
    let arabic_ltr = analyze_text(&mut analyzer, "مرحبا hello", BaseDirection::Ltr);
    assert!(arabic_auto.is_rtl());
    assert!(!arabic_ltr.is_rtl());
    assert_ne!(arabic_auto.bidi_levels(), arabic_ltr.bidi_levels());
}

#[test]
fn paragraph_style_direction_invalidates_analysis_and_reaches_the_scene() {
    let mut document = Document::new(DocumentId::from_bytes(*b"base-direction01"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, "123 / 456")
        .expect("fixture text is valid");
    edit.commit().expect("fixture document is valid");

    let inline = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0).expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let auto_styles = StyleMap::new(inline.clone());
    let mut rtl_styles = StyleMap::new(inline);
    rtl_styles.set_paragraph_style(paragraph, ParagraphStyle::new(BaseDirection::Rtl));
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let constraint = TextConstraint::MaxContent;
    let mut engine = fixture_engine();

    let auto = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(constraint, &auto_styles, &paint),
        )
        .expect("automatic direction prepares");
    assert!(
        auto.scene()
            .fragments()
            .iter()
            .all(|fragment| fragment.bidi_level().is_multiple_of(2))
    );

    let rtl = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(constraint, &rtl_styles, &paint),
        )
        .expect("explicit RTL direction prepares");
    assert_eq!(rtl.work().analysis().paragraphs(), 1);
    assert_eq!(rtl.work().shape().paragraphs(), 1);
    assert!(
        rtl.scene()
            .fragments()
            .iter()
            .any(|fragment| !fragment.bidi_level().is_multiple_of(2))
    );
}

#[test]
fn word_break_is_range_projected_and_invalidates_from_analysis() {
    let mut document = Document::new(DocumentId::from_bytes(*b"word-break-test1"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, "abcd")
        .expect("first fixture leaf is valid");
    let breakable = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "efgh")
        .expect("second fixture leaf is valid");
    edit.commit().expect("fixture edit is valid");
    let normal = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let normal_styles = StyleMap::new(normal.clone());
    let mut break_all_styles = StyleMap::new(normal.clone());
    break_all_styles.set(
        breakable,
        normal.with_analysis(AnalysisStyle::new(WordBreak::BreakAll)),
    );
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let constraint = TextConstraint::Wrap(FiniteWidth::new(24.0).expect("fixture width is valid"));
    let mut engine = fixture_engine();

    let normal = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(constraint, &normal_styles, &paint),
        )
        .expect("normal word breaking prepares");
    assert_eq!(
        normal.scene().lines().len(),
        1,
        "an ordinary Latin word has no internal soft wrap opportunity"
    );

    let broken = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(constraint, &break_all_styles, &paint),
        )
        .expect("break-all word breaking prepares");
    assert_eq!(broken.work().analysis().paragraphs(), 1);
    assert_eq!(broken.work().shape().paragraphs(), 1);
    assert!(
        broken.scene().lines().len() > 1,
        "the leaf-local analysis run must expose break-all opportunities"
    );
}

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
        let analysis = analyze_text(
            &mut parley_engine::Analyzer::new(),
            text,
            BaseDirection::Auto,
        );
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
        let analysis = analyze_text(
            &mut parley_engine::Analyzer::new(),
            text,
            BaseDirection::Auto,
        );
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
fn font_sets_support_empty_catalogs_and_report_registered_families() {
    let empty = FontSet::empty();
    assert!(empty.registered_family_names().is_empty());

    let registered = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid"),
        Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid"),
    ])
    .expect("fixture catalog is valid");
    assert_eq!(
        registered.registered_family_names(),
        ["Noto Kufi Arabic", "Roboto Flex"]
    );
}

#[cfg(feature = "std")]
#[test]
fn font_set_clones_share_catalog_updates() {
    let mut first = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid")
    ])
    .expect("fixture catalog is valid");
    let mut clone = first.clone();

    let (collection, _) = first.resources_mut();
    let first_blob_id = memory_font_blob_id(collection, "Roboto Flex");
    let (collection, _) = clone.resources_mut();
    let clone_blob_id = memory_font_blob_id(collection, "Roboto Flex");
    assert_eq!(
        first_blob_id, clone_blob_id,
        "cloning a catalog must preserve the registered font blob identity"
    );

    let (collection, _) = first.resources_mut();
    let registered = collection.register_fonts(Blob::from(ARABIC_FONT.to_vec()), None);
    assert!(!registered.is_empty());

    let (collection, _) = clone.resources_mut();
    assert!(
        collection.family_id("Noto Kufi Arabic").is_some(),
        "a clone must observe registrations through Fontique's shared backing"
    );
}

#[cfg(feature = "std")]
#[test]
fn font_set_clones_share_file_source_cache() {
    use alloc::{format, sync::Arc};
    use fontique::{SourceId, SourceInfo, SourceKind};

    let source_id = SourceId::new();
    let path = std::env::temp_dir().join(format!(
        "underwood-font-cache-{}-{}.ttf",
        std::process::id(),
        source_id.to_u64()
    ));
    std::fs::write(&path, LATIN_FONT).expect("temporary font fixture must be writable");
    let source = SourceInfo::new(source_id, SourceKind::Path(Arc::from(path.as_path())));

    let mut first = FontSet::empty();
    let mut clone = first.clone();
    let (_, first_cache) = first.resources_mut();
    let first_blob = first_cache
        .get(&source)
        .expect("first cache must load the temporary font");
    std::fs::remove_file(&path).expect("temporary font fixture must be removable");

    let (_, clone_cache) = clone.resources_mut();
    let clone_blob = clone_cache
        .get(&source)
        .expect("clone must reuse shared data after the source is removed");
    assert_eq!(first_blob.id(), clone_blob.id());
}

#[cfg(feature = "std")]
fn memory_font_blob_id(collection: &mut fontique::Collection, family_name: &str) -> u64 {
    let family = collection
        .family_by_name(family_name)
        .expect("registered fixture family must be present");
    let font = family
        .default_font()
        .expect("registered fixture family must contain a font");
    let fontique::SourceKind::Memory(blob) = font.source().kind() else {
        panic!("registered fixture must remain a memory font");
    };
    blob.id()
}

#[cfg(feature = "system-fonts")]
#[test]
fn system_only_font_sets_are_constructible() {
    let system = FontSet::empty().with_system_fonts();
    assert!(
        system.registered_family_names().is_empty(),
        "platform families must not leak into stable registered-family reporting"
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
    let analysis = analyze_text(
        &mut parley_engine::Analyzer::new(),
        &text,
        BaseDirection::Auto,
    );
    let style_indices = vec![0; text.chars().count()];
    let items: Vec<_> = analysis
        .itemize(&text, |range| split_item_after(&range, &style_indices))
        .collect();
    assert_eq!(items.len(), 2, "the oversized item must split once");
    assert_eq!(items[0].range.byte_range, 0..usize::from(u16::MAX) + 1);
    assert_eq!(items[1].range.byte_range, text.len() - 1..text.len());
}
