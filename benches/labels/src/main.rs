// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Label-scale measurements of Underwood's real public retained text path.

use std::hint::black_box;
use std::time::{Duration, Instant};

use underwood::{
    BlockRequest, Brush, CacheBudget, Color, ComputedInlineStyle, DocumentId, FiniteWidth,
    InlineFlowStyle, LayoutEngine, PaintSlot, PaintTable, SceneOutput, ShapingStyle, TextBlock,
    TextConstraint,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine};

const LABELS: usize = 2_048;
const CHURN_BUDGET: usize = 64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 15.0)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let paint = PaintTable::from_brushes([Brush::Solid(Color::from_rgb8(0x20, 0x24, 0x2b))]);
    let narrow = FiniteWidth::new(96.0)?;
    let mut labels = unique_labels()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(LABELS),
    );

    let cold_unique = measure(|| {
        for label in &labels {
            let output = layout
                .prepare_block(
                    &label.snapshot(),
                    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
                )
                .expect("cold unique label must prepare");
            assert_eq!(
                output.work().shape().paragraphs(),
                1,
                "a cold stable identity must shape exactly one paragraph"
            );
            black_box(output.scene().metrics());
        }
    });
    assert_residency(&layout, LABELS);

    let retained_unique = measure(|| {
        for label in &labels {
            let output = layout
                .prepare_block(
                    &label.snapshot(),
                    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
                )
                .expect("retained unique label must prepare");
            assert_no_physics(&output);
            black_box(output.scene().fragments().len());
        }
    });

    let constrained_unique = measure(|| {
        for label in &labels {
            let output = layout
                .prepare_block(
                    &label.snapshot(),
                    &BlockRequest::new(TextConstraint::Wrap(narrow), &style, &paint),
                )
                .expect("constrained unique label must prepare");
            assert_eq!(
                output.work().analysis().paragraphs(),
                0,
                "constraint changes must reuse analysis"
            );
            assert_eq!(
                output.work().itemization().paragraphs(),
                0,
                "constraint changes must reuse itemization"
            );
            assert_eq!(
                output.work().font_selection().paragraphs(),
                0,
                "constraint changes must reuse font selection"
            );
            assert_eq!(
                output.work().shape().paragraphs(),
                0,
                "constraint changes must reuse shaping"
            );
            assert_eq!(
                output.work().flow().paragraphs(),
                1,
                "constraint changes must reform exactly one paragraph"
            );
            black_box(output.scene().lines().len());
        }
    });

    let edited = &mut labels[LABELS / 2];
    edited.set_text("Open the retained workspace")?;
    let localized_edit = measure(|| {
        let output = layout
            .prepare_block(
                &edited.snapshot(),
                &BlockRequest::new(TextConstraint::Wrap(narrow), &style, &paint),
            )
            .expect("localized edit must prepare");
        assert_eq!(
            output.work().analysis().paragraphs(),
            1,
            "one edited block must analyze exactly one paragraph"
        );
        assert_eq!(
            output.work().shape().paragraphs(),
            1,
            "one edited block must shape exactly one paragraph"
        );
        black_box(output.scene().metrics());
    });

    let sample = &labels[1];
    let min = layout.prepare_block(
        &sample.snapshot(),
        &BlockRequest::new(TextConstraint::MinContent, &style, &paint),
    )?;
    let max = layout.prepare_block(
        &sample.snapshot(),
        &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
    )?;
    assert!(
        min.scene().metrics().size().width <= max.scene().metrics().size().width,
        "min-content width cannot exceed max-content width"
    );
    assert!(
        min.scene().metrics().size().height >= max.scene().metrics().size().height,
        "taking every legal break cannot reduce block height"
    );
    assert!(
        max.scene().metrics().first_baseline().is_some(),
        "non-empty max-content text must expose a first baseline"
    );
    assert!(
        max.scene().metrics().last_baseline().is_some(),
        "non-empty max-content text must expose a last baseline"
    );

    let release = measure(|| {
        for label in &labels {
            layout.release_document(label.id());
        }
    });
    assert_residency(&layout, 0);
    assert_eq!(
        layout.cache_diagnostics().releases(),
        LABELS,
        "every explicitly discarded block must record one geometry release"
    );

    let mut identical = identical_labels()?;
    let mut identical_layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(LABELS),
    );
    let cold_identical = measure(|| {
        for label in &identical {
            let output = identical_layout
                .prepare_block(
                    &label.snapshot(),
                    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
                )
                .expect("identity-local repeated text must prepare");
            assert_eq!(
                output.work().shape().paragraphs(),
                1,
                "distinct identities do not imply a cross-paragraph shaping cache"
            );
            black_box(output.scene().metrics());
        }
    });
    let retained_identical = measure(|| {
        for label in &identical {
            let output = identical_layout
                .prepare_block(
                    &label.snapshot(),
                    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
                )
                .expect("retained repeated text must prepare");
            assert_no_physics(&output);
        }
    });
    for label in &identical {
        identical_layout.release_document(label.id());
    }
    identical.clear();
    assert_residency(&identical_layout, 0);

    let mut churn_layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(CHURN_BUDGET),
    );
    let churn = measure(|| {
        for index in 0..LABELS {
            let label = TextBlock::plain(
                identity(3, index),
                if index & 1 == 0 {
                    "Transient label"
                } else {
                    "مرحبا transient label"
                },
            )
            .expect("churn block must initialize");
            let output = churn_layout
                .prepare_block(
                    &label.snapshot(),
                    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
                )
                .expect("churn label must prepare");
            assert_eq!(
                output.work().shape().paragraphs(),
                1,
                "each new churn identity must shape once"
            );
            assert!(
                churn_layout.cache_diagnostics().current_entries() <= CHURN_BUDGET,
                "geometry cache must remain bounded during churn"
            );
            assert!(
                churn_layout
                    .cache_diagnostics()
                    .backend_entries()
                    .is_some_and(|entries| entries <= CHURN_BUDGET),
                "backend cache must remain coordinated with geometry eviction"
            );
            black_box(output.scene().fragments().len());
        }
    });
    let churn_cache = churn_layout.cache_diagnostics();
    assert_eq!(
        churn_cache.current_entries(),
        CHURN_BUDGET,
        "geometry residency must settle at the configured budget"
    );
    assert_eq!(
        churn_cache.backend_entries(),
        Some(CHURN_BUDGET),
        "backend residency must match retained geometry"
    );
    assert_eq!(
        churn_cache.evictions(),
        LABELS - CHURN_BUDGET,
        "every identity beyond the budget must be evicted"
    );
    assert_eq!(
        churn_cache.peak_entries(),
        CHURN_BUDGET + 1,
        "one newly materializing entry is the only allowed transient excess"
    );
    churn_layout.clear_cache();
    assert_residency(&churn_layout, 0);

    report("text_block_cold_unique", LABELS, cold_unique);
    report("text_block_retained_unique", LABELS, retained_unique);
    report("text_block_constrained_unique", LABELS, constrained_unique);
    report("text_block_localized_edit", 1, localized_edit);
    report("text_block_explicit_release", LABELS, release);
    report("text_block_cold_identical", LABELS, cold_identical);
    report("text_block_retained_identical", LABELS, retained_identical);
    report("text_block_budget_churn", LABELS, churn);
    println!(
        "cache_proof\tbudget={CHURN_BUDGET}\tevictions={}\tpeak={}\tfinal_geometry=0\tfinal_backend=0",
        churn_cache.evictions(),
        churn_cache.peak_entries()
    );
    Ok(())
}

fn unique_labels() -> Result<Vec<TextBlock>, Box<dyn std::error::Error>> {
    (0..LABELS)
        .map(|index| {
            TextBlock::plain(
                identity(1, index),
                if index & 1 == 0 {
                    "Save"
                } else {
                    "Open retained document"
                },
            )
            .map_err(Into::into)
        })
        .collect()
}

fn identical_labels() -> Result<Vec<TextBlock>, Box<dyn std::error::Error>> {
    (0..LABELS)
        .map(|index| TextBlock::plain(identity(2, index), "Save changes").map_err(Into::into))
        .collect()
}

fn identity(namespace: u64, index: usize) -> DocumentId {
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&namespace.to_le_bytes());
    bytes[8..].copy_from_slice(&(index as u64).to_le_bytes());
    DocumentId::from_bytes(bytes)
}

fn fonts() -> Result<FontSet, Box<dyn std::error::Error>> {
    Ok(FontSet::try_from_fonts([
        Font::from_bytes(
            "latin",
            include_bytes!("../../../examples/headless/fonts/RobotoFlex-VariableFont.ttf"),
        )?,
        Font::from_bytes(
            "arabic",
            include_bytes!("../../../examples/headless/fonts/NotoKufiArabic-Regular.otf"),
        )?,
    ])?
    .with_fallbacks(
        underwood::Script::from_bytes(*b"Arab"),
        None,
        ["Noto Kufi Arabic"],
    )?)
}

fn assert_no_physics(output: &SceneOutput) {
    assert_eq!(
        output.work().analysis().paragraphs(),
        0,
        "retained text must reuse analysis"
    );
    assert_eq!(
        output.work().itemization().paragraphs(),
        0,
        "retained text must reuse itemization"
    );
    assert_eq!(
        output.work().font_selection().paragraphs(),
        0,
        "retained text must reuse font selection"
    );
    assert_eq!(
        output.work().shape().paragraphs(),
        0,
        "retained text must reuse shaping"
    );
    assert_eq!(
        output.work().flow().paragraphs(),
        0,
        "retained text must reuse formation"
    );
    assert_eq!(
        output.work().geometry().paragraphs(),
        0,
        "retained text must reuse geometry"
    );
}

fn assert_residency(layout: &LayoutEngine, expected: usize) {
    let cache = layout.cache_diagnostics();
    assert_eq!(
        cache.current_entries(),
        expected,
        "geometry residency must match the expected lifecycle state"
    );
    assert_eq!(
        cache.backend_entries(),
        Some(expected),
        "backend residency must remain coordinated with geometry"
    );
}

fn measure(operation: impl FnOnce()) -> Duration {
    let start = Instant::now();
    operation();
    start.elapsed()
}

fn report(name: &str, operations: usize, elapsed: Duration) {
    println!(
        "{name}\tprofile=wind-tunnel\tmachine=local\toperations={operations}\ttotal_ns={}\tns_per_operation={}",
        elapsed.as_nanos(),
        elapsed.as_nanos() / operations as u128
    );
}
