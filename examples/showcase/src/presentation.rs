// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rendering-only bridge from a portable Underwood scene to `imaging`.

use imaging::kurbo::{Affine, Rect, RoundedRect, Stroke};
use imaging::peniko::{Color, Fill, Style};
use imaging::{PaintSink, Painter, record};
use underwood::{TextScene, adapter::LineBreakReason};

const BACKGROUND: Color = Color::from_rgb8(0x08, 0x0d, 0x14);
const PAGE: Color = Color::from_rgb8(0x0f, 0x17, 0x22);
const PAGE_EDGE: Color = Color::from_rgba8(0x8b, 0x9b, 0xb1, 0x30);
const CYAN: Color = Color::from_rgb8(0x4d, 0xd5, 0xe7);
const CORAL: Color = Color::from_rgb8(0xff, 0x6b, 0x67);
const GOLD: Color = Color::from_rgb8(0xf5, 0xc4, 0x51);

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
        let content_bottom = document
            .lines()
            .iter()
            .map(|line| line.bounds().y1)
            .fold(0.0_f64, f64::max);
        self.origin_y + content_bottom > self.page_rect().y1 - 20.0
    }
}

/// Records the document and optional line evidence into an imaging scene.
pub(crate) fn record_frame(
    document: &TextScene,
    layout: FrameLayout,
    show_guides: bool,
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

        TextSceneAdapter::new(document, layout).paint_into(&mut painter, show_guides);
    }
    scene.validate()?;
    Ok(scene)
}

struct TextSceneAdapter<'a> {
    scene: &'a TextScene,
    placement: Affine,
}

impl<'a> TextSceneAdapter<'a> {
    fn new(scene: &'a TextScene, layout: FrameLayout) -> Self {
        Self {
            scene,
            placement: Affine::scale(layout.scale)
                * Affine::translate((layout.origin_x, layout.origin_y)),
        }
    }

    fn paint_into<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>, show_guides: bool) {
        if show_guides {
            self.paint_line_guides(painter);
        }

        let fill = Style::Fill(Fill::NonZero);
        for fragment in self.scene.fragments() {
            let brush = self
                .scene
                .paint()
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
    }

    fn paint_line_guides<S: PaintSink + ?Sized>(&self, painter: &mut Painter<'_, S>) {
        let dashed = Stroke::new(1.0).with_dashes(0.0, [5.0, 5.0]);
        for line in self.scene.lines() {
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
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
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
