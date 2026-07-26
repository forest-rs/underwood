// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rendering-only bridge from a portable Underwood scene to `imaging`.

use crate::page::{LivingPagePlan, PageDecorationKind};
use imaging::kurbo::{Affine, Circle, Line, Rect, RoundedRect, Stroke};
use imaging::peniko::{Color, Fill, Gradient, Style};
use imaging::{PaintSink, Painter, record};
use underwood::{
    CompositionScene, FontData, InlineRole, PaintSlot, PaintTable, ParagraphRole, Point,
    ProjectedSceneFragmentView, ProjectedSceneFragments, ProjectedSceneGlyphView,
    ProjectedSceneGlyphs, ProjectedSceneLineView, ProjectedSceneLines, RegionAttemptOutcome,
    RegionTranscript, SceneFragmentView, SceneFragments, SceneGlyphView, SceneGlyphs,
    SceneLineView, SceneLines, SceneSemantics, SemanticFragmentView, TextScene, Vec2,
    adapter::{FontSynthesis, LineBreakReason},
};

const BACKGROUND: Color = Color::from_rgb8(0x08, 0x0d, 0x14);
const PAGE: Color = Color::from_rgb8(0x0f, 0x17, 0x22);
const PAGE_EDGE: Color = Color::from_rgba8(0x8b, 0x9b, 0xb1, 0x30);
const CYAN: Color = Color::from_rgb8(0x4d, 0xd5, 0xe7);
const CORAL: Color = Color::from_rgb8(0xff, 0x6b, 0x67);
const GOLD: Color = Color::from_rgb8(0xf5, 0xc4, 0x51);
const SELECTION_PRIMARY: Color = Color::from_rgba8(0x4d, 0xd5, 0xe7, 0x58);
const SELECTION_SECONDARY: Color = Color::from_rgba8(0xff, 0x6b, 0x67, 0x52);
const PREEDIT_SELECTION: Color = Color::from_rgba8(0xf5, 0xc4, 0x51, 0x62);

#[derive(Clone)]
enum AnyLines<'a> {
    Committed(SceneLines<'a>),
    Projected(ProjectedSceneLines<'a>),
}

impl<'a> Iterator for AnyLines<'a> {
    type Item = AnyLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Committed(lines) => lines.next().map(AnyLine::Committed),
            Self::Projected(lines) => lines.next().map(AnyLine::Projected),
        }
    }
}

#[derive(Clone, Copy)]
enum AnyLine<'a> {
    Committed(SceneLineView<'a>),
    Projected(ProjectedSceneLineView<'a>),
}

impl AnyLine<'_> {
    fn bounds(self) -> Rect {
        match self {
            Self::Committed(line) => line.bounds(),
            Self::Projected(line) => line.bounds(),
        }
    }

    fn break_reason(self) -> LineBreakReason {
        match self {
            Self::Committed(line) => line.break_reason(),
            Self::Projected(line) => line.break_reason(),
        }
    }

    fn baseline(self) -> f64 {
        match self {
            Self::Committed(line) => line.baseline(),
            Self::Projected(line) => line.baseline(),
        }
    }

    fn content_ascent(self) -> f64 {
        match self {
            Self::Committed(line) => line.content_ascent(),
            Self::Projected(line) => line.content_ascent(),
        }
    }

    fn content_descent(self) -> f64 {
        match self {
            Self::Committed(line) => line.content_descent(),
            Self::Projected(line) => line.content_descent(),
        }
    }
}

#[derive(Clone)]
enum AnyFragments<'a> {
    Committed(SceneFragments<'a>),
    Projected(ProjectedSceneFragments<'a>),
}

impl<'a> Iterator for AnyFragments<'a> {
    type Item = AnyFragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Committed(fragments) => fragments.next().map(AnyFragment::Committed),
            Self::Projected(fragments) => fragments.next().map(AnyFragment::Projected),
        }
    }
}

#[derive(Clone, Copy)]
enum AnyFragment<'a> {
    Committed(SceneFragmentView<'a>),
    Projected(ProjectedSceneFragmentView<'a>),
}

impl<'a> AnyFragment<'a> {
    fn glyphs(self) -> AnyGlyphs<'a> {
        match self {
            Self::Committed(fragment) => AnyGlyphs::Committed(fragment.glyphs()),
            Self::Projected(fragment) => AnyGlyphs::Projected(fragment.glyphs()),
        }
    }

    fn paint(self) -> PaintSlot {
        match self {
            Self::Committed(fragment) => fragment.paint(),
            Self::Projected(fragment) => fragment.paint(),
        }
    }

    fn transform(self) -> Affine {
        match self {
            Self::Committed(fragment) => fragment.transform(),
            Self::Projected(fragment) => fragment.transform(),
        }
    }

    fn paint_clip(self) -> Option<Rect> {
        match self {
            Self::Committed(fragment) => fragment.paint_clip(),
            Self::Projected(fragment) => fragment.paint_clip(),
        }
    }

    fn font(self) -> &'a FontData {
        match self {
            Self::Committed(fragment) => fragment.font(),
            Self::Projected(fragment) => fragment.font(),
        }
    }

    fn font_size(self) -> f32 {
        match self {
            Self::Committed(fragment) => fragment.font_size(),
            Self::Projected(fragment) => fragment.font_size(),
        }
    }

    fn synthesis(self) -> &'a FontSynthesis {
        match self {
            Self::Committed(fragment) => fragment.synthesis(),
            Self::Projected(fragment) => fragment.synthesis(),
        }
    }

    fn normalized_coords(self) -> &'a [i16] {
        match self {
            Self::Committed(fragment) => fragment.normalized_coords(),
            Self::Projected(fragment) => fragment.normalized_coords(),
        }
    }

    fn bidi_level(self) -> u8 {
        match self {
            Self::Committed(fragment) => fragment.bidi_level(),
            Self::Projected(fragment) => fragment.bidi_level(),
        }
    }

    fn script(self) -> [u8; 4] {
        match self {
            Self::Committed(fragment) => fragment.script(),
            Self::Projected(fragment) => fragment.script(),
        }
    }

    fn owns_multiple_sources(self) -> bool {
        match self {
            Self::Committed(fragment) => fragment.sources().nth(1).is_some(),
            Self::Projected(fragment) => fragment.sources().nth(1).is_some(),
        }
    }
}

#[derive(Clone)]
enum AnyGlyphs<'a> {
    Committed(SceneGlyphs<'a>),
    Projected(ProjectedSceneGlyphs<'a>),
}

impl<'a> Iterator for AnyGlyphs<'a> {
    type Item = AnyGlyph<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Committed(glyphs) => glyphs.next().map(AnyGlyph::Committed),
            Self::Projected(glyphs) => glyphs.next().map(AnyGlyph::Projected),
        }
    }
}

#[derive(Clone, Copy)]
enum AnyGlyph<'a> {
    Committed(SceneGlyphView<'a>),
    Projected(ProjectedSceneGlyphView<'a>),
}

impl AnyGlyph<'_> {
    fn id(self) -> u32 {
        match self {
            Self::Committed(glyph) => glyph.id(),
            Self::Projected(glyph) => glyph.id(),
        }
    }

    fn position(self) -> Point {
        match self {
            Self::Committed(glyph) => glyph.position(),
            Self::Projected(glyph) => glyph.position(),
        }
    }

    fn advance(self) -> Vec2 {
        match self {
            Self::Committed(glyph) => glyph.advance(),
            Self::Projected(glyph) => glyph.advance(),
        }
    }

    fn owns_multiple_sources(self) -> bool {
        match self {
            Self::Committed(glyph) => glyph.sources().nth(1).is_some(),
            Self::Projected(glyph) => glyph.sources().nth(1).is_some(),
        }
    }
}

#[derive(Clone)]
struct AnySemantics<'a> {
    committed: Option<SceneSemantics<'a>>,
}

impl<'a> Iterator for AnySemantics<'a> {
    type Item = AnySemantic<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.committed
            .as_mut()
            .and_then(SceneSemantics::next)
            .map(AnySemantic::Committed)
    }
}

#[derive(Clone, Copy)]
enum AnySemantic<'a> {
    Committed(SemanticFragmentView<'a>),
}

impl AnySemantic<'_> {
    fn paragraph_role(self) -> Option<ParagraphRole> {
        match self {
            Self::Committed(semantic) => semantic.paragraph_role(),
        }
    }

    fn inline_role(self) -> Option<InlineRole> {
        match self {
            Self::Committed(semantic) => semantic.inline_role(),
        }
    }

    fn bounds(self) -> Rect {
        match self {
            Self::Committed(semantic) => semantic.bounds(),
        }
    }
}

/// Presentation-only inspection layer over public scene observations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DiagnosticsMode {
    /// Paint only the authored document and editor overlays.
    #[default]
    Off,
    /// Show source regions, floats, exclusions, offered slots, and rejected retries.
    Flow,
    /// Show line boxes, content metrics, baselines, and break kinds.
    Lines,
    /// Show fragment advances against exact line content metrics, scripts, and bidi direction.
    Fragments,
    /// Show every glyph origin, advance vector, and multi-source glyph.
    Glyphs,
    /// Show paragraph and inline semantic geometry.
    Semantics,
}

impl DiagnosticsMode {
    /// Advances through every inspection layer and then returns to the clean view.
    #[must_use]
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Off => Self::Flow,
            Self::Flow => Self::Lines,
            Self::Lines => Self::Fragments,
            Self::Fragments => Self::Glyphs,
            Self::Glyphs => Self::Semantics,
            Self::Semantics => Self::Off,
        }
    }

    /// Returns the compact host-facing name of this inspection layer.
    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Flow => "FLOW",
            Self::Lines => "LINES",
            Self::Fragments => "FRAGMENTS",
            Self::Glyphs => "GLYPHS",
            Self::Semantics => "SEMANTICS",
        }
    }
}

/// One scene-space selection rectangle and its independent-selection index.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectionOverlay {
    pub(crate) bounds: Rect,
    pub(crate) selection: usize,
}

/// Renderer-only geometry layered around the shaped document.
#[derive(Clone, Debug, Default)]
pub(crate) struct EditorOverlay {
    pub(crate) selections: Vec<SelectionOverlay>,
    pub(crate) carets: Vec<Rect>,
    pub(crate) marked_text: Vec<Rect>,
    pub(crate) preedit_selection: Vec<Rect>,
    pub(crate) caret_visible: bool,
}

/// Logical placement and flow constraint derived from one physical window.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameLayout {
    pub(crate) scale: f64,
    pub(crate) origin_x: f64,
    pub(crate) origin_y: f64,
    pub(crate) content_width: f64,
    logical_width: f64,
    logical_height: f64,
}

impl FrameLayout {
    /// Derives a readable document column from the current window dimensions.
    pub(crate) fn new(width: u32, height: u32, scale: f64) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let logical_width = f64::from(width) / scale;
        let logical_height = f64::from(height) / scale;
        let outer = (logical_width * 0.055).clamp(24.0, 72.0);
        let page_width = (logical_width - outer * 2.0).max(240.0);
        let inset = (page_width * 0.065).clamp(28.0, 74.0);
        let content_width = (page_width - inset * 2.0).clamp(180.0, 960.0);
        Self {
            scale,
            origin_x: (logical_width - content_width) * 0.5,
            origin_y: outer + (inset * 0.68).clamp(24.0, 48.0),
            content_width,
            logical_width,
            logical_height,
        }
    }

    fn page_rect(self) -> Rect {
        let outer = (self.logical_width * 0.055).clamp(24.0, 72.0);
        Rect::new(
            outer,
            outer,
            self.logical_width - outer,
            (self.logical_height - outer).max(outer + 1.0),
        )
    }

    /// Reports when the flowing document extends below the page's visible area.
    pub(crate) fn document_is_clipped(self, document: &TextScene) -> bool {
        self.lines_are_clipped(AnyLines::Committed(document.lines()))
    }

    /// Reports whether transient projected lines exceed the page.
    pub(crate) fn composition_lines_are_clipped(self, lines: ProjectedSceneLines<'_>) -> bool {
        self.lines_are_clipped(AnyLines::Projected(lines))
    }

    fn lines_are_clipped(self, lines: AnyLines<'_>) -> bool {
        let content_bottom = lines.map(|line| line.bounds().y1).fold(0.0_f64, f64::max);
        self.origin_y + content_bottom > self.page_rect().y1 - 20.0
    }

    /// Converts a logical window point into Underwood scene coordinates.
    pub(crate) fn document_point(self, point: Point) -> Point {
        Point::new(point.x - self.origin_x, point.y - self.origin_y)
    }

    /// Converts an Underwood scene rectangle into logical window coordinates.
    pub(crate) fn window_rect(self, rect: Rect) -> Rect {
        rect + Vec2::new(self.origin_x, self.origin_y)
    }
}

/// Records the document and optional line evidence into an imaging scene.
pub(crate) fn record_frame(
    document: &TextScene,
    page: &LivingPagePlan,
    transcript: &RegionTranscript,
    layout: FrameLayout,
    diagnostics: DiagnosticsMode,
    overlay: &EditorOverlay,
) -> Result<record::Scene, record::ValidateError> {
    let semantics = if diagnostics == DiagnosticsMode::Semantics {
        AnySemantics {
            committed: Some(document.semantics()),
        }
    } else {
        AnySemantics { committed: None }
    };
    record_scene(
        AnyLines::Committed(document.lines()),
        AnyFragments::Committed(document.fragments()),
        document.paint(),
        semantics,
        page,
        transcript,
        layout,
        diagnostics,
        overlay,
    )
}

/// Records a transient composition scene with the same native presentation.
pub(crate) fn record_composition_frame(
    document: &CompositionScene,
    page: &LivingPagePlan,
    transcript: &RegionTranscript,
    layout: FrameLayout,
    diagnostics: DiagnosticsMode,
    overlay: &EditorOverlay,
) -> Result<record::Scene, record::ValidateError> {
    let semantics = if diagnostics == DiagnosticsMode::Semantics {
        AnySemantics {
            committed: Some(document.semantics()),
        }
    } else {
        AnySemantics { committed: None }
    };
    record_scene(
        AnyLines::Projected(document.lines()),
        AnyFragments::Projected(document.fragments()),
        document.paint(),
        semantics,
        page,
        transcript,
        layout,
        diagnostics,
        overlay,
    )
}

fn record_scene(
    lines: AnyLines<'_>,
    fragments: AnyFragments<'_>,
    paint: &PaintTable,
    semantics: AnySemantics<'_>,
    page_plan: &LivingPagePlan,
    transcript: &RegionTranscript,
    layout: FrameLayout,
    diagnostics: DiagnosticsMode,
    overlay: &EditorOverlay,
) -> Result<record::Scene, record::ValidateError> {
    let mut scene = record::Scene::new();
    {
        let mut painter = Painter::new(&mut scene);
        painter
            .fill(
                Rect::new(0.0, 0.0, layout.logical_width, layout.logical_height),
                BACKGROUND,
            )
            .transform(Affine::scale(layout.scale))
            .draw();

        let page = layout.page_rect();
        painter
            .fill(RoundedRect::from_rect(page, 18.0), PAGE)
            .transform(Affine::scale(layout.scale))
            .draw();
        painter
            .stroke(
                RoundedRect::from_rect(page, 18.0),
                &Stroke::new(1.0),
                PAGE_EDGE,
            )
            .transform(Affine::scale(layout.scale))
            .draw();
        painter
            .fill(Rect::new(page.x0, page.y0, page.x0 + 5.0, page.y1), CORAL)
            .transform(Affine::scale(layout.scale))
            .draw();
        painter
            .fill(
                Rect::new(page.x0 + 8.0, page.y0, page.x0 + 10.0, page.y1),
                CYAN,
            )
            .transform(Affine::scale(layout.scale))
            .draw();

        let content_placement =
            Affine::scale(layout.scale) * Affine::translate((layout.origin_x, layout.origin_y));
        paint_page_foundation(&mut painter, page_plan, content_placement);
        if diagnostics == DiagnosticsMode::Flow {
            paint_flow_diagnostics(&mut painter, page_plan, transcript, content_placement);
        }

        TextSceneAdapter::new(lines, fragments, paint, semantics, layout).paint_into(
            &mut painter,
            diagnostics,
            overlay,
        );
    }
    scene.validate()?;
    Ok(scene)
}

fn paint_page_foundation<S: PaintSink + ?Sized>(
    painter: &mut Painter<'_, S>,
    page: &LivingPagePlan,
    placement: Affine,
) {
    for column in page.column_regions() {
        painter
            .fill(
                RoundedRect::from_rect(*column, 12.0),
                Color::from_rgba8(0x4d, 0xd5, 0xe7, 0x05),
            )
            .transform(placement)
            .draw();
        painter
            .stroke(
                RoundedRect::from_rect(*column, 12.0),
                &Stroke::new(0.7),
                Color::from_rgba8(0x8b, 0x9b, 0xb1, 0x12),
            )
            .transform(placement)
            .draw();
    }

    for decoration in page.decorations() {
        match decoration.kind {
            PageDecorationKind::HeroFloat => {
                let bounds = decoration.bounds;
                let gradient = Gradient::new_linear((bounds.x0, bounds.y0), (bounds.x1, bounds.y1))
                    .with_stops([
                        (0.0_f32, Color::from_rgba8(0x4d, 0xd5, 0xe7, 0x30)),
                        (0.52_f32, Color::from_rgba8(0xff, 0x6b, 0x67, 0x24)),
                        (1.0_f32, Color::from_rgba8(0xf5, 0xc4, 0x51, 0x30)),
                    ]);
                painter
                    .fill(RoundedRect::from_rect(bounds, 18.0), &gradient)
                    .transform(placement)
                    .draw();
                painter
                    .stroke(
                        RoundedRect::from_rect(bounds, 18.0),
                        &Stroke::new(1.0),
                        PAGE_EDGE.with_alpha(0.72),
                    )
                    .transform(placement)
                    .draw();
                let radius = bounds.height() * 0.29;
                let center = bounds.center();
                painter
                    .stroke(
                        Circle::new(center, radius),
                        &Stroke::new(1.2),
                        CYAN.with_alpha(0.72),
                    )
                    .transform(placement)
                    .draw();
                painter
                    .stroke(
                        Circle::new((center.x + radius * 0.72, center.y), radius * 0.68),
                        &Stroke::new(1.2),
                        GOLD.with_alpha(0.7),
                    )
                    .transform(placement)
                    .draw();
                painter
                    .fill(
                        Circle::new((center.x - radius * 0.72, center.y), radius * 0.22),
                        CORAL.with_alpha(0.82),
                    )
                    .transform(placement)
                    .draw();
            }
            PageDecorationKind::ColumnExclusion => {
                let bounds = decoration.bounds;
                painter
                    .fill(
                        RoundedRect::from_rect(bounds, 12.0),
                        Color::from_rgba8(0xf5, 0xc4, 0x51, 0x17),
                    )
                    .transform(placement)
                    .draw();
                painter
                    .stroke(
                        RoundedRect::from_rect(bounds, 12.0),
                        &Stroke::new(1.0),
                        GOLD.with_alpha(0.48),
                    )
                    .transform(placement)
                    .draw();
                for fraction in [0.28, 0.5, 0.72] {
                    let y = bounds.y0 + bounds.height() * fraction;
                    painter
                        .stroke(
                            Line::new((bounds.x0 + 14.0, y), (bounds.x1 - 14.0, y)),
                            &Stroke::new(1.0),
                            CORAL.with_alpha(0.5),
                        )
                        .transform(placement)
                        .draw();
                }
            }
        }
    }
}

fn paint_flow_diagnostics<S: PaintSink + ?Sized>(
    painter: &mut Painter<'_, S>,
    page: &LivingPagePlan,
    transcript: &RegionTranscript,
    placement: Affine,
) {
    let region_stroke = Stroke::new(1.0).with_dashes(0.0, [8.0, 5.0]);
    for (index, region) in page.flow().regions().enumerate() {
        let color = if index == 0 { CORAL } else { CYAN };
        painter
            .stroke(
                region.bounds(),
                &region_stroke,
                color.with_alpha(if index == 0 { 0.72 } else { 0.38 }),
            )
            .transform(placement)
            .draw();
    }
    for attempt in transcript.attempts() {
        let color = match attempt.outcome() {
            RegionAttemptOutcome::Accepted => CYAN,
            RegionAttemptOutcome::HeightRejected => CORAL,
        };
        let bounds = attempt.slot().bounds();
        painter
            .fill(bounds, color.with_alpha(0.045))
            .transform(placement)
            .draw();
        painter
            .stroke(
                bounds,
                &Stroke::new(
                    if attempt.outcome() == RegionAttemptOutcome::HeightRejected {
                        2.0
                    } else {
                        0.7
                    },
                ),
                color.with_alpha(0.6),
            )
            .transform(placement)
            .draw();
    }
}

struct TextSceneAdapter<'a> {
    lines: AnyLines<'a>,
    fragments: AnyFragments<'a>,
    paint: &'a PaintTable,
    semantics: AnySemantics<'a>,
    placement: Affine,
}

impl<'a> TextSceneAdapter<'a> {
    fn new(
        lines: AnyLines<'a>,
        fragments: AnyFragments<'a>,
        paint: &'a PaintTable,
        semantics: AnySemantics<'a>,
        layout: FrameLayout,
    ) -> Self {
        Self {
            lines,
            fragments,
            paint,
            semantics,
            placement: Affine::scale(layout.scale)
                * Affine::translate((layout.origin_x, layout.origin_y)),
        }
    }

    fn paint_into<S: PaintSink + ?Sized>(
        &self,
        painter: &mut Painter<'_, S>,
        diagnostics: DiagnosticsMode,
        overlay: &EditorOverlay,
    ) {
        match diagnostics {
            DiagnosticsMode::Lines => self.paint_line_diagnostics(painter),
            DiagnosticsMode::Fragments => self.paint_fragment_diagnostics(painter),
            DiagnosticsMode::Off
            | DiagnosticsMode::Flow
            | DiagnosticsMode::Glyphs
            | DiagnosticsMode::Semantics => {}
        }

        self.paint_selection_backgrounds(painter, overlay);

        let fill = Style::Fill(Fill::NonZero);
        for fragment in self.fragments.clone() {
            let brush = self
                .paint
                .brush(fragment.paint())
                .expect("validated scene paint slot must exist");
            let glyphs = fragment.glyphs().map(|glyph| record::Glyph {
                id: glyph.id(),
                x: finite_f32(glyph.position().x),
                y: finite_f32(glyph.position().y),
            });
            let transform = self.placement * fragment.transform();
            let draw = |painter: &mut Painter<'_, S>| {
                painter
                    .glyphs(fragment.font(), brush)
                    .transform(transform)
                    .glyph_transform(fragment.synthesis().skew_transform())
                    .font_size(fragment.font_size())
                    .normalized_coords(fragment.normalized_coords())
                    .draw(&fill, glyphs);
            };
            if let Some(clip) = fragment.paint_clip() {
                painter.with_fill_clip_transformed(clip, self.placement, draw);
            } else {
                draw(painter);
            }
        }

        self.paint_editor_marks(painter, overlay);

        match diagnostics {
            DiagnosticsMode::Glyphs => self.paint_glyph_diagnostics(painter),
            DiagnosticsMode::Semantics => self.paint_semantic_diagnostics(painter),
            DiagnosticsMode::Off
            | DiagnosticsMode::Flow
            | DiagnosticsMode::Lines
            | DiagnosticsMode::Fragments => {}
        }
    }

    fn paint_selection_backgrounds<S: PaintSink + ?Sized>(
        &self,
        painter: &mut Painter<'_, S>,
        overlay: &EditorOverlay,
    ) {
        for selection in &overlay.selections {
            let color = if selection.selection == 0 {
                SELECTION_PRIMARY
            } else {
                SELECTION_SECONDARY
            };
            painter
                .fill(selection.bounds, color)
                .transform(self.placement)
                .draw();
        }
        for bounds in &overlay.preedit_selection {
            painter
                .fill(*bounds, PREEDIT_SELECTION)
                .transform(self.placement)
                .draw();
        }
    }

    fn paint_editor_marks<S: PaintSink + ?Sized>(
        &self,
        painter: &mut Painter<'_, S>,
        overlay: &EditorOverlay,
    ) {
        for bounds in &overlay.marked_text {
            painter
                .fill(
                    Rect::new(
                        bounds.x0,
                        bounds.y1 - 2.0,
                        bounds.x1.max(bounds.x0 + 1.0),
                        bounds.y1,
                    ),
                    GOLD,
                )
                .transform(self.placement)
                .draw();
        }
        if overlay.caret_visible {
            for (index, bounds) in overlay.carets.iter().enumerate() {
                painter
                    .fill(
                        Rect::new(bounds.x0, bounds.y0, bounds.x0 + 1.5, bounds.y1),
                        if index == 0 { CYAN } else { CORAL },
                    )
                    .transform(self.placement)
                    .draw();
            }
        }
    }

    fn paint_line_diagnostics<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        let dashed = Stroke::new(1.0).with_dashes(0.0, [5.0, 5.0]);
        for line in self.lines.clone() {
            let color = match line.break_reason() {
                LineBreakReason::Regular => CYAN,
                LineBreakReason::Mandatory => CORAL,
                LineBreakReason::End => GOLD,
            };
            let content_bounds = Rect::new(
                line.bounds().x0,
                line.baseline() - line.content_ascent(),
                line.bounds().x1,
                line.baseline() + line.content_descent(),
            );
            painter
                .fill(content_bounds, color.with_alpha(0.055))
                .transform(self.placement)
                .draw();
            painter
                .stroke(content_bounds, &Stroke::new(0.75), color.with_alpha(0.24))
                .transform(self.placement)
                .draw();
            painter
                .stroke(line.bounds(), &dashed, color.with_alpha(0.42))
                .transform(self.placement)
                .draw();
            painter
                .fill(
                    Rect::new(
                        line.bounds().x0,
                        line.baseline(),
                        line.bounds().x1,
                        line.baseline() + 0.7,
                    ),
                    color.with_alpha(0.28),
                )
                .transform(self.placement)
                .draw();
        }
    }

    fn paint_fragment_diagnostics<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        for fragment in self.fragments.clone() {
            let Some(bounds) = fragment_advance_envelope(fragment, self.lines.clone()) else {
                continue;
            };
            let script = script_color(fragment.script());
            let direction = bidi_color(fragment.bidi_level());
            let owns_multiple_sources = fragment.owns_multiple_sources();
            painter
                .fill(bounds, script.with_alpha(0.075))
                .transform(self.placement)
                .draw();
            painter
                .stroke(
                    bounds,
                    &Stroke::new(if owns_multiple_sources { 2.0 } else { 0.9 }),
                    if owns_multiple_sources {
                        GOLD.with_alpha(0.76)
                    } else {
                        direction.with_alpha(0.58)
                    },
                )
                .transform(self.placement)
                .draw();
            if let Some(clip) = fragment.paint_clip() {
                painter
                    .stroke(clip, &Stroke::new(1.4), CORAL.with_alpha(0.9))
                    .transform(self.placement)
                    .draw();
            }
        }
    }

    fn paint_glyph_diagnostics<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        for fragment in self.fragments.clone() {
            let transform = self.placement * fragment.transform();
            for glyph in fragment.glyphs() {
                let origin = glyph.position();
                let end = origin + glyph.advance();
                let owns_multiple_sources = glyph.owns_multiple_sources();
                let color = if owns_multiple_sources {
                    GOLD
                } else {
                    bidi_color(fragment.bidi_level())
                };
                painter
                    .stroke(
                        Line::new(origin, end),
                        &Stroke::new(if owns_multiple_sources { 1.6 } else { 0.75 }),
                        color.with_alpha(0.72),
                    )
                    .transform(transform)
                    .draw();
                painter
                    .fill(
                        Circle::new(origin, if owns_multiple_sources { 2.8 } else { 1.7 }),
                        color.with_alpha(0.92),
                    )
                    .transform(transform)
                    .draw();
                painter
                    .fill(Circle::new(end, 0.9), color.with_alpha(0.52))
                    .transform(transform)
                    .draw();
            }
        }
    }

    fn paint_semantic_diagnostics<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        let paragraph_stroke = Stroke::new(1.0).with_dashes(0.0, [7.0, 4.0]);
        let inline_stroke = Stroke::new(0.8);
        for semantic in self.semantics.clone() {
            let (color, stroke) = if let Some(role) = semantic.paragraph_role() {
                let color = if role == ParagraphRole::HEADING_1 {
                    GOLD
                } else if role == ParagraphRole::HEADING_2 {
                    CYAN
                } else {
                    PAGE_EDGE.with_alpha(0.9)
                };
                (color, &paragraph_stroke)
            } else {
                let color = if semantic.inline_role() == Some(InlineRole::EMPHASIS) {
                    CORAL
                } else {
                    CYAN
                };
                (color, &inline_stroke)
            };
            painter
                .stroke(semantic.bounds(), stroke, color.with_alpha(0.64))
                .transform(self.placement)
                .draw();
        }
    }
}

fn fragment_advance_envelope(fragment: AnyFragment<'_>, lines: AnyLines<'_>) -> Option<Rect> {
    let mut glyphs = fragment.glyphs();
    let first = glyphs.next()?;
    let transform = fragment.transform();
    let first_origin = transform * first.position();
    let first_end = transform * (first.position() + first.advance());
    let mut x0 = first_origin.x.min(first_end.x);
    let mut x1 = first_origin.x.max(first_end.x);
    for glyph in glyphs {
        let origin = transform * glyph.position();
        let end = transform * (glyph.position() + glyph.advance());
        x0 = x0.min(origin.x.min(end.x));
        x1 = x1.max(origin.x.max(end.x));
    }
    let line = lines.min_by(|left, right| {
        (left.baseline() - first_origin.y)
            .abs()
            .total_cmp(&(right.baseline() - first_origin.y).abs())
    })?;
    Some(Rect::new(
        x0,
        line.baseline() - line.content_ascent(),
        x1.max(x0 + 1.0),
        line.baseline() + line.content_descent(),
    ))
}

const fn bidi_color(level: u8) -> Color {
    if level.is_multiple_of(2) { CYAN } else { CORAL }
}

const fn script_color(script: [u8; 4]) -> Color {
    match script {
        [b'L', b'a', b't', b'n'] => CYAN,
        [b'A', b'r', b'a', b'b'] => GOLD,
        _ => CORAL,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "imaging glyph coordinates are f32; reject non-finite or out-of-range scene values first"
)]
fn finite_f32(value: f64) -> f32 {
    assert!(
        value.is_finite() && value >= f64::from(f32::MIN) && value <= f64::from(f32::MAX),
        "scene coordinate must be finite and representable by imaging"
    );
    value as f32
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticsMode, EditorOverlay, FrameLayout, record_frame};
    use crate::content::ShowcaseContent;

    #[test]
    fn vertical_fit_is_explicit_at_default_and_short_sizes() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let default = FrameLayout::new(1_100, 800, 1.0);
        let default_document = content
            .prepare(default.content_width, 0.62)
            .expect("default document must prepare");
        let content_bottom = default_document
            .scene
            .lines()
            .iter()
            .map(|line| line.bounds().y1)
            .fold(0.0_f64, f64::max);
        assert!(
            !default.document_is_clipped(&default_document.scene),
            "content bottom {content_bottom}, origin {}, page bottom {}",
            default.origin_y,
            default.page_rect().y1
        );

        let short = FrameLayout::new(520, 520, 1.0);
        let short_document = content
            .prepare(short.content_width, 0.62)
            .expect("short document must prepare");
        assert!(short.document_is_clipped(&short_document.scene));
    }

    #[test]
    fn diagnostic_modes_form_one_complete_cycle() {
        let modes = [
            DiagnosticsMode::Off,
            DiagnosticsMode::Flow,
            DiagnosticsMode::Lines,
            DiagnosticsMode::Fragments,
            DiagnosticsMode::Glyphs,
            DiagnosticsMode::Semantics,
        ];
        let mut mode = DiagnosticsMode::Off;
        for expected in modes.into_iter().skip(1).chain([DiagnosticsMode::Off]) {
            mode = mode.next();
            assert_eq!(mode, expected);
        }
        assert_eq!(
            modes.map(DiagnosticsMode::label),
            ["OFF", "FLOW", "LINES", "FRAGMENTS", "GLYPHS", "SEMANTICS"]
        );
    }

    #[test]
    fn every_diagnostic_mode_records_real_scene_observations() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let layout = FrameLayout::new(1_100, 800, 1.0);
        let prepared = content
            .prepare(layout.content_width, 0.62)
            .expect("document must prepare");
        let overlay = EditorOverlay::default();
        let clean = record_frame(
            &prepared.scene,
            &prepared.page,
            &prepared.region_transcript,
            layout,
            DiagnosticsMode::Off,
            &overlay,
        )
        .expect("clean frame must record");
        for mode in [
            DiagnosticsMode::Flow,
            DiagnosticsMode::Lines,
            DiagnosticsMode::Fragments,
            DiagnosticsMode::Glyphs,
            DiagnosticsMode::Semantics,
        ] {
            let debug = record_frame(
                &prepared.scene,
                &prepared.page,
                &prepared.region_transcript,
                layout,
                mode,
                &overlay,
            )
            .expect("diagnostic frame must record");
            assert!(
                debug.commands().len() > clean.commands().len(),
                "{mode:?} must add visible diagnostic commands"
            );
        }
    }
}
