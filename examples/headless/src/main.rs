// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! External headless exercise of Underwood's first semantic-to-scene slice.

use underwood::adapter::PreparationErrorKind;
use underwood::{
    Brush, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth, GenericFamily,
    InlineFlowStyle, InlineRole, Language, LayoutEngine, LineHeight, PaintSlot, PaintTable,
    ParagraphRole, SceneRequest, Script, ShapingStyle, StyleMap, Tag, TextId, TextScene,
};
use underwood::{FontFeature, FontStyle, FontVariation, FontWeight, FontWidth};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"underwood-demo-1"));

    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_prefix = edit.append_text(first, InlineRole::TEXT, "j / ")?;
    let first_suffix = edit.append_text(first, InlineRole::EMPHASIS, "office ")?;
    let first_arabic = edit.append_text(first, InlineRole::EMPHASIS, "مرحبا")?;
    let direct_arabic = edit.append_text(first, InlineRole::TEXT, " خط")?;
    let second = edit.append_paragraph(ParagraphRole::BODY)?;
    let second_text = edit.append_text(second, InlineRole::TEXT, "unchanged sibling")?;
    let variable = edit.append_paragraph(ParagraphRole::BODY)?;
    let variable_light = edit.append_text(variable, InlineRole::TEXT, "Flex")?;
    let variable_black = edit.append_text(variable, InlineRole::EMPHASIS, "Flex")?;
    let variable_override = edit.append_text(variable, InlineRole::TEXT, "Flex")?;
    let features = edit.append_paragraph(ParagraphRole::BODY)?;
    let ligatures_on = edit.append_text(features, InlineRole::TEXT, "office")?;
    let ligatures_off = edit.append_text(features, InlineRole::EMPHASIS, "office")?;
    let published = edit.commit()?;
    let old_snapshot = published.snapshot().clone();

    let english = Language::parse("en")?;
    let arabic = Language::parse("ar")?;
    let wght = Tag::new(b"wght");
    let opsz = Tag::new(b"opsz");
    let liga = Tag::new(b"liga");
    let base_shaping = ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 16.0)?;
    let base = ComputedInlineStyle::new(
        base_shaping.clone(),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let light_style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 24.0)?
            .with_language(Some(english))
            .with_font_weight(FontWeight::THIN)?
            .with_font_width(FontWidth::CONDENSED)?
            .with_variations([FontVariation::new(opsz, 8.0)])?,
        InlineFlowStyle::new(LineHeight::from_multiplier(1.1)?),
        PaintSlot::new(0),
    );
    let black_style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 42.0)?
            .with_language(Some(english))
            .with_font_weight(FontWeight::BLACK)?
            .with_font_width(FontWidth::EXPANDED)?
            .with_variations([FontVariation::new(opsz, 144.0)])?,
        InlineFlowStyle::new(LineHeight::from_multiplier(1.4)?),
        PaintSlot::new(1),
    );
    let override_style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 24.0)?
            .with_language(Some(english))
            .with_font_weight(FontWeight::BLACK)?
            .with_font_width(FontWidth::CONDENSED)?
            .with_variations([
                FontVariation::new(wght, 100.0),
                FontVariation::new(opsz, 8.0),
            ])?,
        InlineFlowStyle::new(LineHeight::from_multiplier(1.1)?),
        PaintSlot::new(0),
    );
    let arabic_style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Absent Primary Family"), 16.0)?
            .with_language(Some(arabic))
            .with_font_style(FontStyle::Oblique(Some(14.0)))?,
        InlineFlowStyle::default(),
        PaintSlot::new(1),
    );
    let direct_arabic_style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Noto Kufi Arabic"), 16.0)?
            .with_language(Some(arabic)),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
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
    styles.set(first_arabic, arabic_style);
    styles.set(direct_arabic, direct_arabic_style);
    styles.set(variable_light, light_style.clone());
    styles.set(variable_black, black_style);
    styles.set(variable_override, override_style);
    styles.set(ligatures_on, ligatures_on_style.clone());
    styles.set(ligatures_off, ligatures_off_style);

    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::from_rgb8(0x20, 0x20, 0x20)),
        Brush::Solid(Color::from_rgb8(0x20, 0x50, 0xa0)),
    ]);

    let fonts = font_catalog()?;
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
        first_scene
            .scene()
            .fragments()
            .iter()
            .all(|fragment| fragment.paint_clip().is_none()),
        "ordinary whole-glyph paint must not manufacture outline-derived clips"
    );
    assert!(
        first_scene
            .scene()
            .fragments()
            .iter()
            .any(|fragment| fragment.script() == *b"Arab" && fragment.bidi_level() & 1 == 1),
        "the real scene must retain an itemized right-to-left Arabic run"
    );
    let arabic_fragment = first_scene
        .scene()
        .fragments()
        .iter()
        .find(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == first_arabic)
        })
        .expect("Arabic fallback leaf must produce a scene fragment");
    assert_eq!(
        arabic_fragment.font().data.as_ref(),
        include_bytes!("../fonts/NotoKufiArabic-Regular.otf"),
        "Arab+ar fallback must skip the absent primary and select Noto Kufi"
    );
    assert_eq!(
        arabic_fragment.synthesis().skew_degrees(),
        Some(14.0),
        "the static fallback must retain Fontique's synthetic oblique evidence"
    );
    let zero_advance_mark = first_scene
        .scene()
        .fragments()
        .iter()
        .find(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == first_arabic)
                && fragment.glyphs()[0].advance().x == 0.0
        })
        .expect("Noto Kufi must expose a zero-advance Arabic mark");
    assert!(
        zero_advance_mark.paint_clip().is_none(),
        "a zero-advance Arabic glyph must reach the renderer without an ordinary paint clip"
    );
    let arabic_visual_sources: Vec<_> = first_scene
        .scene()
        .fragments()
        .iter()
        .filter_map(|fragment| {
            fragment
                .source()
                .filter(|source| source.text() == first_arabic)
                .map(|source| source.bytes())
        })
        .collect();
    assert!(
        arabic_visual_sources.len() > 1
            && arabic_visual_sources
                .first()
                .expect("length was checked")
                .start
                > arabic_visual_sources
                    .last()
                    .expect("length was checked")
                    .start
            && arabic_visual_sources
                .windows(2)
                .all(|pair| pair[0].start >= pair[1].start),
        "RTL interaction units must lower in visual order while retaining logical source ranges: {arabic_visual_sources:?}"
    );
    let direct_arabic_fragment = first_scene
        .scene()
        .fragments()
        .iter()
        .find(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == direct_arabic)
        })
        .expect("direct named-family leaf must produce a scene fragment");
    assert_eq!(
        direct_arabic_fragment.font().data.as_ref(),
        include_bytes!("../fonts/NotoKufiArabic-Regular.otf"),
        "a direct Noto Kufi family request must select the bundled resource"
    );
    let hit_point = first_scene
        .scene()
        .semantics()
        .find(|semantic| {
            semantic
                .source()
                .is_some_and(|source| source.text() == first_prefix)
        })
        .expect("the authored prefix must expose semantic interaction geometry")
        .bounds()
        .center();
    let hit = first_scene
        .scene()
        .hit_test(hit_point)
        .expect("the first fragment must be hittable");
    let caret = first_scene
        .scene()
        .caret(hit.position())
        .expect("the hit position must resolve in its source scene");
    assert!(
        caret.bounds().height() > 0.0,
        "a scene hit must produce visible caret geometry"
    );
    assert!(
        hit.source()
            .sources()
            .iter()
            .all(|source| source.revision() == published.snapshot().revision()),
        "every hit source must name the exact scene snapshot"
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
    assert!(
        first_scene.scene().fragments().iter().any(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == ligatures_on && source.bytes() == (1..4))
        }),
        "the retained ffi glyph must own the full three-character source range"
    );
    let light_coords = coordinates(first_scene.scene(), variable_light);
    let black_coords = coordinates(first_scene.scene(), variable_black);
    let override_coords = coordinates(first_scene.scene(), variable_override);
    assert!(
        !light_coords.is_empty(),
        "explicit axes must resolve coordinates"
    );
    assert_ne!(
        light_coords, black_coords,
        "wght and opsz specimens must produce distinct font instances"
    );
    assert_eq!(
        light_coords, override_coords,
        "explicit wght must override Fontique's synthesized wght coordinate"
    );
    assert_ne!(
        synthesis_variations(first_scene.scene(), variable_light),
        synthesis_variations(first_scene.scene(), variable_override),
        "resolver evidence must retain the two different requested weights"
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
    edit.replace_text(first_suffix, "offices ")?;
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

    font_request_invalidation_proof()?;

    println!(
        "underwood scene: {} lines, {} fragments, {} paint slots",
        first_scene.scene().lines().len(),
        first_scene.scene().fragments().len(),
        first_scene.scene().paint().len(),
    );
    Ok(())
}

fn font_catalog() -> Result<FontSet, Box<dyn std::error::Error>> {
    let arabic = Language::parse("ar")?;
    Ok(FontSet::try_from_fonts([
        Font::from_bytes(
            "latin",
            include_bytes!("../fonts/RobotoFlex-VariableFont.ttf"),
        )?,
        Font::from_bytes(
            "arabic",
            include_bytes!("../fonts/NotoKufiArabic-Regular.otf"),
        )?,
    ])?
    .with_generic_families(GenericFamily::SansSerif, ["Roboto Flex"])?
    .with_fallbacks(
        Script::from_bytes(*b"Arab"),
        Some(arabic),
        ["Noto Kufi Arabic"],
    )?)
}

fn font_request_invalidation_proof() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"font-request-001"));
    let mut edit = document.edit();
    let changed_paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
    let changed_text = edit.append_text(changed_paragraph, InlineRole::TEXT, "Variable")?;
    let sibling = edit.append_paragraph(ParagraphRole::BODY)?;
    edit.append_text(sibling, InlineRole::TEXT, "reusable sibling")?;
    let published = edit.commit()?;

    let light = ShapingStyle::new(underwood::FontFamily::from(GenericFamily::SansSerif), 24.0)?
        .with_font_weight(FontWeight::LIGHT)?;
    let base = ComputedInlineStyle::new(light, InlineFlowStyle::default(), PaintSlot::new(0));
    let mut styles = StyleMap::new(base.clone());
    styles.set(changed_text, base);
    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::BLACK),
        Brush::Solid(Color::from_rgb8(0x20, 0x50, 0xa0)),
    ]);
    let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(
        TextData::compiled_minimal(),
        font_catalog()?,
    )?);
    let request = SceneRequest::new(FiniteWidth::new(400.0)?, &styles, &paint);
    layout.prepare(published.snapshot(), &request)?;

    let black = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 24.0)?
            .with_font_weight(FontWeight::BLACK)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    styles.set(changed_text, black.clone());
    let request = SceneRequest::new(FiniteWidth::new(400.0)?, &styles, &paint);
    let changed = layout.prepare(published.snapshot(), &request)?;
    assert_eq!(
        changed.work().analysis().paragraphs(),
        0,
        "font requests must reuse Unicode analysis"
    );
    assert_eq!(
        changed.work().itemization().paragraphs(),
        1,
        "only the request-changed paragraph may be reitemized"
    );
    assert_eq!(
        changed.work().font_selection().paragraphs(),
        1,
        "font selection must be reported for the affected paragraph"
    );
    assert!(
        changed.work().font_selection().records() > 0,
        "font selection must report the clusters it resolved"
    );
    assert_eq!(
        changed.work().shape().paragraphs(),
        1,
        "only the request-changed paragraph may be reshaped"
    );
    assert_eq!(
        changed.work().reused_paragraphs(),
        1,
        "the unchanged sibling paragraph must remain reusable"
    );

    let missing = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Absent Family"), 24.0)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    styles.set(changed_text, missing);
    let request = SceneRequest::new(FiniteWidth::new(400.0)?, &styles, &paint);
    let error = layout
        .prepare(published.snapshot(), &request)
        .expect_err("an absent family without a covering fallback must fail");
    assert_eq!(
        error.preparation(),
        Some(PreparationErrorKind::MissingFont),
        "missing family failure must retain the stable MissingFont diagnostic"
    );
    styles.set(changed_text, black.with_paint(PaintSlot::new(1)));
    let request = SceneRequest::new(FiniteWidth::new(400.0)?, &styles, &paint);
    let recovered = layout.prepare(published.snapshot(), &request)?;
    assert_eq!(
        recovered.work().shape().paragraphs(),
        1,
        "a paint-driven retry after failed shaping must rebuild invalidated retained text"
    );

    missing_coverage_proof()?;
    Ok(())
}

fn missing_coverage_proof() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"font-coverage-01"));
    let mut edit = document.edit();
    let paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
    edit.append_text(paragraph, InlineRole::TEXT, "مرحبا")?;
    let published = edit.commit()?;
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 24.0)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let fonts = FontSet::try_from_fonts([Font::from_bytes(
        "latin",
        include_bytes!("../fonts/RobotoFlex-VariableFont.ttf"),
    )?])?;
    let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(
        TextData::compiled_minimal(),
        fonts,
    )?);
    let request = SceneRequest::new(FiniteWidth::new(400.0)?, &styles, &paint);
    let error = layout
        .prepare(published.snapshot(), &request)
        .expect_err("a non-covering primary without fallback must fail");
    assert_eq!(
        error.preparation(),
        Some(PreparationErrorKind::MissingFont),
        "coverage failure must retain the stable MissingFont diagnostic"
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

fn synthesis_variations(scene: &TextScene, text: TextId) -> Vec<FontVariation> {
    scene
        .fragments()
        .iter()
        .find(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == text)
        })
        .map(|fragment| fragment.synthesis().variations().to_vec())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn complete_public_path_executes() {
        super::main().expect("the complete external public path must pass");
    }
}
