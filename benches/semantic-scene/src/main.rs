// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Measurements of Underwood's real public semantic-to-scene implementation.

use std::hint::black_box;
use std::time::{Duration, Instant};

use underwood::{
    Brush, CacheBudget, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth,
    InlineFlowStyle, InlineRole, LayoutEngine, PaintSlot, PaintTable, ParagraphRole, Rect,
    RegionFlow, SceneFeatures, SceneRequest, ShapingStyle, StyleMap, TextConstraint, TextId,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine};

const PARAGRAPHS: usize = 64;
const COLD_ITERATIONS: usize = 20;
const RETAINED_ITERATIONS: usize = 200;
const MUTATION_ITERATIONS: usize = 100;
const PROFILE_PARAGRAPHS: usize = 1_000;
const ADAPTER_FACTS_BYTES: usize = 128 * 1024 * 1024;

const fn retained_budget(entries: usize) -> CacheBudget {
    CacheBudget::new(entries).with_adapter_facts_bytes(ADAPTER_FACTS_BYTES)
}

struct DocumentFixture {
    document: Document,
    edited_text: TextId,
    styles: StyleMap,
    dark: PaintTable,
    light: PaintTable,
}

struct LineFixture {
    document: Document,
    styles: StyleMap,
    paint: PaintTable,
}

struct EventMeasurement {
    elapsed: Duration,
    #[cfg(feature = "allocation-counting")]
    allocations: allocation_counter::AllocationInfo,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    if let Some(scenario) = arguments.next() {
        if scenario == "--help" || scenario == "-h" {
            println!(
                "usage: underwood_semantic_scene_benchmark [setup-retained|retained|setup-edit|edit-staging|localized-prepare|localized-edit|localized-region|localized-style|append] [paragraphs]"
            );
            return Ok(());
        }
        let paragraphs = arguments
            .next()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(PROFILE_PARAGRAPHS);
        if paragraphs == 0 {
            return Err("paragraphs must be greater than zero".into());
        }
        if arguments.next().is_some() {
            return Err("expected at most a scenario and paragraph count".into());
        }
        let result = run_profile(&scenario, paragraphs);
        signal_profile_ready()?;
        hold_for_profiler()?;
        return result;
    }
    run_suite()
}

fn run_suite() -> Result<(), Box<dyn std::error::Error>> {
    let fonts = fonts()?;
    let fixture = document_fixture()?;
    let snapshot = fixture.document.snapshot();
    let width = FiniteWidth::new(420.0)?;
    let cold = measure(COLD_ITERATIONS, || {
        let paragraphs = ParleyParagraphEngine::new(fonts.clone());
        let mut layout = LayoutEngine::new(paragraphs, retained_budget(PARAGRAPHS));
        let request =
            SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark);
        let output = layout
            .prepare(&snapshot, &request)
            .expect("cold public-path preparation must succeed");
        assert_eq!(
            output.work.shape.paragraphs, PARAGRAPHS,
            "cold preparation must shape every paragraph"
        );
        black_box(output.scene.fragments().len());
    });

    let fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        retained_budget(PARAGRAPHS),
    );
    let snapshot = fixture.document.snapshot();
    let request = SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark);
    layout.prepare(&snapshot, &request)?;
    let retained = measure(RETAINED_ITERATIONS, || {
        let output = layout
            .prepare(&snapshot, &request)
            .expect("retained public-path preparation must succeed");
        assert_eq!(
            output.work.analysis.paragraphs, 0,
            "unchanged preparation must reuse analysis"
        );
        assert_eq!(
            output.work.shape.paragraphs, 0,
            "unchanged preparation must reuse shaping"
        );
        assert_eq!(
            output.work.flow.paragraphs, 0,
            "unchanged preparation must reuse flow"
        );
        black_box(output.scene.fragments().len());
    });

    let fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        retained_budget(PARAGRAPHS),
    );
    let snapshot = fixture.document.snapshot();
    let request = SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark);
    layout.prepare(&snapshot, &request)?;
    let mut paint_iteration = 0_usize;
    let paint_only = measure(RETAINED_ITERATIONS, || {
        let paint = if paint_iteration & 1 == 0 {
            &fixture.light
        } else {
            &fixture.dark
        };
        paint_iteration += 1;
        let request = SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, paint);
        let output = layout
            .prepare(&snapshot, &request)
            .expect("paint-only public-path preparation must succeed");
        assert_eq!(
            output.work.shape.paragraphs, 0,
            "paint values must reuse shaping"
        );
        assert_eq!(
            output.work.flow.paragraphs, 0,
            "paint values must reuse flow"
        );
        black_box(output.scene.paint().len());
    });

    let fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        retained_budget(PARAGRAPHS),
    );
    let snapshot = fixture.document.snapshot();
    let wide = FiniteWidth::new(420.0)?;
    let narrow = FiniteWidth::new(180.0)?;
    let request = SceneRequest::new(TextConstraint::Wrap(wide), &fixture.styles, &fixture.dark);
    layout.prepare(&snapshot, &request)?;
    let mut width_iteration = 0_usize;
    let mut width_line_reshapes = 0_usize;
    let width_only = measure(MUTATION_ITERATIONS, || {
        let narrow_iteration = width_iteration & 1 == 0;
        let width = if narrow_iteration { narrow } else { wide };
        width_iteration += 1;
        let request =
            SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark);
        let output = layout
            .prepare(&snapshot, &request)
            .expect("width-only public-path preparation must succeed");
        assert_eq!(output.work.shape.paragraphs, 0, "width must reuse shaping");
        assert_eq!(
            output.work.flow.paragraphs, PARAGRAPHS,
            "an alternating width must reflow every paragraph"
        );
        if narrow_iteration {
            assert_eq!(
                output.work.line_shape.paragraphs, 0,
                "whitespace-separated wraps must borrow canonical shaping"
            );
        }
        width_line_reshapes = width_line_reshapes.saturating_add(output.work.line_reshapes);
        black_box(output.scene.lines().len());
    });

    let mut fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        retained_budget(PARAGRAPHS),
    );
    let request = SceneRequest::new(TextConstraint::Wrap(wide), &fixture.styles, &fixture.dark);
    layout.prepare(&fixture.document.snapshot(), &request)?;
    let mut edit_iteration = 0_usize;
    let one_paragraph_edit = measure(MUTATION_ITERATIONS, || {
        let replacement = if edit_iteration & 1 == 0 {
            "offices مرحبا بالعالم"
        } else {
            "office مرحبا بالعالم"
        };
        edit_iteration += 1;
        let mut edit = fixture.document.edit();
        edit.replace_text(fixture.edited_text, replacement)
            .expect("the stable text identity must remain editable");
        let publication = edit.commit().expect("benchmark edit must commit");
        let request = SceneRequest::new(TextConstraint::Wrap(wide), &fixture.styles, &fixture.dark);
        let output = layout
            .prepare(publication.snapshot(), &request)
            .expect("edited public-path preparation must succeed");
        assert_eq!(
            output.work.shape.paragraphs, 1,
            "one edited paragraph must cause one paragraph of shaping"
        );
        assert_eq!(
            output.work.reused_paragraphs,
            PARAGRAPHS - 1,
            "all unchanged sibling paragraphs must be reused"
        );
        black_box(output.scene.fragments().len());
    });

    let visible_space = line_fixture(
        *b"bench-visible-01",
        "alpha beta gamma delta epsilon zeta",
        "Roboto Flex",
    )?;
    let (visible_space_churn, visible_space_reshapes) =
        measure_line_churn(&fonts, &visible_space, 1_000.0, 88.0)?;
    assert_eq!(
        visible_space_reshapes, 0,
        "whitespace-separated wraps must borrow canonical shaping"
    );
    let cursive_zwsp = line_fixture(
        *b"bench-cursive-01",
        "سل\u{200b}ام سل\u{200b}ام سل\u{200b}ام",
        "Noto Kufi Arabic",
    )?;
    let (cursive_zwsp_churn, cursive_zwsp_reshapes) =
        measure_line_churn(&fonts, &cursive_zwsp, 1_000.0, 72.0)?;
    assert!(
        cursive_zwsp_reshapes > 0,
        "joining-sensitive zero-width breaks must retain line shaping"
    );

    report("cold_scene", COLD_ITERATIONS, cold);
    report("retained_unchanged", RETAINED_ITERATIONS, retained);
    report("paint_only", RETAINED_ITERATIONS, paint_only);
    report_with_line_work(
        "width_only",
        MUTATION_ITERATIONS,
        width_only,
        width_line_reshapes,
    );
    report(
        "one_paragraph_edit",
        MUTATION_ITERATIONS,
        one_paragraph_edit,
    );
    report_with_line_work(
        "visible_space_width_churn",
        MUTATION_ITERATIONS,
        visible_space_churn,
        visible_space_reshapes,
    );
    report_with_line_work(
        "cursive_zwsp_width_churn",
        MUTATION_ITERATIONS,
        cursive_zwsp_churn,
        cursive_zwsp_reshapes,
    );
    Ok(())
}

fn document_fixture() -> Result<DocumentFixture, Box<dyn std::error::Error>> {
    document_fixture_with_paragraphs(PARAGRAPHS)
}

fn document_fixture_with_paragraphs(
    paragraphs: usize,
) -> Result<DocumentFixture, Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"und-benchmark-01"));
    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_prefix = edit.append_text(first, InlineRole::TEXT, "proof / ")?;
    let edited_text = edit.append_text(first, InlineRole::EMPHASIS, "office مرحبا بالعالم")?;
    for index in 1..paragraphs {
        let paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
        let text = if index & 1 == 0 {
            "Retained sibling office affinity"
        } else {
            "فقرة عربية ثابتة unchanged sibling"
        };
        edit.append_text(paragraph, InlineRole::TEXT, text)?;
    }
    edit.commit()?;

    let base = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 16.0)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let mut styles = StyleMap::new(base.clone());
    styles.set(first_prefix, base.clone());
    styles.set(edited_text, base.with_paint(PaintSlot::new(1)));
    let dark = PaintTable::from_brushes([
        Brush::Solid(Color::from_rgb8(0x20, 0x20, 0x20)),
        Brush::Solid(Color::from_rgb8(0x20, 0x50, 0xa0)),
    ]);
    let light = PaintTable::from_brushes([
        Brush::Solid(Color::from_rgb8(0xe0, 0xe0, 0xe0)),
        Brush::Solid(Color::from_rgb8(0xa0, 0x20, 0x20)),
    ]);
    Ok(DocumentFixture {
        document,
        edited_text,
        styles,
        dark,
        light,
    })
}

fn run_profile(scenario: &str, paragraphs: usize) -> Result<(), Box<dyn std::error::Error>> {
    let fonts = fonts()?;
    let mut fixture = document_fixture_with_paragraphs(paragraphs)?;
    let width = FiniteWidth::new(420.0)?;
    let cache_entries = if matches!(scenario, "append" | "a0") {
        paragraphs.saturating_add(1)
    } else {
        paragraphs
    };
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts),
        retained_budget(cache_entries),
    );
    let snapshot = fixture.document.snapshot();
    let region_flow = matches!(scenario, "localized-region" | "g0")
        .then(|| {
            RegionFlow::rectangle(Rect::new(
                0.0,
                0.0,
                420.0,
                f64::from(u32::try_from(paragraphs).unwrap_or(u32::MAX)) * 80.0,
            ))
        })
        .transpose()?;
    let features = if matches!(
        scenario,
        "setup-edit"
            | "s1"
            | "edit-staging"
            | "d0"
            | "localized-prepare"
            | "p0"
            | "localized-edit"
            | "e0"
            | "localized-region"
            | "g0"
    ) {
        SceneFeatures::EDITABLE
    } else {
        SceneFeatures::DISPLAY
    };
    let request = match &region_flow {
        Some(flow) => {
            SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark)
                .with_features(features)
                .with_region_flow(flow)
        }
        None => SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark)
            .with_features(features),
    };
    let primed = layout.prepare(&snapshot, &request)?;
    assert_eq!(
        primed.work.shape.paragraphs, paragraphs,
        "the profile fixture must prime every paragraph"
    );

    let event = match scenario {
        "setup-retained" | "s0" => None,
        "retained" | "r0" => {
            let (output, measurement) = measure_event(|| layout.prepare(&snapshot, &request));
            let output = output?;
            assert_no_preparation_work(&output, paragraphs);
            black_box(output);
            Some(measurement)
        }
        "localized-style" | "y0" => {
            let mut styles = fixture.styles.clone();
            styles.set(
                fixture.edited_text,
                ComputedInlineStyle::new(
                    ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 17.0)?,
                    InlineFlowStyle::default(),
                    PaintSlot::new(1),
                ),
            );
            let changed_request =
                SceneRequest::new(TextConstraint::Wrap(width), &styles, &fixture.dark);
            let (output, measurement) =
                measure_event(|| layout.prepare(&snapshot, &changed_request));
            let output = output?;
            assert_eq!(
                output.work.shape.paragraphs, 1,
                "one changed paragraph must reshape"
            );
            assert_eq!(
                output.work.reused_paragraphs,
                paragraphs.saturating_sub(1),
                "unchanged sibling paragraphs must be reused"
            );
            black_box((styles, output));
            Some(measurement)
        }
        "append" | "a0" => {
            let mut edit = fixture.document.edit();
            let paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
            edit.append_text(paragraph, InlineRole::TEXT, "appended retained paragraph")?;
            let publication = edit.commit()?;
            let (output, measurement) =
                measure_event(|| layout.prepare(publication.snapshot(), &request));
            let output = output?;
            assert_eq!(
                output.work.shape.paragraphs, 1,
                "only the appended paragraph must shape"
            );
            assert_eq!(
                output.work.reused_paragraphs, paragraphs,
                "every pre-existing paragraph must be reused"
            );
            black_box((publication, output));
            Some(measurement)
        }
        "setup-edit" | "s1" | "edit-staging" | "d0" | "localized-prepare" | "p0"
        | "localized-edit" | "e0" | "localized-region" | "g0" => {
            let editing = primed.scene.editing()?;
            let position = editing
                .position_at(fixture.edited_text, 1)
                .ok_or("the one-byte insertion point must be represented")?;
            let selection = editing.collapsed(&position)?;
            let selections = editing.set([selection])?;
            match scenario {
                "setup-edit" | "s1" => {
                    black_box(selections);
                    None
                }
                "edit-staging" | "d0" => {
                    let (replacement, measurement) =
                        measure_event(|| fixture.document.replace_selections(&selections, "x"));
                    let replacement = replacement?;
                    assert_eq!(
                        replacement.publication().changes().paragraphs().len(),
                        1,
                        "one-byte insertion must publish one changed paragraph"
                    );
                    black_box(replacement);
                    Some(measurement)
                }
                "localized-prepare" | "p0" | "localized-region" | "g0" => {
                    let replacement = fixture.document.replace_selections(&selections, "x")?;
                    let (output, measurement) = measure_event(|| {
                        layout.prepare(replacement.publication().snapshot(), &request)
                    });
                    let output = output?;
                    assert_eq!(
                        output.work.shape.paragraphs, 1,
                        "one-byte insertion must reshape exactly one paragraph"
                    );
                    assert_eq!(
                        output.work.reused_paragraphs,
                        paragraphs.saturating_sub(1),
                        "one-byte insertion must reuse every unchanged sibling"
                    );
                    black_box((replacement, output));
                    Some(measurement)
                }
                "localized-edit" | "e0" => {
                    let (result, measurement) = measure_event(|| {
                        let replacement = fixture.document.replace_selections(&selections, "x")?;
                        let output =
                            layout.prepare(replacement.publication().snapshot(), &request)?;
                        Ok::<_, Box<dyn std::error::Error>>((replacement, output))
                    });
                    let (replacement, output) = result?;
                    assert_eq!(
                        output.work.shape.paragraphs, 1,
                        "one-byte insertion must reshape exactly one paragraph"
                    );
                    assert_eq!(
                        output.work.reused_paragraphs,
                        paragraphs.saturating_sub(1),
                        "one-byte insertion must reuse every unchanged sibling"
                    );
                    black_box((replacement, output));
                    Some(measurement)
                }
                _ => unreachable!("the outer match admits only edit scenarios"),
            }
        }
        _ => {
            return Err(format!("unknown profile scenario: {scenario}").into());
        }
    };

    black_box((&fixture, &layout, &snapshot, &request, &primed));
    if let Some(measurement) = event {
        report_profile_event(scenario, paragraphs, &measurement);
    }
    Ok(())
}

#[cfg(feature = "allocation-counting")]
fn measure_event<T>(operation: impl FnOnce() -> T) -> (T, EventMeasurement) {
    let mut result = None;
    let start = Instant::now();
    let allocations = allocation_counter::measure(|| {
        result = Some(operation());
    });
    let elapsed = start.elapsed();
    let Some(result) = result else {
        unreachable!("the measured operation always stores its result");
    };
    (
        result,
        EventMeasurement {
            elapsed,
            allocations,
        },
    )
}

#[cfg(not(feature = "allocation-counting"))]
fn measure_event<T>(operation: impl FnOnce() -> T) -> (T, EventMeasurement) {
    let start = Instant::now();
    let result = operation();
    let elapsed = start.elapsed();
    (result, EventMeasurement { elapsed })
}

#[cfg(feature = "allocation-counting")]
fn report_profile_event(scenario: &str, paragraphs: usize, measurement: &EventMeasurement) {
    if std::env::var_os("UNDERWOOD_PROFILE_QUIET").is_none() {
        println!(
            "{scenario}\tprofile=isolated\tparagraphs={paragraphs}\ttotal_ns={}\tallocation_calls={}\tallocated_bytes={}\tpeak_live_allocations={}\tpeak_live_bytes={}\tnet_live_allocations={}\tnet_live_bytes={}",
            measurement.elapsed.as_nanos(),
            measurement.allocations.count_total,
            measurement.allocations.bytes_total,
            measurement.allocations.count_max,
            measurement.allocations.bytes_max,
            measurement.allocations.count_current,
            measurement.allocations.bytes_current,
        );
    }
}

#[cfg(not(feature = "allocation-counting"))]
fn report_profile_event(scenario: &str, paragraphs: usize, measurement: &EventMeasurement) {
    if std::env::var_os("UNDERWOOD_PROFILE_QUIET").is_none() {
        println!(
            "{scenario}\tprofile=isolated\tparagraphs={paragraphs}\ttotal_ns={}",
            measurement.elapsed.as_nanos(),
        );
    }
}

fn assert_no_preparation_work(output: &underwood::SceneOutput, paragraphs: usize) {
    assert_eq!(
        output.work.analysis.paragraphs, 0,
        "unchanged preparation must reuse analysis"
    );
    assert_eq!(
        output.work.shape.paragraphs, 0,
        "unchanged preparation must reuse shaping"
    );
    assert_eq!(
        output.work.flow.paragraphs, 0,
        "unchanged preparation must reuse flow"
    );
    assert_eq!(
        output.work.reused_paragraphs, paragraphs,
        "unchanged preparation must report every paragraph reused"
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

fn signal_profile_ready() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::var_os("UNDERWOOD_PROFILE_READY_FILE") else {
        return Ok(());
    };
    std::fs::write(path, b"ready\n")?;
    Ok(())
}

fn fonts() -> Result<FontSet, Box<dyn std::error::Error>> {
    Ok(FontSet::try_from_fonts([
        Font::from_bytes(include_bytes!(
            "../../../examples/headless/fonts/RobotoFlex-VariableFont.ttf"
        ))?,
        Font::from_bytes(include_bytes!(
            "../../../examples/headless/fonts/NotoKufiArabic-Regular.otf"
        ))?,
    ])?
    .with_fallbacks(
        underwood::Script::from_bytes(*b"Arab"),
        None,
        ["Noto Kufi Arabic"],
    )?)
}

fn line_fixture(
    id: [u8; 16],
    text: &str,
    family: &str,
) -> Result<LineFixture, Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(id));
    let mut edit = document.edit();
    for _ in 0..PARAGRAPHS {
        let paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
        edit.append_text(paragraph, InlineRole::TEXT, text)?;
    }
    edit.commit()?;
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named(family), 16.0)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    Ok(LineFixture {
        document,
        styles: StyleMap::new(style),
        paint: PaintTable::from_brushes([Brush::Solid(Color::BLACK)]),
    })
}

fn measure_line_churn(
    fonts: &FontSet,
    fixture: &LineFixture,
    wide: f64,
    narrow: f64,
) -> Result<(Duration, usize), Box<dyn std::error::Error>> {
    let wide = FiniteWidth::new(wide)?;
    let narrow = FiniteWidth::new(narrow)?;
    let snapshot = fixture.document.snapshot();
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        retained_budget(PARAGRAPHS),
    );
    layout.prepare(
        &snapshot,
        &SceneRequest::new(TextConstraint::Wrap(wide), &fixture.styles, &fixture.paint),
    )?;
    let mut iteration = 0_usize;
    let mut line_reshapes = 0_usize;
    let elapsed = measure(MUTATION_ITERATIONS, || {
        let narrow_iteration = iteration & 1 == 0;
        let width = if narrow_iteration { narrow } else { wide };
        iteration += 1;
        let output = layout
            .prepare(
                &snapshot,
                &SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.paint),
            )
            .expect("line-formation churn must prepare");
        assert_eq!(
            output.work.analysis.paragraphs, 0,
            "width churn must retain canonical analysis"
        );
        assert_eq!(
            output.work.shape.paragraphs, 0,
            "width churn must retain canonical shaping"
        );
        assert_eq!(
            output.work.flow.paragraphs, PARAGRAPHS,
            "width churn must reform every paragraph"
        );
        line_reshapes = line_reshapes.saturating_add(output.work.line_reshapes);
        black_box(output.scene.lines().len());
    });
    Ok((elapsed, line_reshapes))
}

fn measure(iterations: usize, mut operation: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    start.elapsed()
}

fn report(name: &str, iterations: usize, elapsed: Duration) {
    let per_iteration = elapsed.as_nanos() / iterations as u128;
    println!(
        "{name}\titerations={iterations}\ttotal_ns={}\tns_per_iteration={per_iteration}",
        elapsed.as_nanos()
    );
}

fn report_with_line_work(name: &str, iterations: usize, elapsed: Duration, line_reshapes: usize) {
    let per_iteration = elapsed.as_nanos() / iterations as u128;
    println!(
        "{name}\titerations={iterations}\ttotal_ns={}\tns_per_iteration={per_iteration}\tline_reshapes={line_reshapes}",
        elapsed.as_nanos()
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_public_path_workload_executes() {
        super::main().expect("all product benchmark workloads must pass their work assertions");
    }
}
