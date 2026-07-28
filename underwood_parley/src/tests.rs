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
    ParagraphFormation, ParagraphFormationCacheDiagnostics, ParagraphFormationOutput,
    PreparationErrorKind, PreparedClusterSide, PreparedGlyph, PreparedInteractionSlice,
    PreparedInteractionUnit, PreparedLine, PreparedParagraph, PreparedParagraphData, PreparedRun,
};
use underwood::{
    AnalysisStyle, BaseDirection, BlockRequest, Brush, CacheBudget, Color, CompositionId,
    CompositionUpdate, ComputedInlineStyle, Document, DocumentId, EditErrorKind, EditableSurface,
    EditableSurfaceElement, FiniteWidth, FontData, FontFamily, FontWeight, GenericFamily,
    InlineFlowStyle, InlineRole, LayoutEngine, LineHeight, OverflowWrap, PaintSlot, PaintTable,
    ParagraphRole, ParagraphStyle, Point, ProjectedTextPosition, ProjectedTextSource, Rect,
    RegionAttemptOutcome, RegionFlow, ResolvedDirection, SceneFeatures, SceneRequest,
    SelectionErrorKind, ShapingStyle, SnapshotTextUnitView, StyleMap, SurfaceErrorKind,
    SurfaceTextEncoding, TextAffinity, TextBlock, TextConstraint, TextMovement, TextScene,
    TextSelectionMode, TextSpacing, TextWrapMode, Vec2, WhitespaceCollapse, WordBreak,
};
use underwood::{Language, Script};

use super::{AdapterErrorKind, Font, FontSet, ParleyParagraphEngine};
use crate::font::{read_u16, read_u32};
use crate::interaction::collect_analysis_units;
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
const DEVANAGARI_FONT: &[u8] =
    include_bytes!("../../conformance/fonts/NotoSansDevanagari-Regular.subset.ttf");

fn editable_scene_request<'a>(
    constraint: TextConstraint,
    styles: &'a StyleMap,
    paint: &'a PaintTable,
) -> SceneRequest<'a> {
    SceneRequest::new(constraint, styles, paint)
        .with_features(SceneFeatures::EDITABLE.with_semantics())
}

fn editable_block_request<'a>(
    constraint: TextConstraint,
    style: &'a ComputedInlineStyle,
    paint: &'a PaintTable,
) -> BlockRequest<'a> {
    BlockRequest::new(constraint, style, paint)
        .with_features(SceneFeatures::EDITABLE.with_semantics())
}

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

    fn set_retained_facts_budget(&mut self, bytes: usize) {
        self.inner.set_retained_facts_budget(bytes);
    }

    fn commit_preparation(&mut self, preparation: underwood::adapter::ParagraphPreparationId) {
        self.inner.commit_preparation(preparation);
    }

    fn trim_retained_facts(&mut self) {
        self.inner.trim_retained_facts();
    }

    fn retained_facts(&self) -> Option<ParagraphFormationCacheDiagnostics> {
        self.inner.retained_facts()
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
            input.text,
            input.paragraph_style.base_direction(),
            input.analysis_styles,
            input.analysis_runs,
        )?;
        let units = collect_analysis_units(input.text, &analysis)?;
        let source = 0..u32::try_from(input.text.len())
            .map_err(|_| underwood::adapter::PreparationError::invalid_output())?;
        let unit_count = u32::try_from(units.len())
            .map_err(|_| underwood::adapter::PreparationError::invalid_output())?;
        let advance = units.len() as f64;
        let line = PreparedLine::try_new(
            source.clone(),
            TestLineBreakReason::End,
            advance,
            0.8,
            1.0,
            0.8,
            0.2,
        )?;
        let mut data = PreparedParagraphData::with_capacity(1, 1, units.len(), units.len(), 0);
        let units_start = data.unit_count();
        for source in &units {
            let source = checked_source_range(source)?;
            let unit = PreparedInteractionUnit::try_new(
                source.clone(),
                1.0,
                0,
                ClusterBoundary::None,
                ClusterWhitespace::None,
                PreparedClusterSide::new(source.start, TextAffinity::Downstream),
                PreparedClusterSide::new(source.end, TextAffinity::Upstream),
            )?;
            data.push_unit(unit, [PreparedInteractionSlice::try_new(source, 1.0)?])?;
        }
        let run = PreparedRun::try_new(
            source.clone(),
            0,
            *b"Zyyy",
            FontData::new(Blob::from(vec![0_u8]), 0),
            16.0,
            FontSynthesis::default(),
        )?;
        let runs_start = data.run_count();
        let glyphs_start = data.glyph_count();
        for (id, source) in units.iter().enumerate() {
            let source = checked_source_range(source)?;
            input
                .paint_runs
                .iter()
                .find(|run| {
                    let bytes = run.bytes();
                    bytes.start <= source.start && source.end <= bytes.end
                })
                .ok_or_else(underwood::adapter::PreparationError::invalid_output)?;
            let paint = GlyphPaintCoverage::whole();
            data.push_glyph(PreparedGlyph::try_new(
                u32::try_from(id).unwrap_or(u32::MAX),
                source,
                Vec2::new(1.0, 0.0),
                Vec2::ZERO,
                paint,
            )?)?;
        }
        data.push_run(run, 0..0, 0..0, glyphs_start..data.glyph_count())?;
        data.push_line(
            line,
            units_start..data.unit_count(),
            runs_start..data.run_count(),
        )?;
        let paragraph = PreparedParagraph::try_from_data(
            input.paragraph,
            source.end,
            ResolvedDirection::Ltr,
            input.features,
            data,
        )?;
        Ok(ParagraphFormationOutput::new(
            paragraph,
            FormationWork {
                analyzed: true,
                itemized: false,
                selected_clusters: unit_count,
                shaped_runs: 1,
                shaped_glyphs: unit_count,
                formed_lines: 1,
                line_shaping: LineShapingWork::default(),
            },
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
    let interaction = scene
        .interaction()
        .expect("fixture retains hit-testing data");
    let bounds = scene.line(line_index).expect("line exists").bounds();
    let y = bounds.center().y;
    let mut hits: Vec<ScannedHit> = Vec::new();
    let mut x = bounds.x0;
    while x <= bounds.x1 {
        if let Some(hit) = interaction.hit_test(Point::new(x, y)) {
            let source = sole_unit_source(&hit.source).bytes();
            if let Some(existing) = hits.iter_mut().find(|existing| existing.source == source) {
                existing.max_x = x;
            } else {
                hits.push(ScannedHit {
                    source,
                    position: hit.position.byte(),
                    affinity: hit.position.affinity(),
                    min_x: x,
                    max_x: x,
                });
            }
        }
        x += 0.05;
    }
    hits
}

fn sole_unit_source(unit: &SnapshotTextUnitView<'_>) -> underwood::SnapshotTextRange {
    let mut sources = unit.sources();
    let source = sources
        .next()
        .expect("fixture interaction unit must retain one semantic leaf");
    assert!(
        sources.next().is_none(),
        "fixture interaction unit must remain within one semantic leaf"
    );
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
        CacheBudget::new(budget)
            .with_shared_preparation_bytes(shared_preparation_bytes)
            .with_adapter_facts_bytes(64 * 1024 * 1024),
    )
}

fn fixture_paragraph_engine() -> ParleyParagraphEngine {
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid"),
        Font::from_bytes("arabic", ARABIC_FONT).expect("Arabic fixture font is valid"),
        Font::from_bytes("devanagari", DEVANAGARI_FONT).expect("Devanagari fixture font is valid"),
    ])
    .expect("fixture catalog is valid")
    .with_fallbacks(Script::from_bytes(*b"Arab"), None, ["Noto Kufi Arabic"])
    .expect("Arabic fallback is valid")
    .with_fallbacks(Script::from_bytes(*b"Deva"), None, ["Noto Sans Devanagari"])
    .expect("Devanagari fallback is valid");
    ParleyParagraphEngine::new(fonts)
}

#[test]
fn cursor_derivation_adds_no_adapter_graph_during_warm_editable_upgrade() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let adapter = PreparedFactsProbe {
        inner: fixture_paragraph_engine(),
        outputs: Rc::clone(&observed),
    };
    let mut layout = LayoutEngine::new(
        adapter,
        CacheBudget::new(32).with_adapter_facts_bytes(64 * 1024 * 1024),
    );
    let (document, styles, paint) = fixture_document("Display, then edit.", 1.2);
    let display_request =
        SceneRequest::new(TextConstraint::MaxContent, &styles, &paint).with_preparation_trace();
    let display = layout
        .prepare(&document.snapshot(), &display_request)
        .expect("display-only text must prepare");
    assert_eq!(display.scene.fragment_count(), 1);
    let first_observed = observed.borrow();
    assert_eq!(first_observed.len(), 1);
    assert_eq!(first_observed[0].features(), SceneFeatures::DISPLAY);
    let display_bytes = first_observed[0].accounted_owned_bytes();
    drop(first_observed);

    let editable_request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(SceneFeatures::EDITABLE)
        .with_preparation_trace();
    let editable = layout
        .prepare(&document.snapshot(), &editable_request)
        .expect("a retained display paragraph must upgrade");
    assert_eq!(editable.work.analysis.paragraphs, 0);
    assert_eq!(editable.work.shape.paragraphs, 0);
    assert_eq!(editable.work.flow.paragraphs, 0);
    let reuse = editable
        .trace
        .expect("the capability upgrade requested a trace")
        .reuse;
    assert_eq!(reuse.adapter_fact_hits, 1);
    assert_eq!(reuse.adapter_fact_misses, 0);
    assert_eq!(reuse.warm_capability_upgrades, 1);
    assert_eq!(reuse.cold_capability_upgrades, 0);
    let observed = observed.borrow();
    assert_eq!(observed.len(), 2);
    assert!(observed[1].features().contains(SceneFeatures::EDITABLE));
    assert_eq!(
        observed[1].accounted_owned_bytes(),
        display_bytes,
        "editable cursor policy derives from the same formed facts"
    );
    drop(observed);

    let smaller = layout
        .prepare(&document.snapshot(), &display_request)
        .expect("an editable resident segment satisfies a display request");
    let paragraph = smaller
        .scene
        .paragraph_residencies()
        .next()
        .expect("the fixture contains one paragraph");
    assert_eq!(paragraph.requested, SceneFeatures::DISPLAY);
    assert_eq!(paragraph.resident, SceneFeatures::EDITABLE);
    assert_eq!(
        paragraph.bytes.navigation, 0,
        "derived navigation retains no per-position graph"
    );
}

#[test]
fn zero_adapter_budget_keeps_display_scene_and_reports_cold_upgrade() {
    let mut layout = LayoutEngine::new(fixture_paragraph_engine(), CacheBudget::new(32));
    let (document, styles, paint) = fixture_document("Display can outlive formation.", 1.2);
    let display_request =
        SceneRequest::new(TextConstraint::MaxContent, &styles, &paint).with_preparation_trace();
    let display = layout
        .prepare(&document.snapshot(), &display_request)
        .expect("display-only text must prepare");
    let retained_display = display.scene.clone();
    let adapter = layout
        .cache_diagnostics()
        .adapter_facts
        .expect("Parley reports adapter-fact accounting");
    assert_eq!(adapter.budget_bytes, 0);
    assert_eq!(adapter.entries, 0);
    assert_eq!(adapter.resident_bytes, 0);
    assert_eq!(adapter.evictions, 1);
    assert_eq!(retained_display.fragment_count(), 1);

    let editable_request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(SceneFeatures::EDITABLE)
        .with_preparation_trace();
    let editable = layout
        .prepare(&document.snapshot(), &editable_request)
        .expect("cold capability upgrade must remain correct");
    assert_eq!(editable.work.analysis.paragraphs, 1);
    assert_eq!(editable.work.shape.paragraphs, 1);
    assert_eq!(editable.work.flow.paragraphs, 1);
    let reuse = editable
        .trace
        .expect("the cold upgrade requested a trace")
        .reuse;
    assert_eq!(reuse.adapter_fact_hits, 0);
    assert_eq!(reuse.adapter_fact_misses, 1);
    assert_eq!(reuse.warm_capability_upgrades, 0);
    assert_eq!(reuse.cold_capability_upgrades, 1);
    assert!(
        retained_display.interaction().is_err(),
        "the caller-held display scene remains unchanged by the upgrade"
    );
    assert!(
        editable.scene.editing().is_ok(),
        "the cold path still publishes the exact requested capability"
    );
}

#[test]
fn explicit_adapter_trim_preserves_scene_and_degrades_only_later_upgrade() {
    let mut layout = LayoutEngine::new(
        fixture_paragraph_engine(),
        CacheBudget::new(32).with_adapter_facts_bytes(64 * 1024 * 1024),
    );
    let (document, styles, paint) = fixture_document("Trim retained facts.", 1.2);
    let display_request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint);
    let display = layout
        .prepare(&document.snapshot(), &display_request)
        .expect("display-only text must prepare");
    let retained_display = display.scene.clone();
    assert_eq!(
        layout
            .cache_diagnostics()
            .adapter_facts
            .expect("Parley reports adapter-fact accounting")
            .entries,
        1
    );

    layout.trim_adapter_facts();
    let trimmed = layout
        .cache_diagnostics()
        .adapter_facts
        .expect("Parley reports adapter-fact accounting");
    assert_eq!(trimmed.entries, 0);
    assert_eq!(trimmed.resident_bytes, 0);
    assert_eq!(retained_display.fragment_count(), 1);

    let upgraded = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
                .with_features(SceneFeatures::EDITABLE)
                .with_preparation_trace(),
        )
        .expect("upgrade after trim must reform rather than fail");
    assert_eq!(
        upgraded
            .trace
            .expect("upgrade requested a trace")
            .reuse
            .cold_capability_upgrades,
        1
    );
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
