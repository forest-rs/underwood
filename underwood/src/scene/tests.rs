// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{vec, vec::Vec};

use peniko::Blob;

use super::{
    CacheBudget, LayoutEngine, append_analysis_run, append_inline_flow_run, append_shaping_run,
};
use crate::adapter::{
    ClusterBoundary, ClusterWhitespace, FontSynthesis, FormationWork, GlyphPaintCoverage,
    GlyphPaintSegment, LineBreakReason, LineShapingWork, ParagraphConstraints, ParagraphFormation,
    ParagraphFormationOutput, ParagraphInput, PreparationError, PreparationErrorKind,
    PreparedCaret, PreparedClusterSide, PreparedCursorMovement, PreparedCursorStep, PreparedGlyph,
    PreparedInteractionSlice, PreparedInteractionUnit, PreparedLine, PreparedParagraph,
    PreparedRun, TextAffinity,
};
use crate::{
    AnalysisStyle, BaseDirection, Brush, Color, CompositionClause, CompositionClauseKind,
    CompositionErrorKind, CompositionId, CompositionSession, CompositionUpdate,
    ComputedInlineStyle, Document, DocumentId, EditableSurface, EditableSurfaceElement,
    FiniteWidth, FontData, FontFamily, InlineFlowStyle, InlineRole, PaintSlot, PaintTable,
    ParagraphRole, ParagraphStyle, Point, ProjectedTextSource, Rect, SceneErrorKind, SceneRequest,
    ShapingStyle, SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection,
    SnapshotTextSelectionSet, StyleMap, SurfaceErrorKind, SurfaceTextEncoding, TextConstraint,
    TextId, TextMovement, TextSelectionMode, Vec2, WhitespaceCollapse, WordBreak,
};

#[derive(Debug)]
struct EchoAdapter {
    split_utf8: bool,
    split_paint: bool,
    mismatched_paint: bool,
    glyphless: bool,
    interior_cursor: bool,
}

impl ParagraphFormation for EchoAdapter {
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        _constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError> {
        let text_len =
            u32::try_from(input.text().len()).map_err(|_| PreparationError::invalid_output())?;
        if text_len == 0 {
            let position = PreparedClusterSide::new(0, TextAffinity::Downstream);
            let movements = [PreparedCursorMovement::new(
                position,
                PreparedCaret::try_new(0, 0.0)?,
                None,
                None,
                None,
                None,
            )];
            let paragraph = PreparedParagraph::try_new(input.paragraph(), text_len, [], movements)?;
            return Ok(ParagraphFormationOutput::new(
                paragraph,
                FormationWork::new(true, true, 0, 0, 0, 0, LineShapingWork::default()),
            ));
        }
        let glyph_source = if self.split_utf8 {
            1..text_len
        } else {
            0..text_len
        };
        let glyphs = if self.glyphless {
            Vec::new()
        } else {
            let paint = if self.split_paint {
                let [first, second] = input.paint_runs() else {
                    return Err(PreparationError::invalid_output());
                };
                GlyphPaintCoverage::try_from_segments([
                    GlyphPaintSegment::clipped(
                        first.bytes(),
                        first.slot(),
                        Rect::new(0.0, -8.0, 5.0, 2.0),
                    )?,
                    GlyphPaintSegment::clipped(
                        second.bytes(),
                        second.slot(),
                        Rect::new(5.0, -8.0, 10.0, 2.0),
                    )?,
                ])?
            } else {
                let slot = if self.mismatched_paint {
                    PaintSlot::new(99)
                } else {
                    input.paint_runs()[0].slot()
                };
                GlyphPaintCoverage::whole(glyph_source.clone(), slot)?
            };
            let offset = if self.split_paint {
                Vec2::new(3.0, 4.0)
            } else {
                Vec2::ZERO
            };
            vec![PreparedGlyph::try_new(
                7,
                glyph_source,
                Vec2::new(10., 0.),
                offset,
                paint,
            )?]
        };
        let synthesis = if self.split_paint {
            FontSynthesis::try_new([], false, Some(14.0))?
        } else {
            FontSynthesis::default()
        };
        let run = PreparedRun::try_new(
            0..text_len,
            0,
            *b"Latn",
            FontData::new(Blob::from(vec![0_u8]), 0),
            input.shaping_styles()[input.shaping_runs()[0].style().index()].font_size(),
            synthesis,
            [],
            [],
            glyphs,
        )?;
        let font_size = input.shaping_styles()[input.shaping_runs()[0].style().index()].font_size();
        let line_height = f64::from(
            input.inline_flow_styles()[input.inline_flow_runs()[0].style().index()]
                .line_height()
                .resolve(font_size, font_size),
        );
        let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
        let end = PreparedClusterSide::new(text_len, TextAffinity::Upstream);
        let units = if self.split_paint {
            let middle = input.paint_runs()[0].bytes().end;
            vec![
                PreparedInteractionUnit::try_new(
                    0..middle,
                    [PreparedInteractionSlice::try_new(0..middle, 5.0)?],
                    0,
                    ClusterBoundary::None,
                    ClusterWhitespace::None,
                    start,
                    PreparedClusterSide::new(middle, TextAffinity::Upstream),
                )?,
                PreparedInteractionUnit::try_new(
                    middle..text_len,
                    [PreparedInteractionSlice::try_new(middle..text_len, 5.0)?],
                    0,
                    ClusterBoundary::None,
                    ClusterWhitespace::None,
                    PreparedClusterSide::new(middle, TextAffinity::Upstream),
                    end,
                )?,
            ]
        } else {
            vec![PreparedInteractionUnit::try_new(
                0..text_len,
                [PreparedInteractionSlice::try_new(0..text_len, 10.0)?],
                0,
                ClusterBoundary::None,
                ClusterWhitespace::None,
                start,
                end,
            )?]
        };
        let line = PreparedLine::try_new(
            0..text_len,
            LineBreakReason::End,
            10.0,
            line_height / 2.0,
            line_height,
            f64::from(font_size) * 0.75,
            f64::from(font_size) * 0.25,
            units,
            [run],
        )?;
        let mut movements = if self.split_paint {
            let middle_offset = input.paint_runs()[0].bytes().end;
            let middle = PreparedClusterSide::new(middle_offset, TextAffinity::Upstream);
            vec![
                PreparedCursorMovement::new(
                    start,
                    PreparedCaret::try_new(0, 0.0)?,
                    None,
                    Some(PreparedCursorStep::new(middle, Some(0..middle_offset))),
                    None,
                    Some(PreparedCursorStep::new(middle, Some(0..middle_offset))),
                ),
                PreparedCursorMovement::new(
                    middle,
                    PreparedCaret::try_new(0, 5.0)?,
                    Some(PreparedCursorStep::new(start, Some(0..middle_offset))),
                    Some(PreparedCursorStep::new(end, Some(middle_offset..text_len))),
                    Some(PreparedCursorStep::new(start, Some(0..middle_offset))),
                    Some(PreparedCursorStep::new(end, Some(middle_offset..text_len))),
                ),
                PreparedCursorMovement::new(
                    end,
                    PreparedCaret::try_new(0, 10.0)?,
                    Some(PreparedCursorStep::new(
                        middle,
                        Some(middle_offset..text_len),
                    )),
                    None,
                    Some(PreparedCursorStep::new(
                        middle,
                        Some(middle_offset..text_len),
                    )),
                    None,
                ),
            ]
        } else {
            vec![
                PreparedCursorMovement::new(
                    start,
                    PreparedCaret::try_new(0, 0.0)?,
                    None,
                    Some(PreparedCursorStep::new(end, Some(0..text_len))),
                    None,
                    Some(PreparedCursorStep::new(end, Some(0..text_len))),
                ),
                PreparedCursorMovement::new(
                    end,
                    PreparedCaret::try_new(0, 10.0)?,
                    Some(PreparedCursorStep::new(start, Some(0..text_len))),
                    None,
                    Some(PreparedCursorStep::new(start, Some(0..text_len))),
                    None,
                ),
            ]
        };
        if self.interior_cursor {
            movements.push(PreparedCursorMovement::new(
                PreparedClusterSide::new(1, TextAffinity::Downstream),
                PreparedCaret::try_new(0, 5.0)?,
                None,
                None,
                None,
                None,
            ));
        }
        let paragraph = PreparedParagraph::try_new(input.paragraph(), text_len, [line], movements)?;
        Ok(ParagraphFormationOutput::new(
            paragraph,
            FormationWork::new(
                true,
                true,
                1,
                1,
                1,
                1,
                LineShapingWork::new(2, 3, 4, 5).with_formation(6, 1, 2),
            ),
        ))
    }
}

#[derive(Debug, Default)]
struct RetainingInvalidAdapter {
    retained: bool,
}

impl ParagraphFormation for RetainingInvalidAdapter {
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError> {
        self.retained = true;
        EchoAdapter {
            split_utf8: true,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        }
        .form(input, constraints)
    }

    fn release(&mut self, _paragraph: crate::ParagraphId) {
        self.retained = false;
    }

    fn clear(&mut self) {
        self.retained = false;
    }

    fn retained_entries(&self) -> Option<usize> {
        Some(usize::from(self.retained))
    }
}

#[test]
fn invalid_first_output_releases_untracked_backend_state() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc13", "é");
    let mut layout = LayoutEngine::new(RetainingInvalidAdapter::default(), CacheBudget::new(32));
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    );
    layout
        .prepare(&document.snapshot(), &request)
        .expect_err("mid-scalar adapter source must be rejected");

    assert_eq!(
        layout.cache_diagnostics().current_entries(),
        0,
        "invalid output must not create geometry residency"
    );
    assert_eq!(
        layout.cache_diagnostics().backend_entries(),
        Some(0),
        "invalid output must release backend state with no geometry owner"
    );
}

#[test]
fn layout_rejects_adapter_ranges_inside_a_utf8_scalar() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc01", "é");
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: true,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    );
    let error = layout
        .prepare(&document.snapshot(), &request)
        .expect_err("mid-scalar adapter source must be rejected");
    assert_eq!(
        error.kind(),
        SceneErrorKind::SourceCoverage,
        "invalid UTF-8 coverage must be a source-coverage error"
    );
    assert!(
        error.paragraph().is_some(),
        "source-coverage diagnostics must identify the paragraph"
    );
    assert_eq!(
        error.source(),
        Some(1..2),
        "source-coverage diagnostics must retain the invalid range"
    );
}

#[test]
fn layout_rejects_a_cursor_inside_a_utf8_scalar() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc08", "é");
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: true,
        },
        CacheBudget::new(32),
    );
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    );
    let error = layout
        .prepare(&document.snapshot(), &request)
        .expect_err("mid-scalar cursor output must be rejected");
    assert_eq!(error.kind(), SceneErrorKind::SourceCoverage);
    assert_eq!(error.source(), Some(1..1));
}

#[test]
fn layout_rejects_glyphless_non_control_source() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc06", "a");
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: true,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    );
    let error = layout
        .prepare(&document.snapshot(), &request)
        .expect_err("glyphless non-control source must be rejected");
    assert_eq!(error.kind(), SceneErrorKind::SourceCoverage);
    assert_eq!(error.source(), Some(0..1));
}

#[test]
fn layout_rejects_partially_unmapped_run_source() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc07", "ab");
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: true,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    );
    let error = layout
        .prepare(&document.snapshot(), &request)
        .expect_err("every ordinary source scalar must map to a glyph");
    assert_eq!(error.kind(), SceneErrorKind::SourceCoverage);
    assert_eq!(error.source(), Some(0..1));
}

#[test]
fn layout_reports_adapter_paint_mismatch_as_invalid_preparation() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc12", "ab");
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: true,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let error = layout
        .prepare(&document.snapshot(), &request)
        .expect_err("adapter paint must match the projected paint run");
    assert_eq!(error.kind(), SceneErrorKind::Preparation);
    assert_eq!(
        error.preparation(),
        Some(PreparationErrorKind::InvalidOutput)
    );
    assert_eq!(error.source(), Some(0..2));
}

#[test]
fn fragment_identity_is_distinct_across_documents() {
    let (first, first_styles, first_paint) = one_leaf_document(*b"scene-test-doc02", "a");
    let (second, second_styles, second_paint) = one_leaf_document(*b"scene-test-doc03", "b");
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let width = FiniteWidth::new(100.).expect("test width is valid");
    let first_request = SceneRequest::new(TextConstraint::Wrap(width), &first_styles, &first_paint);
    let first_scene = layout
        .prepare(&first.snapshot(), &first_request)
        .expect("first scene must prepare");
    assert_eq!(
        first_scene.work().line_reshapes(),
        2,
        "adapter line-reshape work must survive scene reporting"
    );
    assert_eq!(
        first_scene.work().line_font_resolution().records(),
        3,
        "line-final retained-font resolution must survive scene reporting"
    );
    assert_eq!(
        first_scene.work().line_shape().records(),
        5,
        "line-final shaped glyph work must survive scene reporting"
    );
    assert_eq!(first_scene.work().line_candidates(), 6);
    assert_eq!(first_scene.work().rejected_line_candidates(), 1);
    assert_eq!(first_scene.work().accepted_line_candidates(), 5);
    assert_eq!(first_scene.work().line_checkpoint_restores(), 2);
    let second_request =
        SceneRequest::new(TextConstraint::Wrap(width), &second_styles, &second_paint);
    let second_scene = layout
        .prepare(&second.snapshot(), &second_request)
        .expect("second scene must prepare");
    assert_ne!(
        first_scene.scene().fragments()[0].id(),
        second_scene.scene().fragments()[0].id(),
        "document identity must participate in retained fragment identity"
    );
    assert_eq!(
        first_scene.scene().fragments()[0].paint_clip(),
        None,
        "ordinary whole-glyph paint must not create a renderer clip"
    );
}

#[test]
fn paragraph_style_override_from_another_document_is_rejected() {
    let (first, mut styles, paint) = one_leaf_document(*b"scene-style-doc1", "a");
    let (second, _, _) = one_leaf_document(*b"scene-style-doc2", "b");
    let foreign = second.snapshot().paragraphs()[0].id;
    styles.set_paragraph_style(foreign, ParagraphStyle::new(BaseDirection::Rtl));
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint);
    let error = layout
        .prepare(&first.snapshot(), &request)
        .expect_err("a foreign paragraph style must not be silently ignored");
    assert_eq!(error.kind(), SceneErrorKind::InvalidStyle);
}

#[test]
fn composition_whitespace_collapse_retains_complete_generated_provenance() {
    let (document, mut styles, paint) = one_leaf_document(*b"collapse-preedt1", "x");
    let snapshot = document.snapshot();
    let paragraph = snapshot.paragraphs()[0].id;
    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::DEFAULT.with_whitespace_collapse(WhitespaceCollapse::Collapse),
    );
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint);
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let committed = layout
        .prepare(&snapshot, &request)
        .expect("committed collapse fixture must prepare");
    let left = committed
        .scene()
        .hit_test(Point::new(0.1, 1.0))
        .expect("left side must hit");
    let right = committed
        .scene()
        .hit_test(Point::new(9.9, 1.0))
        .expect("right side must hit");
    let selection = committed
        .scene()
        .selection(
            left.position(),
            right.position(),
            TextSelectionMode::Logical,
        )
        .expect("fixture source must select");
    let selections = committed
        .scene()
        .selection_set([selection])
        .expect("fixture selection must validate");
    let mut session = committed
        .scene()
        .begin_composition(&selections, CompositionId::from_bytes(*b"collapse-preedt1"))
        .expect("composition must begin")
        .into_session();
    session
        .update(
            session.epoch(),
            CompositionUpdate::new("\t\r\n").with_selection(3..3),
        )
        .expect("whitespace preedit must update");

    let transient = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("collapsed whitespace preedit must prepare");
    let source = transient.scene().fragments()[0]
        .source()
        .expect("generated glyph must retain provenance");
    let [ProjectedTextSource::Composition(range)] = source.sources() else {
        panic!("collapsed preedit must have one generated source range");
    };
    assert_eq!(
        range.bytes(),
        0..3,
        "one display space must retain every generated whitespace byte"
    );
    assert_eq!(range.id(), session.id());
    assert_eq!(range.epoch(), session.epoch());
}

#[test]
fn explicit_split_paint_lowers_one_glyph_through_two_clipped_fragments() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-test-doc11"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph must append");
    let first_text = edit
        .append_text(paragraph, InlineRole::TEXT, "a")
        .expect("first text must append");
    let second_text = edit
        .append_text(paragraph, InlineRole::EMPHASIS, "b")
        .expect("second text must append");
    edit.commit().expect("test document must commit");

    let base = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 16.0).expect("test style must be valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let mut styles = StyleMap::new(base.clone());
    styles.set(first_text, base.clone());
    styles.set(second_text, base.with_paint(PaintSlot::new(1)));
    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::BLACK),
        Brush::Solid(Color::from_rgb8(0xff, 0x00, 0x00)),
    ]);
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.0).expect("test width is valid")),
        &styles,
        &paint,
    );
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: true,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let output = layout
        .prepare(&document.snapshot(), &request)
        .expect("explicitly clipped split paint must lower");
    let [first, second] = output.scene().fragments() else {
        panic!("one split glyph must lower to exactly two draw fragments");
    };

    assert_eq!(first.glyphs()[0].id(), second.glyphs()[0].id());
    assert_eq!(
        first.glyphs()[0].instance_id(),
        second.glyphs()[0].instance_id(),
        "partial-paint observations must retain one shaped-glyph identity"
    );
    assert_eq!(
        first.glyphs()[0].position(),
        second.glyphs()[0].position(),
        "paint splitting must not duplicate shaping or move the glyph"
    );
    assert_eq!(first.paint(), PaintSlot::new(0));
    assert_eq!(second.paint(), PaintSlot::new(1));
    assert_eq!(first.source().expect("first source").text(), first_text);
    assert_eq!(second.source().expect("second source").text(), second_text);
    assert!(first.synthesis().skew_transform().is_some());
    let origin = first.glyphs()[0].position();
    assert_eq!(first.paint_clip().expect("first clip").x0, origin.x);
    assert_eq!(first.paint_clip().expect("first clip").x1, origin.x + 5.0);
    assert_eq!(first.paint_clip().expect("first clip").y0, origin.y - 8.0);
    assert_eq!(first.paint_clip().expect("first clip").y1, origin.y + 2.0);
    assert_eq!(second.paint_clip().expect("second clip").x0, origin.x + 5.0);
    assert_eq!(
        second.paint_clip().expect("second clip").x1,
        origin.x + 10.0
    );
    assert_eq!(second.paint_clip().expect("second clip").y0, origin.y - 8.0);
    assert_eq!(second.paint_clip().expect("second clip").y1, origin.y + 2.0);
    assert_eq!(
        output.scene().lines()[0].fragment_range(),
        0..2,
        "the line must identify both paint fragments directly"
    );
}

#[test]
fn paragraph_projection_interns_repeated_style_partitions() {
    let (document, _, _) = one_leaf_document(*b"scene-test-doc04", "abc");
    let paragraph = document.snapshot().paragraphs()[0].id;
    let first = ShapingStyle::new(FontFamily::named("Test"), 16.).expect("test style is valid");
    let second = ShapingStyle::new(FontFamily::named("Test"), 24.).expect("test style is valid");
    let normal = AnalysisStyle::default();
    let keep_all = AnalysisStyle::new(WordBreak::KeepAll);
    let mut analysis_styles = Vec::new();
    let mut analysis_runs = Vec::new();
    append_analysis_run(
        &mut analysis_styles,
        &mut analysis_runs,
        0..1,
        normal,
        paragraph,
    )
    .expect("first analysis style must intern");
    append_analysis_run(
        &mut analysis_styles,
        &mut analysis_runs,
        1..2,
        keep_all,
        paragraph,
    )
    .expect("second analysis style must intern");
    append_analysis_run(
        &mut analysis_styles,
        &mut analysis_runs,
        2..3,
        normal,
        paragraph,
    )
    .expect("repeated analysis style must intern");
    assert_eq!(analysis_styles, [normal, keep_all]);
    assert_eq!(analysis_runs[0].style().index(), 0);
    assert_eq!(analysis_runs[1].style().index(), 1);
    assert_eq!(analysis_runs[2].style().index(), 0);

    let mut shaping_styles = Vec::new();
    let mut shaping_runs = Vec::new();
    append_shaping_run(
        &mut shaping_styles,
        &mut shaping_runs,
        0..1,
        &first,
        paragraph,
    )
    .expect("first style must intern");
    append_shaping_run(
        &mut shaping_styles,
        &mut shaping_runs,
        1..2,
        &second,
        paragraph,
    )
    .expect("second style must intern");
    append_shaping_run(
        &mut shaping_styles,
        &mut shaping_runs,
        2..3,
        &first,
        paragraph,
    )
    .expect("repeated style must intern");
    assert_eq!(shaping_styles, [&first, &second]);
    assert_eq!(shaping_runs[0].style().index(), 0);
    assert_eq!(shaping_runs[1].style().index(), 1);
    assert_eq!(shaping_runs[2].style().index(), 0);

    let compact = InlineFlowStyle::new(
        crate::LineHeight::from_multiplier(1.0).expect("line height is valid"),
    );
    let spacious = InlineFlowStyle::new(
        crate::LineHeight::from_multiplier(2.0).expect("line height is valid"),
    );
    let mut flow_styles = Vec::new();
    let mut flow_runs = Vec::new();
    append_inline_flow_run(&mut flow_styles, &mut flow_runs, 0..1, compact, paragraph)
        .expect("first flow style must intern");
    append_inline_flow_run(&mut flow_styles, &mut flow_runs, 1..2, spacious, paragraph)
        .expect("second flow style must intern");
    append_inline_flow_run(&mut flow_styles, &mut flow_runs, 2..3, compact, paragraph)
        .expect("repeated flow style must intern");
    assert_eq!(flow_styles, [compact, spacious]);
    assert_eq!(flow_runs[0].style().index(), 0);
    assert_eq!(flow_runs[1].style().index(), 1);
    assert_eq!(flow_runs[2].style().index(), 0);
}

#[test]
fn empty_paragraph_line_height_has_a_flow_identity() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-test-doc05"));
    let mut edit = document.edit();
    edit.append_paragraph(ParagraphRole::BODY)
        .expect("empty paragraph must append");
    let second = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("second paragraph must append");
    let text = edit
        .append_text(second, InlineRole::TEXT, "a")
        .expect("second paragraph text must append");
    edit.commit().expect("test document must commit");

    let shaping = ShapingStyle::new(FontFamily::named("Test"), 10.).expect("test style is valid");
    let compact = ComputedInlineStyle::new(
        shaping.clone(),
        InlineFlowStyle::new(
            crate::LineHeight::from_multiplier(1.0).expect("line height is valid"),
        ),
        PaintSlot::new(0),
    );
    let spacious = ComputedInlineStyle::new(
        shaping,
        InlineFlowStyle::new(
            crate::LineHeight::from_multiplier(2.0).expect("line height is valid"),
        ),
        PaintSlot::new(0),
    );
    let mut compact_styles = StyleMap::new(compact.clone());
    compact_styles.set(text, compact.clone());
    let mut spacious_styles = StyleMap::new(spacious);
    spacious_styles.set(text, compact);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let width = FiniteWidth::new(100.).expect("test width is valid");
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );

    let compact_request = SceneRequest::new(TextConstraint::Wrap(width), &compact_styles, &paint);
    let compact_scene = layout
        .prepare(&document.snapshot(), &compact_request)
        .expect("compact scene must prepare");
    let spacious_request = SceneRequest::new(TextConstraint::Wrap(width), &spacious_styles, &paint);
    let spacious_scene = layout
        .prepare(&document.snapshot(), &spacious_request)
        .expect("spacious scene must prepare");
    assert_eq!(spacious_scene.work().shape().paragraphs(), 0);
    assert_eq!(spacious_scene.work().flow().paragraphs(), 1);
    assert_eq!(compact_scene.scene().lines()[0].bounds().y0, 10.0);
    assert_eq!(spacious_scene.scene().lines()[0].bounds().y0, 20.0);
}

#[test]
fn composition_epochs_preserve_generated_provenance_and_committed_cache() {
    let (mut document, styles, paint) = one_leaf_document(*b"scene-test-doc09", "office");
    let snapshot = document.snapshot();
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    );
    let committed = layout
        .prepare(&snapshot, &request)
        .expect("committed scene must prepare");
    let left = committed
        .scene()
        .hit_test(Point::new(0.0, 1.0))
        .expect("left cluster side must hit");
    let right = committed
        .scene()
        .hit_test(Point::new(9.9, 1.0))
        .expect("right cluster side must hit");
    let selection = committed
        .scene()
        .selection(
            left.position(),
            right.position(),
            TextSelectionMode::Logical,
        )
        .expect("whole-leaf selection must form");
    let selections = committed
        .scene()
        .selection_set([selection])
        .expect("selection set must validate");
    let start = committed
        .scene()
        .begin_composition(&selections, CompositionId::from_bytes(*b"composition-0001"))
        .expect("composition must start");
    assert!(!start.selection_changed());
    let mut session = start.into_session();
    let initial_epoch = session.epoch();
    let first_epoch = session
        .update(
            initial_epoch,
            CompositionUpdate::new("a\u{301}")
                .with_selection(0..3)
                .with_clauses([CompositionClause::new(
                    0..3,
                    CompositionClauseKind::Selected,
                )]),
        )
        .expect("combining preedit must validate");
    let invalid_selection = session
        .update(
            first_epoch,
            CompositionUpdate::new("é").with_selection(1..1),
        )
        .expect_err("a selection inside one UTF-8 scalar must fail atomically");
    assert_eq!(
        invalid_selection.kind(),
        CompositionErrorKind::InvalidPreeditRange,
        "the error must identify the preedit selection rather than mutate it"
    );
    assert_eq!(
        session.epoch(),
        first_epoch,
        "a rejected preedit update must not advance the epoch"
    );
    assert_eq!(
        session.text(),
        "a\u{301}",
        "a rejected preedit update must retain the preceding text"
    );
    let invalid_clauses = session
        .update(
            first_epoch,
            CompositionUpdate::new("abcd").with_clauses([
                CompositionClause::new(0..3, CompositionClauseKind::Raw),
                CompositionClause::new(2..4, CompositionClauseKind::Selected),
            ]),
        )
        .expect_err("overlapping native clauses must fail atomically");
    assert_eq!(
        invalid_clauses.kind(),
        CompositionErrorKind::InvalidClauseRange,
        "the error must distinguish clause topology from the preedit selection"
    );
    assert_eq!(
        session.epoch(),
        first_epoch,
        "a rejected clause update must not advance the epoch"
    );

    let first = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("first transient epoch must prepare");
    assert_eq!(first.work().shape().paragraphs(), 1);
    assert_eq!(first.scene().epoch(), first_epoch);
    assert!(first.scene().fragments().iter().all(|fragment| {
        fragment.source().is_some_and(|source| {
            source.sources().iter().all(|segment| {
                matches!(segment, ProjectedTextSource::Composition(range)
                        if range.id() == session.id() && range.epoch() == first_epoch)
            })
        })
    }));
    assert!(
        !first
            .scene()
            .composition_selection_geometry(&session)
            .expect("preedit selection geometry must resolve")
            .is_empty()
    );
    let marked_geometry = first
        .scene()
        .composition_geometry(&session)
        .expect("complete marked-text geometry must resolve");
    assert!(
        !marked_geometry.is_empty(),
        "the complete generated preedit must expose renderer-neutral geometry"
    );
    assert!(
        marked_geometry
            .iter()
            .all(|rect| rect.bounds().width() > 0.0),
        "combining preedit geometry must be cluster based rather than ink based"
    );

    let repeated = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("same epoch must reuse transient work");
    assert_eq!(repeated.work().shape().paragraphs(), 0);
    assert_eq!(repeated.work().reused_paragraphs(), 1);

    let selection_epoch = session
        .update(
            first_epoch,
            CompositionUpdate::new("a\u{301}").with_selection(3..3),
        )
        .expect("selection-only preedit change must advance the epoch");
    let selection_only = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("selection-only epoch must rebind retained geometry");
    assert_eq!(selection_only.work().shape().paragraphs(), 0);
    assert_eq!(selection_only.work().geometry().paragraphs(), 0);
    assert_eq!(selection_only.work().reused_paragraphs(), 1);
    assert!(selection_only.scene().fragments().iter().all(|fragment| {
        fragment.source().is_some_and(|source| {
            source.sources().iter().all(|segment| {
                matches!(segment, ProjectedTextSource::Composition(range)
                        if range.epoch() == selection_epoch)
            })
        })
    }));
    assert!(
        selection_only
            .scene()
            .composition_selection_geometry(&session)
            .expect("rebound selected range must resolve")
            .is_empty()
    );

    let second_epoch = session
        .update(
            selection_epoch,
            CompositionUpdate::new("مرحبا").with_selection(10..10),
        )
        .expect("Arabic preedit must validate");
    assert_eq!(second_epoch.get(), selection_epoch.get() + 1);
    let stale = session
        .update(first_epoch, CompositionUpdate::new("stale"))
        .expect_err("delayed epoch must fail");
    assert_eq!(stale.kind(), CompositionErrorKind::StaleEpoch);
    let second = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("updated transient epoch must prepare");
    assert_eq!(second.work().shape().paragraphs(), 1);
    assert_eq!(snapshot.text(left.position().text()), Some("office"));
    let surface = EditableSurface::new(
        &snapshot,
        [EditableSurfaceElement::text(left.position().text())],
    )
    .expect("focused surface must flatten the selected semantic leaf");
    let host = surface
        .bind_composition(second.scene(), &session)
        .expect("host queries must bind to the exact composition epoch");
    assert_eq!(
        surface
            .bind_composition(first.scene(), &session)
            .expect_err("host queries must reject geometry from an older epoch")
            .kind(),
        SurfaceErrorKind::WrongSnapshot,
        "text and geometry from different composition epochs must never be combined"
    );
    assert_eq!(host.text(), "مرحبا");
    assert_eq!(host.marked_range(), Some(0..10));
    assert_eq!(host.host_selection(), Some(10..10));
    assert_eq!(
        host.range_in_encoding(0..10, SurfaceTextEncoding::Utf16)
            .expect("Arabic range must convert to UTF-16"),
        0..5
    );
    assert_eq!(
        host.range_from_encoding(0..5, SurfaceTextEncoding::Utf16)
            .expect("UTF-16 range must round trip"),
        0..10
    );
    assert_eq!(
        host.text_for_range(0..10)
            .expect("synchronous text query must resolve"),
        "مرحبا"
    );
    assert!(host.caret_rect().is_some());
    assert!(
        host.first_rect_for_range(0..10)
            .expect("synchronous geometry query must resolve")
            .is_some()
    );
    assert!(
        host.offset_for_point(Point::new(0.0, 1.0)).is_some(),
        "point queries must map through the same transient scene"
    );

    let cancelled = layout
        .prepare(&snapshot, &request)
        .expect("cancelling must reveal committed geometry");
    assert_eq!(cancelled.work().shape().paragraphs(), 0);
    assert_eq!(cancelled.work().geometry().paragraphs(), 0);
    assert_eq!(cancelled.work().reused_paragraphs(), 1);

    let stale_session = session.clone();
    let replacement = session
        .commit(&mut document, "مرحبا")
        .expect("commit must publish one replacement");
    assert_eq!(replacement.publication().changes().paragraphs().len(), 1);
    assert_eq!(snapshot.text(left.position().text()), Some("office"));
    assert_eq!(
        replacement
            .publication()
            .snapshot()
            .text(left.position().text()),
        Some("مرحبا")
    );
    assert_eq!(
        layout
            .prepare_composition(
                replacement.publication().snapshot(),
                &request,
                &stale_session,
            )
            .expect_err("a committed document revision must reject its stale preedit")
            .kind(),
        SceneErrorKind::InvalidComposition,
        "composition base revisions are exact rather than relocatable"
    );
}

#[test]
fn visual_selection_uses_the_reciprocal_caret_path() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-visual-dir"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph must append");
    let text = edit
        .append_text(paragraph, InlineRole::TEXT, "ab")
        .expect("test text must append");
    edit.commit().expect("test document must commit");
    let snapshot = document.snapshot();
    let start = SnapshotTextPosition::new(snapshot.revision(), text, 0, TextAffinity::Downstream);
    let end = SnapshotTextPosition::new(snapshot.revision(), text, 2, TextAffinity::Upstream);
    let source = SnapshotTextRange::new(snapshot.revision(), text, 0..2);
    let scene = super::TextScene {
        document: snapshot.id(),
        revision: snapshot.revision(),
        metrics: super::TextMetrics::default(),
        lines: Vec::new(),
        fragments: Vec::new(),
        clusters: Vec::new(),
        carets: Vec::new(),
        movements: vec![
            super::SceneCursorMovement {
                position: start,
                previous_visual: None,
                next_visual: None,
                previous_logical: None,
                next_logical: None,
            },
            super::SceneCursorMovement {
                position: end,
                previous_visual: Some(super::SceneCursorStep {
                    target: start,
                    source: Some(crate::SnapshotTextUnit::new(vec![source.clone()])),
                }),
                next_visual: None,
                previous_logical: None,
                next_logical: None,
            },
        ],
        texts: vec![source.clone()],
        paint: PaintTable::from_brushes([Brush::Solid(Color::BLACK)]),
        semantics: Vec::new(),
    };

    let forward = scene
        .selection(&start, &end, TextSelectionMode::Visual)
        .expect("selection must use the equivalent reverse traversal");
    let reverse = scene
        .selection(&end, &start, TextSelectionMode::Visual)
        .expect("the represented traversal must select");
    assert_eq!(forward.ranges(), reverse.ranges());
    assert_eq!(forward.ranges(), [source]);

    let selections = scene
        .selection_set([forward])
        .expect("direction-independent selection must validate");
    let collapsed = scene
        .move_selections(&selections, TextMovement::PreviousVisual, false)
        .expect("visual ordering must use the reciprocal traversal");
    assert_eq!(
        collapsed.primary().expect("caret must survive").extent(),
        &start
    );
}

#[test]
fn composition_projection_rejects_a_missing_semantic_target() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc10", "office");
    let snapshot = document.snapshot();
    let missing = TextId {
        document: snapshot.id(),
        paragraph: 0,
        index: 99,
    };
    let position =
        SnapshotTextPosition::new(snapshot.revision(), missing, 0, TextAffinity::Downstream);
    let selection = SnapshotTextSelection::new(
        position,
        position,
        TextSelectionMode::Logical,
        vec![SnapshotTextRange::new(snapshot.revision(), missing, 0..0)],
    );
    let selections =
        SnapshotTextSelectionSet::new(snapshot.id(), snapshot.revision(), vec![selection]);
    let session =
        CompositionSession::new(CompositionId::from_bytes(*b"missing-target01"), selections);
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    );
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(32),
    );
    assert_eq!(
        layout
            .prepare_composition(&snapshot, &request, &session)
            .expect_err("generated text must not be projected into a missing leaf")
            .kind(),
        SceneErrorKind::InvalidComposition,
        "a matching paragraph index is insufficient without the semantic text leaf"
    );
}

fn one_leaf_document(identity: [u8; 16], text: &str) -> (Document, StyleMap, PaintTable) {
    let mut document = Document::new(DocumentId::from_bytes(identity));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("test paragraph must append");
    edit.append_text(paragraph, InlineRole::TEXT, text)
        .expect("test text must append");
    edit.commit().expect("test document must commit");
    let styles = StyleMap::new(ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 16.).expect("test style must be valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    ));
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    (document, styles, paint)
}
