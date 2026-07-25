// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use underwood::{FlowRegion, RegionFlow, ResolvedDirection, TextAlignment};

#[test]
fn auto_rtl_start_and_end_consume_the_analyzed_paragraph_direction() {
    let text = "مرحبا بالعالم";
    let (document, paragraph, mut styles, paint) = alignment_fixture(text);
    let flow = RegionFlow::new([
        FlowRegion::new(Rect::new(40.0, 20.0, 340.0, 120.0)).expect("region is valid")
    ])
    .expect("flow is valid");
    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Start),
    );
    let mut engine = fixture_engine();
    let start = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(300.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("logical start alignment prepares");
    let start_line = &start.scene().lines()[0];
    assert_eq!(start_line.adjustment().direction(), ResolvedDirection::Rtl);
    assert_eq!(start_line.adjustment().alignment(), TextAlignment::Start);
    assert!((start_line.bounds().x1 - 340.0).abs() <= 1.0e-6);

    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::End),
    );
    let end = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(300.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("logical end alignment prepares");
    let end_line = &end.scene().lines()[0];
    assert_eq!(end_line.adjustment().direction(), ResolvedDirection::Rtl);
    assert!((end_line.bounds().x0 - 40.0).abs() <= 1.0e-6);
    assert_eq!(end.work().analysis().paragraphs(), 0);
    assert_eq!(end.work().itemization().paragraphs(), 0);
    assert_eq!(end.work().font_selection().paragraphs(), 0);
    assert_eq!(end.work().shape().paragraphs(), 0);
    assert_eq!(end.work().line_shape().paragraphs(), 0);
    assert_eq!(end.work().flow().paragraphs(), 0);
    assert_eq!(end.work().adjustment().paragraphs(), 1);
    assert_eq!(end.work().geometry().paragraphs(), 1);
}

#[test]
fn physical_left_and_right_ignore_rtl_logical_edges() {
    let text = "مرحبا بالعالم";
    let (document, paragraph, mut styles, paint) = alignment_fixture(text);
    let flow = RegionFlow::rectangle(Rect::new(40.0, 20.0, 340.0, 120.0)).expect("region is valid");
    let mut engine = fixture_engine();

    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Left),
    );
    let left = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(300.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("physical left alignment prepares");
    assert_eq!(
        left.scene().lines()[0].adjustment().direction(),
        ResolvedDirection::Rtl
    );
    assert!((left.scene().lines()[0].bounds().x0 - 40.0).abs() <= 1.0e-6);

    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Right),
    );
    let right = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(300.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("physical right alignment prepares");
    assert!((right.scene().lines()[0].bounds().x1 - 340.0).abs() <= 1.0e-6);
    assert_eq!(right.work().analysis().paragraphs(), 0);
    assert_eq!(right.work().shape().paragraphs(), 0);
    assert_eq!(right.work().flow().paragraphs(), 0);
}

#[test]
fn empty_explicit_rtl_paragraph_keeps_its_caret_on_logical_start() {
    let (document, paragraph, mut styles, paint) = alignment_fixture("");
    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Rtl).with_alignment(TextAlignment::Start),
    );
    let flow = RegionFlow::rectangle(Rect::new(80.0, 20.0, 280.0, 100.0)).expect("region is valid");
    let output = fixture_engine()
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(200.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("empty RTL paragraph prepares");
    assert!(output.scene().lines().is_empty());
    let hit = output
        .scene()
        .hit_test_closest(Point::new(280.0, 30.0))
        .expect("empty RTL paragraph retains a represented position");
    assert_eq!(
        output
            .scene()
            .caret(hit.position())
            .expect("empty RTL caret resolves")
            .bounds()
            .x0,
        280.0
    );
}

#[test]
fn composition_projection_consumes_the_same_alignment_geometry() {
    let (document, paragraph, mut styles, paint) = alignment_fixture("office");
    let snapshot = document.snapshot();
    let width = FiniteWidth::new(400.0).expect("width is valid");
    let mut engine = fixture_engine();
    let committed = engine
        .prepare(
            &snapshot,
            &SceneRequest::new(TextConstraint::Wrap(width), &styles, &paint),
        )
        .expect("committed scene prepares");
    let line = &committed.scene().lines()[0];
    let end = *committed
        .scene()
        .hit_test_closest(Point::new(line.bounds().x1, line.bounds().center().y))
        .expect("line end resolves")
        .position();
    let selections = committed
        .scene()
        .selection_set([committed
            .scene()
            .collapsed_selection(&end)
            .expect("composition insertion point is valid")])
        .expect("composition selection set is valid");
    let mut session = committed
        .scene()
        .begin_composition(&selections, CompositionId::from_bytes(*b"aligned-compose1"))
        .expect("composition starts")
        .into_session();
    session
        .update(
            session.epoch(),
            CompositionUpdate::new(" مرحبا").with_selection(11..11),
        )
        .expect("mixed-script preedit updates");

    let start = engine
        .prepare_composition(
            &snapshot,
            &SceneRequest::new(TextConstraint::Wrap(width), &styles, &paint),
            &session,
        )
        .expect("start-aligned composition prepares");
    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Center),
    );
    let centered = engine
        .prepare_composition(
            &snapshot,
            &SceneRequest::new(TextConstraint::Wrap(width), &styles, &paint),
            &session,
        )
        .expect("centered composition prepares");
    let delta = centered.scene().lines()[0].adjustment().inline_offset();
    assert!(delta > 0.0);
    assert_eq!(centered.work().analysis().paragraphs(), 0);
    assert_eq!(centered.work().font_selection().paragraphs(), 0);
    assert_eq!(centered.work().shape().paragraphs(), 0);
    assert_eq!(centered.work().flow().paragraphs(), 0);
    assert_eq!(centered.work().adjustment().paragraphs(), 1);

    for (plain, shifted) in start
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .zip(
            centered
                .scene()
                .fragments()
                .iter()
                .flat_map(|fragment| fragment.glyphs()),
        )
    {
        assert_eq!(shifted.position().x - plain.position().x, delta);
        assert_eq!(shifted.position().y, plain.position().y);
    }
    let plain_marked = start
        .scene()
        .composition_geometry(&session)
        .expect("start-aligned marked geometry resolves");
    let shifted_marked = centered
        .scene()
        .composition_geometry(&session)
        .expect("centered marked geometry resolves");
    assert_eq!(plain_marked.len(), shifted_marked.len());
    for (plain, shifted) in plain_marked.iter().zip(&shifted_marked) {
        assert_eq!(shifted.bounds().x0 - plain.bounds().x0, delta);
        assert_eq!(shifted.bounds().x1 - plain.bounds().x1, delta);
    }
}

#[test]
fn center_moves_mixed_bidi_paint_hits_carets_selections_and_semantics_together() {
    let text = "abc مرحبا XYZ";
    let (document, paragraph, mut styles, paint) = alignment_fixture(text);
    let flow =
        RegionFlow::rectangle(Rect::new(100.0, 30.0, 500.0, 160.0)).expect("region is valid");
    let mut engine = fixture_engine();
    let start = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(400.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("start scene prepares");
    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Center),
    );
    let centered = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(400.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("centered scene prepares");
    let start_scene = start.scene();
    let centered_scene = centered.scene();
    let delta = centered_scene.lines()[0].adjustment().inline_offset();
    assert!(delta > 0.0);
    assert_eq!(
        centered_scene.lines()[0].bounds().x0 - start_scene.lines()[0].bounds().x0,
        delta
    );
    for (plain, shifted) in start_scene
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .zip(
            centered_scene
                .fragments()
                .iter()
                .flat_map(|fragment| fragment.glyphs()),
        )
    {
        assert_eq!(shifted.position().x - plain.position().x, delta);
        assert_eq!(shifted.position().y, plain.position().y);
    }

    let start_hits = scan_line_hits(start_scene, 0);
    let centered_hits = scan_line_hits(centered_scene, 0);
    assert_eq!(
        start_hits
            .iter()
            .map(|hit| hit.source.clone())
            .collect::<Vec<_>>(),
        centered_hits
            .iter()
            .map(|hit| hit.source.clone())
            .collect::<Vec<_>>()
    );
    for (plain, shifted) in start_hits.iter().zip(&centered_hits) {
        assert!((shifted.min_x - plain.min_x - delta).abs() <= 0.06);
        assert!((shifted.max_x - plain.max_x - delta).abs() <= 0.06);
    }

    let y = start_scene.lines()[0].bounds().center().y;
    let anchor = *start_scene
        .hit_test_closest(Point::new(start_scene.lines()[0].bounds().x0, y))
        .expect("line start resolves")
        .position();
    let extent = *start_scene
        .hit_test_closest(Point::new(start_scene.lines()[0].bounds().x1, y))
        .expect("line end resolves")
        .position();
    assert_eq!(
        centered_scene
            .caret(&anchor)
            .expect("centered anchor caret exists")
            .bounds()
            .x0
            - start_scene
                .caret(&anchor)
                .expect("start anchor caret exists")
                .bounds()
                .x0,
        delta
    );
    let selection = start_scene
        .selection(&anchor, &extent, TextSelectionMode::Visual)
        .expect("mixed-bidi visual selection is valid");
    let plain_selection = start_scene
        .selection_geometry(
            &start_scene
                .selection_set([selection.clone()])
                .expect("plain selection set is valid"),
        )
        .expect("plain selection geometry resolves");
    let shifted_selection = centered_scene
        .selection_geometry(
            &centered_scene
                .selection_set([selection])
                .expect("centered selection set is valid"),
        )
        .expect("centered selection geometry resolves");
    for (plain, shifted) in plain_selection.iter().zip(&shifted_selection) {
        assert_eq!(shifted.bidi_level(), plain.bidi_level());
        assert_eq!(shifted.bounds().x0 - plain.bounds().x0, delta);
        assert_eq!(shifted.bounds().x1 - plain.bounds().x1, delta);
    }
    for (plain, shifted) in start_scene.semantics().zip(centered_scene.semantics()) {
        assert_eq!(shifted.bounds().x0 - plain.bounds().x0, delta);
        assert_eq!(shifted.bounds().x1 - plain.bounds().x1, delta);
    }
}

#[test]
fn western_justification_expands_only_eligible_soft_wrapped_lines() {
    let text = "one two three four five six";
    let (document, paragraph, mut styles, paint) = alignment_fixture(text);
    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Justify),
    );
    let flow = RegionFlow::rectangle(Rect::new(20.0, 10.0, 150.0, 220.0)).expect("region is valid");
    let mut engine = fixture_engine();
    let justified = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(130.0).expect("width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("justified scene prepares");
    let lines = justified.scene().lines();
    assert!(lines.len() >= 2);
    assert_eq!(lines[0].break_reason(), TestLineBreakReason::Regular);
    assert!(lines[0].adjustment().opportunity_expansion() > 0.0);
    assert!(lines[0].adjustment().expanded_opportunities() > 0);
    assert!(
        (lines[0].bounds().x1 - lines[0].adjustment().trailing_whitespace_advance() - 150.0).abs()
            <= 1.0e-6
    );
    assert_eq!(
        lines.last().expect("final line exists").break_reason(),
        TestLineBreakReason::End
    );
    assert_eq!(
        lines
            .last()
            .expect("final line exists")
            .adjustment()
            .opportunity_expansion(),
        0.0
    );

    let (mandatory_document, mandatory_paragraph, mut mandatory_styles, mandatory_paint) =
        alignment_fixture("one two\nthree four");
    mandatory_styles.set_paragraph_style(
        mandatory_paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Justify),
    );
    let mandatory = fixture_engine()
        .prepare(
            &mandatory_document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(300.0).expect("width is valid")),
                &mandatory_styles,
                &mandatory_paint,
            ),
        )
        .expect("mandatory line prepares");
    assert_eq!(
        mandatory.scene().lines()[0].break_reason(),
        TestLineBreakReason::Mandatory
    );
    assert_eq!(
        mandatory.scene().lines()[0]
            .adjustment()
            .opportunity_expansion(),
        0.0
    );

    let (arabic_document, arabic_paragraph, mut arabic_styles, arabic_paint) =
        alignment_fixture("مرحبا بالعالم مرحبا بالعالم مرحبا بالعالم");
    arabic_styles.set_paragraph_style(
        arabic_paragraph,
        ParagraphStyle::new(BaseDirection::Auto).with_alignment(TextAlignment::Justify),
    );
    let arabic = fixture_engine()
        .prepare(
            &arabic_document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(FiniteWidth::new(120.0).expect("width is valid")),
                &arabic_styles,
                &arabic_paint,
            ),
        )
        .expect("Arabic paragraph prepares without borrowing Western justification");
    assert!(
        arabic.scene().lines().len() >= 2,
        "fixture must expose a soft-wrapped Arabic line"
    );
    assert_eq!(
        arabic.scene().lines()[0].break_reason(),
        TestLineBreakReason::Regular
    );
    assert_eq!(
        arabic.scene().lines()[0]
            .adjustment()
            .expanded_opportunities(),
        0,
        "Arabic expansion remains a separate strategy"
    );
}

fn alignment_fixture(text: &str) -> (Document, underwood::ParagraphId, StyleMap, PaintTable) {
    let mut document = Document::new(DocumentId::from_bytes(*b"alignment-test01"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, text)
        .expect("fixture text is valid");
    edit.commit().expect("fixture edit is valid");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::new(
            LineHeight::from_multiplier(1.2).expect("fixture line height is valid"),
        ),
        PaintSlot::new(0),
    );
    (
        document,
        paragraph,
        StyleMap::new(style),
        PaintTable::from_brushes([Brush::Solid(Color::BLACK)]),
    )
}
