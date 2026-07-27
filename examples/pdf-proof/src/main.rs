// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic mixed-script PDF proof through Underwood's public scene API.

use std::fs;
use std::path::{Path, PathBuf};

use underwood::{
    Brush, CacheBudget, Color, ComputedInlineStyle, Document, DocumentId, DocumentSnapshot,
    FiniteWidth, FontFeature, GenericFamily, InlineFlowStyle, InlineRole, Language, LayoutEngine,
    LineHeight, PaintSlot, PaintTable, ParagraphRole, SceneFeatures, SceneRequest, Script,
    ShapingStyle, StyleMap, Tag, TextAlignment, TextConstraint, TextScene,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine};
use underwood_pdf::{PdfPage, to_pdf};

const LATIN_FONT_BYTES: &[u8] = include_bytes!("../../headless/fonts/RobotoFlex-VariableFont.ttf");
const ARABIC_FONT_BYTES: &[u8] = include_bytes!("../../headless/fonts/NotoKufiArabic-Regular.otf");

const INK: PaintSlot = PaintSlot::new(0);
const CYAN: PaintSlot = PaintSlot::new(1);
const CORAL: PaintSlot = PaintSlot::new(2);
const GOLD: PaintSlot = PaintSlot::new(3);
const MUTED: PaintSlot = PaintSlot::new(4);

type AnyError = Box<dyn std::error::Error>;

fn main() -> Result<(), AnyError> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_output_path);
    let pdf = specimen_pdf()?;
    write_pdf(&output, &pdf)?;
    println!("wrote {} ({} bytes)", output.display(), pdf.len());
    Ok(())
}

fn default_output_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/underwood-proof.pdf")
}

fn write_pdf(path: &Path, bytes: &[u8]) -> Result<(), AnyError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn specimen_pdf() -> Result<Vec<u8>, AnyError> {
    let (snapshot, scene) = prepare_specimen()?;
    let page = PdfPage::new(720.0, 720.0)?.with_origin(underwood::Point::new(72.0, 68.0))?;
    Ok(to_pdf(&scene, &snapshot, page)?)
}

fn prepare_specimen() -> Result<(DocumentSnapshot, TextScene), AnyError> {
    let mut document = Document::new(DocumentId::from_bytes(*b"underwood-pdf-01"));
    let mut edit = document.edit();

    let title = edit.append_paragraph(ParagraphRole::HEADING_1)?;
    let title_text = edit.append_text(title, InlineRole::TEXT, "UNDERWOOD / PDF")?;

    let deck = edit.append_paragraph(ParagraphRole::BODY)?;
    let deck_text = edit.append_text(
        deck,
        InlineRole::TEXT,
        "One semantic snapshot. Real shaping. Exact glyphs in a portable page.",
    )?;

    let scripts_heading = edit.append_paragraph(ParagraphRole::HEADING_2)?;
    let scripts_heading_text = edit.append_text(
        scripts_heading,
        InlineRole::TEXT,
        "ONE SCENE / MANY SCRIPTS",
    )?;

    let mixed = edit.append_paragraph(ParagraphRole::BODY)?;
    let mixed_prefix = edit.append_text(
        mixed,
        InlineRole::TEXT,
        "Underwood keeps meaning, style, and geometry together. ",
    )?;
    let mixed_arabic_greeting = edit.append_text(mixed, InlineRole::EMPHASIS, "مرحباً ")?;
    let mixed_arabic_world = edit.append_text(mixed, InlineRole::EMPHASIS, "بالعالم")?;
    let mixed_suffix = edit.append_text(
        mixed,
        InlineRole::TEXT,
        " runs right-to-left—with every dot and mark intact—inside one paragraph.",
    )?;

    let detail_heading = edit.append_paragraph(ParagraphRole::HEADING_2)?;
    let detail_heading_text = edit.append_text(
        detail_heading,
        InlineRole::TEXT,
        "OPENTYPE DETAIL / SOURCE-COMPLETE GLYPHS",
    )?;

    let ligature = edit.append_paragraph(ParagraphRole::BODY)?;
    let ligature_label = edit.append_text(ligature, InlineRole::TEXT, "Authored ligatures: ")?;
    let ligature_text =
        edit.append_text(ligature, InlineRole::EMPHASIS, "office / efficient / لا")?;

    let combining = edit.append_paragraph(ParagraphRole::BODY)?;
    let combining_label = edit.append_text(
        combining,
        InlineRole::TEXT,
        "One grapheme crosses semantic leaves: ",
    )?;
    let cafe_prefix = edit.append_text(combining, InlineRole::TEXT, "caf")?;
    let cafe_base = edit.append_text(combining, InlineRole::EMPHASIS, "e")?;
    let cafe_mark = edit.append_text(combining, InlineRole::EMPHASIS, "\u{301}")?;
    let combining_suffix =
        edit.append_text(combining, InlineRole::TEXT, " — still one shaped cluster.")?;

    let arabic_display = edit.append_paragraph(ParagraphRole::BODY)?;
    let arabic_display_prefix = edit.append_text(arabic_display, InlineRole::EMPHASIS, "الخطّ ")?;
    let arabic_display_token = edit.append_text(arabic_display, InlineRole::TEXT, "PDF")?;
    let arabic_display_suffix =
        edit.append_text(arabic_display, InlineRole::EMPHASIS, " العربي حيّ")?;

    let footer = edit.append_paragraph(ParagraphRole::BODY)?;
    let footer_text = edit.append_text(
        footer,
        InlineRole::TEXT,
        "TEXTSCENE  →  UNDERWOOD_PDF  →  KRILLA",
    )?;

    let publication = edit.commit()?;
    let snapshot = publication.snapshot().clone();

    let english = Language::parse("en")?;
    let arabic = Language::parse("ar")?;
    let latin_family = underwood::FontFamily::named("Roboto Flex");
    let arabic_family = underwood::FontFamily::named("Noto Kufi Arabic");
    let base = style(latin_family.clone(), 21.0, english, 1.55, INK)?;
    let title_style = style(latin_family.clone(), 62.0, english, 1.2, CYAN)?;
    let deck_style = style(latin_family.clone(), 27.0, english, 1.5, CORAL)?;
    let section_style = style(latin_family.clone(), 16.0, english, 2.2, CYAN)?;
    let arabic_style = style(arabic_family.clone(), 25.0, arabic, 1.55, GOLD)?;
    let ligature_style = ComputedInlineStyle::new(
        ShapingStyle::new(latin_family.clone(), 29.0)?
            .with_language(Some(english))
            .with_features([FontFeature::new(Tag::new(b"liga"), 1)]),
        InlineFlowStyle::new(LineHeight::from_multiplier(1.5)?),
        CORAL,
    );
    let display_style = style(arabic_family, 47.0, arabic, 1.55, GOLD)?;
    let display_latin_style = style(latin_family.clone(), 43.0, english, 1.55, CORAL)?;
    let footer_style = style(latin_family, 14.0, english, 3.2, MUTED)?;

    let mut styles = StyleMap::new(base.clone());
    styles.set(title_text, title_style);
    styles.set(deck_text, deck_style);
    styles.set(scripts_heading_text, section_style.clone());
    styles.set(mixed_prefix, base.clone());
    styles.set(mixed_arabic_greeting, arabic_style.clone());
    styles.set(mixed_arabic_world, arabic_style.with_paint(CORAL));
    styles.set(mixed_suffix, base.clone());
    styles.set(detail_heading_text, section_style);
    styles.set(ligature_label, base.clone());
    styles.set(ligature_text, ligature_style);
    styles.set(combining_label, base.clone());
    styles.set(cafe_prefix, base.clone());
    styles.set(cafe_base, base.clone().with_paint(GOLD));
    styles.set(cafe_mark, base.clone().with_paint(GOLD));
    styles.set(combining_suffix, base);
    styles.set(arabic_display_prefix, display_style.clone());
    styles.set(arabic_display_token, display_latin_style);
    styles.set(arabic_display_suffix, display_style);
    styles.set(footer_text, footer_style);
    styles.set_paragraph_style(
        title,
        underwood::ParagraphStyle::DEFAULT.with_alignment(TextAlignment::Center),
    );
    styles.set_paragraph_style(
        mixed,
        underwood::ParagraphStyle::DEFAULT.with_alignment(TextAlignment::Justify),
    );
    styles.set_paragraph_style(
        footer,
        underwood::ParagraphStyle::DEFAULT.with_alignment(TextAlignment::End),
    );

    let paints = PaintTable::from_brushes([
        Brush::Solid(Color::from_rgb8(0x16, 0x20, 0x2d)),
        Brush::Solid(Color::from_rgb8(0x08, 0x8e, 0xa8)),
        Brush::Solid(Color::from_rgb8(0xdf, 0x52, 0x4c)),
        Brush::Solid(Color::from_rgb8(0xb9, 0x7b, 0x0d)),
        Brush::Solid(Color::from_rgb8(0x6c, 0x78, 0x89)),
    ]);
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin-default", LATIN_FONT_BYTES)?,
        Font::from_bytes("arabic-static", ARABIC_FONT_BYTES)?,
    ])?
    .with_generic_families(GenericFamily::SansSerif, ["Roboto Flex"])?
    .with_fallbacks(
        Script::from_bytes(*b"Arab"),
        Some(arabic),
        ["Noto Kufi Arabic"],
    )?;
    let paragraphs = ParleyParagraphEngine::new(fonts);
    let mut layout = LayoutEngine::new(paragraphs, CacheBudget::new(64));
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(576.0)?),
        &styles,
        &paints,
    )
    .with_features(SceneFeatures::DISPLAY.with_sources());
    let output = layout.prepare(&snapshot, &request)?;
    let scene = output.scene().clone();
    let sources = scene
        .sources()
        .expect("the PDF proof requests source provenance");

    assert!(
        scene
            .fragments()
            .iter()
            .any(|fragment| fragment.script() == *b"Arab" && fragment.bidi_level() & 1 == 1),
        "the proof must contain a real right-to-left Arabic run"
    );
    assert!(
        scene.fragments().iter().any(|fragment| {
            fragment.glyphs().iter().any(|glyph| {
                sources
                    .for_glyph(glyph)
                    .expect("glyph belongs to source scene")
                    .count()
                    > 1
            })
        }),
        "the decomposed accent must retain cross-leaf glyph provenance"
    );
    let office_glyphs = scene
        .fragments()
        .iter()
        .flat_map(|fragment| fragment.glyphs())
        .filter(|glyph| {
            sources
                .for_glyph(*glyph)
                .expect("glyph belongs to source scene")
                .any(|source| {
                    let bytes = source.bytes();
                    source.text() == ligature_text && bytes.start < 6 && bytes.end > 0
                })
        })
        .count();
    assert_eq!(
        office_glyphs, 4,
        "the authored `office` must reach the scene as o + ffi + c + e"
    );
    assert!(
        scene.fragments().iter().all(|fragment| {
            fragment
                .normalized_coords()
                .iter()
                .all(|coordinate| *coordinate == 0)
                && !fragment.synthesis().embolden()
                && fragment.synthesis().skew_degrees().is_none()
        }),
        "the specimen must remain inside the adapter's exact static/default subset"
    );
    assert!(
        scene.lines().iter().any(|line| {
            line.adjustment().alignment() == TextAlignment::Center
                && line.adjustment().inline_offset() > 0.0
        }) && scene.lines().iter().any(|line| {
            line.adjustment().alignment() == TextAlignment::Justify
                && line.adjustment().expanded_opportunities() > 0
        }) && scene.lines().iter().any(|line| {
            line.adjustment().alignment() == TextAlignment::End
                && line.adjustment().inline_offset() > 0.0
        }),
        "the exported scene must carry real centered, justified, and end-aligned PDF geometry"
    );

    Ok((snapshot, scene))
}

fn style(
    family: underwood::FontFamily<'static>,
    size: f32,
    language: Language,
    line_height: f32,
    paint: PaintSlot,
) -> Result<ComputedInlineStyle, AnyError> {
    Ok(ComputedInlineStyle::new(
        ShapingStyle::new(family, size)?.with_language(Some(language)),
        InlineFlowStyle::new(LineHeight::from_multiplier(line_height)?),
        paint,
    ))
}

#[cfg(test)]
mod tests {
    use super::{prepare_specimen, specimen_pdf};
    use underwood::TextAlignment;

    #[test]
    fn mixed_script_pdf_is_deterministic_and_nontrivial() {
        let first = specimen_pdf().expect("the PDF proof must export");
        let second = specimen_pdf().expect("repeating the PDF proof must export");

        assert_eq!(
            first, second,
            "the proof artifact must be byte-deterministic"
        );
        assert!(
            first.starts_with(b"%PDF-"),
            "the output must have a PDF header"
        );
        assert!(
            first.ends_with(b"%%EOF"),
            "the output must have a PDF trailer"
        );
        assert!(
            first
                .windows(b"startxref".len())
                .any(|window| window == b"startxref"),
            "the output must contain a cross-reference trailer"
        );
        assert!(first.len() > 10_000, "the output must embed a real font");
    }

    #[test]
    fn prepared_scene_contains_the_authored_evidence() {
        let (_, scene) = prepare_specimen().expect("the specimen must prepare");
        assert!(
            scene.lines().len() >= 8,
            "semantic sections must form real lines"
        );
        assert!(
            scene.paint().len() >= 5,
            "the specimen must preserve multiple authored paints"
        );
    }

    #[test]
    fn justified_and_end_aligned_lines_share_the_column_right_edge() {
        let (_, scene) = prepare_specimen().expect("the specimen must prepare");
        let justified = scene
            .lines()
            .iter()
            .find(|line| {
                line.adjustment().alignment() == TextAlignment::Justify
                    && line.adjustment().expanded_opportunities() > 0
            })
            .expect("the specimen must contain a genuinely justified soft line");
        let end_aligned = scene
            .lines()
            .iter()
            .find(|line| line.adjustment().alignment() == TextAlignment::End)
            .expect("the specimen must contain an end-aligned line");
        let justified_right =
            justified.bounds().x1 - justified.adjustment().trailing_whitespace_advance();
        let end_right =
            end_aligned.bounds().x1 - end_aligned.adjustment().trailing_whitespace_advance();

        assert!(
            (justified_right - end_right).abs() <= 1.0e-6,
            "justified content ended at {justified_right}, but end-aligned content ended at {end_right}"
        );
    }
}
