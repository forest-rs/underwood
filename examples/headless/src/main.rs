// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! External headless exercise of Underwood's first semantic-to-scene slice.

use underwood::{
    Brush, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth, InlineFlowStyle,
    InlineRole, Language, LayoutEngine, LineHeight, PaintSlot, PaintTable, ParagraphRole,
    SceneRequest, ShapingStyle, StyleMap, Tag, TextId, TextScene,
};
use underwood::{FontFeature, FontVariation};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"underwood-demo-1"));

    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_prefix = edit.append_text(first, InlineRole::TEXT, "of")?;
    let first_suffix = edit.append_text(first, InlineRole::EMPHASIS, "fice مرحبا")?;
    let second = edit.append_paragraph(ParagraphRole::BODY)?;
    let second_text = edit.append_text(second, InlineRole::TEXT, "unchanged sibling")?;
    let variable = edit.append_paragraph(ParagraphRole::BODY)?;
    let variable_light = edit.append_text(variable, InlineRole::TEXT, "Flex")?;
    let variable_black = edit.append_text(variable, InlineRole::EMPHASIS, "Flex")?;
    let features = edit.append_paragraph(ParagraphRole::BODY)?;
    let ligatures_on = edit.append_text(features, InlineRole::TEXT, "office")?;
    let ligatures_off = edit.append_text(features, InlineRole::EMPHASIS, "office")?;
    let published = edit.commit()?;
    let old_snapshot = published.snapshot().clone();

    let english = Language::parse("en")?;
    let wght = Tag::new(b"wght");
    let opsz = Tag::new(b"opsz");
    let liga = Tag::new(b"liga");
    let base_shaping = ShapingStyle::new(16.0)?.with_language(Some(english));
    let base = ComputedInlineStyle::new(
        base_shaping.clone(),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let light_style = ComputedInlineStyle::new(
        ShapingStyle::new(24.0)?
            .with_language(Some(english))
            .with_variations([
                FontVariation::new(wght, 100.0),
                FontVariation::new(opsz, 8.0),
            ])?,
        InlineFlowStyle::new(LineHeight::from_multiplier(1.1)?),
        PaintSlot::new(0),
    );
    let black_style = ComputedInlineStyle::new(
        ShapingStyle::new(42.0)?
            .with_language(Some(english))
            .with_variations([
                FontVariation::new(wght, 900.0),
                FontVariation::new(opsz, 144.0),
            ])?,
        InlineFlowStyle::new(LineHeight::from_multiplier(1.4)?),
        PaintSlot::new(1),
    );
    let ligatures_on_style = base.clone().with_shaping(
        base_shaping
            .clone()
            .with_features([FontFeature::new(liga, 1)]),
    );
    let ligatures_off_style = base
        .clone()
        .with_shaping(base_shaping.with_features([FontFeature::new(liga, 0)]));
    let mut styles = StyleMap::new(base.clone());
    styles.set(first_prefix, base.clone());
    styles.set(first_suffix, base.clone().with_paint(PaintSlot::new(1)));
    styles.set(variable_light, light_style.clone());
    styles.set(variable_black, black_style);
    styles.set(ligatures_on, ligatures_on_style.clone());
    styles.set(ligatures_off, ligatures_off_style);

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
        first_scene.scene().lines().len() >= 4,
        "four semantic paragraphs must produce at least four visual lines"
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
    assert_eq!(
        fragment.font_size(),
        16.0,
        "scene fragments must retain the font scale required for rendering"
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
    assert_eq!(
        glyph_count(first_scene.scene(), ligatures_on),
        4,
        "explicit liga-on office must substitute ffi to one glyph"
    );
    assert_eq!(
        glyph_count(first_scene.scene(), ligatures_off),
        6,
        "explicit liga-off office must preserve six glyphs"
    );
    let light_coords = coordinates(first_scene.scene(), variable_light);
    let black_coords = coordinates(first_scene.scene(), variable_black);
    assert!(
        !light_coords.is_empty(),
        "explicit axes must resolve coordinates"
    );
    assert_ne!(
        light_coords, black_coords,
        "wght and opsz specimens must produce distinct font instances"
    );
    assert!(
        first_scene
            .scene()
            .fragments()
            .iter()
            .any(|fragment| fragment.font_size() == 42.0),
        "one document must carry heterogeneous font sizes"
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
        3,
        "all three unchanged sibling paragraphs must be reused"
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

    let mut reassigned_paint = styles.clone();
    reassigned_paint.set(first_suffix, base);
    let reassigned_request =
        SceneRequest::new(FiniteWidth::new(420.0)?, &reassigned_paint, &recolored);
    let reassigned_scene = layout.prepare(changed.snapshot(), &reassigned_request)?;
    assert_eq!(
        reassigned_scene.work().shape().paragraphs(),
        0,
        "paint-slot assignment must not invalidate shaping"
    );
    assert_eq!(
        reassigned_scene.work().flow().paragraphs(),
        0,
        "paint-slot assignment must retain flow geometry"
    );
    assert!(
        reassigned_scene
            .scene()
            .fragments()
            .iter()
            .filter(|fragment| {
                fragment
                    .source()
                    .is_some_and(|source| source.text() == first_suffix)
            })
            .all(|fragment| fragment.paint() == PaintSlot::new(0)),
        "retained geometry must still receive the new paint-slot assignment"
    );

    let mut shaping_styles = reassigned_paint.clone();
    shaping_styles.set(ligatures_off, ligatures_on_style);
    let shaping_request = SceneRequest::new(FiniteWidth::new(420.0)?, &shaping_styles, &recolored);
    let shaping_scene = layout.prepare(changed.snapshot(), &shaping_request)?;
    assert_eq!(
        shaping_scene.work().analysis().paragraphs(),
        0,
        "feature changes must reuse Unicode analysis"
    );
    assert_eq!(
        shaping_scene.work().itemization().paragraphs(),
        1,
        "only the feature-changed paragraph may be reitemized"
    );
    assert_eq!(
        shaping_scene.work().shape().paragraphs(),
        1,
        "only the feature-changed paragraph may be reshaped"
    );

    let mut flow_styles = shaping_styles.clone();
    flow_styles.set(
        variable_light,
        light_style.with_inline_flow(InlineFlowStyle::new(LineHeight::from_multiplier(1.8)?)),
    );
    let flow_request = SceneRequest::new(FiniteWidth::new(420.0)?, &flow_styles, &recolored);
    let flow_scene = layout.prepare(changed.snapshot(), &flow_request)?;
    assert_eq!(
        flow_scene.work().analysis().paragraphs(),
        0,
        "line height must not invalidate analysis"
    );
    assert_eq!(
        flow_scene.work().itemization().paragraphs(),
        0,
        "line height must not invalidate itemization"
    );
    assert_eq!(
        flow_scene.work().shape().paragraphs(),
        0,
        "line height must not invalidate shaping"
    );
    assert_eq!(
        flow_scene.work().flow().paragraphs(),
        1,
        "line height must rebuild only its paragraph geometry"
    );

    let narrow_request = SceneRequest::new(FiniteWidth::new(90.0)?, &flow_styles, &recolored);
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
        4,
        "width must reflow all four paragraphs"
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

fn glyph_count(scene: &TextScene, text: TextId) -> usize {
    scene
        .fragments()
        .iter()
        .filter(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == text)
        })
        .map(|fragment| fragment.glyphs().len())
        .sum()
}

fn coordinates(scene: &TextScene, text: TextId) -> Vec<i16> {
    scene
        .fragments()
        .iter()
        .find(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == text)
        })
        .map(|fragment| fragment.normalized_coords().to_vec())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn complete_public_path_executes() {
        super::main().expect("the complete external public path must pass");
    }
}
