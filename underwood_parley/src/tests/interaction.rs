// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn scene_movement_crosses_semantic_paragraph_boundaries() {
    let mut document = Document::new(DocumentId::from_bytes(*b"paragraph-move01"));
    let mut edit = document.edit();
    let first_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("first paragraph is valid");
    let first = edit
        .append_text(first_paragraph, InlineRole::TEXT, "one")
        .expect("first text is valid");
    let second_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("second paragraph is valid");
    let second = edit
        .append_text(second_paragraph, InlineRole::TEXT, "two")
        .expect("second text is valid");
    edit.commit().expect("fixture edit is valid");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = fixture_engine()
        .prepare(&document.snapshot(), &request)
        .expect("multi-paragraph interaction must prepare");
    let scene = output.scene();
    let end = *scene
        .hit_test_closest(Point::new(10_000.0, scene.lines()[0].bounds().center().y))
        .expect("first paragraph end must resolve")
        .position();
    assert_eq!(end.text(), first);
    let carets = scene
        .selection_set([scene
            .collapsed_selection(&end)
            .expect("first paragraph caret is valid")])
        .expect("one caret forms a set");
    for movement in [TextMovement::NextVisual, TextMovement::NextLogical] {
        let moved = scene
            .move_selections(&carets, movement, false)
            .expect("movement must compose across paragraph boundaries");
        assert_eq!(
            moved.primary().expect("primary survives").extent().text(),
            second
        );
    }
}

#[test]
fn exact_interaction_uses_ligature_components_not_glyph_ink() {
    let (document, styles, paint) = fixture_document("office", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("ligature interaction must prepare");
    let scene = output.scene();
    assert!(
        scene.fragments().len() < 6,
        "the fixture must contain a substituted multi-source glyph"
    );

    let hits = scan_line_hits(scene, 0);
    let sources: Vec<_> = hits.iter().map(|hit| hit.source.clone()).collect();
    assert_eq!(
        sources,
        vec![0..1, 1..2, 2..3, 3..4, 4..5, 5..6],
        "each ligature component must retain its own hit interval: {hits:?}"
    );

    let y = scene.lines()[0].bounds().center().y;
    let first = scene
        .hit_test(Point::new(0.1, y))
        .expect("the first cluster must be hittable");
    let second = scene
        .hit_test(Point::new(0.5, y))
        .expect("a second point in the same cluster must be hittable");
    assert_eq!(first.position(), second.position());
    assert_eq!(
        scene
            .caret(first.position())
            .expect("first hit caret must resolve")
            .bounds(),
        scene
            .caret(second.position())
            .expect("second hit caret must resolve")
            .bounds(),
        "caret geometry must come from the prepared stop, not the query x coordinate"
    );
}

#[test]
fn interaction_map_groups_combining_source_and_keeps_whitespace() {
    for (text, expected) in [
        ("e\u{301}", core::iter::once(0..3).collect::<Vec<_>>()),
        ("a b", vec![0..1, 1..2, 2..3]),
    ] {
        let (document, styles, paint) = fixture_document(text, 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("cluster interaction must prepare");
        let hits = scan_line_hits(output.scene(), 0);
        assert_eq!(
            hits.iter()
                .map(|hit| hit.source.clone())
                .collect::<Vec<_>>(),
            expected,
            "source-complete graphemes and whitespace must remain hittable for {text:?}: {hits:?}"
        );
    }
}

#[test]
fn collapsed_whitespace_crosses_semantic_leaves_with_complete_source_and_first_owner() {
    let mut document = Document::new(DocumentId::from_bytes(*b"collapse-leaves1"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    let first = edit
        .append_text(paragraph, InlineRole::TEXT, "a ")
        .expect("first leaf is valid");
    let second = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "\t\r\n b")
        .expect("second leaf is valid");
    edit.commit().expect("fixture edit is valid");

    let first_style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let mut styles = StyleMap::new(first_style.clone()).with_default_paragraph_style(
        ParagraphStyle::DEFAULT.with_whitespace_collapse(WhitespaceCollapse::Collapse),
    );
    styles.set(first, first_style.clone());
    styles.set(second, first_style.with_paint(PaintSlot::new(1)));
    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::BLACK),
        Brush::Solid(Color::from_rgb8(0xff, 0x00, 0x00)),
    ]);
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint);
    let output = fixture_engine()
        .prepare(&document.snapshot(), &request)
        .expect("cross-leaf whitespace collapse must prepare");
    let scene = output.scene();
    let line = &scene.lines()[0];
    let y = line.bounds().center().y;

    let mut collapsed_hits = Vec::new();
    let mut x = line.bounds().x0;
    while x <= line.bounds().x1 {
        if let Some(hit) = scene.hit_test(Point::new(x, y))
            && hit.source().sources().len() == 2
        {
            if collapsed_hits
                .last()
                .is_none_or(|position| position != hit.position())
            {
                collapsed_hits.push(*hit.position());
            }
            assert_eq!(hit.source().sources()[0].text(), first);
            assert_eq!(hit.source().sources()[0].bytes(), 1..2);
            assert_eq!(hit.source().sources()[1].text(), second);
            assert_eq!(hit.source().sources()[1].bytes(), 0..4);
        }
        x += 0.05;
    }
    assert_eq!(
        collapsed_hits.len(),
        2,
        "one collapsed presentation space must expose its two authored sides"
    );

    let selection = scene
        .selection(
            &collapsed_hits[0],
            &collapsed_hits[1],
            TextSelectionMode::Logical,
        )
        .expect("the collapsed unit must be selectable");
    assert_eq!(selection.ranges().len(), 2);
    assert_eq!(selection.ranges()[0].text(), first);
    assert_eq!(selection.ranges()[0].bytes(), 1..2);
    assert_eq!(selection.ranges()[1].text(), second);
    assert_eq!(selection.ranges()[1].bytes(), 0..4);

    let collapsed_fragment = scene
        .fragments()
        .iter()
        .find(|fragment| fragment.sources().count() == 2)
        .expect("the collapsed space must retain both source leaves");
    assert_eq!(
        collapsed_fragment.paint(),
        PaintSlot::new(0),
        "the first authored contributor owns transformed style and paint"
    );
}

#[test]
fn split_leaf_grapheme_is_one_hit_movement_and_atomic_replacement_unit() {
    let mut document = Document::new(DocumentId::from_bytes(*b"split-grapheme-1"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    let base = edit
        .append_text(paragraph, InlineRole::TEXT, "e")
        .expect("base leaf is valid");
    let mark = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "\u{301}")
        .expect("mark leaf is valid");
    edit.commit().expect("fixture edit is valid");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let mut engine = fixture_engine();
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("a grapheme crossing semantic leaves must still prepare");

    let semantic_texts: Vec<_> = output
        .scene()
        .semantics()
        .filter_map(|semantic| semantic.source().map(|source| source.text()))
        .collect();
    assert!(semantic_texts.contains(&base));
    assert!(semantic_texts.contains(&mark));
    assert!(output.scene().fragments().iter().any(|fragment| {
        let texts: Vec<_> = fragment.sources().map(|source| source.text()).collect();
        texts.contains(&base) && texts.contains(&mark)
    }));
    let scene = output.scene();
    let y = scene.lines()[0].bounds().center().y;
    let hit = scene
        .hit_test(Point::new(scene.lines()[0].bounds().x0, y))
        .expect("the source-complete grapheme must be hittable");
    assert_eq!(hit.source().sources().len(), 2);
    assert_eq!(hit.source().sources()[0].text(), base);
    assert_eq!(hit.source().sources()[0].bytes(), 0..1);
    assert_eq!(hit.source().sources()[1].text(), mark);
    assert_eq!(hit.source().sources()[1].bytes(), 0..2);
    let base_semantic = scene
        .semantics()
        .find(|semantic| {
            semantic
                .source()
                .is_some_and(|source| source.text() == base)
        })
        .expect("base semantics must survive")
        .semantic_id();
    assert_eq!(
        hit.semantic_id(),
        base_semantic,
        "a zero-advance mark has no fabricated pointer interior"
    );

    let end = *scene
        .hit_test_closest(Point::new(10_000.0, y))
        .expect("the trailing grapheme side must resolve")
        .position();
    let carets = scene
        .selection_set([scene
            .collapsed_selection(&end)
            .expect("the trailing position is valid")])
        .expect("one caret forms a selection set");
    let deletion = scene
        .move_selections(&carets, TextMovement::PreviousLogical, true)
        .expect("backspace must cross the complete grapheme");
    let ranges = deletion
        .primary()
        .expect("the primary selection survives")
        .ranges();
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].text(), base);
    assert_eq!(ranges[0].bytes(), 0..1);
    assert_eq!(ranges[1].text(), mark);
    assert_eq!(ranges[1].bytes(), 0..2);
    let geometry = scene
        .selection_geometry(&deletion)
        .expect("source-complete selection geometry must resolve");
    assert_eq!(
        geometry.len(),
        1,
        "one grapheme crossing two leaves must paint one selection rectangle"
    );

    let replacement = document
        .replace_selections(&deletion, "")
        .expect("one multi-leaf grapheme must publish atomically");
    assert_eq!(replacement.publication().snapshot().text(base), Some(""));
    assert_eq!(replacement.publication().snapshot().text(mark), Some(""));
    assert_eq!(
        replacement.publication().changes().paragraphs(),
        [paragraph]
    );
}

#[test]
fn rtl_visual_hits_retain_reversed_logical_sides() {
    let (document, styles, paint) = fixture_document("مرحبا", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("RTL interaction must prepare");
    let hits = scan_line_hits(output.scene(), 0);
    assert!(
        hits.len() >= 5,
        "Arabic source must expose real clusters: {hits:?}"
    );
    assert!(
        hits.windows(2)
            .all(|pair| pair[0].source.start > pair[1].source.start),
        "visual left-to-right traversal must retain descending RTL source: {hits:?}"
    );
    assert!(
        hits.iter().all(|hit| {
            hit.position == hit.source.end && hit.affinity == TextAffinity::Upstream
        }),
        "the visual left side of every RTL cluster must resolve to its logical end: {hits:?}"
    );
}

#[test]
fn soft_wrap_exposes_both_affinities_for_one_logical_boundary() {
    let (document, styles, paint) = fixture_document("alpha beta gamma", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(72.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("wrapped interaction must prepare");
    let first = scan_line_hits(output.scene(), 0);
    let second = scan_line_hits(output.scene(), 1);
    let at_end = first.last().expect("first line has a final cluster");
    let at_start = second.first().expect("second line has an initial cluster");
    let end_hit = output
        .scene()
        .hit_test(Point::new(
            at_end.max_x,
            output.scene().lines()[0].bounds().center().y,
        ))
        .expect("line-end cluster must be hittable");
    let start_hit = output
        .scene()
        .hit_test(Point::new(
            at_start.min_x,
            output.scene().lines()[1].bounds().center().y,
        ))
        .expect("next-line cluster must be hittable");
    assert_eq!(end_hit.position().byte(), start_hit.position().byte());
    assert_eq!(end_hit.position().affinity(), TextAffinity::Upstream);
    assert_eq!(start_hit.position().affinity(), TextAffinity::Downstream);
    assert_ne!(
        output
            .scene()
            .caret(end_hit.position())
            .expect("upstream caret must resolve")
            .bounds()
            .y0,
        output
            .scene()
            .caret(start_hit.position())
            .expect("downstream caret must resolve")
            .bounds()
            .y0,
        "affinity must select the correct side of the soft wrap"
    );
}

#[test]
fn empty_editable_leaf_has_a_closest_hit_and_exact_caret() {
    let (document, styles, paint) = fixture_document("", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("empty editable text must prepare");
    let hit = output
        .scene()
        .hit_test_closest(Point::new(200.0, 80.0))
        .expect("empty semantic text must expose a clamped position");
    assert_eq!(sole_unit_source(hit.source()).bytes(), 0..0);
    assert_eq!(hit.position().byte(), 0);
    assert_eq!(hit.position().affinity(), TextAffinity::Downstream);
    let caret = output
        .scene()
        .caret(hit.position())
        .expect("empty position must have caret geometry");
    assert_eq!(caret.bounds().x0, 0.0);
    assert!(caret.bounds().height() > 0.0);
}

#[test]
fn structurally_leafless_paragraph_is_not_editable() {
    let mut document = Document::new(DocumentId::from_bytes(*b"leafless-hit-001"));
    let mut edit = document.edit();
    edit.append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    edit.commit().expect("fixture edit is valid");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = fixture_engine()
        .prepare(&document.snapshot(), &request)
        .expect("a leafless paragraph must still prepare");
    assert!(
        output
            .scene()
            .hit_test_closest(Point::new(0.0, 0.0))
            .is_none(),
        "structure without a semantic text leaf must not manufacture an editable position"
    );
}

#[test]
fn semantic_leaf_boundary_ownership_follows_affinity() {
    let mut document = Document::new(DocumentId::from_bytes(*b"leaf-boundary-01"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    let first_text = edit
        .append_text(paragraph, InlineRole::TEXT, "ab")
        .expect("first leaf is valid");
    let second_text = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "cd")
        .expect("second leaf is valid");
    edit.commit().expect("fixture edit is valid");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let mut engine = fixture_engine();
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("multi-leaf interaction must prepare");
    let scene = output.scene();
    let y = scene.lines()[0].bounds().center().y;
    let mut first_right = None;
    let mut second_left = None;
    let mut x = scene.lines()[0].bounds().x0;
    while x <= scene.lines()[0].bounds().x1 {
        if let Some(hit) = scene.hit_test(Point::new(x, y)) {
            let source = sole_unit_source(hit.source());
            if source.text() == first_text {
                first_right = Some((x, hit.semantic_id()));
            } else if source.text() == second_text && second_left.is_none() {
                second_left = Some((x, hit.semantic_id()));
            }
        }
        x += 0.05;
    }
    let (first_x, first_semantic) = first_right.expect("first leaf must be hittable");
    let (second_x, second_semantic) = second_left.expect("second leaf must be hittable");
    let first_hit = scene
        .hit_test(Point::new(first_x, y))
        .expect("first leaf trailing side must resolve");
    let second_hit = scene
        .hit_test(Point::new(second_x, y))
        .expect("second leaf leading side must resolve");
    assert_eq!(first_hit.position().text(), first_text);
    assert_eq!(first_hit.position().byte(), 2);
    assert_eq!(first_hit.position().affinity(), TextAffinity::Upstream);
    assert_eq!(second_hit.position().text(), second_text);
    assert_eq!(second_hit.position().byte(), 0);
    assert_eq!(second_hit.position().affinity(), TextAffinity::Downstream);
    assert_ne!(first_semantic, second_semantic);
    assert_eq!(first_hit.semantic_id(), first_semantic);
    assert_eq!(second_hit.semantic_id(), second_semantic);
}

#[test]
fn caret_rejects_a_position_from_another_revision() {
    let (mut document, styles, paint) = fixture_document("abc", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let old_output = engine
        .prepare(&document.snapshot(), &request)
        .expect("old interaction must prepare");
    let old_hit = old_output
        .scene()
        .hit_test(Point::new(
            0.0,
            old_output.scene().lines()[0].bounds().center().y,
        ))
        .expect("old scene must be hittable");
    let old_position = *old_hit.position();
    let mut edit = document.edit();
    edit.replace_text(old_position.text(), "abcd")
        .expect("replacement is valid");
    edit.commit().expect("replacement must publish");
    let new_output = engine
        .prepare(&document.snapshot(), &request)
        .expect("new interaction must prepare");
    assert!(
        new_output.scene().caret(&old_position).is_none(),
        "a snapshot position must not silently migrate to a newer revision"
    );
}

#[test]
fn closest_hit_selects_the_nearest_line_before_its_inline_edge() {
    let text = "a\nsupercalifragilisticexpialidocious";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("explicitly broken interaction must prepare");
    assert_eq!(output.scene().lines().len(), 2);
    let hit = output
        .scene()
        .hit_test_closest(Point::new(
            10_000.0,
            output.scene().lines()[0].bounds().center().y,
        ))
        .expect("first line must clamp despite a much wider later line");
    assert!(
        sole_unit_source(hit.source()).bytes().end <= 2,
        "block-axis selection must happen before inline clamping: {hit:?}"
    );
}

#[test]
fn mandatory_break_keeps_before_and_after_carets_on_distinct_lines() {
    let (document, styles, paint) = fixture_document("a\n", 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mandatory-break interaction must prepare");
    assert_eq!(output.scene().lines().len(), 2);
    let before = output
        .scene()
        .hit_test_closest(Point::new(
            10_000.0,
            output.scene().lines()[0].bounds().center().y,
        ))
        .expect("the broken line must clamp before the control");
    let after = output
        .scene()
        .hit_test_closest(Point::new(
            10_000.0,
            output.scene().lines()[1].bounds().center().y,
        ))
        .expect("the final empty line must expose the post-break caret");
    assert_eq!(before.position().byte(), 1);
    assert_eq!(after.position().byte(), 2);
    assert_ne!(
        output
            .scene()
            .caret(before.position())
            .expect("pre-break caret must resolve")
            .bounds()
            .y0,
        output
            .scene()
            .caret(after.position())
            .expect("post-break caret must resolve")
            .bounds()
            .y0
    );
}
