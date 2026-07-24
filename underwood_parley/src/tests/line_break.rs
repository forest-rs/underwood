// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn product_path_wraps_only_at_parley_line_boundaries() {
    let text = "alpha beta gamma";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(72.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("legal wrapping must form a scene");
    let lines = output.scene().lines();
    assert_eq!(lines.len(), 3, "legal opportunities must form three lines");
    assert_eq!(
        lines[0].break_reason(),
        underwood::adapter::LineBreakReason::Regular
    );
    assert_eq!(
        lines[1].break_reason(),
        underwood::adapter::LineBreakReason::Regular
    );
    assert_eq!(
        lines[2].break_reason(),
        underwood::adapter::LineBreakReason::End
    );
    assert_eq!(lines[0].sources()[0].bytes(), 0..6);
    assert_eq!(
        lines[1].sources()[0].bytes().start,
        u32::try_from(text.find("beta").expect("beta is present")).expect("fixture range fits")
    );
    assert_eq!(
        lines[2].sources()[0].bytes().start,
        u32::try_from(text.find("gamma").expect("gamma is present")).expect("fixture range fits")
    );
}

#[test]
fn product_path_coalesces_crlf_and_honors_mandatory_breaks() {
    let text = "a\r\nb\nc\u{2028}d\u{2029}e";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mandatory breaks must form a scene");
    let lines = output.scene().lines();
    let ranges: Vec<_> = lines.iter().map(|line| line.sources()[0].bytes()).collect();
    assert_eq!(
        lines.len(),
        5,
        "CRLF, LF, LS, and PS form four breaks: {ranges:?}"
    );
    assert_eq!(lines[0].sources()[0].bytes(), 0..3, "CRLF stays together");
    assert!(
        lines[..4]
            .iter()
            .all(|line| line.break_reason() == underwood::adapter::LineBreakReason::Mandatory)
    );
    assert_eq!(
        lines[4].break_reason(),
        underwood::adapter::LineBreakReason::End
    );
    assert_eq!(
        lines.last().expect("final line exists").sources()[0]
            .bytes()
            .end,
        u32::try_from(text.len()).expect("fixture length fits")
    );
}

#[test]
fn product_path_uses_font_metrics_for_the_baseline() {
    let (document, styles, paint) = fixture_document("Ag", 1.5);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("metric-backed formation must succeed");
    let line = &output.scene().lines()[0];
    assert_eq!(line.bounds().height(), 30.0);
    assert!(line.baseline() > line.bounds().y0 && line.baseline() < line.bounds().y1);
    assert_eq!(
        line.baseline(),
        output.scene().fragments()[0].glyphs()[0].position().y
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
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mixed leaf formation succeeds");
    let line = &output.scene().lines()[0];
    assert_eq!(line.sources().len(), 2);
    assert_eq!(line.sources()[0].text(), small);
    assert_eq!(line.sources()[0].bytes(), 0..6);
    assert_eq!(line.sources()[1].text(), large);
    assert_eq!(line.sources()[1].bytes(), 0..3);
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
        let request = SceneRequest::new(
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
            output.scene().lines()[0].break_reason(),
            underwood::adapter::LineBreakReason::End
        );
        assert!(
            output.scene().lines()[0].bounds().width() > 10.0,
            "overflow must remain visible rather than report a false fit: {text:?}"
        );
    }
}

#[test]
fn width_and_line_height_reform_without_reshaping() {
    let text = "alpha beta gamma";
    let (document, compact_styles, paint) = fixture_document(text, 1.2);
    let (_, spacious_styles, _) = fixture_document(text, 1.8);
    let mut engine = fixture_engine();
    let wide = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &compact_styles,
        &paint,
    );
    engine
        .prepare(&document.snapshot(), &wide)
        .expect("initial formation succeeds");

    let narrow = SceneRequest::new(
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
    assert_eq!(narrowed.work().flow().paragraphs(), 1);

    let spacious = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(72.0).expect("test width is valid")),
        &spacious_styles,
        &paint,
    );
    let respaced = engine
        .prepare(&document.snapshot(), &spacious)
        .expect("line-height-only formation succeeds");
    assert_eq!(respaced.work().analysis().paragraphs(), 0);
    assert_eq!(respaced.work().shape().paragraphs(), 0);
    assert_eq!(respaced.work().flow().paragraphs(), 1);
    assert!(
        respaced.scene().lines()[0].bounds().height()
            > narrowed.scene().lines()[0].bounds().height()
    );
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
    let wide = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let unbroken = engine
        .prepare(&document.snapshot(), &wide)
        .expect("unbroken shaping succeeds");
    let unbroken_glyphs: Vec<_> = unbroken
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| (glyph.id(), glyph.source().bytes()))
        .collect();

    let narrow = SceneRequest::new(TextConstraint::MinContent, &styles, &paint);
    let output = engine
        .prepare(&document.snapshot(), &narrow)
        .expect("the legal break reshapes its bounded cursive context");
    assert_eq!(output.work().analysis().paragraphs(), 0);
    assert_eq!(output.work().shape().paragraphs(), 0);
    assert_eq!(output.work().break_reshapes(), 1);
    let broken_glyphs: Vec<_> = output
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| (glyph.id(), glyph.source().bytes()))
        .collect();
    assert_ne!(
        broken_glyphs, unbroken_glyphs,
        "committing the break must change real Arabic glyph output"
    );
    assert_eq!(output.scene().lines().len(), 2);
    assert_eq!(output.scene().lines()[0].sources()[0].bytes(), 0..break_at);
    assert_eq!(
        output.scene().lines()[1].sources()[0].bytes(),
        break_at..u32::try_from(text.len()).expect("fixture range fits")
    );
    assert!(output.scene().fragments().iter().all(|fragment| {
        fragment.glyphs().iter().all(|glyph| {
            let source = glyph.source().bytes();
            source.end <= break_at || source.start >= break_at
        })
    }));
}

#[test]
fn reshape_overflow_backs_up_and_restores_the_rejected_seam() {
    let text = "س سل\u{200b}ام";
    let pos = text.find("ام").expect("unsafe suffix exists");
    let (analysis, mut shaper, canonical) = shape_arabic(text);
    let mut formed = canonical.clone();
    let mut clusters =
        collect_logical_clusters(text, &formed).expect("canonical clusters are valid");
    let unsafe_end = clusters
        .iter()
        .position(|cluster| cluster.source.start == pos)
        .expect("unsafe break cluster exists");
    let prior_safe = (1..unsafe_end)
        .rev()
        .find(|&index| {
            let cluster = &clusters[index];
            cluster.boundary == parley_core::Boundary::Line && !cluster.ligature_component
        })
        .expect("fixture has an earlier legal break");
    let unbroken_advance: f64 = clusters[..unsafe_end]
        .iter()
        .map(|cluster| cluster.advance)
        .sum();
    shaper.apply_break(text, &analysis, &mut formed, pos);
    let broken_clusters =
        collect_logical_clusters(text, &formed).expect("broken clusters are valid");
    let broken_advance: f64 = broken_clusters[..unsafe_end]
        .iter()
        .map(|cluster| cluster.advance)
        .sum();
    assert!(
        broken_advance > unbroken_advance,
        "the fixture must make break shaping change fit"
    );
    shaper.apply_concat(text, &analysis, &mut formed, pos);
    clusters = collect_logical_clusters(text, &formed).expect("restored clusters are valid");

    let width = (unbroken_advance + broken_advance) * 0.5;
    let constraint = TextConstraint::Wrap(
        FiniteWidth::new(width).expect("derived test width is finite and positive"),
    );
    let initial = choose_line(&clusters, 0, constraint).expect("initial selection succeeds");
    assert_eq!(initial.reason, TestLineBreakReason::Regular);
    assert_eq!(initial.end, unsafe_end, "clusters: {clusters:#?}");
    let (committed_end, committed_advance, committed_reshape) = commit_regular_break(
        &mut shaper,
        &analysis,
        text,
        &mut formed,
        &mut clusters,
        0,
        unsafe_end,
        constraint,
    )
    .expect("overflowing reshaped break backs up");
    assert_eq!(committed_end, prior_safe);
    assert!(committed_advance <= width);
    assert!(!committed_reshape, "the committed earlier seam is safe");
    assert_eq!(
        formed, canonical,
        "rejecting the unsafe seam must concat it before backing up"
    );
}

#[test]
fn mixed_bidi_glyphs_are_visual_inside_a_logical_line() {
    let (document, styles, paint) = fixture_document("office مرحبا world", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mixed bidi formation succeeds");
    let arabic: Vec<_> = output
        .scene()
        .fragments()
        .iter()
        .filter(|fragment| fragment.bidi_level() & 1 == 1)
        .map(|fragment| {
            let glyph = &fragment.glyphs()[0];
            (glyph.position().x, glyph.source().bytes().start)
        })
        .collect();
    assert!(arabic.len() > 1, "Arabic run must expose multiple glyphs");
    assert!(
        arabic.windows(2).all(|pair| pair[0].1 >= pair[1].1)
            && arabic.windows(2).any(|pair| pair[0].1 > pair[1].1),
        "RTL glyph records run in visual order opposite logical source: {arabic:?}"
    );
}
