// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Matched retained-memory and interaction wind tunnel for Underwood and Parley.

use std::collections::VecDeque;
use std::hint::black_box;
use std::time::{Duration, Instant};

use fontique::{Blob, Collection, CollectionOptions, FallbackKey, SourceCache};
use parley::{
    Alignment, AlignmentOptions, Cluster, FontContext, FontFamily, Layout, LayoutContext,
    StyleProperty,
};
use underwood::{
    BlockRequest, Brush, CacheBudget, Color, ComputedInlineStyle, Document, DocumentId,
    FontFamily as UnderwoodFontFamily, InlineFlowStyle, InlineRole, LayoutEngine, PaintSlot,
    PaintTable, ParagraphId, ParagraphRole, Point, SceneFeaturePolicy, SceneFeatures, SceneOutput,
    SceneRequest, ShapingStyle, StyleMap, TextBlock, TextConstraint,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine};

const FONT_SIZE: f32 = 15.0;
const WIDTH: f32 = 180.0;
const CHURN_WINDOW: usize = 64;
const ADAPTER_FACTS_BYTES: usize = 128 * 1024 * 1024;
const LATIN_FONT: &[u8] =
    include_bytes!("../../../examples/headless/fonts/RobotoFlex-VariableFont.ttf");
const ARABIC_FONT: &[u8] =
    include_bytes!("../../../examples/headless/fonts/NotoKufiArabic-Regular.otf");

type Error = Box<dyn std::error::Error>;
type ParleyLayout = Layout<[u8; 4]>;

fn main() -> Result<(), Error> {
    let mut arguments = std::env::args().skip(1);
    let Some(scenario) = arguments.next() else {
        print_usage();
        return Ok(());
    };
    if scenario == "--help" || scenario == "-h" {
        print_usage();
        return Ok(());
    }
    let scale = arguments
        .next()
        .ok_or("expected a 64- or 1000-scale value")?
        .parse::<usize>()?;
    if scale == 0 {
        return Err("scale must be greater than zero".into());
    }
    let rounds = arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1_000);
    if rounds == 0 {
        return Err("rounds must be greater than zero".into());
    }
    if arguments.next().is_some() {
        return Err("expected a scenario, scale, and optional round count".into());
    }

    match scenario.as_str() {
        "runtime-baseline" => hold_for_profiler(&()),
        "underwood-font-baseline" => hold_underwood_font_baseline(scale),
        "parley-font-baseline" => hold_parley_font_baseline(scale),
        "underwood-label-display" => hold_underwood_labels(scale, SceneFeatures::DISPLAY, false),
        "underwood-label-editable" => hold_underwood_labels(scale, SceneFeatures::EDITABLE, false),
        "underwood-label-editable-warm" => {
            hold_underwood_labels(scale, SceneFeatures::EDITABLE, true)
        }
        "parley-label" => hold_parley_labels(scale),
        "underwood-document-display" => {
            hold_underwood_document(scale, SceneFeatures::DISPLAY, false, false)
        }
        "underwood-document-mixed" => {
            hold_underwood_document(scale, SceneFeatures::EDITABLE, true, false)
        }
        "underwood-document-mixed-warm" => {
            hold_underwood_document(scale, SceneFeatures::EDITABLE, true, true)
        }
        "parley-document-paragraphs" => hold_parley_labels(scale),
        "parley-document-flat" => hold_parley_flat_document(scale),
        "underwood-repeat" => profile_underwood_repeat(scale, rounds),
        "parley-repeat" => profile_parley_repeat(scale, rounds),
        "underwood-edit" => profile_underwood_edit(scale, rounds, false),
        "underwood-edit-warm" => profile_underwood_edit(scale, rounds, true),
        "parley-edit" => profile_parley_edit(scale, rounds),
        "underwood-hit-exact" => profile_underwood_hit(scale, rounds, Query::Exact),
        "underwood-hit-closest" => profile_underwood_hit(scale, rounds, Query::Closest),
        "underwood-position" => profile_underwood_hit(scale, rounds, Query::Position),
        "underwood-hit-setup" => profile_underwood_hit(scale, rounds, Query::Setup),
        "parley-hit-exact" => profile_parley_hit(scale, rounds, Query::Exact),
        "parley-hit-closest" => profile_parley_hit(scale, rounds, Query::Closest),
        "parley-position" => profile_parley_hit(scale, rounds, Query::Position),
        "parley-hit-setup" => profile_parley_hit(scale, rounds, Query::Setup),
        "underwood-churn" => profile_underwood_churn(scale),
        "parley-churn" => profile_parley_churn(scale),
        _ => Err(format!("unknown comparison scenario: {scenario}").into()),
    }
}

fn print_usage() {
    println!(
        "usage: underwood_residency_compare <scenario> <scale> [rounds]\n\
         scenarios:\n\
         runtime-baseline | underwood-font-baseline | parley-font-baseline\n\
         underwood-label-display | underwood-label-editable\n\
         underwood-label-editable-warm | parley-label\n\
         underwood-document-display | underwood-document-mixed\n\
         underwood-document-mixed-warm\n\
         parley-document-paragraphs | parley-document-flat\n\
         underwood-repeat | parley-repeat\n\
         underwood-edit | underwood-edit-warm | parley-edit\n\
         underwood-hit-exact | underwood-hit-closest | underwood-position\n\
         parley-hit-exact | parley-hit-closest | parley-position\n\
         underwood-churn | parley-churn"
    );
}

#[derive(Clone, Copy, Debug)]
enum Query {
    Setup,
    Exact,
    Closest,
    Position,
}

impl Query {
    const fn name(self) -> &'static str {
        match self {
            Self::Setup => "hit-setup",
            Self::Exact => "hit-exact",
            Self::Closest => "hit-closest",
            Self::Position => "position",
        }
    }
}

struct UnderwoodLabels {
    blocks: Vec<TextBlock>,
    outputs: Vec<SceneOutput>,
    layout: LayoutEngine,
    style: ComputedInlineStyle,
    paint: PaintTable,
    features: SceneFeatures,
}

impl UnderwoodLabels {
    fn new(count: usize, features: SceneFeatures, retain_adapter: bool) -> Result<Self, Error> {
        let blocks = (0..count)
            .map(|index| TextBlock::plain(identity(1, index), corpus_text(index)))
            .collect::<Result<Vec<_>, _>>()?;
        let style = underwood_style()?;
        let paint = paint();
        let budget = if retain_adapter {
            CacheBudget::new(count).with_adapter_facts_bytes(ADAPTER_FACTS_BYTES)
        } else {
            CacheBudget::new(count)
        };
        let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(underwood_fonts()?), budget);
        let mut outputs = Vec::with_capacity(count);
        for block in &blocks {
            outputs.push(
                layout.prepare_block(
                    &block.snapshot(),
                    &BlockRequest::new(
                        TextConstraint::Wrap(underwood::FiniteWidth::new(f64::from(WIDTH))?),
                        &style,
                        &paint,
                    )
                    .with_features(features),
                )?,
            );
        }
        Ok(Self {
            blocks,
            outputs,
            layout,
            style,
            paint,
            features,
        })
    }

    fn repeat(&mut self) -> Result<(), Error> {
        for (block, output) in self.blocks.iter().zip(&mut self.outputs) {
            *output = self.layout.prepare_block(
                &block.snapshot(),
                &BlockRequest::new(
                    TextConstraint::Wrap(underwood::FiniteWidth::new(f64::from(WIDTH))?),
                    &self.style,
                    &self.paint,
                )
                .with_features(self.features),
            )?;
        }
        Ok(())
    }

    fn edit_middle(&mut self, text: &str) -> Result<(), Error> {
        let middle = self.blocks.len() / 2;
        self.blocks[middle].set_text(text)?;
        self.outputs[middle] = self.layout.prepare_block(
            &self.blocks[middle].snapshot(),
            &BlockRequest::new(
                TextConstraint::Wrap(underwood::FiniteWidth::new(f64::from(WIDTH))?),
                &self.style,
                &self.paint,
            )
            .with_features(self.features),
        )?;
        Ok(())
    }

    fn source_bytes(&self) -> usize {
        self.blocks
            .iter()
            .map(|block| block.snapshot().text().len())
            .sum()
    }

    fn report(&self, scenario: &str) {
        let diagnostics = self.layout.cache_diagnostics();
        let adapter = diagnostics.adapter_facts();
        let mut lines = 0_usize;
        let mut fragments = 0_usize;
        let mut glyphs = 0_usize;
        let mut scene_handle_bytes = 0_usize;
        let mut scene_handle_structure_bytes = 0_usize;
        for output in &self.outputs {
            lines = lines.saturating_add(output.scene().line_count());
            fragments = fragments.saturating_add(output.scene().fragment_count());
            scene_handle_bytes =
                scene_handle_bytes.saturating_add(output.scene().residency().bytes().total());
            scene_handle_structure_bytes = scene_handle_structure_bytes
                .saturating_add(output.scene().residency().bytes().structure());
            glyphs = glyphs.saturating_add(
                output
                    .scene()
                    .fragments()
                    .map(|fragment| fragment.glyphs().len())
                    .sum::<usize>(),
            );
        }
        println!(
            "retained\tengine=underwood\tscenario={scenario}\tinstances={}\tsource_bytes={}\tfont_blob_bytes={}\tfont_blobs=2\tlines={lines}\tfragments={fragments}\tglyphs={glyphs}\tscene_cache_bytes={}\tscene_handle_bytes={scene_handle_bytes}\tscene_handle_structure_bytes={scene_handle_structure_bytes}\tscene_layout_bytes={}\tscene_paint_bytes={}\tscene_source_bytes={}\tscene_semantics_bytes={}\tscene_hit_bytes={}\tscene_selection_bytes={}\tscene_navigation_bytes={}\tadapter_bytes={}\tadapter_scratch_bytes={}\tshared_preparation_bytes={}",
            self.blocks.len(),
            self.source_bytes(),
            font_blob_bytes(),
            diagnostics.scene_cache_accounted_bytes(),
            diagnostics.scene_cache_residency().layout(),
            diagnostics.scene_cache_residency().paint(),
            diagnostics.scene_cache_residency().sources(),
            diagnostics.scene_cache_residency().semantics(),
            diagnostics.scene_cache_residency().hit_testing(),
            diagnostics.scene_cache_residency().selection(),
            diagnostics.scene_cache_residency().navigation(),
            adapter.map_or(0, |facts| facts.resident_bytes()),
            adapter.map_or(0, |facts| facts.scratch_bytes()),
            diagnostics.shared_preparation_resident_bytes(),
        );
    }
}

struct UnderwoodDocument {
    document: Document,
    output: SceneOutput,
    layout: LayoutEngine,
    styles: StyleMap,
    paint: PaintTable,
    source_bytes: usize,
    editor: ParagraphId,
}

impl UnderwoodDocument {
    fn new(count: usize, mixed: bool, retain_adapter: bool) -> Result<Self, Error> {
        let style = underwood_style()?;
        let styles = StyleMap::new(style);
        let paint = paint();
        let mut document = Document::new(identity(2, count));
        let mut source_bytes = 0_usize;
        let mut editor = None;
        let mut edit = document.edit();
        for index in 0..count {
            let paragraph = edit.append_paragraph(ParagraphRole::BODY)?;
            let text = corpus_text(index);
            source_bytes = source_bytes.saturating_add(text.len());
            edit.append_text(paragraph, InlineRole::TEXT, text)?;
            if index == count / 2 {
                editor = Some(paragraph);
            }
        }
        edit.commit()?;
        let editor = editor.ok_or("document fixture has no editor paragraph")?;
        let features = if mixed {
            SceneFeaturePolicy::uniform(SceneFeatures::DISPLAY)
                .with_paragraph(editor, SceneFeatures::EDITABLE)
        } else {
            SceneFeaturePolicy::uniform(SceneFeatures::DISPLAY)
        };
        let budget = if retain_adapter {
            CacheBudget::new(count).with_adapter_facts_bytes(ADAPTER_FACTS_BYTES)
        } else {
            CacheBudget::new(count)
        };
        let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(underwood_fonts()?), budget);
        let output = layout.prepare(
            &document.snapshot(),
            &SceneRequest::new(
                TextConstraint::Wrap(underwood::FiniteWidth::new(f64::from(WIDTH))?),
                &styles,
                &paint,
            )
            .with_feature_policy(features),
        )?;
        Ok(Self {
            document,
            output,
            layout,
            styles,
            paint,
            source_bytes,
            editor,
        })
    }

    fn report(&self, scenario: &str) {
        let diagnostics = self.layout.cache_diagnostics();
        let adapter = diagnostics.adapter_facts();
        let scene = self.output.scene();
        let scene_handle = scene.residency().bytes();
        let glyphs = scene
            .fragments()
            .map(|fragment| fragment.glyphs().len())
            .sum::<usize>();
        println!(
            "retained\tengine=underwood\tscenario={scenario}\tinstances=1\tparagraphs={}\tsource_bytes={}\tfont_blob_bytes={}\tfont_blobs=2\tlines={}\tfragments={}\tglyphs={glyphs}\tscene_cache_bytes={}\tscene_handle_bytes={}\tscene_handle_structure_bytes={}\tscene_layout_bytes={}\tscene_paint_bytes={}\tscene_source_bytes={}\tscene_semantics_bytes={}\tscene_hit_bytes={}\tscene_selection_bytes={}\tscene_navigation_bytes={}\tadapter_bytes={}\tadapter_scratch_bytes={}\tshared_preparation_bytes={}",
            scene.residency().paragraphs(),
            self.source_bytes,
            font_blob_bytes(),
            scene.line_count(),
            scene.fragment_count(),
            diagnostics.scene_cache_accounted_bytes(),
            scene_handle.total(),
            scene_handle.structure(),
            diagnostics.scene_cache_residency().layout(),
            diagnostics.scene_cache_residency().paint(),
            diagnostics.scene_cache_residency().sources(),
            diagnostics.scene_cache_residency().semantics(),
            diagnostics.scene_cache_residency().hit_testing(),
            diagnostics.scene_cache_residency().selection(),
            diagnostics.scene_cache_residency().navigation(),
            adapter.map_or(0, |facts| facts.resident_bytes()),
            adapter.map_or(0, |facts| facts.scratch_bytes()),
            diagnostics.shared_preparation_resident_bytes(),
        );
    }
}

struct ParleyLayouts {
    sources: Vec<String>,
    layouts: Vec<ParleyLayout>,
    fonts: FontContext,
    scratch: LayoutContext<[u8; 4]>,
}

impl ParleyLayouts {
    fn new(count: usize) -> Result<Self, Error> {
        let sources = (0..count)
            .map(|index| String::from(corpus_text(index)))
            .collect::<Vec<_>>();
        let mut fonts = parley_fonts()?;
        let mut scratch = LayoutContext::new();
        let mut layouts = Vec::with_capacity(count);
        for source in &sources {
            layouts.push(build_parley_layout(
                &mut fonts,
                &mut scratch,
                source,
                Some(WIDTH),
            ));
        }
        Ok(Self {
            sources,
            layouts,
            fonts,
            scratch,
        })
    }

    fn flat_document(count: usize) -> Result<Self, Error> {
        let source = (0..count).map(corpus_text).collect::<Vec<_>>().join("\n");
        let mut fonts = parley_fonts()?;
        let mut scratch = LayoutContext::new();
        let layout = build_parley_layout(&mut fonts, &mut scratch, &source, Some(WIDTH));
        Ok(Self {
            sources: vec![source],
            layouts: vec![layout],
            fonts,
            scratch,
        })
    }

    fn repeat(&mut self) {
        for layout in &mut self.layouts {
            layout.break_all_lines(Some(WIDTH));
            layout.align(Alignment::Start, AlignmentOptions::default());
        }
    }

    fn edit_middle(&mut self, text: &str) {
        let middle = self.layouts.len() / 2;
        self.sources[middle].clear();
        self.sources[middle].push_str(text);
        rebuild_parley_layout(
            &mut self.fonts,
            &mut self.scratch,
            &self.sources[middle],
            &mut self.layouts[middle],
            Some(WIDTH),
        );
    }

    fn report(&self, scenario: &str, paragraphs: usize) {
        let topology = parley_topology(&self.layouts);
        println!(
            "retained\tengine=parley\tscenario={scenario}\tinstances={}\tparagraphs={paragraphs}\tsource_bytes={}\tfont_blob_bytes={}\tfont_blobs=2\tstyles={}\tlines={}\truns={}\tclusters={}\tglyphs={}\taccounted_bytes=unavailable",
            self.layouts.len(),
            self.sources.iter().map(String::len).sum::<usize>(),
            font_blob_bytes(),
            topology.styles,
            topology.lines,
            topology.runs,
            topology.clusters,
            topology.glyphs,
        );
    }
}

#[derive(Default)]
struct ParleyTopology {
    styles: usize,
    lines: usize,
    runs: usize,
    clusters: usize,
    glyphs: usize,
}

fn parley_topology(layouts: &[ParleyLayout]) -> ParleyTopology {
    let mut result = ParleyTopology::default();
    for layout in layouts {
        result.styles = result.styles.saturating_add(layout.styles().len());
        for line in layout.lines() {
            result.lines = result.lines.saturating_add(1);
            for run in line.runs() {
                result.runs = result.runs.saturating_add(1);
                for cluster in run.clusters() {
                    result.clusters = result.clusters.saturating_add(1);
                    result.glyphs = result.glyphs.saturating_add(cluster.glyphs().count());
                }
            }
        }
    }
    result
}

fn hold_underwood_labels(
    count: usize,
    features: SceneFeatures,
    retain_adapter: bool,
) -> Result<(), Error> {
    let state = UnderwoodLabels::new(count, features, retain_adapter)?;
    state.report(match (features == SceneFeatures::DISPLAY, retain_adapter) {
        (true, _) => "label-display",
        (false, false) => "label-editable",
        (false, true) => "label-editable-warm",
    });
    hold_for_profiler(&state)
}

fn hold_underwood_font_baseline(scale: usize) -> Result<(), Error> {
    let state = (
        underwood_style()?,
        paint(),
        LayoutEngine::new(
            ParleyParagraphEngine::new(underwood_fonts()?),
            CacheBudget::new(scale).with_adapter_facts_bytes(ADAPTER_FACTS_BYTES),
        ),
    );
    println!(
        "baseline\tengine=underwood\tfont_blob_bytes={}\tfont_blobs=2",
        font_blob_bytes()
    );
    hold_for_profiler(&state)
}

fn hold_parley_font_baseline(_scale: usize) -> Result<(), Error> {
    let state = (parley_fonts()?, LayoutContext::<[u8; 4]>::new());
    println!(
        "baseline\tengine=parley\tfont_blob_bytes={}\tfont_blobs=2",
        font_blob_bytes()
    );
    hold_for_profiler(&state)
}

fn hold_underwood_document(
    count: usize,
    _features: SceneFeatures,
    mixed: bool,
    retain_adapter: bool,
) -> Result<(), Error> {
    let state = UnderwoodDocument::new(count, mixed, retain_adapter)?;
    state.report(match (mixed, retain_adapter) {
        (false, _) => "document-display",
        (true, false) => "document-mixed",
        (true, true) => "document-mixed-warm",
    });
    black_box((&state.document, &state.styles, &state.paint, state.editor));
    hold_for_profiler(&state)
}

fn hold_parley_labels(count: usize) -> Result<(), Error> {
    let state = ParleyLayouts::new(count)?;
    state.report("paragraph-layouts", count);
    hold_for_profiler(&state)
}

fn hold_parley_flat_document(count: usize) -> Result<(), Error> {
    let state = ParleyLayouts::flat_document(count)?;
    state.report("flat-document", count);
    hold_for_profiler(&state)
}

fn profile_underwood_repeat(count: usize, rounds: usize) -> Result<(), Error> {
    let mut state = UnderwoodLabels::new(count, SceneFeatures::EDITABLE, false)?;
    let elapsed = measure(|| {
        for _ in 0..rounds {
            state.repeat().expect("retained repeat must prepare");
        }
    });
    report_time(
        "underwood",
        "repeat",
        count,
        rounds,
        count.saturating_mul(rounds),
        elapsed,
    );
    state.report("repeat");
    hold_for_profiler(&state)
}

fn profile_parley_repeat(count: usize, rounds: usize) -> Result<(), Error> {
    let mut state = ParleyLayouts::new(count)?;
    let elapsed = measure(|| {
        for _ in 0..rounds {
            state.repeat();
        }
    });
    report_time(
        "parley",
        "repeat",
        count,
        rounds,
        count.saturating_mul(rounds),
        elapsed,
    );
    state.report("repeat", count);
    hold_for_profiler(&state)
}

fn profile_underwood_edit(count: usize, rounds: usize, retain_adapter: bool) -> Result<(), Error> {
    let mut state = UnderwoodLabels::new(count, SceneFeatures::EDITABLE, retain_adapter)?;
    let elapsed = measure(|| {
        for round in 0..rounds {
            state
                .edit_middle(if round.is_multiple_of(2) {
                    "Save the retained change now"
                } else {
                    "Save changes"
                })
                .expect("localized edit must prepare");
        }
    });
    let operation = if retain_adapter {
        "localized-edit-warm"
    } else {
        "localized-edit"
    };
    report_time("underwood", operation, count, rounds, rounds, elapsed);
    state.report(operation);
    hold_for_profiler(&state)
}

fn profile_parley_edit(count: usize, rounds: usize) -> Result<(), Error> {
    let mut state = ParleyLayouts::new(count)?;
    let elapsed = measure(|| {
        for round in 0..rounds {
            state.edit_middle(if round.is_multiple_of(2) {
                "Save the retained change now"
            } else {
                "Save changes"
            });
        }
    });
    report_time("parley", "localized-edit", count, rounds, rounds, elapsed);
    state.report("localized-edit", count);
    hold_for_profiler(&state)
}

fn profile_underwood_hit(units: usize, rounds: usize, query: Query) -> Result<(), Error> {
    let text = long_text(units);
    let block = TextBlock::plain(identity(3, units), &text)?;
    let snapshot = block.snapshot();
    let text_id = snapshot.text_id();
    let byte = final_scalar_start(snapshot.text())?;
    let style = underwood_style()?;
    let paint = paint();
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(underwood_fonts()?),
        CacheBudget::new(1).with_adapter_facts_bytes(ADAPTER_FACTS_BYTES),
    );
    let output = layout.prepare_block(
        &snapshot,
        &BlockRequest::new(TextConstraint::MaxContent, &style, &paint)
            .with_features(SceneFeatures::EDITABLE),
    )?;
    let scene = output.scene();
    let line = scene.lines().last().ok_or("hit fixture has no line")?;
    let point = Point::new(line.bounds().x1 - 0.25, line.bounds().center().y);
    let editing = scene.editing()?;
    let elapsed = measure(|| {
        for _ in 0..rounds {
            match query {
                Query::Setup => {
                    black_box(editing);
                }
                Query::Exact => {
                    black_box(
                        black_box(editing)
                            .hit_test(black_box(point))
                            .expect("exact hit must resolve"),
                    );
                }
                Query::Closest => {
                    black_box(
                        black_box(editing)
                            .hit_test_closest(black_box(point))
                            .expect("closest hit must resolve"),
                    );
                }
                Query::Position => {
                    black_box(
                        black_box(editing)
                            .position_at(black_box(text_id), black_box(byte))
                            .expect("byte position must resolve"),
                    );
                }
            }
        }
    });
    report_query_time("underwood", query, units, rounds, elapsed);
    black_box((&block, &layout, &output));
    hold_for_profiler(&output)
}

fn profile_parley_hit(units: usize, rounds: usize, query: Query) -> Result<(), Error> {
    let text = long_text(units);
    let byte = usize::try_from(final_scalar_start(&text)?)?;
    let mut fonts = parley_fonts()?;
    let mut scratch = LayoutContext::new();
    let layout = build_parley_layout(&mut fonts, &mut scratch, &text, None);
    let line = layout.lines().last().ok_or("hit fixture has no line")?;
    let metrics = line.metrics();
    let target_run = line.runs().last().ok_or("hit fixture has no run")?;
    let target = target_run
        .visual_clusters()
        .last()
        .ok_or("hit fixture has no cluster")?;
    let point = (
        target
            .visual_offset()
            .ok_or("target cluster has no visual offset")?
            + target.advance() * 0.5,
        (metrics.block_min_coord + metrics.block_max_coord) * 0.5,
    );
    let elapsed = measure(|| {
        for _ in 0..rounds {
            match query {
                Query::Setup => {
                    black_box(&layout);
                }
                Query::Exact => {
                    black_box(
                        Cluster::from_point_exact(
                            black_box(&layout),
                            black_box(point.0),
                            black_box(point.1),
                        )
                        .expect("exact hit must resolve"),
                    );
                }
                Query::Closest => {
                    black_box(
                        Cluster::from_point(
                            black_box(&layout),
                            black_box(point.0),
                            black_box(point.1),
                        )
                        .expect("closest hit must resolve"),
                    );
                }
                Query::Position => {
                    black_box(
                        Cluster::from_byte_index(black_box(&layout), black_box(byte))
                            .expect("byte position must resolve"),
                    );
                }
            }
        }
    });
    report_query_time("parley", query, units, rounds, elapsed);
    black_box((&text, &fonts, &scratch, &layout));
    hold_for_profiler(&layout)
}

fn profile_underwood_churn(count: usize) -> Result<(), Error> {
    let style = underwood_style()?;
    let paint = paint();
    let mut layout = LayoutEngine::new(
        ParleyParagraphEngine::new(underwood_fonts()?),
        CacheBudget::new(CHURN_WINDOW),
    );
    let mut retained = VecDeque::with_capacity(CHURN_WINDOW);
    let elapsed = measure(|| {
        for index in 0..count {
            let block = TextBlock::plain(identity(4, index), corpus_text(index))
                .expect("churn block must initialize");
            let output = layout
                .prepare_block(
                    &block.snapshot(),
                    &BlockRequest::new(
                        TextConstraint::Wrap(
                            underwood::FiniteWidth::new(f64::from(WIDTH)).expect("width is finite"),
                        ),
                        &style,
                        &paint,
                    ),
                )
                .expect("churn block must prepare");
            retained.push_back((block, output));
            if retained.len() > CHURN_WINDOW {
                retained.pop_front();
            }
        }
    });
    report_time("underwood", "churn", count, 1, count, elapsed);
    let diagnostics = layout.cache_diagnostics();
    println!(
        "churn\tengine=underwood\tcreated={count}\tretained={}\tcache_entries={}\tevictions={}\tscene_cache_bytes={}\tadapter_bytes={}",
        retained.len(),
        diagnostics.current_entries(),
        diagnostics.evictions(),
        diagnostics.scene_cache_accounted_bytes(),
        diagnostics
            .adapter_facts()
            .map_or(0, |facts| facts.resident_bytes()),
    );
    black_box((&style, &paint, &layout));
    hold_for_profiler(&retained)
}

fn profile_parley_churn(count: usize) -> Result<(), Error> {
    let mut fonts = parley_fonts()?;
    let mut scratch = LayoutContext::new();
    let mut retained = VecDeque::with_capacity(CHURN_WINDOW);
    let elapsed = measure(|| {
        for index in 0..count {
            let source = String::from(corpus_text(index));
            let layout = build_parley_layout(&mut fonts, &mut scratch, &source, Some(WIDTH));
            retained.push_back((source, layout));
            if retained.len() > CHURN_WINDOW {
                retained.pop_front();
            }
        }
    });
    report_time("parley", "churn", count, 1, count, elapsed);
    println!(
        "churn\tengine=parley\tcreated={count}\tretained={}",
        retained.len()
    );
    black_box((&fonts, &scratch));
    hold_for_profiler(&retained)
}

fn build_parley_layout(
    fonts: &mut FontContext,
    scratch: &mut LayoutContext<[u8; 4]>,
    text: &str,
    width: Option<f32>,
) -> ParleyLayout {
    let mut layout = ParleyLayout::new();
    rebuild_parley_layout(fonts, scratch, text, &mut layout, width);
    layout
}

fn rebuild_parley_layout(
    fonts: &mut FontContext,
    scratch: &mut LayoutContext<[u8; 4]>,
    text: &str,
    layout: &mut ParleyLayout,
    width: Option<f32>,
) {
    let mut builder = scratch.ranged_builder(fonts, text, 1.0, false);
    builder.push_default(FontFamily::named("Roboto Flex"));
    builder.push_default(StyleProperty::FontSize(FONT_SIZE));
    builder.build_into(layout, text);
    layout.break_all_lines(width);
    layout.align(Alignment::Start, AlignmentOptions::default());
}

fn underwood_style() -> Result<ComputedInlineStyle, Error> {
    Ok(ComputedInlineStyle::new(
        ShapingStyle::new(UnderwoodFontFamily::named("Roboto Flex"), FONT_SIZE)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    ))
}

fn paint() -> PaintTable {
    PaintTable::from_brushes([Brush::Solid(Color::from_rgb8(0x20, 0x24, 0x2b))])
}

fn underwood_fonts() -> Result<FontSet, Error> {
    Ok(FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT)?,
        Font::from_bytes("arabic", ARABIC_FONT)?,
    ])?
    .with_fallbacks(
        underwood::Script::from_bytes(*b"Arab"),
        None,
        ["Noto Kufi Arabic"],
    )?)
}

fn parley_fonts() -> Result<FontContext, Error> {
    let mut collection = Collection::new(CollectionOptions {
        shared: true,
        system_fonts: false,
    });
    if collection
        .register_fonts(Blob::from(LATIN_FONT.to_vec()), None)
        .is_empty()
    {
        return Err("failed to register the Latin comparison font".into());
    }
    if collection
        .register_fonts(Blob::from(ARABIC_FONT.to_vec()), None)
        .is_empty()
    {
        return Err("failed to register the Arabic comparison font".into());
    }
    let arabic = collection
        .family_id("Noto Kufi Arabic")
        .ok_or("registered Arabic family is unavailable")?;
    if !collection.set_fallbacks(
        FallbackKey::new(underwood::Script::from_bytes(*b"Arab"), None),
        [arabic].into_iter(),
    ) {
        return Err("failed to configure the Arabic comparison fallback".into());
    }
    Ok(FontContext {
        collection,
        source_cache: SourceCache::new_shared(),
    })
}

fn corpus_text(index: usize) -> &'static str {
    match index % 4 {
        0 => "Save changes",
        1 => "Open the retained document",
        2 => "Office affine — real ligatures",
        _ => "Underwood — مرحبا بالعالم",
    }
}

fn long_text(units: usize) -> String {
    let mut text = String::with_capacity(units.saturating_mul(5));
    for index in 0..units {
        if index != 0 {
            text.push(' ');
        }
        text.push_str(if index % 16 == 15 {
            "مرحبا"
        } else {
            "word"
        });
    }
    text
}

fn final_scalar_start(text: &str) -> Result<u32, Error> {
    let byte = text
        .char_indices()
        .next_back()
        .map(|(byte, _)| byte)
        .ok_or("fixture text must not be empty")?;
    Ok(u32::try_from(byte)?)
}

fn font_blob_bytes() -> usize {
    LATIN_FONT.len().saturating_add(ARABIC_FONT.len())
}

fn identity(namespace: u64, index: usize) -> DocumentId {
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&namespace.to_le_bytes());
    bytes[8..].copy_from_slice(&(index as u64).to_le_bytes());
    DocumentId::from_bytes(bytes)
}

fn measure(operation: impl FnOnce()) -> Duration {
    let started = Instant::now();
    operation();
    started.elapsed()
}

fn report_time(
    engine: &str,
    operation: &str,
    scale: usize,
    rounds: usize,
    operations: usize,
    elapsed: Duration,
) {
    println!(
        "timing\tengine={engine}\toperation={operation}\tscale={scale}\trounds={rounds}\toperations={operations}\ttotal_ns={}\tns_per_operation={}",
        elapsed.as_nanos(),
        elapsed.as_nanos() / operations as u128,
    );
}

fn report_query_time(engine: &str, query: Query, units: usize, rounds: usize, elapsed: Duration) {
    println!(
        "timing\tengine={engine}\toperation={}\tunits={units}\trounds={rounds}\toperations={rounds}\ttotal_ns={}\tns_per_operation={}",
        query.name(),
        elapsed.as_nanos(),
        elapsed.as_nanos() / rounds as u128,
    );
}

fn hold_for_profiler<T>(value: &T) -> Result<(), Error> {
    black_box(value);
    let Some(seconds) = std::env::var_os("RESIDENCY_PROFILE_HOLD_SECS") else {
        return Ok(());
    };
    let seconds = seconds
        .to_str()
        .ok_or("RESIDENCY_PROFILE_HOLD_SECS must be valid UTF-8")?
        .parse::<u64>()?;
    println!("profiler_ready\tpid={}", std::process::id());
    std::thread::sleep(Duration::from_secs(seconds));
    Ok(())
}
