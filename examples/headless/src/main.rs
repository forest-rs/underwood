// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! External headless exercise of Underwood's first semantic-to-scene slice.

use underwood::{
    Brush, Color, Document, DocumentId, FiniteWidth, InlineRole, LayoutEngine, PaintSlot,
    PaintTable, ParagraphRole, SceneRequest, StyleMap, TextStyle,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"underwood-demo-1"));

    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_prefix = edit.append_text(first, InlineRole::TEXT, "of")?;
    let first_suffix = edit.append_text(first, InlineRole::EMPHASIS, "fice مرحبا")?;
    let second = edit.append_paragraph(ParagraphRole::BODY)?;
    let second_text = edit.append_text(second, InlineRole::TEXT, "unchanged sibling")?;
    let published = edit.commit()?;
    let old_snapshot = published.snapshot().clone();

    let base = TextStyle::new(16.0, PaintSlot::new(0))?;
    let mut styles = StyleMap::new(base);
    styles.set_paint(first_prefix, PaintSlot::new(0))?;
    styles.set_paint(first_suffix, PaintSlot::new(1))?;

    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::from_rgb8(0x20, 0x20, 0x20)),
        Brush::Solid(Color::from_rgb8(0x20, 0x50, 0xa0)),
    ]);

    let fonts = FontSet::try_from_fonts([
        Font::from_bytes(
            "latin",
            include_bytes!("../fonts/RobotoFlex-VariableFont.ttf"),
        )?,
        Font::from_bytes(
            "arabic",
            include_bytes!("../fonts/NotoKufiArabic-Regular.otf"),
        )?,
    ])?;
    let data = TextData::compiled_minimal();
    let paragraphs = ParleyParagraphEngine::new(data, fonts)?;
    let mut layout = LayoutEngine::new(paragraphs);
    let request = SceneRequest::new(FiniteWidth::new(420.0)?, &styles, &paint);

    let first_scene = layout.prepare(published.snapshot(), &request)?;
    assert!(
        first_scene.scene().lines().len() >= 2,
        "two semantic paragraphs must produce at least two visual lines"
    );
    let fragment = &first_scene.scene().fragments()[0];
    assert!(
        !fragment.glyphs().is_empty(),
        "the first real Parley fragment must contain shaped glyphs"
    );
    assert!(
        fragment.source().is_some(),
        "authored glyph fragments must retain snapshot source"
    );
    assert!(
        !fragment.font().data.is_empty(),
        "scene fragments must retain exact font bytes"
    );
    assert!(
        first_scene.scene().fragments().iter().any(|left| {
            first_scene.scene().fragments().iter().any(|right| {
                left.paint() != right.paint()
                    && left.glyphs()[0].id() == right.glyphs()[0].id()
                    && left.glyphs()[0].position() == right.glyphs()[0].position()
            })
        }),
        "one shaped ligature must lower into multiple paint clips without reshaping"
    );
    assert!(
        first_scene
            .scene()
            .fragments()
            .iter()
            .any(|fragment| fragment.script() == *b"Arab" && fragment.bidi_level() & 1 == 1),
        "the real scene must retain an itemized right-to-left Arabic run"
    );
    let hit_point = fragment.clip().center();
    let hit = first_scene
        .scene()
        .hit_test(hit_point)
        .expect("the first fragment must be hittable");
    let caret = first_scene.scene().caret(&hit);
    assert!(
        caret.bounds().height() > 0.0,
        "a scene hit must produce visible caret geometry"
    );
    assert_eq!(
        hit.source().revision(),
        published.snapshot().revision(),
        "hit source must name the exact scene snapshot"
    );
    assert!(
        first_scene
            .scene()
            .semantics()
            .any(|fragment| { fragment.inline_role() == Some(InlineRole::EMPHASIS) }),
        "inline emphasis must survive projection into scene semantics"
    );

    let mut edit = document.edit();
    edit.replace_text(first_suffix, "fices مرحبا")?;
    let changed = edit.commit()?;
    assert_eq!(
        old_snapshot.text(second_text),
        Some("unchanged sibling"),
        "publishing a later revision must not mutate an old snapshot"
    );

    let second_scene = layout.prepare(changed.snapshot(), &request)?;
    assert_eq!(
        second_scene.work().analysis().paragraphs(),
        1,
        "only the edited paragraph may be reanalyzed"
    );
    assert_eq!(
        second_scene.work().shape().paragraphs(),
        1,
        "only the edited paragraph may be reshaped"
    );
    assert_eq!(
        second_scene.work().reused_paragraphs(),
        1,
        "the unchanged sibling paragraph must be reused"
    );

    let recolored = paint.with_brush(
        PaintSlot::new(1),
        Brush::Solid(Color::from_rgb8(0xa0, 0x20, 0x20)),
    )?;
    let paint_request = SceneRequest::new(FiniteWidth::new(420.0)?, &styles, &recolored);
    let paint_scene = layout.prepare(changed.snapshot(), &paint_request)?;
    assert_eq!(
        paint_scene.work().analysis().paragraphs(),
        0,
        "paint values must not invalidate analysis"
    );
    assert_eq!(
        paint_scene.work().shape().paragraphs(),
        0,
        "paint values must not invalidate shaping"
    );
    assert_eq!(
        paint_scene.work().flow().paragraphs(),
        0,
        "paint values must not invalidate flow"
    );
    assert_ne!(
        second_scene.scene().paint(),
        paint_scene.scene().paint(),
        "paint-only updates must still reach the scene"
    );

    let narrow_request = SceneRequest::new(FiniteWidth::new(90.0)?, &styles, &recolored);
    let narrow_scene = layout.prepare(changed.snapshot(), &narrow_request)?;
    assert_eq!(
        narrow_scene.work().analysis().paragraphs(),
        0,
        "width must not invalidate analysis"
    );
    assert_eq!(
        narrow_scene.work().shape().paragraphs(),
        0,
        "width must not invalidate shaping"
    );
    assert_eq!(
        narrow_scene.work().flow().paragraphs(),
        2,
        "width must reflow both paragraphs"
    );
    assert!(
        narrow_scene.scene().lines().len() > paint_scene.scene().lines().len(),
        "narrow width must produce additional visual lines"
    );

    println!(
        "underwood scene: {} lines, {} fragments, {} paint slots",
        first_scene.scene().lines().len(),
        first_scene.scene().fragments().len(),
        first_scene.scene().paint().len(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn complete_public_path_executes() {
        super::main().expect("the complete external public path must pass");
    }
}
