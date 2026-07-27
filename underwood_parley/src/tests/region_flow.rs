// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use underwood::{FloatSide, FlowRegion, RegionFloat, RegionFlow, Size};

#[test]
fn product_path_restores_text_after_height_rejection_and_continues_in_a_column() {
    let text = "alpha beta gamma delta";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let flow = RegionFlow::new([
        FlowRegion::new(Rect::new(0.0, 0.0, 72.0, 10.0)).expect("short column is valid"),
        FlowRegion::new(Rect::new(100.0, 0.0, 172.0, 200.0)).expect("second column is valid"),
    ])
    .expect("column flow is valid");
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(72.0).expect("fallback width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow)
    .with_preparation_trace();
    let mut engine = fixture_engine();
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("text must retry into the second column");
    let sources = scene_sources(output.scene());
    let transcript = output
        .region_transcript()
        .expect("region preparation retains a transcript");

    assert_eq!(
        transcript
            .replay(&flow)
            .expect("transcript must replay exactly"),
        transcript.end()
    );
    let mut attempts = transcript.attempts();
    assert_eq!(
        attempts.next().expect("rejected attempt exists").outcome(),
        RegionAttemptOutcome::HeightRejected
    );
    assert_eq!(
        attempts.next().expect("accepted attempt exists").outcome(),
        RegionAttemptOutcome::Accepted
    );
    assert_eq!(
        output.scene().line(0).expect("line exists").bounds().x0,
        100.0
    );
    assert_eq!(
        output.scene().line(0).expect("line exists").bounds().y0,
        0.0
    );
    assert_eq!(
        sources
            .for_line(output.scene().line(0).expect("line exists"))
            .expect("line belongs to source scene")
            .iter()
            .next()
            .expect("source exists")
            .bytes()
            .start,
        0
    );
    assert!(output.work().rejected_line_candidates() >= 1);
    assert!(output.work().line_checkpoint_restores() >= 1);
    let trace = output.trace().expect("trace was requested");
    assert_eq!(trace.region_attempts(), transcript.attempts().len());
    assert_eq!(trace.region_height_rejections(), 1);
    assert!(
        trace.memory().scratch_growth_bytes() > 0,
        "cold region preparation retains reusable projection capacity"
    );

    let retained = engine
        .prepare(&document.snapshot(), &request)
        .expect("retained region trace prepares");
    let retained_trace = retained.trace().expect("trace was requested");
    assert_eq!(retained_trace.reuse().exact_geometry_reuses(), 1);
    assert_eq!(retained_trace.memory().scratch_growth_bytes(), 0);
    assert_eq!(
        retained_trace.memory().scratch_capacity_before(),
        retained_trace.memory().scratch_capacity_after()
    );
}

#[test]
fn exclusion_intervals_share_a_row_without_overlapping_text_geometry() {
    let text = "one two three four five six seven eight nine ten";
    let (document, styles, paint) = fixture_document(text, 1.0);
    let region = FlowRegion::new(Rect::new(0.0, 0.0, 180.0, 200.0))
        .expect("region is valid")
        .with_exclusions([Rect::new(70.0, 0.0, 110.0, 50.0)])
        .expect("central exclusion is valid");
    let flow = RegionFlow::new([region]).expect("exclusion flow is valid");
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(180.0).expect("fallback width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let output = fixture_engine()
        .prepare(&document.snapshot(), &request)
        .expect("text must fill both exclusion intervals");
    let lines = output.scene().lines();
    let sources = scene_sources(output.scene());

    assert!(lines.len() >= 2);
    assert_eq!(
        lines.get(0).expect("line exists").bounds().y0,
        lines.get(1).expect("line exists").bounds().y0
    );
    assert!(lines.get(0).expect("line exists").bounds().x1 <= 70.0);
    assert!(lines.get(1).expect("line exists").bounds().x0 >= 110.0);
    let left_source = sources
        .for_line(lines.get(0).expect("line exists"))
        .expect("line belongs to source scene")
        .iter()
        .next()
        .expect("source exists")
        .bytes();
    let right_source = sources
        .for_line(lines.get(1).expect("line exists"))
        .expect("line belongs to source scene")
        .iter()
        .next()
        .expect("source exists")
        .bytes();
    assert_eq!(left_source.end, right_source.start);
    for (line, source) in [
        (&lines.get(0).expect("line exists"), left_source),
        (&lines.get(1).expect("line exists"), right_source),
    ] {
        let hit = output
            .scene()
            .editing()
            .expect("fixture retains editable scene data")
            .hit_test(line.bounds().center())
            .expect("each same-row interval retains exact hit geometry");
        assert!(source.start <= hit.position().byte() && hit.position().byte() <= source.end);
    }
    let transcript = output.region_transcript().expect("transcript exists");
    for line in lines {
        let attempt = transcript
            .attempts()
            .find(|attempt| {
                attempt.outcome() == RegionAttemptOutcome::Accepted
                    && attempt.source()
                        == sources
                            .for_line(line)
                            .expect("line belongs to source scene")
                            .iter()
                            .fold(None, |range, source| {
                                let bytes = source.bytes();
                                Some(range.map_or(bytes.clone(), |range: core::ops::Range<u32>| {
                                    range.start.min(bytes.start)..range.end.max(bytes.end)
                                }))
                            })
                            .expect("line has source")
            })
            .expect("every scene line has an accepted slot");
        assert!(line.bounds().x0 >= attempt.slot().inline_start());
        assert!(
            line.bounds().x1 <= attempt.slot().bounds().x1
                || line.advance() > attempt.slot().inline_size()
        );
        assert!(line.bounds().y1 <= attempt.slot().bounds().y1);
    }
}

#[test]
fn floats_decompose_into_distinct_zero_allocation_slot_bands() {
    let text = "alpha beta gamma delta epsilon zeta eta theta iota";
    let (document, styles, paint) = fixture_document(text, 1.0);
    let region = FlowRegion::new(Rect::new(0.0, 0.0, 180.0, 200.0))
        .expect("region is valid")
        .with_floats([
            RegionFloat::new(FloatSide::Left, 0.0, Size::new(50.0, 30.0))
                .expect("left float is valid"),
            RegionFloat::new(FloatSide::Right, 30.0, Size::new(60.0, 30.0))
                .expect("right float is valid"),
        ])
        .expect("floats fit");
    let flow = RegionFlow::new([region]).expect("float flow is valid");
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(180.0).expect("fallback width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let output = fixture_engine()
        .prepare(&document.snapshot(), &request)
        .expect("text must flow around floats");
    let transcript = output.region_transcript().expect("transcript exists");
    let accepted: Vec<_> = transcript
        .attempts()
        .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::Accepted)
        .collect();

    assert_eq!(accepted[0].slot().inline_start(), 50.0);
    assert_eq!(accepted[0].slot().bounds().x1, 180.0);
    assert!(
        accepted
            .iter()
            .any(|attempt| attempt.slot().bounds().x1 == 120.0),
        "a later line must consume the right-float band"
    );
}

#[test]
fn paragraphs_resume_one_cursor_across_region_boundaries() {
    let mut document = Document::new(DocumentId::from_bytes(*b"region-paragraph"));
    let mut edit = document.edit();
    for text in ["first", "second"] {
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("paragraph is valid");
        edit.append_text(paragraph, InlineRole::TEXT, text)
            .expect("text is valid");
    }
    edit.commit().expect("document publishes");
    let (_, styles, paint) = fixture_document("style", 1.0);
    let flow = RegionFlow::new([
        FlowRegion::new(Rect::new(0.0, 0.0, 100.0, 20.0)).expect("first column is valid"),
        FlowRegion::new(Rect::new(120.0, 0.0, 220.0, 40.0)).expect("second column is valid"),
    ])
    .expect("column flow is valid");
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(100.0).expect("fallback width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let mut engine = fixture_engine();
    let output = engine
        .prepare(&document.snapshot(), &request)
        .expect("paragraphs must share the region cursor");

    assert_eq!(output.scene().lines().len(), 2);
    assert_eq!(
        output.scene().line(0).expect("line exists").bounds().x0,
        0.0
    );
    assert_eq!(
        output.scene().line(1).expect("line exists").bounds().x0,
        120.0
    );
    let transcript = output
        .region_transcript()
        .expect("document transcript exists");
    assert_eq!(
        transcript
            .replay(&flow)
            .expect("document transcript replays"),
        transcript.end()
    );

    let retained = engine
        .prepare(&document.snapshot(), &request)
        .expect("cached paragraph transcripts resume the same cursor");
    assert_eq!(retained.work().reused_paragraphs(), 2);
    assert_eq!(retained.work().flow().paragraphs(), 0);
    assert_eq!(
        retained.scene().line(0).expect("line exists").bounds().x0,
        0.0
    );
    assert_eq!(
        retained.scene().line(1).expect("line exists").bounds().x0,
        120.0
    );
    assert_eq!(
        retained
            .region_transcript()
            .expect("retained transcript exists")
            .replay(&flow)
            .expect("retained transcript replays"),
        retained
            .region_transcript()
            .expect("retained transcript exists")
            .end()
    );
}

#[test]
fn localized_region_edit_stops_when_the_cursor_converges() {
    let mut document = Document::new(DocumentId::from_bytes(*b"region-localized"));
    let mut edit = document.edit();
    let mut target = None;
    for index in 0..64 {
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("paragraph is valid");
        let text = edit
            .append_text(paragraph, InlineRole::TEXT, "stable")
            .expect("text is valid");
        if index == 31 {
            target = Some(text);
        }
    }
    edit.commit().expect("document publishes");
    let (_, styles, paint) = fixture_document("style", 1.0);
    let flow = RegionFlow::rectangle(Rect::new(0.0, 0.0, 120.0, 2_000.0)).expect("flow is valid");
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(120.0).expect("fallback width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow)
    .with_preparation_trace();
    let mut engine = fixture_engine_with_budgets(128, 8 * 1024 * 1024);
    let cold = engine
        .prepare(&document.snapshot(), &request)
        .expect("initial region scene prepares");

    let mut edit = document.edit();
    edit.replace_text(target.expect("target exists"), "changed")
        .expect("target edits");
    let publication = edit.commit().expect("edit publishes");
    let changed = engine
        .prepare(publication.snapshot(), &request)
        .expect("localized region scene prepares");

    assert_eq!(changed.work().shape().paragraphs(), 1);
    assert_eq!(changed.work().flow().paragraphs(), 1);
    assert_eq!(changed.work().paint().paragraphs(), 1);
    assert_eq!(changed.work().reused_paragraphs(), 63);
    assert_eq!(
        changed
            .region_transcript()
            .expect("changed transcript exists")
            .attempts()
            .len(),
        cold.region_transcript()
            .expect("cold transcript exists")
            .attempts()
            .len()
    );
    assert!(
        changed
            .trace()
            .expect("trace exists")
            .memory()
            .scene_output_capacity_bytes()
            < cold
                .trace()
                .expect("trace exists")
                .memory()
                .scene_output_capacity_bytes(),
        "localized publication must retain the unchanged region-scene paths"
    );
}

#[test]
fn changing_only_region_geometry_reuses_analysis_and_canonical_shaping() {
    let text = "alpha beta gamma delta";
    let (document, styles, paint) = fixture_document(text, 1.0);
    let wide =
        RegionFlow::rectangle(Rect::new(0.0, 0.0, 180.0, 200.0)).expect("wide flow is valid");
    let narrow =
        RegionFlow::rectangle(Rect::new(0.0, 0.0, 72.0, 200.0)).expect("narrow flow is valid");
    let mut engine = fixture_engine();
    let request = |flow| {
        editable_scene_request(
            TextConstraint::Wrap(FiniteWidth::new(180.0).expect("fallback width is valid")),
            &styles,
            &paint,
        )
        .with_region_flow(flow)
    };
    engine
        .prepare(&document.snapshot(), &request(&wide))
        .expect("wide region prepares");
    let changed = engine
        .prepare(&document.snapshot(), &request(&narrow))
        .expect("narrow region prepares");

    assert_eq!(changed.work().analysis().paragraphs(), 0);
    assert_eq!(changed.work().itemization().paragraphs(), 0);
    assert_eq!(changed.work().font_selection().paragraphs(), 0);
    assert_eq!(changed.work().shape().paragraphs(), 0);
    assert_eq!(changed.work().flow().paragraphs(), 1);
}

#[test]
fn empty_paragraph_consumes_height_without_fabricating_text() {
    let (document, styles, paint) = fixture_document("", 1.0);
    let flow = RegionFlow::new([
        FlowRegion::new(Rect::new(0.0, 0.0, 80.0, 5.0)).expect("short region is valid"),
        FlowRegion::new(Rect::new(100.0, 0.0, 180.0, 50.0)).expect("second region is valid"),
    ])
    .expect("empty-line flow is valid");
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(80.0).expect("fallback width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let output = fixture_engine()
        .prepare(&document.snapshot(), &request)
        .expect("empty paragraph must consume an exact slot");
    let transcript = output
        .region_transcript()
        .expect("empty paragraph retains a transcript");
    let attempts: Vec<_> = transcript.attempts().collect();

    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].outcome(), RegionAttemptOutcome::HeightRejected);
    assert_eq!(attempts[1].outcome(), RegionAttemptOutcome::Accepted);
    assert!(attempts[1].source().is_empty());
    assert!(output.scene().lines().is_empty());
    assert_eq!(output.scene().metrics().size().height, 20.0);
    let editing = output
        .scene()
        .editing()
        .expect("fixture retains editable scene data");
    let hit = editing
        .hit_test_closest(Point::new(100.0, 10.0))
        .expect("empty paragraph keeps its represented caret");
    assert_eq!(
        editing
            .caret(hit.position())
            .expect("empty caret resolves")
            .bounds()
            .x0,
        100.0
    );
}

#[test]
fn line_height_change_retries_regions_without_reshaping() {
    let text = "one line";
    let (document, compact_styles, paint) = fixture_document(text, 1.0);
    let (_, spacious_styles, _) = fixture_document(text, 1.5);
    let flow = RegionFlow::new([
        FlowRegion::new(Rect::new(0.0, 0.0, 120.0, 22.0)).expect("short region is valid"),
        FlowRegion::new(Rect::new(140.0, 0.0, 260.0, 60.0)).expect("second region is valid"),
    ])
    .expect("height-sensitive flow is valid");
    let mut engine = fixture_engine();
    let compact_request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(120.0).expect("fallback width is valid")),
        &compact_styles,
        &paint,
    )
    .with_region_flow(&flow);
    let compact = engine
        .prepare(&document.snapshot(), &compact_request)
        .expect("compact line fits first region");
    assert_eq!(
        compact.scene().line(0).expect("line exists").bounds().x0,
        0.0
    );

    let spacious_request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(120.0).expect("fallback width is valid")),
        &spacious_styles,
        &paint,
    )
    .with_region_flow(&flow);
    let spacious = engine
        .prepare(&document.snapshot(), &spacious_request)
        .expect("spacious line retries second region");
    assert_eq!(
        spacious.scene().line(0).expect("line exists").bounds().x0,
        140.0
    );
    assert_eq!(spacious.work().analysis().paragraphs(), 0);
    assert_eq!(spacious.work().shape().paragraphs(), 0);
    assert_eq!(spacious.work().line_shape().paragraphs(), 0);
    assert!(spacious.work().rejected_line_candidates() >= 1);
}

#[test]
fn region_offsets_move_mixed_bidi_hits_carets_and_selections_together() {
    let text = "abc مرحبا xyz";
    let (document, styles, paint) = fixture_document(text, 1.0);
    let snapshot = document.snapshot();
    let mut engine = fixture_engine();
    let plain = engine
        .prepare(
            &snapshot,
            &editable_scene_request(
                TextConstraint::Wrap(FiniteWidth::new(400.0).expect("plain width is valid")),
                &styles,
                &paint,
            ),
        )
        .expect("plain bidi text prepares");
    let flow =
        RegionFlow::rectangle(Rect::new(80.0, 40.0, 480.0, 200.0)).expect("offset region is valid");
    let shifted = engine
        .prepare(
            &snapshot,
            &editable_scene_request(
                TextConstraint::Wrap(FiniteWidth::new(400.0).expect("fallback width is valid")),
                &styles,
                &paint,
            )
            .with_region_flow(&flow),
        )
        .expect("region bidi text prepares");
    let plain_scene = plain.scene();
    let shifted_scene = shifted.scene();

    assert_eq!(plain_scene.lines().len(), shifted_scene.lines().len());
    assert_eq!(
        shifted_scene.line(0).expect("line exists").bounds().x0
            - plain_scene.line(0).expect("line exists").bounds().x0,
        80.0
    );
    assert_eq!(
        shifted_scene.line(0).expect("line exists").bounds().y0
            - plain_scene.line(0).expect("line exists").bounds().y0,
        40.0
    );
    assert_eq!(
        plain_scene
            .fragments()
            .iter()
            .map(|fragment| fragment.bidi_level())
            .collect::<Vec<_>>(),
        shifted_scene
            .fragments()
            .iter()
            .map(|fragment| fragment.bidi_level())
            .collect::<Vec<_>>()
    );

    let plain_hits = scan_line_hits(plain_scene, 0);
    let shifted_hits = scan_line_hits(shifted_scene, 0);
    assert_eq!(
        plain_hits
            .iter()
            .map(|hit| hit.source.clone())
            .collect::<Vec<_>>(),
        shifted_hits
            .iter()
            .map(|hit| hit.source.clone())
            .collect::<Vec<_>>()
    );
    for (plain, shifted) in plain_hits.iter().zip(&shifted_hits) {
        assert!((shifted.min_x - plain.min_x - 80.0).abs() <= 0.06);
        assert!((shifted.max_x - plain.max_x - 80.0).abs() <= 0.06);
    }

    let y = plain_scene
        .line(0)
        .expect("line exists")
        .bounds()
        .center()
        .y;
    let plain_editing = plain_scene
        .editing()
        .expect("fixture retains editable scene data");
    let shifted_editing = shifted_scene
        .editing()
        .expect("fixture retains editable scene data");
    let anchor = *plain_editing
        .hit_test_closest(Point::new(
            plain_scene.line(0).expect("line exists").bounds().x0,
            y,
        ))
        .expect("plain start resolves")
        .position();
    let extent = *plain_editing
        .hit_test_closest(Point::new(
            plain_scene.line(0).expect("line exists").bounds().x1,
            y,
        ))
        .expect("plain end resolves")
        .position();
    let selection = plain_editing
        .selection_between(&anchor, &extent, TextSelectionMode::Visual)
        .expect("visual selection is valid");
    let plain_geometry = plain_editing
        .selection_geometry(
            &plain_editing
                .selection_set([selection.clone()])
                .expect("plain selection set is valid"),
        )
        .expect("plain selection geometry resolves");
    let shifted_geometry = shifted_editing
        .selection_geometry(
            &shifted_editing
                .selection_set([selection])
                .expect("shifted selection set is valid"),
        )
        .expect("shifted selection geometry resolves");
    assert_eq!(plain_geometry.len(), shifted_geometry.len());
    for (plain, shifted) in plain_geometry.iter().zip(&shifted_geometry) {
        assert_eq!(plain.bidi_level(), shifted.bidi_level());
        assert_eq!(shifted.bounds().x0 - plain.bounds().x0, 80.0);
        assert_eq!(shifted.bounds().y0 - plain.bounds().y0, 40.0);
    }
}

#[test]
fn composition_projection_flows_through_the_same_exact_region_transcript() {
    let (document, styles, paint) = fixture_document("office", 1.0);
    let snapshot = document.snapshot();
    let flow = RegionFlow::rectangle(Rect::new(40.0, 20.0, 440.0, 180.0)).expect("region is valid");
    let request = editable_scene_request(
        TextConstraint::Wrap(FiniteWidth::new(400.0).expect("fallback width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let mut engine = fixture_engine();
    let committed = engine
        .prepare(&snapshot, &request)
        .expect("committed region scene prepares");
    let editing = committed
        .scene()
        .editing()
        .expect("fixture retains editable scene data");
    let line = &committed.scene().line(0).expect("line exists");
    let end = *editing
        .hit_test_closest(Point::new(line.bounds().x1, line.bounds().center().y))
        .expect("line end resolves")
        .position();
    let selections = editing
        .selection_set([editing
            .collapsed_selection(&end)
            .expect("insertion selection is valid")])
        .expect("selection set is valid");
    let mut session = editing
        .begin_composition(&selections, CompositionId::from_bytes(*b"region-compose01"))
        .expect("composition starts")
        .into_session();
    session
        .update(
            session.epoch(),
            CompositionUpdate::new(" مرحبا").with_selection(11..11),
        )
        .expect("generated Arabic text updates");

    let transient = engine
        .prepare_composition(&snapshot, &request, &session)
        .expect("composition uses region formation");
    let sources = projected_scene_sources(transient.scene());
    let transcript = transient
        .region_transcript()
        .expect("composition retains exact region attempts");
    assert_eq!(
        transcript
            .replay(&flow)
            .expect("composition transcript replays"),
        transcript.end()
    );
    assert!(
        transient
            .scene()
            .lines()
            .iter()
            .all(|line| line.bounds().x0 >= 40.0 && line.bounds().y0 >= 20.0)
    );
    assert!(transient.scene().fragments().iter().any(|fragment| {
        sources
            .for_fragment(fragment)
            .expect("fragment belongs to source scene")
            .any(|source| {
                matches!(
                    source,
                    ProjectedTextSource::Composition(range)
                        if range.id() == session.id() && range.epoch() == session.epoch()
                )
            })
    }));
}
