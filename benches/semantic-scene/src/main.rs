// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Measurements of Underwood's real public semantic-to-scene implementation.

use std::hint::black_box;
use std::time::{Duration, Instant};

use underwood::{
    Brush, CacheBudget, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth,
    InlineFlowStyle, InlineRole, LayoutEngine, PaintSlot, PaintTable, ParagraphRole, SceneRequest,
    ShapingStyle, StyleMap, TextConstraint, TextId,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine};

const PARAGRAPHS: usize = 64;
const COLD_ITERATIONS: usize = 20;
const RETAINED_ITERATIONS: usize = 200;
const MUTATION_ITERATIONS: usize = 100;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fonts = fonts()?;
    let fixture = document_fixture()?;
    let snapshot = fixture.document.snapshot();
    let width = FiniteWidth::new(420.0)?;
    let cold = measure(COLD_ITERATIONS, || {
        let paragraphs = ParleyParagraphEngine::new(fonts.clone());
        let mut layout = LayoutEngine::new(paragraphs, CacheBudget::new(PARAGRAPHS));
        let request =
            SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark);
        let output = layout
            .prepare(&snapshot, &request)
            .expect("cold public-path preparation must succeed");
        assert_eq!(
            output.work().shape().paragraphs(),
            PARAGRAPHS,
            "cold preparation must shape every paragraph"
        );
        black_box(output.scene().fragments().len());
    });

    let fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        CacheBudget::new(PARAGRAPHS),
    );
    let snapshot = fixture.document.snapshot();
    let request = SceneRequest::new(TextConstraint::Wrap(width), &fixture.styles, &fixture.dark);
    layout.prepare(&snapshot, &request)?;
    let retained = measure(RETAINED_ITERATIONS, || {
        let output = layout
            .prepare(&snapshot, &request)
            .expect("retained public-path preparation must succeed");
        assert_eq!(
            output.work().analysis().paragraphs(),
            0,
            "unchanged preparation must reuse analysis"
        );
        assert_eq!(
            output.work().shape().paragraphs(),
            0,
            "unchanged preparation must reuse shaping"
        );
        assert_eq!(
            output.work().flow().paragraphs(),
            0,
            "unchanged preparation must reuse flow"
        );
        black_box(output.scene().fragments().len());
    });

    let fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        CacheBudget::new(PARAGRAPHS),
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
            output.work().shape().paragraphs(),
            0,
            "paint values must reuse shaping"
        );
        assert_eq!(
            output.work().flow().paragraphs(),
            0,
            "paint values must reuse flow"
        );
        black_box(output.scene().paint().len());
    });

    let fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        CacheBudget::new(PARAGRAPHS),
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
        assert_eq!(
            output.work().shape().paragraphs(),
            0,
            "width must reuse shaping"
        );
        assert_eq!(
            output.work().flow().paragraphs(),
            PARAGRAPHS,
            "an alternating width must reflow every paragraph"
        );
        if narrow_iteration {
            assert!(
                output.work().line_shape().paragraphs() > 0,
                "wrapped paragraphs must expose line-final shaping"
            );
        }
        width_line_reshapes = width_line_reshapes.saturating_add(output.work().line_reshapes());
        black_box(output.scene().lines().len());
    });

    let mut fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(fonts.clone()),
        CacheBudget::new(PARAGRAPHS),
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
            output.work().shape().paragraphs(),
            1,
            "one edited paragraph must cause one paragraph of shaping"
        );
        assert_eq!(
            output.work().reused_paragraphs(),
            PARAGRAPHS - 1,
            "all unchanged sibling paragraphs must be reused"
        );
        black_box(output.scene().fragments().len());
    });

    let visible_space = line_fixture(
        *b"bench-visible-01",
        "alpha beta gamma delta epsilon zeta",
        "Roboto Flex",
    )?;
    let (visible_space_churn, visible_space_reshapes) =
        measure_line_churn(&fonts, &visible_space, 1_000.0, 88.0)?;
    let cursive_zwsp = line_fixture(
        *b"bench-cursive-01",
        "سل\u{200b}ام سل\u{200b}ام سل\u{200b}ام",
        "Noto Kufi Arabic",
    )?;
    let (cursive_zwsp_churn, cursive_zwsp_reshapes) =
        measure_line_churn(&fonts, &cursive_zwsp, 1_000.0, 72.0)?;

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
    let mut document = Document::new(DocumentId::from_bytes(*b"und-benchmark-01"));
    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_prefix = edit.append_text(first, InlineRole::TEXT, "proof / ")?;
    let edited_text = edit.append_text(first, InlineRole::EMPHASIS, "office مرحبا بالعالم")?;
    for index in 1..PARAGRAPHS {
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
        CacheBudget::new(PARAGRAPHS),
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
            output.work().analysis().paragraphs(),
            0,
            "width churn must retain canonical analysis"
        );
        assert_eq!(
            output.work().shape().paragraphs(),
            0,
            "width churn must retain canonical shaping"
        );
        assert_eq!(
            output.work().flow().paragraphs(),
            PARAGRAPHS,
            "width churn must reform every paragraph"
        );
        if narrow_iteration {
            assert_eq!(
                output.work().line_shape().paragraphs(),
                PARAGRAPHS,
                "wrapped paragraphs must expose their line-final shaping"
            );
            assert!(
                output.work().line_reshapes() >= PARAGRAPHS,
                "each wrapped paragraph must attempt at least one line shape"
            );
        }
        line_reshapes = line_reshapes.saturating_add(output.work().line_reshapes());
        black_box(output.scene().lines().len());
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
