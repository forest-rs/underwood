// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Label-scale measurements of Underwood's real public retained text path.

use std::hint::black_box;
use std::time::{Duration, Instant};

use underwood::{
    BlockRequest, Brush, CacheBudget, Color, ComputedInlineStyle, DocumentId, FiniteWidth,
    FloatSide, FlowRegion, InlineFlowStyle, LayoutEngine, PaintSlot, PaintTable, ParagraphStyle,
    ProjectedText, ProjectionBuilder, Rect, RegionFloat, RegionFlow, SceneOutput, ShapingStyle,
    Size, TextAlignment, TextBlock, TextConstraint, WhitespaceCollapse,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine};

const LABELS: usize = 2_048;
const CHURN_BUDGET: usize = 64;
const SHARED_PREPARATION_BYTES: usize = 8 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let Some(scenario) = arguments.next() else {
        return run_suite();
    };
    if scenario == "--help" || scenario == "-h" {
        println!(
            "usage: underwood_label_benchmark [setup-identical|setup-identity|setup-cross-identical|setup-cross-distinct|setup-shared-hit|primed-identical|primed-paint|primed-unique|primed-region|primed-adjustment|cold-identical|cross-identical|cross-distinct|shared-hit|retained-identical|retained-adjustment|paint-change|alignment-churn|justification-churn|localized-edit|interaction-materialization|width-churn|region-ready|region-churn|identity-churn|projection-identity-setup|projection-identity|projection-collapse-setup|projection-collapse|projection-expansion-setup|projection-expansion] [rounds] [labels]"
        );
        return Ok(());
    }
    let rounds = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(100);
    if rounds == 0 {
        return Err("rounds must be greater than zero".into());
    }
    let labels = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(LABELS);
    if labels == 0 {
        return Err("labels must be greater than zero".into());
    }
    if arguments.next().is_some() {
        return Err("expected at most a scenario, round count, and label count".into());
    }
    let result = run_profile(&scenario, rounds, labels);
    hold_for_profiler()?;
    result
}

fn run_suite() -> Result<(), Box<dyn std::error::Error>> {
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

    let mut constrained_line_reshapes = 0_usize;
    let mut constrained_line_paragraphs = 0_usize;
    let mut constrained_line_candidates = 0_usize;
    let mut constrained_rejected_candidates = 0_usize;
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
                "constraint changes must reuse canonical shaping"
            );
            assert_eq!(
                output.work().flow().paragraphs(),
                1,
                "constraint changes must reform exactly one paragraph"
            );
            constrained_line_reshapes =
                constrained_line_reshapes.saturating_add(output.work().line_reshapes());
            constrained_line_paragraphs =
                constrained_line_paragraphs.saturating_add(output.work().line_shape().paragraphs());
            constrained_line_candidates =
                constrained_line_candidates.saturating_add(output.work().line_candidates());
            constrained_rejected_candidates = constrained_rejected_candidates
                .saturating_add(output.work().rejected_line_candidates());
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
    println!(
        "text_block_constrained_line_work\tparagraphs={constrained_line_paragraphs}\tline_reshapes={constrained_line_reshapes}\tcandidates={constrained_line_candidates}\trejected_candidates={constrained_rejected_candidates}"
    );
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

fn run_profile(
    scenario: &str,
    rounds: usize,
    labels: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 15.0)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let paint = PaintTable::from_brushes([Brush::Solid(Color::from_rgb8(0x20, 0x24, 0x2b))]);
    match scenario {
        "setup-identical" | "s0" => profile_setup_identical(rounds, labels),
        "setup-identity" | "s1" => profile_setup_identity(rounds, labels),
        "setup-cross-identical" | "x0" => {
            profile_setup_cross_identity("setup-cross-identical", rounds, labels, false)
        }
        "setup-cross-distinct" | "x2" => {
            profile_setup_cross_identity("setup-cross-distinct", rounds, labels, true)
        }
        "setup-shared-hit" | "y0" => profile_setup_shared_hit(rounds, labels, &style, &paint),
        "primed-identical" | "p0" => {
            profile_primed_identical("primed-identical", rounds, labels, &style, &paint, false)
        }
        "primed-paint" | "p1" => {
            profile_primed_identical("primed-paint", rounds, labels, &style, &paint, true)
        }
        "primed-unique" | "p2" => profile_primed_unique(rounds, labels, &style, &paint),
        "primed-region" | "p3" => profile_primed_region(rounds, labels, &style, &paint),
        "primed-adjustment" | "p4" => profile_primed_adjustment(rounds, labels, &style, &paint),
        "cold-identical" | "c0" => {
            profile_cold_identical("cold-identical", rounds, labels, &style, &paint)
        }
        "cross-identical" | "x1" => {
            profile_cross_identity("cross-identical", rounds, labels, &style, &paint, false)
        }
        "cross-distinct" | "x3" => {
            profile_cross_identity("cross-distinct", rounds, labels, &style, &paint, true)
        }
        "shared-hit" | "y1" => profile_shared_hit(rounds, labels, &style, &paint),
        "retained-identical" | "r0" => profile_retained_identical(rounds, labels, &style, &paint),
        "retained-adjustment" | "r1" => profile_retained_adjustment(rounds, labels, &style, &paint),
        "paint-change" | "a0" => profile_paint_change(rounds, labels, &style, &paint),
        "alignment-churn" | "a1" => profile_alignment_churn(rounds, labels, &style, &paint, false),
        "justification-churn" | "a2" => {
            profile_alignment_churn(rounds, labels, &style, &paint, true)
        }
        "localized-edit" | "e0" => profile_localized_edit(rounds, labels, &style, &paint),
        "interaction-materialization" | "i0" => profile_cold_identical(
            "interaction-materialization",
            rounds,
            labels,
            &style,
            &paint,
        ),
        "width-churn" | "w0" => profile_width_churn("width-churn", rounds, labels, &style, &paint),
        "region-ready" | "g0" => {
            profile_width_churn("region-ready", rounds, labels, &style, &paint)
        }
        "region-churn" | "g1" => profile_region_churn(rounds, labels, &style, &paint),
        "identity-churn" | "h0" => profile_identity_churn(rounds, labels, &style, &paint),
        "projection-identity-setup" | "q0" => {
            profile_projection_setup("projection-identity-setup", rounds, labels, "stable label")
        }
        "projection-identity" | "q1" => profile_projection_identity(rounds, labels),
        "projection-collapse-setup" | "q2" => {
            profile_projection_setup("projection-collapse-setup", rounds, labels, " \t\r\n")
        }
        "projection-collapse" | "q3" => profile_projection_collapse(rounds, labels),
        "projection-expansion-setup" | "q4" => {
            profile_projection_setup("projection-expansion-setup", rounds, labels, "İ")
        }
        "projection-expansion" | "q5" => profile_projection_expansion(rounds, labels),
        _ => Err(format!("unknown scenario: {scenario}").into()),
    }
}

fn profile_projection_setup(
    name: &str,
    rounds: usize,
    label_count: usize,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let operations = rounds
        .checked_mul(label_count)
        .ok_or("projection operation count overflowed")?;
    let sources = vec![source.to_string(); operations];
    black_box(sources);
    report_profile(name, rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_projection_identity(
    rounds: usize,
    label_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let operations = rounds
        .checked_mul(label_count)
        .ok_or("projection operation count overflowed")?;
    let sources = vec!["stable label".to_string(); operations];
    let elapsed = measure(|| {
        for source in sources {
            let projection = ProjectedText::identity(source).expect("identity source is valid");
            assert!(
                projection.is_identity(),
                "identity scenario must not materialize presentation text"
            );
            assert_eq!(
                projection.segments().len(),
                1,
                "identity scenario must store one relation run"
            );
            black_box(projection);
        }
    });
    report_profile("projection-identity", rounds, label_count, elapsed);
    Ok(())
}

fn profile_projection_collapse(
    rounds: usize,
    label_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let operations = rounds
        .checked_mul(label_count)
        .ok_or("projection operation count overflowed")?;
    let sources = vec![" \t\r\n".to_string(); operations];
    let elapsed = measure(|| {
        for source in sources {
            let projection = ProjectedText::from_whitespace(source, WhitespaceCollapse::Collapse)
                .expect("collapse source is valid");
            assert_eq!(
                projection.text(),
                " ",
                "dense whitespace must collapse to one space"
            );
            assert_eq!(
                projection.segments().len(),
                1,
                "dense whitespace must store one collapsed run"
            );
            black_box(projection);
        }
    });
    report_profile("projection-collapse", rounds, label_count, elapsed);
    Ok(())
}

fn profile_projection_expansion(
    rounds: usize,
    label_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let operations = rounds
        .checked_mul(label_count)
        .ok_or("projection operation count overflowed")?;
    let sources = vec!["İ".to_string(); operations];
    let elapsed = measure(|| {
        for source in sources {
            let mut builder = ProjectionBuilder::new(source).expect("expansion source is valid");
            builder
                .push_replacement(2, "i\u{307}")
                .expect("one-to-many expansion is valid");
            let projection = builder.finish().expect("expansion source is covered");
            assert_eq!(
                projection.text(),
                "i\u{307}",
                "expansion must retain the requested presentation scalars"
            );
            assert_eq!(
                projection.segments().len(),
                1,
                "one expansion must store one replacement run"
            );
            black_box(projection);
        }
    });
    report_profile("projection-expansion", rounds, label_count, elapsed);
    Ok(())
}

fn profile_setup_identical(
    rounds: usize,
    label_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = identical_labels_with_count(label_count)?;
    let layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    black_box((&labels, &layout));
    report_profile("setup-identical", rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_setup_identity(
    rounds: usize,
    label_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(CHURN_BUDGET),
    );
    black_box(&layout);
    report_profile("setup-identity", rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_setup_cross_identity(
    name: &str,
    rounds: usize,
    label_count: usize,
    distinct_text: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = if distinct_text {
        distinct_labels_with_count(label_count)?
    } else {
        identical_labels_with_count(label_count)?
    };
    let layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count).with_shared_preparation_bytes(SHARED_PREPARATION_BYTES),
    );
    black_box((&labels, &layout));
    report_profile(name, rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_setup_shared_hit(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    if label_count < 2 {
        return Err("shared-hit scenarios require at least two labels".into());
    }
    let labels = identical_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count).with_shared_preparation_bytes(SHARED_PREPARATION_BYTES),
    );
    for _ in 0..rounds {
        layout.clear_cache();
        layout.prepare_block(
            &labels[0].snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    black_box((&labels, &layout));
    report_profile("setup-shared-hit", rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_shared_hit(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    if label_count < 2 {
        return Err("shared-hit scenarios require at least two labels".into());
    }
    let labels = identical_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count).with_shared_preparation_bytes(SHARED_PREPARATION_BYTES),
    );
    let mut elapsed = Duration::ZERO;
    for _ in 0..rounds {
        layout.clear_cache();
        let seed = layout.prepare_block(
            &labels[0].snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
        assert_eq!(
            seed.work().shape().paragraphs(),
            1,
            "each cleared round must seed one fresh prepared value"
        );
        elapsed += measure(|| {
            for label in &labels[1..] {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint),
                    )
                    .expect("eligible identical identity must share preparation");
                assert_eq!(
                    output.work().shared_preparations(),
                    1,
                    "every non-seed identical identity must hit shared preparation"
                );
                assert_eq!(
                    output.work().shape().paragraphs(),
                    0,
                    "a shared hit must perform no canonical shaping"
                );
                assert_eq!(
                    output.work().flow().paragraphs(),
                    0,
                    "a shared hit must perform no line formation"
                );
                assert_eq!(
                    output.work().geometry().paragraphs(),
                    1,
                    "each consumer must still build its own geometry"
                );
                black_box(output.scene().metrics());
            }
        });
    }
    let operations = rounds
        .checked_mul(label_count - 1)
        .ok_or("shared-hit operation count overflowed")?;
    report("shared-hit", operations, elapsed);
    if std::env::var_os("UNDERWOOD_PROFILE_QUIET").is_none() {
        let cache = layout.cache_diagnostics();
        println!(
            "shared-hit_work\toperations={operations}\thits={}\tmisses={}\tresident_entries={}\tresident_bytes={}\tpeak_bytes={}",
            cache.shared_preparation_hits(),
            cache.shared_preparation_misses(),
            cache.shared_preparation_entries(),
            cache.shared_preparation_resident_bytes(),
            cache.shared_preparation_peak_bytes()
        );
    }
    Ok(())
}

fn profile_primed_identical(
    name: &str,
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
    include_alternate_paint: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = identical_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    let alternate_paint = include_alternate_paint
        .then(|| PaintTable::from_brushes([Brush::Solid(Color::from_rgb8(0x74, 0x48, 0xe8))]));
    black_box((&labels, &layout, alternate_paint));
    report_profile(name, rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_primed_unique(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = unique_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    black_box((&labels, &layout));
    report_profile("primed-unique", rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_primed_region(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = unique_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    let flows = region_flows()?;
    black_box((&labels, &layout, flows));
    report_profile("primed-region", rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_primed_adjustment(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = adjustment_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    let flow = adjustment_flow()?;
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint).with_region_flow(&flow),
        )?;
    }
    black_box((&labels, &layout, flow));
    report_profile("primed-adjustment", rounds, label_count, Duration::ZERO);
    Ok(())
}

fn profile_cold_identical(
    name: &str,
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = identical_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    let elapsed = measure(|| {
        for _ in 0..rounds {
            layout.clear_cache();
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint),
                    )
                    .expect("cold identical label must prepare");
                assert_eq!(
                    output.work().shape().paragraphs(),
                    1,
                    "cleared identities must shape exactly once"
                );
                black_box(output.scene().metrics());
            }
        }
    });
    report_profile(name, rounds, label_count, elapsed);
    Ok(())
}

fn profile_cross_identity(
    name: &str,
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
    distinct_text: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = if distinct_text {
        distinct_labels_with_count(label_count)?
    } else {
        identical_labels_with_count(label_count)?
    };
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count).with_shared_preparation_bytes(SHARED_PREPARATION_BYTES),
    );
    let mut analyzed = 0_usize;
    let mut shaped = 0_usize;
    let mut formed = 0_usize;
    let mut shared = 0_usize;
    let elapsed = measure(|| {
        for _ in 0..rounds {
            layout.clear_cache();
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint),
                    )
                    .expect("cross-identity label must prepare");
                analyzed += output.work().analysis().paragraphs();
                shaped += output.work().shape().paragraphs();
                formed += output.work().flow().paragraphs();
                shared += output.work().shared_preparations();
                black_box(output.scene().metrics());
            }
        }
    });
    let operations = rounds
        .checked_mul(label_count)
        .ok_or("cross-identity operation count overflowed")?;
    if distinct_text {
        assert_eq!(
            (analyzed, shaped, formed, shared),
            (operations, operations, operations, 0),
            "distinct text must not cross-reuse preparation"
        );
    } else {
        assert_eq!(
            (analyzed, shaped, formed, shared),
            (rounds, rounds, rounds, operations.saturating_sub(rounds)),
            "each cleared identical round must prepare once and share every remaining identity"
        );
    }
    report_profile(name, rounds, label_count, elapsed);
    if std::env::var_os("UNDERWOOD_PROFILE_QUIET").is_none() {
        let cache = layout.cache_diagnostics();
        println!(
            "{name}_work\toperations={operations}\tanalyzed={analyzed}\tshaped={shaped}\tformed={formed}\tshared={shared}\tresident_entries={}\tresident_bytes={}\tpeak_bytes={}",
            cache.shared_preparation_entries(),
            cache.shared_preparation_resident_bytes(),
            cache.shared_preparation_peak_bytes()
        );
    }
    Ok(())
}

fn profile_retained_identical(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = identical_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    let elapsed = measure(|| {
        for _ in 0..rounds {
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint),
                    )
                    .expect("retained identical label must prepare");
                assert_no_physics(&output);
                black_box(output.scene().metrics());
            }
        }
    });
    report_profile("retained-identical", rounds, label_count, elapsed);
    Ok(())
}

fn profile_retained_adjustment(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = adjustment_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    let flow = adjustment_flow()?;
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint).with_region_flow(&flow),
        )?;
    }
    let elapsed = measure(|| {
        for _ in 0..rounds {
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint)
                            .with_region_flow(&flow),
                    )
                    .expect("retained adjustment fixture must prepare");
                assert_no_physics(&output);
                black_box(output.scene().metrics());
            }
        }
    });
    report_profile("retained-adjustment", rounds, label_count, elapsed);
    Ok(())
}

fn profile_paint_change(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    original_paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = identical_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, original_paint),
        )?;
    }
    let alternate_paint =
        PaintTable::from_brushes([Brush::Solid(Color::from_rgb8(0x74, 0x48, 0xe8))]);
    let elapsed = measure(|| {
        for round in 0..rounds {
            let paint = if round.is_multiple_of(2) {
                &alternate_paint
            } else {
                original_paint
            };
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint),
                    )
                    .expect("paint-only label must prepare");
                assert_no_physics(&output);
                black_box(output.scene().fragments().len());
            }
        }
    });
    report_profile("paint-change", rounds, label_count, elapsed);
    Ok(())
}

fn profile_alignment_churn(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
    justify: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = adjustment_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    let flow = adjustment_flow()?;
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint).with_region_flow(&flow),
        )?;
    }
    let elapsed = measure(|| {
        for round in 0..rounds {
            let alignment = if justify {
                if round.is_multiple_of(2) {
                    TextAlignment::Justify
                } else {
                    TextAlignment::Start
                }
            } else if round.is_multiple_of(2) {
                TextAlignment::Center
            } else {
                TextAlignment::End
            };
            let paragraph_style = ParagraphStyle::DEFAULT.with_alignment(alignment);
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint)
                            .with_paragraph_style(paragraph_style)
                            .with_region_flow(&flow),
                    )
                    .expect("adjustment-only label must prepare");
                assert_adjustment_only(&output);
                if alignment == TextAlignment::Justify {
                    assert!(
                        output.scene().lines()[0]
                            .adjustment()
                            .opportunity_expansion()
                            > 0.0,
                        "the wind tunnel must execute real Western justification"
                    );
                }
                black_box(output.scene().metrics());
            }
        }
    });
    report_profile(
        if justify {
            "justification-churn"
        } else {
            "alignment-churn"
        },
        rounds,
        label_count,
        elapsed,
    );
    Ok(())
}

fn profile_localized_edit(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut labels = identical_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    let elapsed = measure(|| {
        for round in 0..rounds {
            let text = if round.is_multiple_of(2) {
                "Save edited changes"
            } else {
                "Save changes"
            };
            for label in &mut labels {
                label.set_text(text).expect("profile edit must publish");
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint),
                    )
                    .expect("edited label must prepare");
                assert_eq!(
                    output.work().shape().paragraphs(),
                    1,
                    "each changed label must reshape exactly once"
                );
                black_box(output.scene().metrics());
            }
        }
    });
    report_profile("localized-edit", rounds, label_count, elapsed);
    Ok(())
}

fn profile_width_churn(
    name: &str,
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = unique_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    let widths = [FiniteWidth::new(96.0)?, FiniteWidth::new(132.0)?];
    let elapsed = measure(|| {
        for round in 0..rounds {
            let constraint = TextConstraint::Wrap(widths[round % widths.len()]);
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(constraint, style, paint),
                    )
                    .expect("width-churn label must prepare");
                assert_eq!(
                    output.work().shape().paragraphs(),
                    0,
                    "width churn must retain canonical shaping"
                );
                assert_eq!(
                    output.work().line_candidates(),
                    output.work().accepted_line_candidates()
                        + output.work().rejected_line_candidates(),
                    "every proposed line candidate must be accepted or visibly rejected"
                );
                black_box(output.scene().lines().len());
            }
        }
    });
    report_profile(name, rounds, label_count, elapsed);
    Ok(())
}

fn profile_region_churn(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let labels = unique_labels_with_count(label_count)?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(label_count),
    );
    for label in &labels {
        layout.prepare_block(
            &label.snapshot(),
            &BlockRequest::new(TextConstraint::MaxContent, style, paint),
        )?;
    }
    let flows = region_flows()?;
    let elapsed = measure(|| {
        for round in 0..rounds {
            let flow = &flows[round % flows.len()];
            for label in &labels {
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint)
                            .with_region_flow(flow),
                    )
                    .expect("region-churn label must prepare");
                assert_eq!(
                    output.work().analysis().paragraphs(),
                    0,
                    "region churn must retain analysis"
                );
                assert_eq!(
                    output.work().font_selection().paragraphs(),
                    0,
                    "region churn must retain selected fonts"
                );
                assert_eq!(
                    output.work().shape().paragraphs(),
                    0,
                    "region churn must retain canonical shaping"
                );
                assert_eq!(
                    output.work().flow().paragraphs(),
                    1,
                    "region churn must reform exactly one paragraph"
                );
                let transcript = output
                    .region_transcript()
                    .expect("region churn must publish a transcript");
                assert_eq!(
                    transcript
                        .replay(flow)
                        .expect("region transcript must replay"),
                    transcript.end(),
                    "replay must reach the recorded cursor"
                );
                black_box((output.scene().lines().len(), transcript.attempts().len()));
            }
        }
    });
    report_profile("region-churn", rounds, label_count, elapsed);
    Ok(())
}

fn profile_identity_churn(
    rounds: usize,
    label_count: usize,
    style: &ComputedInlineStyle,
    paint: &PaintTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts()?),
        CacheBudget::new(CHURN_BUDGET),
    );
    let elapsed = measure(|| {
        for round in 0..rounds {
            for index in 0..label_count {
                let label = TextBlock::plain(
                    identity(
                        u64::try_from(round).unwrap_or(u64::MAX).saturating_add(10),
                        index,
                    ),
                    "Transient identical label",
                )
                .expect("identity-churn block must initialize");
                let output = layout
                    .prepare_block(
                        &label.snapshot(),
                        &BlockRequest::new(TextConstraint::MaxContent, style, paint),
                    )
                    .expect("identity-churn label must prepare");
                assert_eq!(
                    output.work().shape().paragraphs(),
                    1,
                    "each new identity must shape once before shared reuse exists"
                );
                assert!(
                    layout.cache_diagnostics().current_entries() <= CHURN_BUDGET,
                    "identity churn must remain within the retained budget"
                );
                black_box(output.scene().fragments().len());
            }
        }
    });
    report_profile("identity-churn", rounds, label_count, elapsed);
    Ok(())
}

fn report_profile(name: &str, rounds: usize, labels: usize, elapsed: Duration) {
    if std::env::var_os("UNDERWOOD_PROFILE_QUIET").is_some() {
        return;
    }
    let operations = labels.saturating_mul(rounds);
    println!(
        "{name}\tprofile=isolated\tmachine=local\trounds={rounds}\tlabels={labels}\toperations={operations}\ttotal_ns={}\tns_per_operation={}",
        elapsed.as_nanos(),
        elapsed.as_nanos() / operations as u128
    );
}

fn hold_for_profiler() -> Result<(), Box<dyn std::error::Error>> {
    let Some(seconds) = std::env::var_os("UNDERWOOD_PROFILE_HOLD_SECS") else {
        return Ok(());
    };
    let seconds = seconds
        .to_str()
        .ok_or("UNDERWOOD_PROFILE_HOLD_SECS must be valid UTF-8")?
        .parse::<u64>()?;
    if std::env::var_os("UNDERWOOD_PROFILE_QUIET").is_none() {
        eprintln!("holding process for profiler: {seconds}s");
    }
    std::thread::sleep(Duration::from_secs(seconds));
    Ok(())
}

fn unique_labels() -> Result<Vec<TextBlock>, Box<dyn std::error::Error>> {
    unique_labels_with_count(LABELS)
}

fn unique_labels_with_count(count: usize) -> Result<Vec<TextBlock>, Box<dyn std::error::Error>> {
    (0..count)
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
    identical_labels_with_count(LABELS)
}

fn identical_labels_with_count(count: usize) -> Result<Vec<TextBlock>, Box<dyn std::error::Error>> {
    (0..count)
        .map(|index| TextBlock::plain(identity(2, index), "Save changes").map_err(Into::into))
        .collect()
}

fn distinct_labels_with_count(count: usize) -> Result<Vec<TextBlock>, Box<dyn std::error::Error>> {
    (0..count)
        .map(|index| {
            let text = format!("Distinct label {index:08}");
            TextBlock::plain(identity(5, index), &text).map_err(Into::into)
        })
        .collect()
}

fn adjustment_labels_with_count(
    count: usize,
) -> Result<Vec<TextBlock>, Box<dyn std::error::Error>> {
    (0..count)
        .map(|index| {
            TextBlock::plain(
                identity(4, index),
                "Save the carefully retained document changes now",
            )
            .map_err(Into::into)
        })
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

fn region_flows() -> Result<[RegionFlow; 2], Box<dyn std::error::Error>> {
    let exclusion = FlowRegion::new(Rect::new(0.0, 0.0, 132.0, 240.0))?
        .with_exclusions([Rect::new(0.0, 0.0, 28.0, 42.0)])?;
    let floated = FlowRegion::new(Rect::new(0.0, 0.0, 96.0, 120.0))?.with_floats([
        RegionFloat::new(FloatSide::Left, 0.0, Size::new(24.0, 36.0))?,
        RegionFloat::new(FloatSide::Right, 48.0, Size::new(20.0, 42.0))?,
    ])?;
    let second_column = FlowRegion::new(Rect::new(116.0, 0.0, 212.0, 160.0))?;
    Ok([
        RegionFlow::new([exclusion])?,
        RegionFlow::new([floated, second_column])?,
    ])
}

fn adjustment_flow() -> Result<RegionFlow, Box<dyn std::error::Error>> {
    Ok(RegionFlow::rectangle(Rect::new(0.0, 0.0, 132.0, 240.0))?)
}

fn assert_adjustment_only(output: &SceneOutput) {
    assert_eq!(
        output.work().analysis().paragraphs(),
        0,
        "adjustment-only work must retain analysis"
    );
    assert_eq!(
        output.work().itemization().paragraphs(),
        0,
        "adjustment-only work must retain itemization"
    );
    assert_eq!(
        output.work().font_selection().paragraphs(),
        0,
        "adjustment-only work must retain font selection"
    );
    assert_eq!(
        output.work().shape().paragraphs(),
        0,
        "adjustment-only work must retain canonical shaping"
    );
    assert_eq!(
        output.work().line_shape().paragraphs(),
        0,
        "adjustment-only work must retain accepted line shaping"
    );
    assert_eq!(
        output.work().flow().paragraphs(),
        0,
        "adjustment-only work must retain formation"
    );
    assert_eq!(
        output.work().adjustment().paragraphs(),
        1,
        "adjustment-only work must replace one accepted-slot adjustment"
    );
    assert_eq!(
        output.work().geometry().paragraphs(),
        1,
        "adjustment-only work must rebuild one geometry projection"
    );
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
        "retained text must reuse canonical shaping"
    );
    assert_eq!(
        output.work().line_shape().paragraphs(),
        0,
        "retained text must reuse accepted line shaping"
    );
    assert_eq!(
        output.work().flow().paragraphs(),
        0,
        "retained text must reuse formation"
    );
    assert_eq!(
        output.work().adjustment().paragraphs(),
        0,
        "retained text must reuse accepted-slot adjustment"
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
