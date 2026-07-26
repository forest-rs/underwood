// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared fixtures for adapter unit tests.

use alloc::{rc::Rc, vec, vec::Vec};
use core::cell::RefCell;

use fontique::{Blob, Synthesis};
use parley_engine::{FontInstance, ShapeOptions, ShapedText, Shaper};

use underwood::adapter::{
    ClusterBoundary, ClusterWhitespace, FontSynthesis, FormationWork, GlyphPaintCoverage,
    LineBreakReason as TestLineBreakReason, LineShapingWork, ParagraphConstraints,
    ParagraphFormation, ParagraphFormationOutput, PreparationErrorKind, PreparedClusterSide,
    PreparedGlyph, PreparedInteractionSlice, PreparedInteractionUnit, PreparedLine,
    PreparedParagraph, PreparedRun,
};
use underwood::{
    AnalysisStyle, BaseDirection, BlockRequest, Brush, CacheBudget, Color, CompositionId,
    CompositionUpdate, ComputedInlineStyle, Document, DocumentId, EditErrorKind, EditableSurface,
    EditableSurfaceElement, FiniteWidth, FontData, FontFamily, FontWeight, GenericFamily,
    InlineFlowStyle, InlineRole, LayoutEngine, LineHeight, OverflowWrap, PaintSlot, PaintTable,
    ParagraphRole, ParagraphStyle, Point, ProjectedTextPosition, ProjectedTextSource, Rect,
    RegionAttemptOutcome, RegionFlow, ResolvedDirection, SceneRequest, SelectionErrorKind,
    ShapingStyle, SnapshotTextUnit, StyleMap, SurfaceErrorKind, SurfaceTextEncoding, TextAffinity,
    TextBlock, TextConstraint, TextMovement, TextScene, TextSelectionMode, TextSpacing,
    TextWrapMode, Vec2, WhitespaceCollapse, WordBreak,
};
use underwood::{Language, Script};

use super::{AdapterErrorKind, Font, FontSet, ParleyParagraphEngine};
use crate::font::{read_u16, read_u32};
use crate::interaction::{collect_analysis_units, prepared_cursor_movements};
use crate::line_break::{choose_line, collect_logical_clusters};
use crate::lowering::checked_source_range;
use crate::shaping::{analyze_text, analyze_text_with_styles, split_item_after};

mod alignment;
mod cjk_line_break;
mod editing;
mod font_and_analysis;
mod interaction;
mod intrinsic_and_cache;
mod line_break;
mod line_former;
mod paint;
mod region_flow;

const LATIN_FONT: &[u8] =
    include_bytes!("../../examples/headless/fonts/RobotoFlex-VariableFont.ttf");
const ARABIC_FONT: &[u8] =
    include_bytes!("../../examples/headless/fonts/NotoKufiArabic-Regular.otf");

#[derive(Debug)]
struct PreparedFactsProbe {
    inner: ParleyParagraphEngine,
    outputs: Rc<RefCell<Vec<PreparedParagraph>>>,
}

impl ParagraphFormation for PreparedFactsProbe {
    fn form(
        &mut self,
        input: underwood::adapter::ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, underwood::adapter::PreparationError> {
        let output = self.inner.form(input, constraints)?;
        self.outputs.borrow_mut().push(output.paragraph().clone());
        Ok(output)
    }

    fn shared_preparation_epoch(&self) -> Option<u64> {
        self.inner.shared_preparation_epoch()
    }

    fn release(&mut self, preparation: underwood::adapter::ParagraphPreparationId) {
        self.inner.release(preparation);
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn retained_entries(&self) -> Option<usize> {
        self.inner.retained_entries()
    }
}

#[derive(Debug)]
struct AnalysisCursorProof;

impl ParagraphFormation for AnalysisCursorProof {
    fn form(
        &mut self,
        input: underwood::adapter::ParagraphInput<'_>,
        _constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, underwood::adapter::PreparationError> {
        let analysis = analyze_text_with_styles(
            &mut parley_engine::Analyzer::new(),
            input.text(),
            input.paragraph_style().base_direction(),
            input.analysis_styles(),
            input.analysis_runs(),
        )?;
        let units = collect_analysis_units(input.text(), &analysis)?;
        let mut prepared_units = Vec::with_capacity(units.len());
        let mut glyphs = Vec::with_capacity(units.len());
        for (id, source) in units.into_iter().enumerate() {
            let source = checked_source_range(&source)?;
            prepared_units.push(PreparedInteractionUnit::try_new(
                source.clone(),
                [PreparedInteractionSlice::try_new(source.clone(), 1.0)?],
                0,
                ClusterBoundary::None,
                ClusterWhitespace::None,
                PreparedClusterSide::new(source.start, TextAffinity::Downstream),
                PreparedClusterSide::new(source.end, TextAffinity::Upstream),
            )?);
            let slot = input
                .paint_runs()
                .iter()
                .find(|run| {
                    let bytes = run.bytes();
                    bytes.start <= source.start && source.end <= bytes.end
                })
                .ok_or_else(underwood::adapter::PreparationError::invalid_output)?
                .slot();
            let paint = GlyphPaintCoverage::whole(source.clone(), slot)?;
            glyphs.push(PreparedGlyph::try_new(
                u32::try_from(id).unwrap_or(u32::MAX),
                source,
                Vec2::new(1.0, 0.0),
                Vec2::ZERO,
                paint,
            )?);
        }
        let source = 0..u32::try_from(input.text().len())
            .map_err(|_| underwood::adapter::PreparationError::invalid_output())?;
        let unit_count = u32::try_from(prepared_units.len())
            .map_err(|_| underwood::adapter::PreparationError::invalid_output())?;
        let advance = prepared_units.len() as f64;
        let run = PreparedRun::try_new(
            source.clone(),
            0,
            *b"Zyyy",
            FontData::new(Blob::from(vec![0_u8]), 0),
            16.0,
            FontSynthesis::default(),
            [],
            [],
            glyphs,
        )?;
        let line = PreparedLine::try_new(
            source.clone(),
            TestLineBreakReason::End,
            advance,
            0.8,
            1.0,
            0.8,
            0.2,
            prepared_units,
            [run],
        )?;
        let movements = prepared_cursor_movements(core::slice::from_ref(&line), source.end)?;
        let paragraph = PreparedParagraph::try_new(
            input.paragraph(),
            source.end,
            ResolvedDirection::Ltr,
            [line],
            movements,
        )?;
        Ok(ParagraphFormationOutput::new(
            paragraph,
            FormationWork::new(
                true,
                false,
                unit_count,
                1,
                unit_count,
                1,
                LineShapingWork::default(),
            ),
        ))
    }
}

fn shape_arabic(text: &str) -> (parley_engine::Analysis, ShapedText) {
    let analysis = analyze_text(
        &mut parley_engine::Analyzer::new(),
        text,
        BaseDirection::Auto,
    );
    let shaped = shape_arabic_range(text, &analysis, 0..text.len());
    (analysis, shaped)
}

fn shape_arabic_range(
    text: &str,
    analysis: &parley_engine::Analysis,
    source: core::ops::Range<usize>,
) -> ShapedText {
    let font = FontInstance {
        font: FontData::new(Blob::from(ARABIC_FONT.to_vec()), 0),
        synthesis: Synthesis::default(),
    };
    let style_indices = vec![0; text.chars().count()];
    let mut shaper = Shaper::default();
    let mut shaped = ShapedText::new();
    for item in analysis.itemize(text, |range| {
        range.byte_range.end == source.start || range.byte_range.end == source.end
    }) {
        if item.range.byte_range.start < source.start || item.range.byte_range.end > source.end {
            continue;
        }
        shaper.shape_item(
            text,
            analysis,
            &item,
            &ShapeOptions {
                font_size: 20.0,
                language: None,
                features: &[],
                variations: &[],
                char_style_indices: &style_indices,
            },
            |_| Some(font.clone()),
            &mut shaped,
        );
    }
    shaped
}

#[derive(Clone, Debug)]
struct ScannedHit {
    source: core::ops::Range<u32>,
    position: u32,
    affinity: TextAffinity,
    min_x: f64,
    max_x: f64,
}

fn scan_line_hits(scene: &TextScene, line_index: usize) -> Vec<ScannedHit> {
    let bounds = scene.line(line_index).expect("line exists").bounds();
    let y = bounds.center().y;
    let mut hits: Vec<ScannedHit> = Vec::new();
    let mut x = bounds.x0;
    while x <= bounds.x1 {
        if let Some(hit) = scene.hit_test(Point::new(x, y)) {
            let source = sole_unit_source(hit.source()).bytes();
            if let Some(existing) = hits.iter_mut().find(|existing| existing.source == source) {
                existing.max_x = x;
            } else {
                hits.push(ScannedHit {
                    source,
                    position: hit.position().byte(),
                    affinity: hit.position().affinity(),
                    min_x: x,
                    max_x: x,
                });
            }
        }
        x += 0.05;
    }
    hits
}

fn sole_unit_source(unit: &SnapshotTextUnit) -> &underwood::SnapshotTextRange {
    let [source] = unit.sources() else {
        panic!("fixture interaction unit must remain within one semantic leaf");
    };
    source
}

fn fixture_engine() -> LayoutEngine {
    fixture_engine_with_budget(32)
}

fn fixture_engine_with_budget(budget: usize) -> LayoutEngine {
    fixture_engine_with_budgets(budget, 0)
}

fn fixture_engine_with_budgets(budget: usize, shared_preparation_bytes: usize) -> LayoutEngine {
    let paragraphs = fixture_paragraph_engine();
    LayoutEngine::new(
        paragraphs,
        CacheBudget::new(budget).with_shared_preparation_bytes(shared_preparation_bytes),
    )
}

fn fixture_paragraph_engine() -> ParleyParagraphEngine {
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid"),
        Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid"),
    ])
    .expect("fixture catalog is valid")
    .with_fallbacks(Script::from_bytes(*b"Arab"), None, ["Noto Kufi Arabic"])
    .expect("Arabic fallback is valid");
    ParleyParagraphEngine::new(fonts)
}

fn fixture_document(text: &str, line_height: f32) -> (Document, StyleMap, PaintTable) {
    let mut document = Document::new(DocumentId::from_bytes(*b"breaking-test-01"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("fixture paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, text)
        .expect("fixture text is valid");
    edit.commit().expect("fixture edit is valid");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)
            .expect("fixture shaping style is valid"),
        InlineFlowStyle::new(
            LineHeight::from_multiplier(line_height).expect("fixture line height is valid"),
        ),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    (document, styles, paint)
}
