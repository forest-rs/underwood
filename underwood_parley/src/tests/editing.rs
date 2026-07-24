// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn visual_bidi_selection_retains_disjoint_ranges_and_set_ownership() {
    let text = "abc مرحبا XYZ";
    let arabic_start =
        u32::try_from(text.find('م').expect("Arabic run exists")).expect("fixture offset fits");
    let arabic_end =
        u32::try_from(text.find(" X").expect("trailing run exists")).expect("fixture offset fits");
    let trailing_start = arabic_end + 1;
    let (mut document, styles, paint) = fixture_document(text, 1.2);
    let mut engine = fixture_engine();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("mixed-bidi interaction must prepare");
    let scene = output.scene();
    let hits = scan_line_hits(scene, 0);
    let arabic: Vec<_> = hits
        .iter()
        .filter(|hit| hit.source.start >= arabic_start && hit.source.end <= arabic_end)
        .collect();
    assert!(arabic.len() >= 4, "fixture needs several Arabic clusters");
    let anchor_hit = arabic[arabic.len() / 2];
    let extent_hit = hits
        .iter()
        .find(|hit| hit.source.start == trailing_start)
        .expect("trailing Latin cluster is visible");
    let prefix_hit = hits.first().expect("prefix cluster is visible");
    let y = scene.lines()[0].bounds().center().y;
    let anchor = *scene
        .hit_test(Point::new(anchor_hit.min_x + 0.01, y))
        .expect("Arabic anchor must hit")
        .position();
    let extent = *scene
        .hit_test(Point::new(extent_hit.max_x - 0.01, y))
        .expect("Latin extent must hit")
        .position();
    let visual = scene
        .selection(&anchor, &extent, TextSelectionMode::Visual)
        .expect("visual caret path must select");
    let reverse = scene
        .selection(&extent, &anchor, TextSelectionMode::Visual)
        .expect("the reverse visual caret path must select");
    assert_eq!(
        visual.ranges(),
        reverse.ranges(),
        "visual selection source is independent of drag direction"
    );
    assert_eq!(
        visual
            .ranges()
            .iter()
            .map(|range| range.bytes())
            .collect::<Vec<_>>(),
        [4..10, 14..16],
        "the pinned mixed-bidi fixture must preserve its exact logical gap"
    );
    assert!(
        visual.ranges().windows(2).any(|ranges| {
            ranges[0].text() == ranges[1].text() && ranges[0].bytes().end < ranges[1].bytes().start
        }),
        "a visually contiguous bidi gesture must retain its logical gap: {:?}",
        visual.ranges()
    );

    let prefix_start = *scene
        .hit_test(Point::new(prefix_hit.min_x + 0.01, y))
        .expect("prefix start must hit")
        .position();
    let prefix_end = *scene
        .hit_test(Point::new(prefix_hit.max_x - 0.01, y))
        .expect("prefix end must hit")
        .position();
    let prefix = scene
        .selection(&prefix_start, &prefix_end, TextSelectionMode::Logical)
        .expect("prefix selection must form");
    let selections = scene
        .selection_set([visual, prefix])
        .expect("nonoverlapping insertion points form one set");
    assert_eq!(selections.selections().len(), 2);
    let geometry = scene
        .selection_geometry(&selections)
        .expect("the complete selection set has geometry");
    assert!(
        geometry.iter().any(|rect| rect.selection() == 0)
            && geometry.iter().any(|rect| rect.selection() == 1),
        "geometry must preserve independent selection ownership: {geometry:?}"
    );
    assert!(
        geometry.iter().any(|rect| rect.range() > 0),
        "visual selection geometry must preserve disjoint-range ownership"
    );

    let carets = scene
        .selection_set([
            scene
                .collapsed_selection(&prefix_start)
                .expect("prefix caret is valid"),
            scene
                .collapsed_selection(&anchor)
                .expect("Arabic caret is valid"),
        ])
        .expect("two independent carets form one set");
    let duplicate = scene
        .collapsed_selection(&prefix_start)
        .expect("prefix caret is valid");
    assert_eq!(
        scene
            .selection_set([duplicate.clone(), duplicate])
            .expect_err("duplicate insertion points must fail as one set")
            .kind(),
        SelectionErrorKind::OverlappingSelections
    );
    let moved = scene
        .move_selections(&carets, TextMovement::NextVisual, false)
        .expect("the whole set moves through adapter transitions");
    assert_eq!(
        document.snapshot().revision(),
        selections.revision(),
        "selection and movement must not publish document work"
    );
    assert_eq!(moved.selections().len(), 2);
    assert!(
        moved
            .selections()
            .iter()
            .all(|selection| selection.is_collapsed())
    );
    assert_ne!(
        moved.selections()[0].extent(),
        moved.selections()[1].extent(),
        "independent carets must remain independent after movement"
    );

    let text_id = selections.selections()[0].ranges()[0].text();
    let replacement = document
        .replace_selections(&selections, "§")
        .expect("the complete set must publish atomically");
    assert_eq!(replacement.publication().changes().paragraphs().len(), 1);
    assert_eq!(replacement.selections().selections().len(), 2);
    assert!(
        replacement
            .selections()
            .selections()
            .iter()
            .all(|selection| selection.is_collapsed()),
        "every input insertion point must receive one post-edit caret"
    );
    assert_eq!(
        replacement
            .publication()
            .snapshot()
            .text(text_id)
            .expect("edited leaf survives")
            .matches('§')
            .count(),
        2,
        "one multi-range visual selection plus one prefix selection inserts twice, not once per range"
    );
    let error = document
        .replace_selections(&selections, "stale")
        .expect_err("old scene selections must not migrate across publication");
    assert_eq!(error.kind(), EditErrorKind::RevisionConflict);
}

#[test]
fn logical_delete_and_backspace_remove_one_extended_grapheme() {
    for (source, movement, at_end, expected_range, expected_text) in [
        ("aé", TextMovement::PreviousLogical, true, 1..3, "a"),
        ("ae\u{301}", TextMovement::PreviousLogical, true, 1..4, "a"),
        ("éa", TextMovement::NextLogical, false, 0..2, "a"),
        ("a\r\n", TextMovement::PreviousLogical, true, 1..3, "a"),
        ("ب\u{64e}", TextMovement::NextLogical, true, 0..4, ""),
    ] {
        let (mut document, styles, paint) = fixture_document(source, 1.2);
        let mut engine = fixture_engine();
        let request = SceneRequest::new(
            TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
            &styles,
            &paint,
        );
        let output = engine
            .prepare(&document.snapshot(), &request)
            .expect("grapheme interaction must prepare");
        let scene = output.scene();
        let line = if at_end {
            scene.lines().last()
        } else {
            scene.lines().first()
        }
        .expect("the fixture must expose a line");
        let position = *scene
            .hit_test_closest(Point::new(
                if at_end { 10_000.0 } else { -10_000.0 },
                line.bounds().center().y,
            ))
            .expect("line edge must resolve")
            .position();
        let carets = scene
            .selection_set([scene
                .collapsed_selection(&position)
                .expect("line-edge caret is valid")])
            .expect("one caret forms a set");
        let deletion = scene
            .move_selections(&carets, movement, true)
            .expect("deletion range follows the adapter grapheme transition");
        assert_eq!(
            deletion.primary().expect("primary survives").ranges()[0].bytes(),
            expected_range,
            "deletion must select the complete extended grapheme for {source:?}"
        );
        let text = position.text();
        let replacement = document
            .replace_selections(&deletion, "")
            .expect("grapheme deletion must publish once");
        assert_eq!(
            replacement.publication().snapshot().text(text),
            Some(expected_text)
        );
    }
}

#[test]
fn multi_paragraph_selection_edit_reshapes_only_affected_paragraphs() {
    let mut document = Document::new(DocumentId::from_bytes(*b"selection-cache1"));
    let mut edit = document.edit();
    let first_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("first paragraph is valid");
    edit.append_text(first_paragraph, InlineRole::TEXT, "alpha")
        .expect("first text is valid");
    let middle_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("middle paragraph is valid");
    edit.append_text(middle_paragraph, InlineRole::TEXT, "bravo")
        .expect("middle text is valid");
    let last_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("last paragraph is valid");
    edit.append_text(last_paragraph, InlineRole::TEXT, "charlie")
        .expect("last text is valid");
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
    let initial = engine
        .prepare(&document.snapshot(), &request)
        .expect("initial document must prepare");
    assert_eq!(initial.work().shape().paragraphs(), 3);
    let scene = initial.scene();
    let first = *scene
        .hit_test_closest(Point::new(10_000.0, scene.lines()[0].bounds().center().y))
        .expect("first paragraph end resolves")
        .position();
    let last = *scene
        .hit_test_closest(Point::new(10_000.0, scene.lines()[2].bounds().center().y))
        .expect("last paragraph end resolves")
        .position();
    let selections = scene
        .selection_set([
            scene
                .collapsed_selection(&first)
                .expect("first caret is valid"),
            scene
                .collapsed_selection(&last)
                .expect("last caret is valid"),
        ])
        .expect("two paragraph-local carets form one set");
    let replacement = document
        .replace_selections(&selections, "!")
        .expect("both insertions publish atomically");
    assert_eq!(
        replacement.publication().changes().paragraphs(),
        [first_paragraph, last_paragraph]
    );

    let updated = engine
        .prepare(replacement.publication().snapshot(), &request)
        .expect("updated document must prepare");
    assert_eq!(updated.work().shape().paragraphs(), 2);
    assert_eq!(updated.work().reused_paragraphs(), 1);
    assert_eq!(
        updated.work().analysis().paragraphs(),
        2,
        "only changed paragraphs return to Unicode analysis"
    );
    assert_eq!(
        updated.work().geometry().paragraphs(),
        2,
        "the unchanged middle paragraph must retain its geometry"
    );
}

#[test]
fn event_feed_composition_normalizes_multi_selection_and_retains_committed_work() {
    let mut document = Document::new(DocumentId::from_bytes(*b"ime-feed-cache01"));
    let mut edit = document.edit();
    let first_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("first paragraph is valid");
    let first_text = edit
        .append_text(first_paragraph, InlineRole::TEXT, "alpha")
        .expect("first text is valid");
    let middle_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("middle paragraph is valid");
    edit.append_text(middle_paragraph, InlineRole::TEXT, "bravo")
        .expect("middle text is valid");
    let last_paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("last paragraph is valid");
    edit.append_text(last_paragraph, InlineRole::TEXT, "charlie")
        .expect("last text is valid");
    edit.commit().expect("fixture document must publish");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0).expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let snapshot = document.snapshot();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let mut engine = fixture_engine();
    let committed = engine
        .prepare(&snapshot, &request)
        .expect("committed scene must prepare");
    let scene = committed.scene();
    let first_y = scene.lines()[0].bounds().center().y;
    let last_y = scene.lines()[2].bounds().center().y;
    let start = *scene
        .hit_test_closest(Point::new(-100.0, first_y))
        .expect("paragraph start must resolve")
        .position();
    let end = *scene
        .hit_test_closest(Point::new(10_000.0, last_y))
        .expect("paragraph end must resolve")
        .position();
    let selections = scene
        .selection_set([
            scene
                .collapsed_selection(&start)
                .expect("primary caret must be valid"),
            scene
                .collapsed_selection(&end)
                .expect("secondary caret must be valid"),
        ])
        .expect("two independent carets must form one set");
    let started = scene
        .begin_composition(&selections, CompositionId::from_bytes(*b"feed-composition"))
        .expect("event-feed composition must start");
    assert!(started.selection_changed());
    assert_eq!(selections.selections().len(), 2);
    assert_eq!(started.selections().selections().len(), 1);
    assert!(
        started
            .selections()
            .primary()
            .expect("normalized primary exists")
            .is_collapsed()
    );

    let mut session = started.into_session();
    let expected = session.epoch();
    session
        .update(
            expected,
            CompositionUpdate::new("مرحبا").with_selection(10..10),
        )
        .expect("Arabic event-feed snapshot must update one epoch");
    let transient = engine
        .prepare_composition(&snapshot, &request, &session)
        .expect("Arabic preedit must shape through Parley");
    assert_eq!(transient.work().shape().paragraphs(), 1);
    assert_eq!(transient.work().reused_paragraphs(), 2);
    assert!(transient.scene().fragments().iter().any(|fragment| {
        fragment.script() == *b"Arab"
            && fragment.source().is_some_and(|source| {
                source.sources().iter().any(|segment| {
                    matches!(segment, ProjectedTextSource::Composition(range)
                            if range.id() == session.id() && range.epoch() == session.epoch())
                })
            })
    }));
    assert_eq!(snapshot.text(start.text()), Some("alpha"));

    let cancelled = engine
        .prepare(&snapshot, &request)
        .expect("ending the feed without commit must reveal committed scene");
    assert_eq!(cancelled.work().shape().paragraphs(), 0);
    assert_eq!(cancelled.work().geometry().paragraphs(), 0);
    assert_eq!(cancelled.work().reused_paragraphs(), 3);

    let replacement = session
        .commit(&mut document, "مرحبا")
        .expect("feed commit must publish exactly once");
    assert_eq!(replacement.publication().changes().paragraphs().len(), 1);
    assert_eq!(
        replacement.publication().changes().paragraphs(),
        [first_paragraph]
    );
    assert_eq!(snapshot.text(start.text()), Some("alpha"));
    assert_eq!(
        replacement.publication().snapshot().text(first_text),
        Some("مرحباalpha")
    );
    let committed_update = engine
        .prepare(replacement.publication().snapshot(), &request)
        .expect("one IME commit must retain unaffected siblings");
    assert_eq!(
        committed_update.work().shape().paragraphs(),
        0,
        "the committed publication can reuse physics already formed for the identical preedit"
    );
    assert_eq!(committed_update.work().geometry().paragraphs(), 1);
    assert_eq!(committed_update.work().reused_paragraphs(), 2);
}

#[test]
fn host_driven_queries_share_the_exact_parley_composition_epoch() {
    let (document, styles, paint) = fixture_document("Aé office", 1.2);
    let snapshot = document.snapshot();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let mut engine = fixture_engine();
    let committed = engine
        .prepare(&snapshot, &request)
        .expect("committed host surface must prepare");
    let scene = committed.scene();
    let y = scene.lines()[0].bounds().center().y;
    let end = *scene
        .hit_test_closest(Point::new(10_000.0, y))
        .expect("host insertion point must resolve")
        .position();
    let selections = scene
        .selection_set([scene
            .collapsed_selection(&end)
            .expect("host caret must be valid")])
        .expect("host selection set must validate");
    let surface = EditableSurface::new(&snapshot, [EditableSurfaceElement::text(end.text())])
        .expect("host chooses one explicit semantic surface");
    let committed_host = surface
        .bind(scene, &selections)
        .expect("host selection and committed geometry must bind atomically");
    let replacement = committed_host
        .replacement_selection(1..3)
        .expect("the native byte range for precomposed é must map through the surface");
    let mut session = scene
        .begin_composition(
            &replacement,
            CompositionId::from_bytes(*b"host-composition"),
        )
        .expect("host-driven composition must replace its explicit authored range")
        .into_session();
    let expected = session.epoch();
    session
        .update(
            expected,
            CompositionUpdate::new("écho").with_selection(5..5),
        )
        .expect("host marked text must update");
    let transient = engine
        .prepare_composition(&snapshot, &request, &session)
        .expect("host marked text must shape through Parley");
    let host = surface
        .bind_composition(transient.scene(), &session)
        .expect("text and geometry must bind to the same epoch");

    assert_eq!(host.composition(), Some((session.id(), session.epoch())));
    assert_eq!(host.text(), "Aécho office");
    assert_eq!(host.marked_range(), Some(1..6));
    assert_eq!(host.host_selection(), Some(6..6));
    assert_eq!(
        host.range_in_encoding(0..13, SurfaceTextEncoding::Utf16)
            .expect("surface must answer UTF-16 conversion"),
        0..12
    );
    assert_eq!(
        host.range_from_encoding(0..12, SurfaceTextEncoding::Utf16)
            .expect("UTF-16 conversion must round trip"),
        0..13
    );
    assert_eq!(
        host.text_for_range(1..6)
            .expect("arbitrary marked-text query must resolve"),
        "écho"
    );
    assert_eq!(
        host.snapshot_range(1..6)
            .expect_err("generated marked text cannot become authored source")
            .kind(),
        SurfaceErrorKind::UnmappedRange
    );
    assert_eq!(
        host.replacement_selection(1..6)
            .expect_err("a transient marked range cannot become authored replacement source")
            .kind(),
        SurfaceErrorKind::UnsupportedSelection
    );
    assert!(host.caret_rect().is_some());
    let marked_rect = host
        .first_rect_for_range(1..6)
        .expect("marked range geometry must answer synchronously")
        .expect("marked text has visible geometry");
    let hit_offset = host
        .offset_for_point(marked_rect.center())
        .expect("point hit must map back to the same surface");
    assert!((1..=6).contains(&hit_offset));
    let line = transient.scene().lines()[0].bounds();
    let mut x = line.x0;
    let mut generated_step = None;
    while x <= line.x1 && generated_step.is_none() {
        if let Some(hit) = transient.scene().hit_test(Point::new(x, line.center().y)) {
            let position = *hit.position();
            if matches!(position, ProjectedTextPosition::Composition(_)) {
                generated_step = [TextMovement::PreviousLogical, TextMovement::NextLogical]
                    .into_iter()
                    .filter_map(|movement| transient.scene().move_position(&position, movement))
                    .find(|moved| matches!(moved, ProjectedTextPosition::Composition(_)))
                    .map(|moved| (position, moved));
            }
        }
        x += 0.05;
    }
    let (position, moved) =
        generated_step.expect("preedit movement must stay in the prepared cursor graph");
    assert_ne!(moved, position);
}

#[test]
fn generated_combining_mark_shapes_identically_without_authored_provenance() {
    let (document, styles, paint) = fixture_document("eX", 1.2);
    let snapshot = document.snapshot();
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let mut engine = fixture_engine();
    let committed = engine
        .prepare(&snapshot, &request)
        .expect("committed base must prepare");
    let scene = committed.scene();
    let y = scene.lines()[0].bounds().center().y;
    let start = *scene
        .hit_test_closest(Point::new(-100.0, y))
        .expect("base start must resolve")
        .position();
    let caret = scene
        .selection_set([scene
            .collapsed_selection(&start)
            .expect("base caret must be valid")])
        .expect("one caret forms a set");
    let after_base = scene
        .move_selections(&caret, TextMovement::NextLogical, false)
        .expect("logical movement must cross the base cluster");
    assert_eq!(
        after_base
            .primary()
            .expect("moved primary exists")
            .extent()
            .byte(),
        1
    );
    let mut session = scene
        .begin_composition(&after_base, CompositionId::from_bytes(*b"combining-preedt"))
        .expect("combining composition must start")
        .into_session();
    session
        .update(
            session.epoch(),
            CompositionUpdate::new("\u{301}").with_selection(2..2),
        )
        .expect("combining mark is a valid preedit snapshot");
    let transient = engine
        .prepare_composition(&snapshot, &request, &session)
        .expect("generated combining mark must shape through Parley");

    let (authored, authored_styles, authored_paint) = fixture_document("e\u{301}X", 1.2);
    let authored_request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(1_000.0).expect("test width is valid")),
        &authored_styles,
        &authored_paint,
    );
    let authored_scene = fixture_engine()
        .prepare(&authored.snapshot(), &authored_request)
        .expect("authored comparison must shape")
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| (glyph.id(), glyph.position(), glyph.advance()))
        .collect::<Vec<_>>();
    let projected_scene = transient
        .scene()
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .map(|glyph| (glyph.id(), glyph.position(), glyph.advance()))
        .collect::<Vec<_>>();
    assert_eq!(
        projected_scene, authored_scene,
        "generated provenance must not split the shaping run or change glyph geometry"
    );
    assert!(transient.scene().fragments().iter().any(|fragment| {
        fragment.source().is_some_and(|source| {
            source
                .sources()
                .iter()
                .any(|segment| matches!(segment, ProjectedTextSource::Composition(_)))
        })
    }));
    let line = transient.scene().lines()[0].bounds();
    let mut unit_source = None;
    let mut unit_positions = Vec::new();
    let mut x = line.x0;
    while x <= line.x1 {
        if let Some(hit) = transient.scene().hit_test(Point::new(x, line.center().y)) {
            let has_snapshot = hit
                .source()
                .sources()
                .iter()
                .any(|source| matches!(source, ProjectedTextSource::Snapshot(_)));
            let has_generated = hit
                .source()
                .sources()
                .iter()
                .any(|source| matches!(source, ProjectedTextSource::Composition(_)));
            if has_snapshot && has_generated {
                unit_source.get_or_insert_with(|| hit.source().clone());
                if !unit_positions.contains(hit.position()) {
                    unit_positions.push(*hit.position());
                }
            }
        }
        x += 0.05;
    }
    let unit_source = unit_source.expect("the composed grapheme must expose one mixed source");
    assert_eq!(
        unit_source.sources().len(),
        2,
        "authored base and generated mark must both remain in the hit unit"
    );
    assert_eq!(
        unit_positions.len(),
        2,
        "the provenance boundary inside one grapheme must not become a caret stop"
    );
    assert!(unit_positions.iter().enumerate().any(|(index, position)| {
        unit_positions
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .any(|(_, other)| {
                transient
                    .scene()
                    .move_position(position, TextMovement::PreviousLogical)
                    .is_some_and(|moved| moved == *other)
                    || transient
                        .scene()
                        .move_position(position, TextMovement::NextLogical)
                        .is_some_and(|moved| moved == *other)
            })
    }));
    assert_eq!(snapshot.text(start.text()), Some("eX"));
}
