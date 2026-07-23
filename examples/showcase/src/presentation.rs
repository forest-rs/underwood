// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rendering-only bridge from a portable Underwood scene to `imaging`.

use imaging::kurbo::{Affine, Rect, RoundedRect, Stroke};
use imaging::peniko::{Color, Fill, Style};
use imaging::{PaintSink, Painter, record};
use underwood::{
    CompositionScene, PaintTable, Point, SceneFragment, SceneLine, TextScene, Vec2,
    adapter::LineBreakReason,
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
            origin_y: outer + inset,
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
        self.lines_are_clipped(document.lines())
    }

    /// Reports whether arbitrary committed or projected lines exceed the page.
    pub(crate) fn lines_are_clipped<Source>(self, lines: &[SceneLine<Source>]) -> bool {
        let content_bottom = lines
            .iter()
            .map(|line| line.bounds().y1)
            .fold(0.0_f64, f64::max);
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
    layout: FrameLayout,
    show_guides: bool,
    overlay: &EditorOverlay,
) -> Result<record::Scene, record::ValidateError> {
    record_scene(
        document.lines(),
        document.fragments(),
        document.paint(),
        layout,
        show_guides,
        overlay,
    )
}

/// Records a transient composition scene with the same native presentation.
pub(crate) fn record_composition_frame(
    document: &CompositionScene,
    layout: FrameLayout,
    show_guides: bool,
    overlay: &EditorOverlay,
) -> Result<record::Scene, record::ValidateError> {
    record_scene(
        document.lines(),
        document.fragments(),
        document.paint(),
        layout,
        show_guides,
        overlay,
    )
}

fn record_scene<Source>(
    lines: &[SceneLine<Source>],
    fragments: &[SceneFragment<Source>],
    paint: &PaintTable,
    layout: FrameLayout,
    show_guides: bool,
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

        TextSceneAdapter::new(lines, fragments, paint, layout).paint_into(
            &mut painter,
            show_guides,
            overlay,
        );
    }
    scene.validate()?;
    Ok(scene)
}

struct TextSceneAdapter<'a, Source> {
    lines: &'a [SceneLine<Source>],
    fragments: &'a [SceneFragment<Source>],
    paint: &'a PaintTable,
    placement: Affine,
}

impl<'a, Source> TextSceneAdapter<'a, Source> {
    fn new(
        lines: &'a [SceneLine<Source>],
        fragments: &'a [SceneFragment<Source>],
        paint: &'a PaintTable,
        layout: FrameLayout,
    ) -> Self {
        Self {
            lines,
            fragments,
            paint,
            placement: Affine::scale(layout.scale)
                * Affine::translate((layout.origin_x, layout.origin_y)),
        }
    }

    fn paint_into<S: PaintSink + ?Sized>(
        &self,
        painter: &mut Painter<'_, S>,
        show_guides: bool,
        overlay: &EditorOverlay,
    ) {
        if show_guides {
            self.paint_line_guides(painter);
        }

        self.paint_selection_backgrounds(painter, overlay);

        let fill = Style::Fill(Fill::NonZero);
        for fragment in self.fragments {
            let brush = self
                .paint
                .brush(fragment.paint())
                .expect("validated scene paint slot must exist");
            let glyphs = fragment.glyphs().iter().map(|glyph| record::Glyph {
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

    fn paint_line_guides<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        let dashed = Stroke::new(1.0).with_dashes(0.0, [5.0, 5.0]);
        for line in self.lines {
            let color = match line.break_reason() {
                LineBreakReason::Regular => CYAN,
                LineBreakReason::Mandatory => CORAL,
                LineBreakReason::End => GOLD,
            };
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
    use super::FrameLayout;
    use crate::content::ShowcaseContent;

    #[test]
    fn vertical_fit_is_explicit_at_default_and_short_sizes() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let default = FrameLayout::new(1_100, 800, 1.0);
        let default_document = content
            .prepare(default.content_width, 0.62)
            .expect("default document must prepare");
        assert!(!default.document_is_clipped(&default_document.scene));

        let short = FrameLayout::new(520, 520, 1.0);
        let short_document = content
            .prepare(short.content_width, 0.62)
            .expect("short document must prepare");
        assert!(short.document_is_clipped(&short_document.scene));
    }
}
