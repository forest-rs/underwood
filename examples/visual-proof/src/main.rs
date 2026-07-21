// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A poster-quality CPU rendering of Underwood's real semantic scene.

use std::fs::{self, File};
#[cfg(test)]
use std::io::BufReader;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use imaging::kurbo::{Affine, Circle, Rect, RoundedRect, Stroke};
use imaging::peniko::{Color, Fill, Style};
use imaging::{PaintSink, Painter, RgbaImage, record};
use imaging_vello_cpu::VelloCpuRenderer;
use underwood::{
    Brush, Document, DocumentId, FiniteWidth, InlineRole, LayoutEngine, PaintSlot, PaintTable,
    ParagraphRole, SceneRequest, StyleMap, TextScene, TextStyle,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

const WIDTH: u16 = 1_600;
const HEIGHT: u16 = 1_000;

const LATIN_FONT_BYTES: &[u8] = include_bytes!("../../headless/fonts/RobotoFlex-VariableFont.ttf");
const ARABIC_FONT_BYTES: &[u8] = include_bytes!("../../headless/fonts/NotoKufiArabic-Regular.otf");

const INK: PaintSlot = PaintSlot::new(0);
const CYAN: PaintSlot = PaintSlot::new(1);
const CORAL: PaintSlot = PaintSlot::new(2);
const GOLD: PaintSlot = PaintSlot::new(3);
const MUTED: PaintSlot = PaintSlot::new(4);

const BACKGROUND: Color = Color::from_rgb8(0x0b, 0x10, 0x18);
const PANEL: Color = Color::from_rgb8(0x12, 0x1a, 0x25);
const PANEL_EDGE: Color = Color::from_rgba8(0x78, 0x8a, 0xa3, 0x30);
const INK_COLOR: Color = Color::from_rgb8(0xee, 0xf3, 0xf8);
const CYAN_COLOR: Color = Color::from_rgb8(0x4d, 0xd5, 0xe7);
const CORAL_COLOR: Color = Color::from_rgb8(0xff, 0x6b, 0x67);
const GOLD_COLOR: Color = Color::from_rgb8(0xf5, 0xc4, 0x51);
const MUTED_COLOR: Color = Color::from_rgb8(0x85, 0x96, 0xad);

type AnyError = Box<dyn std::error::Error>;

#[derive(Clone, Copy)]
struct Piece<'a> {
    text: &'a str,
    role: InlineRole,
    paint: PaintSlot,
}

impl<'a> Piece<'a> {
    const fn new(text: &'a str, role: InlineRole, paint: PaintSlot) -> Self {
        Self { text, role, paint }
    }
}

#[derive(Clone, Copy, Debug)]
struct RetainedProof {
    reshaped: usize,
    reused: usize,
    paint_reshaped: usize,
}

struct TextSceneAdapter<'a> {
    scene: &'a TextScene,
    placement: Affine,
    diagnostics: bool,
}

impl<'a> TextSceneAdapter<'a> {
    fn new(scene: &'a TextScene, x: f64, y: f64) -> Self {
        Self {
            scene,
            placement: Affine::translate((x, y)),
            diagnostics: false,
        }
    }

    fn with_diagnostics(mut self) -> Self {
        self.diagnostics = true;
        self
    }

    fn paint_into<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        let fill = Style::Fill(Fill::NonZero);
        if self.diagnostics {
            self.paint_diagnostics_behind(painter);
        }

        for fragment in self.scene.fragments() {
            let brush = self
                .scene
                .paint()
                .brush(fragment.paint())
                .expect("validated scene paint slot must exist");
            let glyphs = fragment.glyphs().iter().map(|glyph| record::Glyph {
                id: glyph.id(),
                x: imaging_coord(glyph.position().x),
                y: imaging_coord(glyph.position().y),
            });
            let transform = self.placement * fragment.transform();
            painter.with_fill_clip_transformed(fragment.clip(), self.placement, |painter| {
                painter
                    .glyphs(fragment.font(), brush)
                    .transform(transform)
                    .font_size(fragment.font_size())
                    .normalized_coords(fragment.normalized_coords())
                    .draw(&fill, glyphs);
            });
        }

        if self.diagnostics {
            self.paint_diagnostics_above(painter);
        }
    }

    fn paint_diagnostics_behind<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        for line in self.scene.lines() {
            painter
                .fill(line.bounds(), Color::from_rgba8(0x4d, 0xd5, 0xe7, 0x08))
                .transform(self.placement)
                .draw();
        }
        for fragment in self.scene.fragments() {
            painter
                .fill(fragment.clip(), diagnostic_fill(fragment.paint()))
                .transform(self.placement)
                .draw();
        }
    }

    fn paint_diagnostics_above<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        let line_stroke = Stroke::new(1.0);
        for line in self.scene.lines() {
            painter
                .stroke(
                    line.bounds(),
                    &line_stroke,
                    Color::from_rgba8(0x4d, 0xd5, 0xe7, 0x58),
                )
                .transform(self.placement)
                .draw();
        }

        let semantic_stroke = Stroke::new(1.0).with_dashes(0.0, [5.0, 5.0]);
        for semantic in self.scene.semantics() {
            painter
                .stroke(
                    semantic.bounds(),
                    &semantic_stroke,
                    Color::from_rgba8(0xf5, 0xc4, 0x51, 0x68),
                )
                .transform(self.placement)
                .draw();
        }

        let clip_stroke = Stroke::new(1.5);
        for fragment in self.scene.fragments() {
            painter
                .stroke(
                    fragment.clip(),
                    &clip_stroke,
                    diagnostic_edge(fragment.paint()),
                )
                .transform(self.placement)
                .draw();
        }

        let fragment = self
            .scene
            .fragments()
            .first()
            .expect("visual proof scene must contain a fragment");
        let point = fragment.clip().center();
        let hit = self
            .scene
            .hit_test(point)
            .expect("visual proof fragment center must hit");
        let caret = self.scene.caret(&hit);
        painter
            .fill(caret.bounds(), INK_COLOR)
            .transform(self.placement)
            .draw();
        painter
            .fill(
                Circle::new(hit.point(), 4.0),
                Color::from_rgba8(0xee, 0xf3, 0xf8, 0xb0),
            )
            .transform(self.placement)
            .draw();
    }
}

fn main() -> Result<(), AnyError> {
    let image = render_poster()?;
    let path = snapshot_path();
    write_png(&path, &image)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn render_poster() -> Result<RgbaImage, AnyError> {
    let mut layout = layout_engine()?;
    let proof = retained_proof(&mut layout)?;

    let hero = layout_scene(
        &mut layout,
        0x21,
        230.0,
        1_200.0,
        &[
            Piece::new("of", InlineRole::TEXT, CORAL),
            Piece::new("fice", InlineRole::EMPHASIS, CYAN),
        ],
    )?;
    let arabic = layout_scene(
        &mut layout,
        0x22,
        104.0,
        1_250.0,
        &[Piece::new("مرحبا بالعالم", InlineRole::TEXT, GOLD)],
    )?;

    let (ligature_id, ligature_clips) = split_ligature_evidence(&hero)
        .expect("poster must paint one real ligature through multiple source clips");
    assert_eq!(
        hero.fragments()[0].font().data.as_ref(),
        LATIN_FONT_BYTES,
        "Latin poster text must retain the bundled Roboto Flex resource"
    );
    assert!(
        hero.semantics()
            .any(|fragment| fragment.inline_role() == Some(InlineRole::EMPHASIS)),
        "the diagnostic overlay must be backed by real inline semantics"
    );
    let arabic_fragment = arabic
        .fragments()
        .iter()
        .find(|fragment| fragment.script() == *b"Arab" && fragment.bidi_level() & 1 == 1)
        .expect("poster must contain real right-to-left Arabic shaping");
    assert!(
        arabic_fragment.bidi_level() & 1 == 1,
        "poster must contain real right-to-left Arabic shaping"
    );
    assert_eq!(
        arabic_fragment.font().data.as_ref(),
        ARABIC_FONT_BYTES,
        "Arabic poster text must select the bundled Noto Kufi fallback"
    );

    let title = layout_label(&mut layout, 0x23, 72.0, "UNDERWOOD", INK)?;
    let kicker = layout_label(
        &mut layout,
        0x24,
        18.0,
        "SEMANTIC TEXT / RETAINED SCENE / DETERMINISTIC PIXELS",
        MUTED,
    )?;
    let statement = layout_label(
        &mut layout,
        0x25,
        24.0,
        "ONE GLYPH / TWO PAINTS / ZERO SHORTCUTS",
        INK,
    )?;
    let ligature_evidence = layout_label(
        &mut layout,
        0x2d,
        18.0,
        &format!("GLYPH {ligature_id} / {ligature_clips} SOURCE CLIPS"),
        CORAL,
    )?;
    let arabic_evidence = layout_label(
        &mut layout,
        0x2e,
        18.0,
        &format!(
            "SCRIPT ARAB / BIDI LEVEL {} / REAL FALLBACK",
            arabic_fragment.bidi_level()
        ),
        GOLD,
    )?;
    let edit_label = layout_label(&mut layout, 0x26, 18.0, "LOCAL EDIT", CORAL)?;
    let edit_value = layout_label(
        &mut layout,
        0x27,
        28.0,
        &format!("{} PARAGRAPH RESHAPED", proof.reshaped),
        INK,
    )?;
    let reuse_label = layout_label(&mut layout, 0x28, 18.0, "RETAINED", CYAN)?;
    let reuse_value = layout_label(
        &mut layout,
        0x29,
        28.0,
        &format!("{} SIBLING REUSED", proof.reused),
        INK,
    )?;
    let paint_label = layout_label(&mut layout, 0x2a, 18.0, "PAINT ONLY", GOLD)?;
    let paint_value = layout_label(
        &mut layout,
        0x2b,
        28.0,
        &format!("{} PARAGRAPHS RESHAPED", proof.paint_reshaped),
        INK,
    )?;
    let footer = layout_label(
        &mut layout,
        0x2c,
        17.0,
        "DOCUMENT  /  PARLEY  /  TEXTSCENE  /  IMAGING  /  VELLO CPU",
        MUTED,
    )?;

    let mut scene = record::Scene::new();
    {
        let mut painter = Painter::new(&mut scene);
        paint_backdrop(&mut painter);

        TextSceneAdapter::new(&kicker, 124.0, 44.0).paint_into(&mut painter);
        TextSceneAdapter::new(&title, 120.0, 70.0).paint_into(&mut painter);
        TextSceneAdapter::new(&ligature_evidence, 124.0, 180.0).paint_into(&mut painter);
        TextSceneAdapter::new(&hero, 116.0, 205.0)
            .with_diagnostics()
            .paint_into(&mut painter);
        TextSceneAdapter::new(&arabic_evidence, 856.0, 180.0).paint_into(&mut painter);
        TextSceneAdapter::new(&arabic, 850.0, 245.0).paint_into(&mut painter);
        TextSceneAdapter::new(&statement, 124.0, 608.0).paint_into(&mut painter);

        paint_card(
            &mut painter,
            Rect::new(120.0, 710.0, 545.0, 895.0),
            CORAL_COLOR,
        );
        paint_card(
            &mut painter,
            Rect::new(588.0, 710.0, 1_013.0, 895.0),
            CYAN_COLOR,
        );
        paint_card(
            &mut painter,
            Rect::new(1_056.0, 710.0, 1_480.0, 895.0),
            GOLD_COLOR,
        );

        TextSceneAdapter::new(&edit_label, 148.0, 738.0).paint_into(&mut painter);
        TextSceneAdapter::new(&edit_value, 148.0, 786.0).paint_into(&mut painter);
        TextSceneAdapter::new(&reuse_label, 616.0, 738.0).paint_into(&mut painter);
        TextSceneAdapter::new(&reuse_value, 616.0, 786.0).paint_into(&mut painter);
        TextSceneAdapter::new(&paint_label, 1_084.0, 738.0).paint_into(&mut painter);
        TextSceneAdapter::new(&paint_value, 1_084.0, 786.0).paint_into(&mut painter);
        TextSceneAdapter::new(&footer, 124.0, 936.0).paint_into(&mut painter);
    }
    scene.validate()?;

    let mut renderer = VelloCpuRenderer::new(WIDTH, HEIGHT);
    Ok(renderer.render_scene(&scene, WIDTH, HEIGHT)?)
}

fn layout_engine() -> Result<LayoutEngine, AnyError> {
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT_BYTES)?,
        Font::from_bytes("arabic", ARABIC_FONT_BYTES)?,
    ])?;
    let paragraphs = ParleyParagraphEngine::new(TextData::compiled_minimal(), fonts)?;
    Ok(LayoutEngine::new(paragraphs))
}

fn retained_proof(layout: &mut LayoutEngine) -> Result<RetainedProof, AnyError> {
    let mut document = Document::new(DocumentId::from_bytes([0x31; 16]));
    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let prefix = edit.append_text(first, InlineRole::TEXT, "of")?;
    let suffix = edit.append_text(first, InlineRole::EMPHASIS, "fice")?;
    let second = edit.append_paragraph(ParagraphRole::BODY)?;
    edit.append_text(second, InlineRole::TEXT, "unchanged sibling")?;
    let published = edit.commit()?;

    let mut styles = StyleMap::new(TextStyle::new(40.0, INK)?);
    styles.set_paint(prefix, CORAL)?;
    styles.set_paint(suffix, CYAN)?;
    let paint = poster_paints();
    let request = SceneRequest::new(FiniteWidth::new(700.0)?, &styles, &paint);
    let initial = layout.prepare(published.snapshot(), &request)?;
    assert_split_ligature(initial.scene());

    let mut edit = document.edit();
    edit.replace_text(suffix, "fices")?;
    let changed = edit.commit()?;
    let edited = layout.prepare(changed.snapshot(), &request)?;
    assert_eq!(
        edited.work().shape().paragraphs(),
        1,
        "only the edited paragraph may be reshaped"
    );
    assert_eq!(
        edited.work().reused_paragraphs(),
        1,
        "the unchanged sibling must be retained"
    );

    let recolored = paint.with_brush(CYAN, Brush::Solid(GOLD_COLOR))?;
    let paint_request = SceneRequest::new(FiniteWidth::new(700.0)?, &styles, &recolored);
    let paint_only = layout.prepare(changed.snapshot(), &paint_request)?;
    assert_eq!(
        paint_only.work().analysis().paragraphs(),
        0,
        "paint-only work must not repeat analysis"
    );
    assert_eq!(
        paint_only.work().shape().paragraphs(),
        0,
        "paint-only work must not repeat shaping"
    );
    assert_eq!(
        paint_only.work().flow().paragraphs(),
        0,
        "paint-only work must not repeat flow"
    );

    Ok(RetainedProof {
        reshaped: edited.work().shape().paragraphs(),
        reused: edited.work().reused_paragraphs(),
        paint_reshaped: paint_only.work().shape().paragraphs(),
    })
}

fn layout_label(
    layout: &mut LayoutEngine,
    document_byte: u8,
    font_size: f32,
    text: &str,
    paint: PaintSlot,
) -> Result<TextScene, AnyError> {
    layout_scene(
        layout,
        document_byte,
        font_size,
        1_400.0,
        &[Piece::new(text, InlineRole::TEXT, paint)],
    )
}

fn layout_scene(
    layout: &mut LayoutEngine,
    document_byte: u8,
    font_size: f32,
    width: f64,
    pieces: &[Piece<'_>],
) -> Result<TextScene, AnyError> {
    let mut document = Document::new(DocumentId::from_bytes([document_byte; 16]));
    let mut edit = document.edit();
    let paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
    let mut authored = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let text = edit.append_text(paragraph, piece.role, piece.text)?;
        authored.push((text, piece.paint));
    }
    let published = edit.commit()?;

    let mut styles = StyleMap::new(TextStyle::new(font_size, INK)?);
    for (text, paint) in authored {
        styles.set_paint(text, paint)?;
    }
    let paints = poster_paints();
    let request = SceneRequest::new(FiniteWidth::new(width)?, &styles, &paints);
    let output = layout.prepare(published.snapshot(), &request)?;
    Ok(output.scene().clone())
}

fn poster_paints() -> PaintTable {
    PaintTable::from_brushes([
        Brush::Solid(INK_COLOR),
        Brush::Solid(CYAN_COLOR),
        Brush::Solid(CORAL_COLOR),
        Brush::Solid(GOLD_COLOR),
        Brush::Solid(MUTED_COLOR),
    ])
}

fn assert_split_ligature(scene: &TextScene) {
    assert!(
        split_ligature_evidence(scene).is_some(),
        "poster must paint one real ligature through multiple source clips"
    );
}

fn split_ligature_evidence(scene: &TextScene) -> Option<(u32, usize)> {
    scene.fragments().iter().find_map(|left| {
        let glyph = &left.glyphs()[0];
        let matching = scene.fragments().iter().filter(|right| {
            glyph.id() == right.glyphs()[0].id() && glyph.position() == right.glyphs()[0].position()
        });
        let clips = matching.clone().count();
        let proves_source_partition = matching.into_iter().any(|right| {
            left.paint() != right.paint()
                && left.source() != right.source()
                && left.clip() != right.clip()
        });
        proves_source_partition.then_some((glyph.id(), clips))
    })
}

fn paint_backdrop<S: PaintSink + ?Sized>(painter: &mut Painter<'_, S>) {
    painter.fill_rect(
        Rect::new(0.0, 0.0, f64::from(WIDTH), f64::from(HEIGHT)),
        BACKGROUND,
    );
    painter.fill_rect(Rect::new(76.0, 0.0, 80.0, f64::from(HEIGHT)), CORAL_COLOR);
    painter.fill_rect(Rect::new(84.0, 0.0, 87.0, f64::from(HEIGHT)), CYAN_COLOR);

    for x in [354.0, 824.0, 1_294.0] {
        painter.fill_rect(
            Rect::new(x, 0.0, x + 1.0, f64::from(HEIGHT)),
            Color::from_rgba8(0x78, 0x8a, 0xa3, 0x14),
        );
    }
    painter.fill_rect(
        Rect::new(120.0, 675.0, 1_480.0, 676.0),
        Color::from_rgba8(0x78, 0x8a, 0xa3, 0x38),
    );

    for radius in [22.0, 39.0, 58.0] {
        painter
            .stroke(
                Circle::new((1_408.0, 102.0), radius),
                &Stroke::new(1.0),
                Color::from_rgba8(0x78, 0x8a, 0xa3, 0x30),
            )
            .draw();
    }
    painter
        .fill(Circle::new((1_408.0, 102.0), 4.0), GOLD_COLOR)
        .draw();
}

fn paint_card<S: PaintSink + ?Sized>(painter: &mut Painter<'_, S>, rect: Rect, accent: Color) {
    painter
        .fill(RoundedRect::from_rect(rect, 18.0), PANEL)
        .draw();
    painter
        .stroke(
            RoundedRect::from_rect(rect, 18.0),
            &Stroke::new(1.0),
            PANEL_EDGE,
        )
        .draw();
    painter.fill_rect(Rect::new(rect.x0, rect.y0, rect.x0 + 7.0, rect.y1), accent);
    painter.fill_rect(
        Rect::new(
            rect.x0 + 28.0,
            rect.y1 - 24.0,
            rect.x1 - 28.0,
            rect.y1 - 22.0,
        ),
        Color::from_rgba8(0x78, 0x8a, 0xa3, 0x24),
    );
}

fn diagnostic_fill(slot: PaintSlot) -> Color {
    if slot == CORAL {
        Color::from_rgba8(0xff, 0x6b, 0x67, 0x14)
    } else if slot == CYAN {
        Color::from_rgba8(0x4d, 0xd5, 0xe7, 0x14)
    } else {
        Color::from_rgba8(0xf5, 0xc4, 0x51, 0x10)
    }
}

fn diagnostic_edge(slot: PaintSlot) -> Color {
    if slot == CORAL {
        Color::from_rgba8(0xff, 0x6b, 0x67, 0xa0)
    } else if slot == CYAN {
        Color::from_rgba8(0x4d, 0xd5, 0xe7, 0xa0)
    } else {
        Color::from_rgba8(0xf5, 0xc4, 0x51, 0x90)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "imaging glyph coordinates are f32; reject non-finite or out-of-range scene values first"
)]
fn imaging_coord(value: f64) -> f32 {
    assert!(
        value.is_finite() && value >= f64::from(f32::MIN) && value <= f64::from(f32::MAX),
        "scene coordinate must be finite and representable by imaging"
    );
    value as f32
}

fn snapshot_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("snapshots")
        .join("underwood-visual-proof.png")
}

fn write_png(path: &Path, image: &RgbaImage) -> Result<(), AnyError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data)?;
    Ok(())
}

#[cfg(test)]
fn read_png(path: &Path) -> Result<RgbaImage, AnyError> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut data = vec![0; reader.output_buffer_size().ok_or("snapshot is too large")?];
    let info = reader.next_frame(&mut data)?;
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    data.truncate(info.buffer_size());
    Ok(RgbaImage {
        width: info.width,
        height: info.height,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poster_matches_committed_cpu_snapshot() {
        let actual = render_poster().expect("poster must render through the complete public path");
        let expected = read_png(&snapshot_path()).expect("committed poster snapshot must decode");
        assert_eq!(actual, expected, "rendered poster pixels drifted");
    }
}
