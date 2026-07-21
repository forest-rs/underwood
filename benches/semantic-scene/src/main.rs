// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Measurements of Underwood's real public semantic-to-scene implementation.

use std::hint::black_box;
use std::time::{Duration, Instant};

use underwood::{
    Brush, Color, Document, DocumentId, FiniteWidth, InlineRole, LayoutEngine, PaintSlot,
    PaintTable, ParagraphRole, SceneRequest, StyleMap, TextId, TextStyle,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fonts = fonts()?;
    let data = TextData::compiled_minimal();

    let fixture = document_fixture()?;
    let snapshot = fixture.document.snapshot();
    let width = FiniteWidth::new(420.0)?;
    let cold = measure(COLD_ITERATIONS, || {
        let paragraphs = ParleyParagraphEngine::new(data.clone(), fonts.clone())
            .expect("validated immutable adapter inputs must remain valid");
        let mut layout = LayoutEngine::new(paragraphs);
        let request = SceneRequest::new(width, &fixture.styles, &fixture.dark);
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
    let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(data.clone(), fonts.clone())?);
    let snapshot = fixture.document.snapshot();
    let request = SceneRequest::new(width, &fixture.styles, &fixture.dark);
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
    let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(data.clone(), fonts.clone())?);
    let snapshot = fixture.document.snapshot();
    let request = SceneRequest::new(width, &fixture.styles, &fixture.dark);
    layout.prepare(&snapshot, &request)?;
    let mut paint_iteration = 0_usize;
    let paint_only = measure(RETAINED_ITERATIONS, || {
        let paint = if paint_iteration & 1 == 0 {
            &fixture.light
        } else {
            &fixture.dark
        };
        paint_iteration += 1;
        let request = SceneRequest::new(width, &fixture.styles, paint);
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
    let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(data.clone(), fonts.clone())?);
    let snapshot = fixture.document.snapshot();
    let wide = FiniteWidth::new(420.0)?;
    let narrow = FiniteWidth::new(180.0)?;
    let request = SceneRequest::new(wide, &fixture.styles, &fixture.dark);
    layout.prepare(&snapshot, &request)?;
    let mut width_iteration = 0_usize;
    let width_only = measure(MUTATION_ITERATIONS, || {
        let width = if width_iteration & 1 == 0 {
            narrow
        } else {
            wide
        };
        width_iteration += 1;
        let request = SceneRequest::new(width, &fixture.styles, &fixture.dark);
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
        black_box(output.scene().lines().len());
    });

    let mut fixture = document_fixture()?;
    let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(data, fonts)?);
    let request = SceneRequest::new(wide, &fixture.styles, &fixture.dark);
    layout.prepare(&fixture.document.snapshot(), &request)?;
    let mut edit_iteration = 0_usize;
    let one_paragraph_edit = measure(MUTATION_ITERATIONS, || {
        let replacement = if edit_iteration & 1 == 0 {
            "fices مرحبا بالعالم"
        } else {
            "fice مرحبا بالعالم"
        };
        edit_iteration += 1;
        let mut edit = fixture.document.edit();
        edit.replace_text(fixture.edited_text, replacement)
            .expect("the stable text identity must remain editable");
        let publication = edit.commit().expect("benchmark edit must commit");
        let request = SceneRequest::new(wide, &fixture.styles, &fixture.dark);
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

    report("cold_scene", COLD_ITERATIONS, cold);
    report("retained_unchanged", RETAINED_ITERATIONS, retained);
    report("paint_only", RETAINED_ITERATIONS, paint_only);
    report("width_only", MUTATION_ITERATIONS, width_only);
    report(
        "one_paragraph_edit",
        MUTATION_ITERATIONS,
        one_paragraph_edit,
    );
    Ok(())
}

fn document_fixture() -> Result<DocumentFixture, Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"und-benchmark-01"));
    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_prefix = edit.append_text(first, InlineRole::TEXT, "of")?;
    let edited_text = edit.append_text(first, InlineRole::EMPHASIS, "fice مرحبا بالعالم")?;
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

    let mut styles = StyleMap::new(TextStyle::new(16.0, PaintSlot::new(0))?);
    styles.set_paint(first_prefix, PaintSlot::new(0))?;
    styles.set_paint(edited_text, PaintSlot::new(1))?;
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
    ])?)
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

#[cfg(test)]
mod tests {
    #[test]
    fn every_public_path_workload_executes() {
        super::main().expect("all product benchmark workloads must pass their work assertions");
    }
}
