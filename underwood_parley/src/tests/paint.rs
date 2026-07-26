// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn paint_slot_change_retains_non_paint_prepared_facts() {
    let mut document = Document::new(DocumentId::from_bytes(*b"paint-retain-001"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph is valid");
    let text = edit
        .append_text(paragraph, InlineRole::TEXT, "plain paint")
        .expect("test text is valid");
    edit.commit().expect("fixture document is valid");
    let base = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0).expect("test style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let mut styles = StyleMap::new(base.clone());
    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::BLACK),
        Brush::Solid(Color::from_rgb8(0xcc, 0x44, 0x33)),
    ]);
    let outputs = Rc::new(RefCell::new(Vec::new()));
    let probe = PreparedFactsProbe {
        inner: fixture_paragraph_engine(),
        outputs: Rc::clone(&outputs),
    };
    let mut engine = LayoutEngine::new(
        probe,
        CacheBudget::new(8).with_adapter_facts_bytes(64 * 1024 * 1024),
    );
    let request = editable_scene_request(TextConstraint::MaxContent, &styles, &paint);
    engine
        .prepare(&document.snapshot(), &request)
        .expect("initial paint prepares");

    styles.set(text, base.with_paint(PaintSlot::new(1)));
    let repainted = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("paint-slot change prepares");
    assert_eq!(repainted.work().analysis().paragraphs(), 0);
    assert_eq!(repainted.work().font_selection().paragraphs(), 0);
    assert_eq!(repainted.work().shape().paragraphs(), 0);
    assert_eq!(repainted.work().flow().paragraphs(), 0);
    assert!(
        repainted
            .scene()
            .fragments()
            .iter()
            .all(|fragment| fragment.paint() == PaintSlot::new(1))
    );

    let outputs = outputs.borrow();
    let [initial, changed] = outputs.as_slice() else {
        panic!("initial and paint-only prepared facts must be observed");
    };
    assert_eq!(
        initial.movements().as_ptr(),
        changed.movements().as_ptr(),
        "paint-only lowering must retain the complete cursor graph"
    );
    assert_eq!(
        initial.lines()[0]
            .units()
            .next()
            .expect("initial line has an interaction unit")
            .slices()
            .as_ptr(),
        changed.lines()[0]
            .units()
            .next()
            .expect("repainted line has an interaction unit")
            .slices()
            .as_ptr(),
        "paint-only lowering must retain the packed interaction table"
    );
    assert_eq!(
        initial.lines()[0].runs()[0].normalized_coords().as_ptr(),
        changed.lines()[0].runs()[0].normalized_coords().as_ptr(),
        "paint-only lowering must retain font-instance coordinates"
    );
}

#[test]
fn zero_advance_arabic_mark_uses_unclipped_whole_glyph_paint() {
    let (document, styles, paint) = fixture_document("ب", 1.2);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("Arabic mark shaping must form a scene");
    let mark = output
        .scene()
        .fragments()
        .iter()
        .find(|fragment| {
            fragment
                .glyphs()
                .iter()
                .next()
                .expect("glyph exists")
                .advance()
                .x
                == 0.0
        })
        .expect("Noto Kufi beh must expose its zero-advance dot glyph");
    assert_eq!(mark.paint(), PaintSlot::new(0));
    assert_eq!(
        mark.paint_clip(),
        None,
        "ordinary zero-advance marks must let the font rasterizer paint the complete glyph"
    );
}

#[test]
fn ordinary_glyphs_do_not_require_outline_metrics_or_paint_clips() {
    let (document, styles, paint) = fixture_document("j office ب", 1.2);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("ordinary glyph shaping must not require outline metrics");
    assert!(
        !output.scene().fragments().is_empty(),
        "the mixed fixture must produce renderable glyphs"
    );
    assert!(
        output
            .scene()
            .fragments()
            .iter()
            .all(|fragment| fragment.paint_clip().is_none()),
        "single-paint glyphs must be complete unclipped draws"
    );
    let fragments = output.scene().fragments();
    let glyphs: usize = fragments
        .iter()
        .map(|fragment| fragment.glyphs().len())
        .sum();
    assert!(
        glyphs > fragments.len(),
        "ordinary paint must coalesce multiple glyphs into run-sized fragments"
    );
}

#[test]
fn synthetic_embolden_prepares_without_outline_metrics() {
    let mut document = Document::new(DocumentId::from_bytes(*b"embolden-test-01"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, "مرحبا")
        .expect("test source is valid");
    edit.commit().expect("test edit is valid");

    let shaping = ShapingStyle::new(FontFamily::named("Noto Kufi Arabic"), 20.0)
        .expect("test style is valid")
        .with_font_weight(FontWeight::BOLD)
        .expect("bold request is valid");
    let styles = StyleMap::new(ComputedInlineStyle::new(
        shaping,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    ));
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid")
    ])
    .expect("fixture catalog is valid");
    let mut engine = LayoutEngine::new(ParleyParagraphEngine::new(fonts), CacheBudget::new(32));
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("synthetic emboldening must not require outline bounds to prepare");

    assert!(!output.scene().fragments().is_empty());
    assert!(
        output
            .scene()
            .fragments()
            .iter()
            .all(|fragment| { fragment.synthesis().embolden() && fragment.paint_clip().is_none() })
    );
}

#[cfg(all(feature = "system-fonts", target_vendor = "apple"))]
#[test]
fn system_font_fallback_prepares_han_without_outline_metrics() {
    let mut document = Document::new(DocumentId::from_bytes(*b"system-han-test1"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, "漢字")
        .expect("test source is valid");
    edit.commit().expect("test edit is valid");

    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0).expect("test style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid")
    ])
    .expect("fixture catalog is valid")
    .with_system_fonts();
    let mut engine = LayoutEngine::new(ParleyParagraphEngine::new(fonts), CacheBudget::new(32));
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("Han source must prepare through the native fallback catalog");

    assert!(output.scene().fragments().iter().any(|fragment| {
        fragment.script() == *b"Hani"
            && fragment.font().data.as_ref() != LATIN_FONT
            && fragment.paint_clip().is_none()
    }));
}

#[test]
fn split_paint_ligature_without_component_geometry_fails_explicitly() {
    let mut document = Document::new(DocumentId::from_bytes(*b"paint-ligature01"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph is valid");
    let prefix = edit
        .append_text(paragraph, InlineRole::TEXT, "of")
        .expect("prefix is valid");
    let suffix = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "fice")
        .expect("suffix is valid");
    edit.commit().expect("test edit is valid");

    let base = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 40.0).expect("test style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let mut styles = StyleMap::new(base.clone());
    styles.set(prefix, base.clone());
    styles.set(suffix, base.with_paint(PaintSlot::new(1)));
    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::BLACK),
        Brush::Solid(Color::from_rgba8(0xff, 0x00, 0x00, 0xff)),
    ]);
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let error = fixture_engine()
        .prepare(&document.snapshot(), &request)
        .expect_err("Roboto Flex has no GDEF ligature carets for an exact paint split");
    assert_eq!(
        error.preparation(),
        Some(PreparationErrorKind::UnsupportedPaintCoverage)
    );
}

#[test]
fn bidi_format_controls_remain_source_complete_without_phantom_glyphs() {
    let text = "office \u{2067}مرحبا\u{2069} world";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("bidi format controls must not become phantom glyphs or source gaps");
    let sources = scene_sources(output.scene());
    assert_eq!(output.scene().lines().len(), 1);
    assert_eq!(
        sources
            .for_line(output.scene().line(0).expect("line exists"))
            .iter()
            .next()
            .expect("source exists")
            .bytes(),
        0..u32::try_from(text.len()).expect("fixture length fits")
    );
    let isolate =
        u32::try_from(text.find('\u{2067}').expect("isolate exists")).expect("fixture range fits");
    let pop = u32::try_from(text.find('\u{2069}').expect("pop isolate exists"))
        .expect("fixture range fits");
    assert!(output.scene().fragments().iter().all(|fragment| {
        fragment.glyphs().iter().all(|glyph| {
            let source = sources
                .first_for_glyph(glyph)
                .expect("source-aware glyph retains source")
                .bytes();
            !((source.start <= isolate && source.end >= isolate + 3)
                || (source.start <= pop && source.end >= pop + 3))
        })
    }));
}
