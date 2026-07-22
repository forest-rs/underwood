// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The single semantic document presented by the native showcase.

use imaging::peniko::Gradient;
use underwood::{
    Brush, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth, FontFeature,
    FontVariation, FontWeight, InlineFlowStyle, InlineRole, Language, LayoutEngine, LineHeight,
    PaintSlot, PaintTable, ParagraphRole, SceneRequest, Script, ShapingStyle, StyleMap, Tag,
    TextId, TextScene, WorkReport,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

const LATIN_FONT_BYTES: &[u8] = include_bytes!("../../headless/fonts/RobotoFlex-VariableFont.ttf");
const ARABIC_FONT_BYTES: &[u8] = include_bytes!("../../headless/fonts/NotoKufiArabic-Regular.otf");

const INK: PaintSlot = PaintSlot::new(0);
const CYAN: PaintSlot = PaintSlot::new(1);
const CORAL: PaintSlot = PaintSlot::new(2);
const GOLD: PaintSlot = PaintSlot::new(3);
const MUTED: PaintSlot = PaintSlot::new(4);
const TITLE: PaintSlot = PaintSlot::new(5);

const ORIGINAL_EDIT_TEXT: &str = "Change one sentence and only this paragraph returns to shaping. Nine sibling formations stay ready to reuse.";
const CHANGED_EDIT_TEXT: &str =
    "One local edit landed here. This paragraph reshaped; nine sibling formations stayed retained.";

type AnyError = Box<dyn std::error::Error>;

/// One prepared live frame and its observed work.
#[derive(Clone, Debug)]
pub(crate) struct PreparedDocumentFrame {
    pub(crate) scene: TextScene,
    pub(crate) work: WorkReport,
    pub(crate) line_count: usize,
    pub(crate) axis_weight: f32,
}

/// Retained document, paragraph engine, and interaction state.
pub(crate) struct ShowcaseContent {
    document: Document,
    layout: LayoutEngine,
    leaves: Leaves,
    edited: bool,
    alternate_paint: bool,
}

#[derive(Clone, Copy, Debug)]
struct Leaves {
    title: TextId,
    deck: TextId,
    section_one: TextId,
    #[cfg(test)]
    mixed_prefix: TextId,
    arabic: TextId,
    #[cfg(test)]
    mixed_suffix: TextId,
    section_two: TextId,
    width_narrow: TextId,
    width_regular: TextId,
    width_wide: TextId,
    ligatures_on: TextId,
    ligatures_off: TextId,
    section_three: TextId,
    editable: TextId,
    controls: TextId,
}

impl ShowcaseContent {
    /// Builds the deterministic font catalog and publishes the initial document.
    pub(crate) fn new() -> Result<Self, AnyError> {
        let arabic = Language::parse("ar")?;
        let fonts = FontSet::try_from_fonts([
            Font::from_bytes("latin", LATIN_FONT_BYTES)?,
            Font::from_bytes("arabic", ARABIC_FONT_BYTES)?,
        ])?
        .with_fallbacks(Script::from_bytes(*b"Arab"), None, ["Noto Kufi Arabic"])?
        .with_fallbacks(
            Script::from_bytes(*b"Arab"),
            Some(arabic),
            ["Noto Kufi Arabic"],
        )?;
        let paragraphs = ParleyParagraphEngine::new(TextData::compiled_minimal(), fonts)?;

        let mut document = Document::new(DocumentId::from_bytes(*b"underwood-live-1"));
        let mut edit = document.edit();

        let title = edit.append_paragraph(ParagraphRole::HEADING_1)?;
        let title = edit.append_text(title, InlineRole::TEXT, "TYPE, ALIVE.")?;

        let deck = edit.append_paragraph(ParagraphRole::BODY)?;
        let deck = edit.append_text(
            deck,
            InlineRole::EMPHASIS,
            "One semantic document. Real shaping. Retained work. No toolkit in the core.",
        )?;

        let section_one = edit.append_paragraph(ParagraphRole::HEADING_2)?;
        let section_one =
            edit.append_text(section_one, InlineRole::TEXT, "ONE DOCUMENT / MANY SCRIPTS")?;

        let body = edit.append_paragraph(ParagraphRole::BODY)?;
        let _mixed_prefix = edit.append_text(
            body,
            InlineRole::TEXT,
            "Underwood keeps meaning, style, flow, and scene geometry together. ",
        )?;
        let arabic = edit.append_text(body, InlineRole::EMPHASIS, "مرحبا بالعالم")?;
        let _mixed_suffix = edit.append_text(
            body,
            InlineRole::TEXT,
            " runs right-to-left—with every dot intact—inside the same flowing paragraph.",
        )?;

        let section_two = edit.append_paragraph(ParagraphRole::HEADING_2)?;
        let section_two = edit.append_text(
            section_two,
            InlineRole::TEXT,
            "VARIABLE FORM / OPENTYPE DETAIL",
        )?;

        let widths = edit.append_paragraph(ParagraphRole::BODY)?;
        let width_narrow = edit.append_text(widths, InlineRole::EMPHASIS, "CONDENSED 75")?;
        edit.append_text(widths, InlineRole::TEXT, "   /   ")?;
        let width_regular = edit.append_text(widths, InlineRole::EMPHASIS, "REGULAR 100")?;
        edit.append_text(widths, InlineRole::TEXT, "   /   ")?;
        let width_wide = edit.append_text(widths, InlineRole::EMPHASIS, "EXPANDED 125")?;

        let features = edit.append_paragraph(ParagraphRole::BODY)?;
        edit.append_text(
            features,
            InlineRole::TEXT,
            "Live wght axis above. liga on — ",
        )?;
        let ligatures_on = edit.append_text(features, InlineRole::EMPHASIS, "office")?;
        edit.append_text(features, InlineRole::TEXT, " — 4 glyphs   /   off — ")?;
        let ligatures_off = edit.append_text(features, InlineRole::EMPHASIS, "office")?;
        edit.append_text(features, InlineRole::TEXT, " — 6 glyphs.")?;

        let section_three = edit.append_paragraph(ParagraphRole::HEADING_2)?;
        let section_three = edit.append_text(
            section_three,
            InlineRole::TEXT,
            "LOCAL CHANGE / GLOBAL CALM",
        )?;

        let editable = edit.append_paragraph(ParagraphRole::BODY)?;
        let editable = edit.append_text(editable, InlineRole::EMPHASIS, ORIGINAL_EDIT_TEXT)?;

        let controls = edit.append_paragraph(ParagraphRole::BODY)?;
        let controls = edit.append_text(
            controls,
            InlineRole::TEXT,
            "SPACE edit   /   P paint   /   A animate weight   /   G guides   /   R reset   /   drag to reflow",
        )?;

        edit.commit()?;

        Ok(Self {
            document,
            layout: LayoutEngine::new(paragraphs),
            leaves: Leaves {
                title,
                deck,
                section_one,
                #[cfg(test)]
                mixed_prefix: _mixed_prefix,
                arabic,
                #[cfg(test)]
                mixed_suffix: _mixed_suffix,
                section_two,
                width_narrow,
                width_regular,
                width_wide,
                ligatures_on,
                ligatures_off,
                section_three,
                editable,
                controls,
            },
            edited: false,
            alternate_paint: false,
        })
    }

    /// Prepares the current revision at a finite width and variable-axis phase.
    pub(crate) fn prepare(
        &mut self,
        width: f64,
        axis_phase: f32,
    ) -> Result<PreparedDocumentFrame, AnyError> {
        let axis_weight = 100.0 + axis_phase.clamp(0.0, 1.0) * 800.0;
        let styles = self.styles(axis_weight)?;
        let paints = paint_table(self.alternate_paint);
        let request = SceneRequest::new(FiniteWidth::new(width)?, &styles, &paints);
        let output = self.layout.prepare(&self.document.snapshot(), &request)?;
        Ok(PreparedDocumentFrame {
            line_count: output.scene().lines().len(),
            scene: output.scene().clone(),
            work: output.work().clone(),
            axis_weight,
        })
    }

    /// Toggles one real paragraph-local document edit.
    pub(crate) fn toggle_edit(&mut self) {
        self.edited = !self.edited;
        self.replace_editable(if self.edited {
            CHANGED_EDIT_TEXT
        } else {
            ORIGINAL_EDIT_TEXT
        });
    }

    /// Toggles brush values without changing shaping, flow, or paint slots.
    pub(crate) fn toggle_paint(&mut self) {
        self.alternate_paint = !self.alternate_paint;
    }

    /// Restores the initial document and paint state.
    pub(crate) fn reset(&mut self) {
        if self.edited {
            self.replace_editable(ORIGINAL_EDIT_TEXT);
            self.edited = false;
        }
        self.alternate_paint = false;
    }

    fn replace_editable(&mut self, text: &str) {
        let mut edit = self.document.edit();
        edit.replace_text(self.leaves.editable, text)
            .expect("showcase TextId must remain valid for its owning document");
        edit.commit()
            .expect("showcase replacement must preserve document structure");
    }

    fn styles(&self, axis_weight: f32) -> Result<StyleMap, AnyError> {
        let english = Language::parse("en")?;
        let wght = Tag::new(b"wght");
        let wdth = Tag::new(b"wdth");
        let opsz = Tag::new(b"opsz");
        let liga = Tag::new(b"liga");

        let body_shaping = ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 20.0)?
            .with_language(Some(english))
            .with_font_weight(FontWeight::new(390.0))?
            .with_variations([FontVariation::new(opsz, 18.0)])?;
        let body = ComputedInlineStyle::new(
            body_shaping.clone(),
            InlineFlowStyle::new(LineHeight::from_multiplier(1.48)?),
            INK,
        );
        let title = ComputedInlineStyle::new(
            ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 62.0)?
                .with_language(Some(english))
                .with_font_weight(FontWeight::new(axis_weight))?
                .with_variations([
                    FontVariation::new(wght, axis_weight),
                    FontVariation::new(opsz, 72.0),
                ])?,
            InlineFlowStyle::new(LineHeight::from_multiplier(1.18)?),
            TITLE,
        );
        let deck = ComputedInlineStyle::new(
            ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 28.0)?
                .with_language(Some(english))
                .with_font_weight(FontWeight::new(420.0))?
                .with_variations([FontVariation::new(opsz, 32.0)])?,
            InlineFlowStyle::new(LineHeight::from_multiplier(1.45)?),
            GOLD,
        );
        let section = ComputedInlineStyle::new(
            ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 16.0)?
                .with_language(Some(english))
                .with_font_weight(FontWeight::new(720.0))?
                .with_variations([FontVariation::new(opsz, 18.0)])?,
            InlineFlowStyle::new(LineHeight::from_multiplier(2.0)?),
            CYAN,
        );
        let arabic = ComputedInlineStyle::new(
            ShapingStyle::new(underwood::FontFamily::named("Absent Primary Family"), 23.0)?
                .with_language(Some(Language::parse("ar")?)),
            InlineFlowStyle::new(LineHeight::from_multiplier(1.48)?),
            GOLD,
        );
        let width_style = |axis_width, paint| -> Result<ComputedInlineStyle, AnyError> {
            Ok(ComputedInlineStyle::new(
                ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 24.0)?
                    .with_language(Some(english))
                    .with_font_weight(FontWeight::new(650.0))?
                    .with_variations([
                        FontVariation::new(wdth, axis_width),
                        FontVariation::new(opsz, 32.0),
                    ])?,
                InlineFlowStyle::new(LineHeight::from_multiplier(1.45)?),
                paint,
            ))
        };
        let ligatures_on = ComputedInlineStyle::new(
            body_shaping
                .clone()
                .with_font_weight(FontWeight::new(680.0))?
                .with_features([FontFeature::new(liga, 1)]),
            InlineFlowStyle::new(LineHeight::from_multiplier(1.48)?),
            CYAN,
        );
        let ligatures_off = ComputedInlineStyle::new(
            body_shaping
                .clone()
                .with_font_weight(FontWeight::new(680.0))?
                .with_features([FontFeature::new(liga, 0)]),
            InlineFlowStyle::new(LineHeight::from_multiplier(1.48)?),
            CORAL,
        );
        let editable = ComputedInlineStyle::new(
            body_shaping
                .clone()
                .with_font_weight(FontWeight::new(560.0))?,
            InlineFlowStyle::new(LineHeight::from_multiplier(1.6)?),
            CORAL,
        );
        let controls = ComputedInlineStyle::new(
            ShapingStyle::new(underwood::FontFamily::named("Roboto Flex"), 14.0)?
                .with_language(Some(english))
                .with_font_weight(FontWeight::new(460.0))?
                .with_variations([FontVariation::new(opsz, 14.0)])?,
            InlineFlowStyle::new(LineHeight::from_multiplier(2.0)?),
            MUTED,
        );

        let mut styles = StyleMap::new(body);
        styles.set(self.leaves.title, title);
        styles.set(self.leaves.deck, deck);
        for leaf in [
            self.leaves.section_one,
            self.leaves.section_two,
            self.leaves.section_three,
        ] {
            styles.set(leaf, section.clone());
        }
        styles.set(self.leaves.arabic, arabic);
        styles.set(self.leaves.width_narrow, width_style(75.0, CYAN)?);
        styles.set(self.leaves.width_regular, width_style(100.0, INK)?);
        styles.set(self.leaves.width_wide, width_style(125.0, GOLD)?);
        styles.set(self.leaves.ligatures_on, ligatures_on);
        styles.set(self.leaves.ligatures_off, ligatures_off);
        styles.set(self.leaves.editable, editable);
        styles.set(self.leaves.controls, controls);
        Ok(styles)
    }
}

fn paint_table(alternate: bool) -> PaintTable {
    let coral = if alternate {
        Color::from_rgb8(0xa7, 0x8b, 0xfa)
    } else {
        Color::from_rgb8(0xff, 0x6b, 0x67)
    };
    let gold = if alternate {
        Color::from_rgb8(0x67, 0xe8, 0xb5)
    } else {
        Color::from_rgb8(0xf5, 0xc4, 0x51)
    };
    let title = if alternate {
        Gradient::new_linear((0.0, 0.0), (330.0, 0.0)).with_stops([
            (0.0_f32, Color::from_rgb8(0xa7, 0x8b, 0xfa)),
            (0.52_f32, Color::from_rgb8(0xee, 0xf3, 0xf8)),
            (1.0_f32, Color::from_rgb8(0x67, 0xe8, 0xb5)),
        ])
    } else {
        Gradient::new_linear((0.0, 0.0), (330.0, 0.0)).with_stops([
            (0.0_f32, Color::from_rgb8(0x4d, 0xd5, 0xe7)),
            (0.52_f32, Color::from_rgb8(0xee, 0xf3, 0xf8)),
            (1.0_f32, Color::from_rgb8(0xf5, 0xc4, 0x51)),
        ])
    };
    PaintTable::from_brushes([
        Brush::Solid(Color::from_rgb8(0xee, 0xf3, 0xf8)),
        Brush::Solid(Color::from_rgb8(0x4d, 0xd5, 0xe7)),
        Brush::Solid(coral),
        Brush::Solid(gold),
        Brush::Solid(Color::from_rgb8(0x85, 0x96, 0xad)),
        Brush::Gradient(title),
    ])
}

#[cfg(test)]
mod tests {
    use super::{ARABIC_FONT_BYTES, ShowcaseContent, TITLE, TextId};
    use underwood::{Brush, ParagraphRole};

    #[test]
    fn document_exposes_real_heading_and_body_semantics() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let frame = content.prepare(900.0, 0.5).expect("document must prepare");
        let roles: Vec<_> = frame
            .scene
            .semantics()
            .filter_map(|semantic| semantic.paragraph_role())
            .collect();
        assert!(roles.contains(&ParagraphRole::HEADING_1));
        assert!(roles.contains(&ParagraphRole::HEADING_2));
        assert!(roles.contains(&ParagraphRole::BODY));
    }

    #[test]
    fn narrower_document_forms_more_visual_lines() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let wide = content.prepare(900.0, 0.5).expect("wide must prepare");
        let narrow = content.prepare(420.0, 0.5).expect("narrow must prepare");
        assert!(narrow.line_count > wide.line_count);
        assert!(
            line_count_for_any(
                &narrow.scene,
                &[
                    content.leaves.mixed_prefix,
                    content.leaves.arabic,
                    content.leaves.mixed_suffix,
                ],
            ) > line_count_for_any(
                &wide.scene,
                &[
                    content.leaves.mixed_prefix,
                    content.leaves.arabic,
                    content.leaves.mixed_suffix,
                ],
            )
        );
        assert_eq!(narrow.work.shape().paragraphs(), 0);
        assert!(narrow.work.flow().paragraphs() > 0);
    }

    #[test]
    fn local_edit_reshapes_only_its_paragraph() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let initial = content.prepare(760.0, 0.5).expect("initial must prepare");
        let sibling_ids: Vec<_> = initial
            .scene
            .fragments()
            .iter()
            .filter(|fragment| {
                fragment
                    .source()
                    .is_none_or(|source| source.text() != content.leaves.editable)
            })
            .map(underwood::SceneFragment::id)
            .collect();
        content.toggle_edit();
        let edited = content.prepare(760.0, 0.5).expect("edit must prepare");
        assert_eq!(edited.work.shape().paragraphs(), 1);
        assert_eq!(edited.work.reused_paragraphs(), 9);
        let edited_sibling_ids: Vec<_> = edited
            .scene
            .fragments()
            .iter()
            .filter(|fragment| {
                fragment
                    .source()
                    .is_none_or(|source| source.text() != content.leaves.editable)
            })
            .map(underwood::SceneFragment::id)
            .collect();
        assert_eq!(edited_sibling_ids, sibling_ids);
    }

    #[test]
    fn paint_toggle_does_not_repeat_text_physics() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        content.prepare(760.0, 0.5).expect("initial must prepare");
        content.toggle_paint();
        let painted = content.prepare(760.0, 0.5).expect("paint must prepare");
        assert_eq!(painted.work.analysis().paragraphs(), 0);
        assert_eq!(painted.work.shape().paragraphs(), 0);
        assert_eq!(painted.work.flow().paragraphs(), 0);
        assert_eq!(painted.work.geometry().paragraphs(), 0);
        assert_eq!(painted.work.reused_paragraphs(), 10);
        assert!(painted.work.paint().paragraphs() > 0);
    }

    #[test]
    fn heading_uses_a_real_gradient_brush() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let frame = content.prepare(760.0, 0.5).expect("document must prepare");
        assert!(matches!(
            frame.scene.paint().brush(TITLE),
            Some(Brush::Gradient(_))
        ));
        let mut title_fragments = 0;
        for fragment in frame.scene.fragments() {
            if fragment
                .source()
                .is_some_and(|source| source.text() == content.leaves.title)
            {
                assert_eq!(fragment.paint(), TITLE);
                title_fragments += 1;
            } else {
                assert_ne!(fragment.paint(), TITLE);
            }
        }
        assert!(title_fragments > 0);
    }

    #[test]
    fn axis_motion_is_isolated_to_the_heading_paragraph() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let first = content.prepare(760.0, 0.1).expect("initial must prepare");
        let title = title_fragment_coords(&first.scene, content.leaves.title);
        let moved = content.prepare(760.0, 0.9).expect("axis must prepare");
        let moved_title = title_fragment_coords(&moved.scene, content.leaves.title);
        assert_ne!(title, moved_title);
        assert!((moved.axis_weight - 820.0).abs() < f32::EPSILON);
        assert_eq!(moved.work.shape().paragraphs(), 1);
        assert_eq!(moved.work.reused_paragraphs(), 9);

        content.toggle_edit();
        content.toggle_paint();
        content.reset();
        let reset = content.prepare(760.0, 0.9).expect("reset must prepare");
        assert_eq!(
            reset.work.shape().paragraphs(),
            0,
            "restoring the cached authored content may reuse its retained shape"
        );
    }

    #[test]
    fn feature_specimen_executes_distinct_ligature_results() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let frame = content.prepare(760.0, 0.5).expect("document must prepare");
        assert_eq!(glyph_count(&frame.scene, content.leaves.ligatures_on), 4);
        assert_eq!(glyph_count(&frame.scene, content.leaves.ligatures_off), 6);
    }

    #[test]
    fn width_specimen_executes_three_variable_axis_instances() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let frame = content.prepare(760.0, 0.5).expect("document must prepare");
        let narrow = title_fragment_coords(&frame.scene, content.leaves.width_narrow);
        let regular = title_fragment_coords(&frame.scene, content.leaves.width_regular);
        let wide = title_fragment_coords(&frame.scene, content.leaves.width_wide);
        assert_ne!(narrow, regular);
        assert_ne!(regular, wide);
        assert_ne!(narrow, wide);
    }

    #[test]
    fn arabic_specimen_uses_real_rtl_fallback_with_visible_mark_ink() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let frame = content.prepare(760.0, 0.5).expect("document must prepare");
        let arabic: Vec<_> = frame
            .scene
            .fragments()
            .iter()
            .filter(|fragment| {
                fragment
                    .source()
                    .is_some_and(|source| source.text() == content.leaves.arabic)
            })
            .collect();
        assert!(!arabic.is_empty());
        assert!(arabic.iter().all(|fragment| {
            fragment.font().data.as_ref() == ARABIC_FONT_BYTES
                && fragment.script() == *b"Arab"
                && fragment.bidi_level() & 1 == 1
        }));
        assert!(arabic.iter().any(|fragment| {
            fragment
                .glyphs()
                .iter()
                .any(|glyph| glyph.advance().x == 0.0)
                && fragment.clip().width() > 0.0
                && fragment.clip().height() > 0.0
        }));
        let visual_sources: Vec<_> = arabic
            .iter()
            .filter_map(|fragment| fragment.source().map(|source| source.bytes()))
            .collect();
        assert!(
            visual_sources.len() > 1
                && visual_sources
                    .windows(2)
                    .all(|pair| pair[0].start >= pair[1].start)
        );
    }

    fn title_fragment_coords(scene: &underwood::TextScene, title: TextId) -> Vec<i16> {
        scene
            .fragments()
            .iter()
            .find(|fragment| {
                fragment
                    .source()
                    .is_some_and(|source| source.text() == title)
            })
            .expect("title must produce a fragment")
            .normalized_coords()
            .to_vec()
    }

    fn glyph_count(scene: &underwood::TextScene, text: TextId) -> usize {
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

    fn line_count_for_any(scene: &underwood::TextScene, texts: &[TextId]) -> usize {
        scene
            .lines()
            .iter()
            .filter(|line| {
                line.sources()
                    .iter()
                    .any(|source| texts.contains(&source.text()))
            })
            .count()
    }
}
