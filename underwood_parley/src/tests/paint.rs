// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn zero_advance_arabic_mark_uses_unclipped_whole_glyph_paint() {
    let (document, styles, paint) = fixture_document("ب", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
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
    let request = SceneRequest::new(
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
    let request = SceneRequest::new(
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
    let request = SceneRequest::new(
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
    let request = SceneRequest::new(
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
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("bidi format controls must not become phantom glyphs or source gaps");
    assert_eq!(output.scene().lines().len(), 1);
    assert_eq!(
        output
            .scene()
            .line(0)
            .expect("line exists")
            .sources()
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
            let source = glyph.source().bytes();
            !((source.start <= isolate && source.end >= isolate + 3)
                || (source.start <= pop && source.end >= pop + 3))
        })
    }));
}
