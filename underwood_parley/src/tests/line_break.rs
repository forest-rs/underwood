// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn product_path_wraps_only_at_parley_line_boundaries() {
    let text = "alpha beta gamma";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(72.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("legal wrapping must form a scene");
    let sources = scene_sources(output.scene());
    let lines = output.scene().lines();
    assert_eq!(lines.len(), 3, "legal opportunities must form three lines");
    assert_eq!(
        lines.get(0).expect("line exists").break_reason(),
        underwood::adapter::LineBreakReason::Regular
    );
    assert_eq!(
        lines.get(1).expect("line exists").break_reason(),
        underwood::adapter::LineBreakReason::Regular
    );
    assert_eq!(
        lines.get(2).expect("line exists").break_reason(),
        underwood::adapter::LineBreakReason::End
    );
    assert_eq!(
        sources
            .for_line(lines.get(0).expect("line exists"))
            .iter()
            .next()
            .expect("source exists")
            .bytes(),
        0..6
    );
    assert_eq!(
        sources
            .for_line(lines.get(1).expect("line exists"))
            .iter()
            .next()
            .expect("source exists")
            .bytes()
            .start,
        u32::try_from(text.find("beta").expect("beta is present")).expect("fixture range fits")
    );
    assert_eq!(
        sources
            .for_line(lines.get(2).expect("line exists"))
            .iter()
            .next()
            .expect("source exists")
            .bytes()
            .start,
        u32::try_from(text.find("gamma").expect("gamma is present")).expect("fixture range fits")
    );
}

#[test]
fn product_path_coalesces_crlf_and_honors_mandatory_breaks() {
    let text = "a\r\nb\nc\u{2028}d\u{2029}e";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mandatory breaks must form a scene");
    let sources = scene_sources(output.scene());
    let lines = output.scene().lines();
    let ranges: Vec<_> = lines
        .iter()
        .map(|line| {
            sources
                .for_line(line)
                .next()
                .expect("source exists")
                .bytes()
        })
        .collect();
    assert_eq!(
        lines.len(),
        5,
        "CRLF, LF, LS, and PS form four breaks: {ranges:?}"
    );
    assert_eq!(
        sources
            .for_line(lines.get(0).expect("line exists"))
            .next()
            .expect("source exists")
            .bytes(),
        0..3,
        "CRLF stays together"
    );
    assert!(
        lines
            .iter()
            .take(4)
            .all(|line| line.break_reason() == underwood::adapter::LineBreakReason::Mandatory)
    );
    assert_eq!(
        lines.get(4).expect("line exists").break_reason(),
        underwood::adapter::LineBreakReason::End
    );
    assert_eq!(
        sources
            .for_line(lines.get(lines.len() - 1).expect("final line exists"))
            .next()
            .expect("source exists")
            .bytes()
            .end,
        u32::try_from(text.len()).expect("fixture length fits")
    );
}

#[test]
fn product_path_uses_font_metrics_for_the_baseline() {
    let (document, styles, paint) = fixture_document("Ag", 1.5);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("metric-backed formation must succeed");
    let line = &output.scene().line(0).expect("line exists");
    assert_eq!(line.bounds().height(), 30.0);
    assert!(line.baseline() > line.bounds().y0 && line.baseline() < line.bounds().y1);
    assert_eq!(
        line.baseline(),
        output
            .scene()
            .fragment(0)
            .expect("fragment exists")
            .glyphs()
            .iter()
            .next()
            .expect("glyph exists")
            .position()
            .y
    );
    assert_ne!(
        line.baseline() - line.bounds().y0,
        24.0,
        "the 80/20 split is gone"
    );
    assert!(line.content_ascent() > line.content_descent());
}

#[test]
fn line_metrics_and_source_slices_span_mixed_semantic_leaves() {
    let mut document = Document::new(DocumentId::from_bytes(*b"mixed-leaf-test1"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    let small = edit
        .append_text(paragraph, InlineRole::TEXT, "small ")
        .expect("first fixture leaf is valid");
    let large = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "BIG")
        .expect("second fixture leaf is valid");
    edit.commit().expect("fixture edit is valid");

    let small_style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("small shaping style is valid"),
        InlineFlowStyle::new(LineHeight::from_multiplier(1.2).expect("small line height is valid")),
        PaintSlot::new(0),
    );
    let large_style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 40.0)
            .expect("large shaping style is valid"),
        InlineFlowStyle::new(LineHeight::from_multiplier(1.5).expect("large line height is valid")),
        PaintSlot::new(0),
    );
    let mut styles = StyleMap::new(small_style);
    styles.set(large, large_style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mixed leaf formation succeeds");
    let line = &output.scene().line(0).expect("line exists");
    let sources = scene_sources(output.scene());
    assert_eq!(sources.for_line(*line).len(), 2);
    assert_eq!(
        sources
            .for_line(*line)
            .next()
            .expect("source exists")
            .text(),
        small
    );
    assert_eq!(
        sources
            .for_line(*line)
            .next()
            .expect("source exists")
            .bytes(),
        0..6
    );
    assert_eq!(
        sources
            .for_line(*line)
            .nth(1)
            .expect("source exists")
            .text(),
        large
    );
    assert_eq!(
        sources
            .for_line(*line)
            .nth(1)
            .expect("source exists")
            .bytes(),
        0..3
    );
    assert_eq!(line.bounds().height(), 60.0);
    assert!(
        output
            .scene()
            .fragments()
            .iter()
            .any(|fragment| fragment.font_size() == 20.0)
    );
    assert!(
        output
            .scene()
            .fragments()
            .iter()
            .any(|fragment| fragment.font_size() == 40.0)
    );
}

#[test]
fn non_breaking_space_and_unbreakable_words_overflow_honestly() {
    for text in ["alpha\u{a0}beta", "supercalifragilisticexpialidocious"] {
        let (document, styles, paint) = fixture_document(text, 1.2);
        let mut engine = fixture_engine();
        let request = editable_scene_request(
            TextConstraint::Wrap(FiniteWidth::new(10.0).expect("test width is valid")),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("an unbreakable unit may overflow");
        assert_eq!(
            output.scene().lines().len(),
            1,
            "unbreakable source must not be split: {text:?}"
        );
        assert_eq!(
            output.scene().line(0).expect("line exists").break_reason(),
            underwood::adapter::LineBreakReason::End
        );
        assert!(
            output
                .scene()
                .line(0)
                .expect("line exists")
                .bounds()
                .width()
                > 10.0,
            "overflow must remain visible rather than report a false fit: {text:?}"
        );
    }
}

#[test]
fn width_reshapes_committed_lines_while_line_height_reuses_them() {
    let text = "alpha beta gamma";
    let (document, compact_styles, paint) = fixture_document(text, 1.2);
    let (_, spacious_styles, _) = fixture_document(text, 1.8);
    let mut engine = fixture_engine();
    let wide = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &compact_styles,
        &paint,
    );
    engine
        .prepare(&document.snapshot(), &wide)
        .expect("initial formation succeeds");

    let narrow = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(72.0).expect("test width is valid")),
        &compact_styles,
        &paint,
    );
    let narrowed = engine
        .prepare(&document.snapshot(), &narrow)
        .expect("width-only formation succeeds");
    assert_eq!(narrowed.work().analysis().paragraphs(), 0);
    assert_eq!(narrowed.work().itemization().paragraphs(), 0);
    assert_eq!(narrowed.work().font_selection().paragraphs(), 0);
    assert_eq!(narrowed.work().shape().paragraphs(), 0);
    assert_eq!(narrowed.work().line_font_resolution().paragraphs(), 1);
    assert_eq!(narrowed.work().line_shape().paragraphs(), 1);
    assert!(narrowed.work().line_reshapes() > 0);
    assert_eq!(narrowed.work().flow().paragraphs(), 1);

    let spacious = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(72.0).expect("test width is valid")),
        &spacious_styles,
        &paint,
    );
    let respaced = engine
        .prepare(&document.snapshot(), &spacious)
        .expect("line-height-only formation succeeds");
    assert_eq!(respaced.work().analysis().paragraphs(), 0);
    assert_eq!(respaced.work().shape().paragraphs(), 0);
    assert_eq!(respaced.work().line_shape().paragraphs(), 0);
    assert_eq!(respaced.work().line_reshapes(), 0);
    assert_eq!(respaced.work().flow().paragraphs(), 1);
    assert!(
        respaced
            .scene()
            .line(0)
            .expect("line exists")
            .bounds()
            .height()
            > narrowed
                .scene()
                .line(0)
                .expect("line exists")
                .bounds()
                .height()
    );
}

#[test]
fn all_line_height_bases_recompute_metrics_without_reshaping() {
    let (document, base_styles, paint) = fixture_document("Metrics", 1.0);
    let base = base_styles.default_style().clone();
    let metrics = StyleMap::new(base.clone().with_inline_flow(InlineFlowStyle::new(
        LineHeight::metrics_relative(1.0).expect("metrics-relative height is valid"),
    )));
    let font_relative = StyleMap::new(base.clone().with_inline_flow(InlineFlowStyle::new(
        LineHeight::font_size_relative(2.0).expect("font-relative height is valid"),
    )));
    let absolute = StyleMap::new(base.with_inline_flow(InlineFlowStyle::new(
        LineHeight::absolute(50.0).expect("absolute height is valid"),
    )));
    let mut engine = fixture_engine();
    let constraint = TextConstraint::MaxContent;

    let metrics_output = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &metrics, &paint),
        )
        .expect("metrics-relative line height prepares");
    let metrics_height = metrics_output
        .scene()
        .line(0)
        .expect("line exists")
        .bounds()
        .height();
    assert!(metrics_height > 0.0);

    let font_output = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &font_relative, &paint),
        )
        .expect("font-size-relative line height prepares");
    assert_eq!(
        font_output
            .scene()
            .line(0)
            .expect("line exists")
            .bounds()
            .height(),
        40.0
    );
    assert_eq!(font_output.work().analysis().paragraphs(), 0);
    assert_eq!(font_output.work().font_selection().paragraphs(), 0);
    assert_eq!(font_output.work().shape().paragraphs(), 0);
    assert_eq!(font_output.work().line_shape().paragraphs(), 0);

    let absolute_output = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &absolute, &paint),
        )
        .expect("absolute line height prepares");
    assert_eq!(
        absolute_output
            .scene()
            .line(0)
            .expect("line exists")
            .bounds()
            .height(),
        50.0
    );
    assert_eq!(absolute_output.work().analysis().paragraphs(), 0);
    assert_eq!(absolute_output.work().font_selection().paragraphs(), 0);
    assert_eq!(absolute_output.work().shape().paragraphs(), 0);
    assert_eq!(absolute_output.work().line_shape().paragraphs(), 0);
}

#[test]
fn spacing_reuses_fonts_and_keeps_joining_text_connected() {
    let (document, plain_styles, paint) = fixture_document("office word", 1.2);
    let plain_style = plain_styles.default_style().clone();
    let tracked =
        StyleMap::new(plain_style.clone().with_inline_flow(
            InlineFlowStyle::default().with_spacing(
                TextSpacing::new(2.0, 3.0).expect("tracked spacing values are valid"),
            ),
        ));
    let wider = StyleMap::new(
        plain_style.with_inline_flow(
            InlineFlowStyle::default()
                .with_spacing(TextSpacing::new(4.0, 3.0).expect("wider spacing values are valid")),
        ),
    );
    let mut engine = fixture_engine();
    let constraint = TextConstraint::MaxContent;
    let plain = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &plain_styles, &paint),
        )
        .expect("plain shaping prepares");
    let plain_width = plain.scene().line(0).expect("line exists").bounds().width();
    let plain_glyphs: usize = plain
        .scene()
        .fragments()
        .iter()
        .map(|fragment| fragment.glyphs().len())
        .sum();

    let tracked_output = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &tracked, &paint),
        )
        .expect("tracked shaping prepares");
    let tracked_glyphs: usize = tracked_output
        .scene()
        .fragments()
        .iter()
        .map(|fragment| fragment.glyphs().len())
        .sum();
    assert_eq!(tracked_output.work().analysis().paragraphs(), 0);
    assert_eq!(tracked_output.work().font_selection().paragraphs(), 0);
    assert_eq!(tracked_output.work().shape().paragraphs(), 1);
    assert!(
        tracked_output
            .scene()
            .line(0)
            .expect("line exists")
            .bounds()
            .width()
            > plain_width
    );
    assert!(
        tracked_glyphs > plain_glyphs,
        "default optional ligatures must be disabled when tracking is nonzero"
    );

    let wider_output = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &wider, &paint),
        )
        .expect("changing a nonzero spacing amount prepares");
    assert_eq!(wider_output.work().analysis().paragraphs(), 0);
    assert_eq!(wider_output.work().font_selection().paragraphs(), 0);
    assert_eq!(wider_output.work().shape().paragraphs(), 0);
    assert!(
        wider_output
            .scene()
            .line(0)
            .expect("line exists")
            .bounds()
            .width()
            > tracked_output
                .scene()
                .line(0)
                .expect("line exists")
                .bounds()
                .width(),
        "advance-only changes must reach geometry without reshaping"
    );

    let wider_words = StyleMap::new(
        wider.default_style().clone().with_inline_flow(
            InlineFlowStyle::default()
                .with_spacing(TextSpacing::new(4.0, 6.0).expect("word spacing values are valid")),
        ),
    );
    let wider_words_output = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &wider_words, &paint),
        )
        .expect("changing only word spacing prepares");
    assert_eq!(wider_words_output.work().analysis().paragraphs(), 0);
    assert_eq!(wider_words_output.work().font_selection().paragraphs(), 0);
    assert_eq!(wider_words_output.work().shape().paragraphs(), 0);
    assert!(
        wider_words_output
            .scene()
            .line(0)
            .expect("line exists")
            .bounds()
            .width()
            > wider_output
                .scene()
                .line(0)
                .expect("line exists")
                .bounds()
                .width(),
        "word spacing must adjust retained separator advances"
    );

    let (arabic_document, arabic_plain, _) = fixture_document("سلام", 1.2);
    let arabic_tracked = StyleMap::new(
        arabic_plain.default_style().clone().with_inline_flow(
            InlineFlowStyle::default()
                .with_spacing(TextSpacing::new(8.0, 0.0).expect("Arabic tracking value is valid")),
        ),
    );
    let mut arabic_engine = fixture_engine();
    let plain = arabic_engine
        .prepare(
            &arabic_document.snapshot(),
            &editable_scene_request(constraint, &arabic_plain, &paint),
        )
        .expect("plain Arabic prepares");
    let plain_glyphs: Vec<_> = plain
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| glyph.id())
        .collect();
    let tracked = arabic_engine
        .prepare(
            &arabic_document.snapshot(),
            &editable_scene_request(constraint, &arabic_tracked, &paint),
        )
        .expect("tracked Arabic prepares");
    let tracked_glyphs: Vec<_> = tracked
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| glyph.id())
        .collect();
    assert_eq!(tracked_glyphs, plain_glyphs);
    assert_eq!(
        tracked
            .scene()
            .line(0)
            .expect("line exists")
            .bounds()
            .width(),
        plain.scene().line(0).expect("line exists").bounds().width()
    );
}

#[test]
fn wrap_and_overflow_policy_reach_product_formation() {
    let text = "supercalifragilistic";
    let (document, normal_styles, paint) = fixture_document(text, 1.2);
    let base = normal_styles.default_style().clone();
    let anywhere =
        StyleMap::new(base.clone().with_inline_flow(
            InlineFlowStyle::default().with_overflow_wrap(OverflowWrap::Anywhere),
        ));
    let break_word =
        StyleMap::new(base.clone().with_inline_flow(
            InlineFlowStyle::default().with_overflow_wrap(OverflowWrap::BreakWord),
        ));
    let no_wrap = StyleMap::new(
        base.with_inline_flow(InlineFlowStyle::default().with_text_wrap_mode(TextWrapMode::NoWrap)),
    );
    let narrow =
        TextConstraint::Wrap(FiniteWidth::new(30.0).expect("fixture width is finite and positive"));
    let mut engine = fixture_engine();

    let normal = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(narrow, &normal_styles, &paint),
        )
        .expect("normal overflow prepares");
    assert_eq!(normal.scene().lines().len(), 1);

    let emergency = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(narrow, &anywhere, &paint),
        )
        .expect("anywhere overflow prepares");
    assert!(emergency.scene().lines().len() > 1);
    assert_eq!(emergency.work().analysis().paragraphs(), 0);
    assert_eq!(emergency.work().shape().paragraphs(), 0);

    let no_wrap_output = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(narrow, &no_wrap, &paint),
        )
        .expect("no-wrap prepares");
    assert_eq!(no_wrap_output.scene().lines().len(), 1);
    assert_eq!(no_wrap_output.work().analysis().paragraphs(), 0);
    assert_eq!(no_wrap_output.work().shape().paragraphs(), 0);

    let anywhere_min = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(TextConstraint::MinContent, &anywhere, &paint),
        )
        .expect("anywhere min-content prepares");
    let break_word_min = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(TextConstraint::MinContent, &break_word, &paint),
        )
        .expect("break-word min-content prepares");
    assert!(anywhere_min.scene().lines().len() > 1);
    assert_eq!(break_word_min.scene().lines().len(), 1);
}

#[test]
fn legal_zero_width_break_reshapes_an_arabic_join() {
    let text = "سل\u{200b}ام";
    let break_at = u32::try_from(text.find("ام").expect("break suffix is present"))
        .expect("fixture range fits");
    let (document, styles, paint) = fixture_document(text, 1.2);
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid")
    ])
    .expect("fixture catalog is valid")
    .with_fallbacks(Script::from_bytes(*b"Arab"), None, ["Noto Kufi Arabic"])
    .expect("Arabic fallback is valid");
    let mut engine = LayoutEngine::new(ParleyParagraphEngine::new(fonts), CacheBudget::new(32));
    let wide = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let unbroken = engine
        .prepare(&document.snapshot(), &wide)
        .expect("unbroken shaping succeeds");
    let unbroken_sources = scene_sources(unbroken.scene());
    let unbroken_glyphs: Vec<_> = unbroken
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| {
            (
                glyph.id(),
                unbroken_sources
                    .first_for_glyph(glyph)
                    .expect("glyph source exists")
                    .bytes(),
            )
        })
        .collect();

    let narrow = editable_scene_request(TextConstraint::MinContent, &styles, &paint);
    let output = engine
        .prepare(&document.snapshot(), &narrow)
        .expect("the legal break reshapes its bounded cursive context");
    let sources = scene_sources(output.scene());
    assert_eq!(output.work().analysis().paragraphs(), 0);
    assert_eq!(output.work().font_selection().paragraphs(), 0);
    assert_eq!(output.work().shape().paragraphs(), 0);
    assert_eq!(output.work().line_font_resolution().paragraphs(), 1);
    assert_eq!(output.work().line_shape().paragraphs(), 1);
    assert_eq!(output.work().line_reshapes(), 2);
    let broken_glyphs: Vec<_> = output
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| {
            (
                glyph.id(),
                sources
                    .first_for_glyph(glyph)
                    .expect("glyph source exists")
                    .bytes(),
            )
        })
        .collect();
    assert_ne!(
        broken_glyphs, unbroken_glyphs,
        "committing the break must change real Arabic glyph output"
    );
    assert_eq!(output.scene().lines().len(), 2);
    assert_eq!(
        sources
            .for_line(output.scene().line(0).expect("line exists"))
            .iter()
            .next()
            .expect("source exists")
            .bytes(),
        0..break_at
    );
    assert_eq!(
        sources
            .for_line(output.scene().line(1).expect("line exists"))
            .iter()
            .next()
            .expect("source exists")
            .bytes(),
        break_at..u32::try_from(text.len()).expect("fixture range fits")
    );
    assert!(output.scene().fragments().iter().all(|fragment| {
        fragment.glyphs().iter().all(|glyph| {
            let source = sources
                .first_for_glyph(glyph)
                .expect("glyph source exists")
                .bytes();
            source.end <= break_at || source.start >= break_at
        })
    }));
}

#[test]
fn reshape_overflow_backs_up_and_restores_the_rejected_seam() {
    let text = "س سل\u{200b}ام";
    let pos = text.find("ام").expect("unsafe suffix exists");
    let (analysis, canonical) = shape_arabic(text);
    let clusters =
        collect_logical_clusters(text, &canonical).expect("canonical clusters are valid");
    let unsafe_end = clusters
        .iter()
        .position(|cluster| cluster.source.start == pos)
        .expect("unsafe break cluster exists");
    let prior_safe = (1..unsafe_end)
        .rev()
        .find(|&index| {
            let cluster = &clusters[index];
            cluster.boundary == parley_engine::Boundary::Line && !cluster.ligature_component
        })
        .expect("fixture has an earlier legal break");
    let unbroken_advance: f64 = clusters[..unsafe_end]
        .iter()
        .map(|cluster| cluster.advance)
        .sum();
    let formed = shape_arabic_range(text, &analysis, 0..pos);
    let broken_clusters =
        collect_logical_clusters(text, &formed).expect("broken clusters are valid");
    let broken_advance: f64 = broken_clusters.iter().map(|cluster| cluster.advance).sum();
    assert!(
        broken_advance > unbroken_advance,
        "the fixture must make break shaping change fit"
    );
    let width = (unbroken_advance + broken_advance) * 0.5;
    let constraint = TextConstraint::Wrap(
        FiniteWidth::new(width).expect("derived test width is finite and positive"),
    );
    let initial = choose_line(&clusters, 0, constraint).expect("initial selection succeeds");
    assert_eq!(initial.reason, TestLineBreakReason::Regular);
    assert_eq!(initial.end, unsafe_end, "clusters: {clusters:#?}");
    let mut document = Document::new(DocumentId::from_bytes(*b"reshape-overflow"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, text)
        .expect("fixture text is valid");
    edit.commit().expect("fixture document is valid");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Noto Kufi Arabic"), 20.0)
            .expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let output = fixture_engine()
        .prepare(
            &document.snapshot(),
            &editable_scene_request(constraint, &styles, &paint),
        )
        .expect("overflowing line-final shaping backs up");
    let sources = scene_sources(output.scene());
    let prior_source =
        u32::try_from(clusters[prior_safe].source.start).expect("fixture source range fits");
    assert_eq!(
        sources
            .for_line(output.scene().line(0).expect("line exists"))
            .iter()
            .next()
            .expect("source exists")
            .bytes()
            .end,
        prior_source
    );
    assert!(
        output.work().line_reshapes() >= 3,
        "rejected candidate, accepted candidate, and remainder must be visible work"
    );
    assert_eq!(
        output.work().rejected_line_candidates(),
        1,
        "the failed line-final fit must be observable independently of shaping"
    );
    assert_eq!(
        output.work().line_candidates(),
        output.work().accepted_line_candidates() + 1,
        "the retry is the only rejected candidate"
    );
}

#[test]
fn mixed_bidi_glyphs_are_visual_inside_a_logical_line() {
    let (document, styles, paint) = fixture_document("office مرحبا world", 1.2);
    let mut engine = fixture_engine();
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mixed bidi formation succeeds");
    let sources = scene_sources(output.scene());
    let arabic: Vec<_> = output
        .scene()
        .fragments()
        .iter()
        .filter(|fragment| fragment.bidi_level() & 1 == 1)
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| {
            (
                glyph.position().x,
                sources
                    .first_for_glyph(glyph)
                    .expect("glyph source exists")
                    .bytes()
                    .start,
            )
        })
        .collect();
    assert!(arabic.len() > 1, "Arabic run must expose multiple glyphs");
    assert!(
        arabic.windows(2).all(|pair| pair[0].1 >= pair[1].1)
            && arabic.windows(2).any(|pair| pair[0].1 > pair[1].1),
        "RTL glyph records run in visual order opposite logical source: {arabic:?}"
    );
}
