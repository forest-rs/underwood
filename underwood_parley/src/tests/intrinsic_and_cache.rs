// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn intrinsic_constraints_honor_mandatory_breaks_and_report_exact_metrics() {
    let (document, styles, paint) = fixture_document("alpha beta\ngamma delta", 1.2);
    let mut engine = fixture_engine();

    let max = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("max-content formation succeeds");
    assert_eq!(max.scene().lines().len(), 2);
    assert_eq!(
        max.scene().line(0).expect("line exists").break_reason(),
        TestLineBreakReason::Mandatory
    );
    assert_eq!(
        max.scene().line(1).expect("line exists").break_reason(),
        TestLineBreakReason::End
    );
    assert_eq!(
        max.scene().metrics().size().width,
        max.scene()
            .lines()
            .iter()
            .map(|line| line.advance())
            .fold(0.0_f64, f64::max)
    );
    assert_eq!(
        max.scene().metrics().size().height,
        max.scene().lines().last().expect("line exists").bounds().y1
    );
    assert_eq!(
        max.scene().metrics().first_baseline(),
        Some(max.scene().line(0).expect("line exists").baseline())
    );
    assert_eq!(
        max.scene().metrics().last_baseline(),
        Some(max.scene().line(1).expect("line exists").baseline())
    );

    let min = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MinContent, &styles, &paint),
        )
        .expect("min-content formation succeeds");
    assert_eq!(min.work().analysis().paragraphs(), 0);
    assert_eq!(min.work().shape().paragraphs(), 0);
    assert_eq!(min.work().flow().paragraphs(), 1);
    assert!(min.scene().lines().len() > max.scene().lines().len());
    assert!(min.scene().metrics().size().width <= max.scene().metrics().size().width);
    assert!(min.scene().metrics().size().height >= max.scene().metrics().size().height);

    let wrapped = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(
                    FiniteWidth::new(90.0).expect("test width is finite and positive"),
                ),
                &styles,
                &paint,
            ),
        )
        .expect("constrained formation succeeds");
    assert_eq!(wrapped.work().shape().paragraphs(), 0);
    assert!(
        wrapped
            .scene()
            .lines()
            .iter()
            .all(|line| line.advance() <= 90.0)
    );
}

#[test]
fn hit_area_padding_does_not_inflate_zero_advance_intrinsic_width() {
    let (document, styles, paint) = fixture_document("\n", 1.0);
    let output = fixture_engine()
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("mandatory break prepares");
    let max_advance = output
        .scene()
        .lines()
        .iter()
        .map(|line| line.advance())
        .fold(0.0_f64, f64::max);

    assert_eq!(max_advance, 0.0);
    assert!(
        output
            .scene()
            .lines()
            .iter()
            .all(|line| line.bounds().width() >= 1.0),
        "interaction geometry retains its minimum hit area"
    );
    assert_eq!(
        output.scene().metrics().size().width,
        max_advance,
        "intrinsic measurement must use actual advance rather than padded bounds"
    );
}

#[test]
fn text_block_matches_document_path_and_empty_metrics_are_explicit() {
    let text = "office مرحبا";
    let (document, styles, paint) = fixture_document(text, 1.2);
    let style = styles.default_style().clone();
    let block = TextBlock::plain(DocumentId::from_bytes(*b"text-block-proof"), text)
        .expect("block initializes");
    let mut engine = fixture_engine();
    let document_output = engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("document prepares");
    let block_output = engine
        .prepare_block(
            &block.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
        )
        .expect("block prepares");
    assert_eq!(
        block_output.scene().metrics(),
        document_output.scene().metrics()
    );
    assert_eq!(
        block_output
            .scene()
            .lines()
            .iter()
            .map(|line| (line.advance(), line.break_reason(), line.baseline()))
            .collect::<Vec<_>>(),
        document_output
            .scene()
            .lines()
            .iter()
            .map(|line| (line.advance(), line.break_reason(), line.baseline()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        block_output
            .scene()
            .fragments()
            .iter()
            .flat_map(|fragment| fragment.glyphs())
            .map(|glyph| (glyph.id(), glyph.position(), glyph.advance()))
            .collect::<Vec<_>>(),
        document_output
            .scene()
            .fragments()
            .iter()
            .flat_map(|fragment| fragment.glyphs())
            .map(|glyph| (glyph.id(), glyph.position(), glyph.advance()))
            .collect::<Vec<_>>()
    );

    let empty = TextBlock::plain(DocumentId::from_bytes(*b"empty-block-test"), "")
        .expect("empty block initializes");
    let empty_output = engine
        .prepare_block(
            &empty.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
        )
        .expect("empty block prepares");
    assert_eq!(empty_output.scene().metrics().size().width, 0.0);
    assert!(
        (empty_output.scene().metrics().size().height - 24.0).abs() < 0.001,
        "empty height must be the resolved 20px × 1.2 line height"
    );
    assert_eq!(empty_output.scene().metrics().first_baseline(), None);
    assert_eq!(empty_output.scene().metrics().last_baseline(), None);
    assert!(empty_output.scene().lines().is_empty());
}

#[test]
fn cache_budget_and_explicit_release_coordinate_all_retained_layers() {
    let (_, styles, paint) = fixture_document("cache", 1.2);
    let style = styles.default_style().clone();
    let blocks = [
        TextBlock::plain(DocumentId::from_bytes(*b"cache-block-0001"), "one")
            .expect("block initializes"),
        TextBlock::plain(DocumentId::from_bytes(*b"cache-block-0002"), "two")
            .expect("block initializes"),
        TextBlock::plain(DocumentId::from_bytes(*b"cache-block-0003"), "three")
            .expect("block initializes"),
    ];
    let mut engine = fixture_engine_with_budget(2);
    for block in &blocks {
        let output = engine
            .prepare_block(
                &block.snapshot(),
                &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
            )
            .expect("block prepares");
        assert_eq!(output.work().shape().paragraphs(), 1);
    }
    let after_churn = engine.cache_diagnostics();
    assert_eq!(after_churn.current_entries(), 2);
    assert_eq!(after_churn.backend_entries(), Some(2));
    assert_eq!(after_churn.evictions(), 1);
    assert_eq!(after_churn.peak_entries(), 3);
    assert_eq!(after_churn.budget(), 2);
    assert_eq!(after_churn.misses(), 3);
    assert_eq!(after_churn.hits(), 0);
    assert!(after_churn.scene_cache_accounted_bytes() > 0);

    let retained = engine
        .prepare_block(
            &blocks[1].snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
        )
        .expect("resident block prepares");
    assert_eq!(retained.work().analysis().paragraphs(), 0);
    assert_eq!(retained.work().shape().paragraphs(), 0);
    assert_eq!(retained.work().flow().paragraphs(), 0);
    assert_eq!(
        engine.cache_diagnostics().hits(),
        0,
        "an exact published-root hit must not fabricate a paragraph lookup"
    );

    engine.release_document(blocks[1].id());
    let after_release = engine.cache_diagnostics();
    assert_eq!(after_release.current_entries(), 1);
    assert_eq!(after_release.backend_entries(), Some(1));
    assert_eq!(after_release.releases(), 1);
    assert!(
        after_release.scene_cache_accounted_bytes() < after_churn.scene_cache_accounted_bytes()
    );

    let reloaded = engine
        .prepare_block(
            &blocks[0].snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
        )
        .expect("evicted block prepares again");
    assert_eq!(reloaded.work().shape().paragraphs(), 1);
    engine.clear_cache();
    assert_eq!(engine.cache_diagnostics().current_entries(), 0);
    assert_eq!(engine.cache_diagnostics().backend_entries(), Some(0));
    assert_eq!(engine.cache_diagnostics().scene_cache_accounted_bytes(), 0);

    let mut zero = fixture_engine_with_budget(0);
    let owned = zero
        .prepare_block(
            &blocks[0].snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
        )
        .expect("zero-budget output still materializes");
    assert!(!owned.scene().fragments().is_empty());
    assert_eq!(zero.cache_diagnostics().current_entries(), 0);
    assert_eq!(zero.cache_diagnostics().backend_entries(), Some(0));
    assert_eq!(zero.cache_diagnostics().scene_cache_accounted_bytes(), 0);
    assert_eq!(zero.cache_diagnostics().evictions(), 1);
}

#[test]
fn identical_blocks_share_only_identity_free_preparation() {
    let (_, styles, paint) = fixture_document("fixture", 1.2);
    let style = styles.default_style().clone();
    let first = TextBlock::plain(DocumentId::from_bytes(*b"shared-label-001"), "office مرحبا")
        .expect("first block is valid");
    let second = TextBlock::plain(DocumentId::from_bytes(*b"shared-label-002"), "office مرحبا")
        .expect("second block is valid");
    let mut layout = fixture_engine_with_budgets(8, 8 * 1024 * 1024);
    let request = BlockRequest::new(TextConstraint::MaxContent, &style, &paint);

    let first_output = layout
        .prepare_block(&first.snapshot(), &request)
        .expect("first block prepares");
    let second_output = layout
        .prepare_block(&second.snapshot(), &request)
        .expect("second block prepares from shared facts");

    assert_eq!(first_output.work().analysis().paragraphs(), 1);
    assert_eq!(first_output.work().shape().paragraphs(), 1);
    assert_eq!(first_output.work().flow().paragraphs(), 1);
    assert_eq!(first_output.work().shared_preparations(), 0);
    assert_eq!(second_output.work().analysis().paragraphs(), 0);
    assert_eq!(second_output.work().shape().paragraphs(), 0);
    assert_eq!(second_output.work().flow().paragraphs(), 0);
    assert_eq!(second_output.work().shared_preparations(), 1);
    assert_eq!(second_output.work().geometry().paragraphs(), 1);
    assert_ne!(
        first_output.scene().document(),
        second_output.scene().document(),
        "a shared hit must retain the consuming document identity"
    );
    assert_ne!(
        first_output
            .scene()
            .semantics()
            .find_map(|semantic| semantic.source())
            .expect("first inline semantic has source")
            .text(),
        second_output
            .scene()
            .semantics()
            .find_map(|semantic| semantic.source())
            .expect("second inline semantic has source")
            .text(),
        "source-leaf identity must be rebuilt for each consumer"
    );
    assert_ne!(
        first_output
            .scene()
            .semantics()
            .find(|semantic| semantic.inline_role().is_some())
            .expect("first inline semantic exists")
            .semantic_id(),
        second_output
            .scene()
            .semantics()
            .find(|semantic| semantic.inline_role().is_some())
            .expect("second inline semantic exists")
            .semantic_id(),
        "semantic identity must be rebuilt for each consumer"
    );

    let diagnostics = layout.cache_diagnostics();
    assert_eq!(diagnostics.shared_preparation_entries(), 1);
    assert_eq!(diagnostics.shared_preparation_hits(), 1);
    assert_eq!(diagnostics.shared_preparation_misses(), 1);
    assert_eq!(
        diagnostics.backend_entries(),
        Some(1),
        "a shared consumer must not manufacture a backend identity entry"
    );

    let stable = layout
        .prepare_block(&second.snapshot(), &request)
        .expect("stable second block reuses retained geometry");
    assert_eq!(stable.work().reused_paragraphs(), 1);
    assert_eq!(stable.work().shared_preparations(), 0);

    layout.release_document(first.id());
    let third = TextBlock::plain(DocumentId::from_bytes(*b"shared-label-003"), "office مرحبا")
        .expect("third block is valid");
    let third_output = layout
        .prepare_block(&third.snapshot(), &request)
        .expect("shared facts survive release of their producing document");
    assert_eq!(third_output.work().shared_preparations(), 1);
    assert_eq!(layout.cache_diagnostics().shared_preparation_entries(), 1);
}

#[test]
fn shared_preparation_rebuilds_distinct_leaf_and_semantic_topology() {
    let (_, styles, paint) = fixture_document("fixture", 1.2);
    let mut first = Document::new(DocumentId::from_bytes(*b"shared-leaf--001"));
    let mut first_edit = first.edit();
    let first_paragraph = first_edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("first paragraph is valid");
    first_edit
        .append_text(first_paragraph, InlineRole::TEXT, "office")
        .expect("first leaf is valid");
    first_edit.commit().expect("first document commits");

    let mut second = Document::new(DocumentId::from_bytes(*b"shared-leaf--002"));
    let mut second_edit = second.edit();
    let second_paragraph = second_edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("second paragraph is valid");
    second_edit
        .append_text(second_paragraph, InlineRole::TEXT, "of")
        .expect("second first leaf is valid");
    second_edit
        .append_text(second_paragraph, InlineRole::EMPHASIS, "fice")
        .expect("second emphasized leaf is valid");
    second_edit.commit().expect("second document commits");

    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint);
    let mut layout = fixture_engine_with_budgets(8, 8 * 1024 * 1024);
    let first_output = layout
        .prepare(&first.snapshot(), &request)
        .expect("one-leaf document prepares");
    let second_output = layout
        .prepare(&second.snapshot(), &request)
        .expect("split semantic document shares preparation");

    assert_eq!(second_output.work().shared_preparations(), 1);
    assert_eq!(second_output.work().shape().paragraphs(), 0);
    assert_eq!(
        first_output
            .scene()
            .semantics()
            .filter(|semantic| semantic.inline_role().is_some())
            .count(),
        1
    );
    let second_roles: Vec<_> = second_output
        .scene()
        .semantics()
        .filter_map(|semantic| semantic.inline_role())
        .collect();
    assert_eq!(second_roles, [InlineRole::TEXT, InlineRole::EMPHASIS]);
    let second_texts: Vec<_> = second_output
        .scene()
        .semantics()
        .filter_map(|semantic| semantic.source().map(|source| source.text()))
        .collect();
    assert_eq!(second_texts.len(), 2);
    assert_ne!(
        second_texts[0], second_texts[1],
        "the consuming source map must retain both leaf identities"
    );
}

#[test]
fn shared_composition_preparation_rebinds_native_identity_and_epoch() {
    let (_, styles, paint) = fixture_document("fixture", 1.2);
    let mut first = Document::new(DocumentId::from_bytes(*b"shared-ime---001"));
    let mut first_edit = first.edit();
    let first_paragraph = first_edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("first paragraph is valid");
    first_edit
        .append_text(first_paragraph, InlineRole::TEXT, "a")
        .expect("first text is valid");
    first_edit.commit().expect("first document commits");
    let mut second = Document::new(DocumentId::from_bytes(*b"shared-ime---002"));
    let mut second_edit = second.edit();
    let second_paragraph = second_edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("second paragraph is valid");
    second_edit
        .append_text(second_paragraph, InlineRole::TEXT, "a")
        .expect("second text is valid");
    second_edit.commit().expect("second document commits");

    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint);
    let mut layout = fixture_engine_with_budgets(8, 8 * 1024 * 1024);
    let first_snapshot = first.snapshot();
    let second_snapshot = second.snapshot();
    let first_committed = layout
        .prepare(&first_snapshot, &request)
        .expect("first committed scene prepares");
    let second_committed = layout
        .prepare(&second_snapshot, &request)
        .expect("second committed scene prepares");
    let first_line = first_committed
        .scene()
        .line(0)
        .expect("line exists")
        .bounds();
    let second_line = second_committed
        .scene()
        .line(0)
        .expect("line exists")
        .bounds();
    let first_end = *first_committed
        .scene()
        .hit_test_closest(Point::new(first_line.x1, first_line.center().y))
        .expect("first insertion point resolves")
        .position();
    let second_end = *second_committed
        .scene()
        .hit_test_closest(Point::new(second_line.x1, second_line.center().y))
        .expect("second insertion point resolves")
        .position();
    let first_selections = first_committed
        .scene()
        .selection_set([first_committed
            .scene()
            .collapsed_selection(&first_end)
            .expect("first caret is valid")])
        .expect("first selection set is valid");
    let second_selections = second_committed
        .scene()
        .selection_set([second_committed
            .scene()
            .collapsed_selection(&second_end)
            .expect("second caret is valid")])
        .expect("second selection set is valid");
    let mut first_session = first_committed
        .scene()
        .begin_composition(
            &first_selections,
            CompositionId::from_bytes(*b"share-compose-01"),
        )
        .expect("first composition begins")
        .into_session();
    let mut second_session = second_committed
        .scene()
        .begin_composition(
            &second_selections,
            CompositionId::from_bytes(*b"share-compose-02"),
        )
        .expect("second composition begins")
        .into_session();
    first_session
        .update(
            first_session.epoch(),
            CompositionUpdate::new("office").with_selection(6..6),
        )
        .expect("first composition updates");
    second_session
        .update(
            second_session.epoch(),
            CompositionUpdate::new("office").with_selection(6..6),
        )
        .expect("second composition updates");

    let first_output = layout
        .prepare_composition(&first_snapshot, &request, &first_session)
        .expect("first composition prepares");
    let second_output = layout
        .prepare_composition(&second_snapshot, &request, &second_session)
        .expect("second composition shares preparation");
    assert_eq!(first_output.work().shared_preparations(), 0);
    assert_eq!(second_output.work().shared_preparations(), 1);
    assert_eq!(second_output.work().shape().paragraphs(), 0);
    assert_eq!(first_output.scene().composition(), first_session.id());
    assert_eq!(second_output.scene().composition(), second_session.id());
    assert_ne!(
        first_output.scene().composition(),
        second_output.scene().composition()
    );
    assert!(first_output.scene().fragments().iter().any(|fragment| {
        fragment.sources().any(|source| {
            matches!(
                source,
                ProjectedTextSource::Composition(range)
                    if range.id() == first_session.id()
                        && range.epoch() == first_session.epoch()
            )
        })
    }));
    assert!(second_output.scene().fragments().iter().any(|fragment| {
        fragment.sources().any(|source| {
            matches!(
                source,
                ProjectedTextSource::Composition(range)
                    if range.id() == second_session.id()
                        && range.epoch() == second_session.epoch()
            )
        })
    }));
}

#[test]
fn shared_key_separates_formation_inputs_but_not_brushes_or_alignment() {
    let (_, styles, paint) = fixture_document("fixture", 1.2);
    let style = styles.default_style().clone();
    let alternate_paint =
        PaintTable::from_brushes([Brush::Solid(Color::from_rgb8(0x22, 0x88, 0xcc))]);
    let second_slot_style = ComputedInlineStyle::new(
        style.shaping().clone(),
        style.inline_flow(),
        PaintSlot::new(1),
    )
    .with_analysis(style.analysis());
    let two_slot_paint = PaintTable::from_brushes([
        Brush::Solid(Color::BLACK),
        Brush::Solid(Color::from_rgb8(0xcc, 0x44, 0x33)),
    ]);
    let blocks = [
        TextBlock::plain(DocumentId::from_bytes(*b"shared-key--0001"), "office")
            .expect("block is valid"),
        TextBlock::plain(DocumentId::from_bytes(*b"shared-key--0002"), "office")
            .expect("block is valid"),
        TextBlock::plain(DocumentId::from_bytes(*b"shared-key--0003"), "office")
            .expect("block is valid"),
        TextBlock::plain(DocumentId::from_bytes(*b"shared-key--0004"), "office")
            .expect("block is valid"),
        TextBlock::plain(DocumentId::from_bytes(*b"shared-key--0005"), "office")
            .expect("block is valid"),
    ];
    let mut layout = fixture_engine_with_budgets(8, 8 * 1024 * 1024);
    let max = BlockRequest::new(TextConstraint::MaxContent, &style, &paint);
    layout
        .prepare_block(&blocks[0].snapshot(), &max)
        .expect("seed block prepares");

    let brush_only = BlockRequest::new(TextConstraint::MaxContent, &style, &alternate_paint);
    let brush_output = layout
        .prepare_block(&blocks[1].snapshot(), &brush_only)
        .expect("brush-only consumer prepares");
    assert_eq!(brush_output.work().shared_preparations(), 1);

    let centered = max.with_paragraph_style(
        ParagraphStyle::DEFAULT.with_alignment(underwood::TextAlignment::Center),
    );
    let centered_output = layout
        .prepare_block(&blocks[2].snapshot(), &centered)
        .expect("alignment-only consumer prepares");
    assert_eq!(centered_output.work().shared_preparations(), 1);

    let different_slot = BlockRequest::new(
        TextConstraint::MaxContent,
        &second_slot_style,
        &two_slot_paint,
    );
    let slot_output = layout
        .prepare_block(&blocks[3].snapshot(), &different_slot)
        .expect("different paint coverage prepares");
    assert_eq!(slot_output.work().shared_preparations(), 0);
    assert_eq!(slot_output.work().shape().paragraphs(), 1);

    let wrapped = BlockRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(200.0).expect("width is valid")),
        &style,
        &paint,
    );
    let width_output = layout
        .prepare_block(&blocks[4].snapshot(), &wrapped)
        .expect("different width prepares");
    assert_eq!(width_output.work().shared_preparations(), 0);
    assert_eq!(width_output.work().flow().paragraphs(), 1);

    let diagnostics = layout.cache_diagnostics();
    assert_eq!(diagnostics.shared_preparation_hits(), 2);
    assert_eq!(diagnostics.shared_preparation_misses(), 3);
    assert_eq!(diagnostics.shared_preparation_entries(), 3);
}

#[test]
fn every_preparation_input_partition_invalidates_cross_identity_reuse() {
    let (_, styles, _) = fixture_document("fixture", 1.2);
    let base = styles.default_style().clone();
    let language = base.clone().with_shaping(
        base.shaping()
            .clone()
            .with_language(Some(Language::parse("en").expect("language is valid"))),
    );
    let word_break = base
        .clone()
        .with_analysis(AnalysisStyle::new(WordBreak::BreakAll));
    let weight = base.clone().with_shaping(
        base.shaping()
            .clone()
            .with_font_weight(FontWeight::new(700.0))
            .expect("font weight is valid"),
    );
    let spacing = base.clone().with_inline_flow(
        base.inline_flow()
            .with_spacing(TextSpacing::new(1.0, 2.0).expect("spacing is valid")),
    );
    let line_height = base.clone().with_inline_flow(InlineFlowStyle::new(
        LineHeight::absolute(28.0).expect("line height is valid"),
    ));
    let overflow = base.clone().with_inline_flow(
        base.inline_flow()
            .with_overflow_wrap(OverflowWrap::Anywhere),
    );
    let no_wrap = base
        .clone()
        .with_inline_flow(base.inline_flow().with_text_wrap_mode(TextWrapMode::NoWrap));
    let max = TextConstraint::MaxContent;
    let wrapped = TextConstraint::Wrap(FiniteWidth::new(160.0).expect("width is valid"));
    let auto = ParagraphStyle::DEFAULT;
    let rtl = ParagraphStyle::new(BaseDirection::Rtl);

    for (name, work) in [
        (
            "language",
            cross_identity_second_work(&base, &language, auto, auto, max, max, None, None),
        ),
        (
            "word break",
            cross_identity_second_work(&base, &word_break, auto, auto, max, max, None, None),
        ),
        (
            "shaping",
            cross_identity_second_work(&base, &weight, auto, auto, max, max, None, None),
        ),
        (
            "spacing",
            cross_identity_second_work(&base, &spacing, auto, auto, max, max, None, None),
        ),
        (
            "line height",
            cross_identity_second_work(&base, &line_height, auto, auto, max, max, None, None),
        ),
        (
            "overflow wrap",
            cross_identity_second_work(&base, &overflow, auto, auto, max, max, None, None),
        ),
        (
            "wrap mode",
            cross_identity_second_work(&base, &no_wrap, auto, auto, max, max, None, None),
        ),
        (
            "base direction",
            cross_identity_second_work(&base, &base, auto, rtl, max, max, None, None),
        ),
        (
            "constraint",
            cross_identity_second_work(&base, &base, auto, auto, max, wrapped, None, None),
        ),
    ] {
        assert_eq!(
            work.shared_preparations(),
            0,
            "{name} must be an exact shared-preparation key input"
        );
        assert_eq!(
            work.shape().paragraphs(),
            1,
            "{name} must call the backend for a new paragraph identity"
        );
    }

    let first_flow =
        RegionFlow::rectangle(Rect::new(0.0, 0.0, 180.0, 200.0)).expect("first flow is valid");
    let second_flow =
        RegionFlow::rectangle(Rect::new(20.0, 0.0, 200.0, 200.0)).expect("second flow is valid");
    let work = cross_identity_second_work(
        &base,
        &base,
        auto,
        auto,
        wrapped,
        wrapped,
        Some(&first_flow),
        Some(&second_flow),
    );
    assert_eq!(work.shared_preparations(), 0);
    assert_eq!(work.shape().paragraphs(), 1);
}

#[test]
fn shared_region_transcripts_rebind_the_consuming_paragraph() {
    let (_, styles, paint) = fixture_document("fixture", 1.2);
    let style = styles.default_style().clone();
    let first = TextBlock::plain(
        DocumentId::from_bytes(*b"shared-flow--001"),
        "alpha beta gamma",
    )
    .expect("first block is valid");
    let second = TextBlock::plain(
        DocumentId::from_bytes(*b"shared-flow--002"),
        "alpha beta gamma",
    )
    .expect("second block is valid");
    let flow =
        RegionFlow::rectangle(Rect::new(25.0, 40.0, 145.0, 180.0)).expect("flow region is valid");
    let request =
        BlockRequest::new(TextConstraint::MaxContent, &style, &paint).with_region_flow(&flow);
    let mut layout = fixture_engine_with_budgets(8, 8 * 1024 * 1024);

    let first_output = layout
        .prepare_block(&first.snapshot(), &request)
        .expect("first region block prepares");
    let second_output = layout
        .prepare_block(&second.snapshot(), &request)
        .expect("second region block shares preparation");
    assert_eq!(second_output.work().shared_preparations(), 1);
    let first_transcript = first_output
        .region_transcript()
        .expect("first transcript exists");
    let second_transcript = second_output
        .region_transcript()
        .expect("second transcript exists");
    let first_attempts = first_transcript.attempts();
    let second_attempts = second_transcript.attempts();
    assert_eq!(first_attempts.len(), second_attempts.len());
    assert!(
        first_attempts.zip(second_attempts).all(|(first, second)| {
            first.paragraph() != second.paragraph()
                && first.source() == second.source()
                && first.slot() == second.slot()
                && first.outcome() == second.outcome()
        }),
        "shared attempt facts must be rebound to the consuming paragraph"
    );
}

#[test]
fn shared_preparation_budget_is_byte_bounded_lru_and_oversized_safe() {
    let (_, styles, paint) = fixture_document("fixture", 1.2);
    let style = styles.default_style().clone();
    let request = BlockRequest::new(TextConstraint::MaxContent, &style, &paint);
    let probe = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--001"), "aaaa")
        .expect("probe block is valid");
    let mut probe_layout = fixture_engine_with_budgets(2, 1024 * 1024);
    probe_layout
        .prepare_block(&probe.snapshot(), &request)
        .expect("probe preparation succeeds");
    let one_entry_bytes = probe_layout
        .cache_diagnostics()
        .shared_preparation_resident_bytes();
    assert!(one_entry_bytes > 1, "every shared entry has a fixed charge");

    let first = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--002"), "aaaa")
        .expect("first block is valid");
    let second = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--003"), "bbbb")
        .expect("second block is valid");
    let first_again = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--004"), "aaaa")
        .expect("repeated block is valid");
    let third = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--005"), "cccc")
        .expect("third block is valid");
    let second_again = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--006"), "bbbb")
        .expect("second repeated block is valid");
    let mut layout = fixture_engine_with_budgets(8, one_entry_bytes.saturating_mul(2));

    layout
        .prepare_block(&first.snapshot(), &request)
        .expect("first key prepares");
    layout
        .prepare_block(&second.snapshot(), &request)
        .expect("second key prepares");
    let touched = layout
        .prepare_block(&first_again.snapshot(), &request)
        .expect("first key is touched through shared lookup");
    assert_eq!(touched.work().shared_preparations(), 1);
    layout
        .prepare_block(&third.snapshot(), &request)
        .expect("third key evicts the least recently used key");
    let after_eviction = layout.cache_diagnostics();
    assert_eq!(after_eviction.shared_preparation_entries(), 2);
    assert_eq!(after_eviction.shared_preparation_evictions(), 1);
    assert!(
        after_eviction.shared_preparation_resident_bytes()
            <= after_eviction.shared_preparation_budget()
    );
    let evicted = layout
        .prepare_block(&second_again.snapshot(), &request)
        .expect("evicted key prepares again");
    assert_eq!(
        evicted.work().shared_preparations(),
        0,
        "the untouched second key must be the first LRU victim"
    );
    assert_eq!(evicted.work().shape().paragraphs(), 1);

    let oversized_first = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--007"), "aaaa")
        .expect("oversized first block is valid");
    let oversized_second = TextBlock::plain(DocumentId::from_bytes(*b"shared-size--008"), "aaaa")
        .expect("oversized second block is valid");
    let mut oversized = fixture_engine_with_budgets(4, one_entry_bytes.saturating_sub(1));
    oversized
        .prepare_block(&oversized_first.snapshot(), &request)
        .expect("an oversized value is still served");
    let repeated = oversized
        .prepare_block(&oversized_second.snapshot(), &request)
        .expect("an unretained oversized value prepares again");
    assert_eq!(repeated.work().shared_preparations(), 0);
    assert_eq!(repeated.work().shape().paragraphs(), 1);
    let diagnostics = oversized.cache_diagnostics();
    assert_eq!(diagnostics.shared_preparation_entries(), 0);
    assert_eq!(diagnostics.shared_preparation_oversized_non_retentions(), 2);
    assert_eq!(diagnostics.shared_preparation_resident_bytes(), 0);

    let mut disabled = fixture_engine_with_budgets(4, 0);
    disabled
        .prepare_block(&oversized_first.snapshot(), &request)
        .expect("zero shared budget still serves output");
    disabled
        .prepare_block(&oversized_second.snapshot(), &request)
        .expect("zero shared budget remains disabled");
    let diagnostics = disabled.cache_diagnostics();
    assert_eq!(diagnostics.shared_preparation_entries(), 0);
    assert_eq!(diagnostics.shared_preparation_hits(), 0);
    assert_eq!(diagnostics.shared_preparation_misses(), 0);
}

#[test]
fn failed_first_preparation_releases_untracked_backend_state() {
    let (document, styles, paint) = fixture_document("中文", 1.2);
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid")
    ])
    .expect("fixture catalog is valid");
    let mut engine = LayoutEngine::new(ParleyParagraphEngine::new(fonts), CacheBudget::new(32));
    engine
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect_err("a catalog without Han coverage must fail");

    let cache = engine.cache_diagnostics();
    assert_eq!(
        cache.current_entries(),
        0,
        "failed preparation must not create geometry residency"
    );
    assert_eq!(
        cache.backend_entries(),
        Some(0),
        "failed preparation must not strand untracked Parley preparation"
    );
}

fn cross_identity_second_work(
    first_style: &ComputedInlineStyle,
    second_style: &ComputedInlineStyle,
    first_paragraph: ParagraphStyle,
    second_paragraph: ParagraphStyle,
    first_constraint: TextConstraint,
    second_constraint: TextConstraint,
    first_flow: Option<&RegionFlow>,
    second_flow: Option<&RegionFlow>,
) -> underwood::WorkReport {
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let first = TextBlock::plain(DocumentId::from_bytes(*b"shared-input-001"), "alpha beta")
        .expect("first input block is valid");
    let second = TextBlock::plain(DocumentId::from_bytes(*b"shared-input-002"), "alpha beta")
        .expect("second input block is valid");
    let mut first_request = BlockRequest::new(first_constraint, first_style, &paint)
        .with_paragraph_style(first_paragraph);
    if let Some(flow) = first_flow {
        first_request = first_request.with_region_flow(flow);
    }
    let mut second_request = BlockRequest::new(second_constraint, second_style, &paint)
        .with_paragraph_style(second_paragraph);
    if let Some(flow) = second_flow {
        second_request = second_request.with_region_flow(flow);
    }
    let mut layout = fixture_engine_with_budgets(4, 8 * 1024 * 1024);
    layout
        .prepare_block(&first.snapshot(), &first_request)
        .expect("first input prepares");
    layout
        .prepare_block(&second.snapshot(), &second_request)
        .expect("second input prepares")
        .work()
        .clone()
}
