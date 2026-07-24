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
        max.scene().lines()[0].break_reason(),
        TestLineBreakReason::Mandatory
    );
    assert_eq!(
        max.scene().lines()[1].break_reason(),
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
        Some(max.scene().lines()[0].baseline())
    );
    assert_eq!(
        max.scene().metrics().last_baseline(),
        Some(max.scene().lines()[1].baseline())
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

    let retained = engine
        .prepare_block(
            &blocks[1].snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
        )
        .expect("resident block prepares");
    assert_eq!(retained.work().analysis().paragraphs(), 0);
    assert_eq!(retained.work().shape().paragraphs(), 0);
    assert_eq!(retained.work().flow().paragraphs(), 0);
    assert_eq!(engine.cache_diagnostics().hits(), 1);

    engine.release_document(blocks[1].id());
    let after_release = engine.cache_diagnostics();
    assert_eq!(after_release.current_entries(), 1);
    assert_eq!(after_release.backend_entries(), Some(1));
    assert_eq!(after_release.releases(), 1);

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
    assert_eq!(zero.cache_diagnostics().evictions(), 1);
}

#[test]
fn failed_first_preparation_releases_untracked_backend_physics() {
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
        "failed preparation must not strand untracked Parley physics"
    );
}
