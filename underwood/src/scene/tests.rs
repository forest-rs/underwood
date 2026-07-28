// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{rc::Rc, sync::Arc, vec, vec::Vec};
use core::cell::Cell;

use peniko::Blob;

use super::{
    CacheBudget, LayoutEngine, append_analysis_run, append_inline_flow_run, append_shaping_run,
};
use crate::adapter::{
    ClusterBoundary, ClusterWhitespace, FontSynthesis, FormationWork, GlyphPaintCoverage,
    GlyphPaintSegment, LineBreakReason, LineShapingWork, ParagraphConstraints, ParagraphFormation,
    ParagraphFormationCacheDiagnostics, ParagraphFormationOutput, ParagraphInput,
    ParagraphPreparationId, PreparationError, PreparationErrorKind, PreparedClusterSide,
    PreparedGlyph, PreparedInteractionSlice, PreparedInteractionUnit, PreparedLine,
    PreparedParagraph, PreparedParagraphData, PreparedRun, TextAffinity,
};
use crate::{
    AnalysisStyle, BaseDirection, Brush, Color, CompositionClause, CompositionClauseKind,
    CompositionErrorKind, CompositionId, CompositionSession, CompositionUpdate,
    ComputedInlineStyle, Document, DocumentId, EditableSurface, EditableSurfaceElement,
    FiniteWidth, FontData, FontFamily, InlineFlowStyle, InlineRole, PaintSlot, PaintTable,
    ParagraphRole, ParagraphStyle, Point, ProjectedTextSource, Rect, RegionAttempt,
    RegionAttemptOutcome, RegionFlow, RegionTranscript, ResolvedDirection, SceneErrorKind,
    SceneRequest, ShapingStyle, SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection,
    SnapshotTextSelectionSet, StyleMap, SurfaceErrorKind, SurfaceTextEncoding, TextAlignment,
    TextConstraint, TextId, TextSelectionMode, Vec2, WhitespaceCollapse, WordBreak,
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
            u32::try_from(input.text.len()).map_err(|_| PreparationError::invalid_output())?;
        if text_len == 0 {
            let paragraph = PreparedParagraph::try_from_data(
                input.paragraph,
                text_len,
                ResolvedDirection::Ltr,
                input.features,
                PreparedParagraphData::new(),
            )?;
            return Ok(ParagraphFormationOutput::new(
                paragraph,
                FormationWork {
                    analyzed: true,
                    itemized: true,
                    ..FormationWork::default()
                },
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
                let [first, second] = input.paint_runs else {
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
            } else if self.mismatched_paint {
                if glyph_source.end - glyph_source.start < 2 {
                    return Err(PreparationError::invalid_output());
                }
                let middle = glyph_source.start + 1;
                GlyphPaintCoverage::try_from_segments([
                    GlyphPaintSegment::clipped(
                        glyph_source.start..middle,
                        PaintSlot::new(99),
                        Rect::new(0.0, -8.0, 5.0, 2.0),
                    )?,
                    GlyphPaintSegment::clipped(
                        middle..glyph_source.end,
                        PaintSlot::new(99),
                        Rect::new(5.0, -8.0, 10.0, 2.0),
                    )?,
                ])?
            } else {
                GlyphPaintCoverage::whole()
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
            input.shaping_styles[input.shaping_runs[0].style().index()].font_size(),
            synthesis,
        )?;
        let font_size = input.shaping_styles[input.shaping_runs[0].style().index()].font_size();
        let line_height = f64::from(
            input.inline_flow_styles[input.inline_flow_runs[0].style().index()]
                .line_height()
                .resolve(font_size, font_size),
        );
        let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
        let end = PreparedClusterSide::new(text_len, TextAffinity::Upstream);
        let (slices, units) = if self.interior_cursor {
            (
                vec![
                    PreparedInteractionSlice::try_new(0..1, 5.0)?,
                    PreparedInteractionSlice::try_new(1..text_len, 5.0)?,
                ],
                vec![
                    PreparedInteractionUnit::try_new(
                        0..1,
                        5.0,
                        0,
                        ClusterBoundary::None,
                        ClusterWhitespace::None,
                        start,
                        PreparedClusterSide::new(1, TextAffinity::Upstream),
                    )?,
                    PreparedInteractionUnit::try_new(
                        1..text_len,
                        5.0,
                        0,
                        ClusterBoundary::None,
                        ClusterWhitespace::None,
                        PreparedClusterSide::new(1, TextAffinity::Downstream),
                        end,
                    )?,
                ],
            )
        } else if self.split_paint {
            let middle = input.paint_runs[0].bytes().end;
            (
                vec![
                    PreparedInteractionSlice::try_new(0..middle, 5.0)?,
                    PreparedInteractionSlice::try_new(middle..text_len, 5.0)?,
                ],
                vec![
                    PreparedInteractionUnit::try_new(
                        0..middle,
                        5.0,
                        0,
                        ClusterBoundary::None,
                        ClusterWhitespace::None,
                        start,
                        PreparedClusterSide::new(middle, TextAffinity::Downstream),
                    )?,
                    PreparedInteractionUnit::try_new(
                        middle..text_len,
                        5.0,
                        0,
                        ClusterBoundary::None,
                        ClusterWhitespace::None,
                        PreparedClusterSide::new(middle, TextAffinity::Upstream),
                        end,
                    )?,
                ],
            )
        } else {
            (
                vec![PreparedInteractionSlice::try_new(0..text_len, 10.0)?],
                vec![PreparedInteractionUnit::try_new(
                    0..text_len,
                    10.0,
                    0,
                    ClusterBoundary::None,
                    ClusterWhitespace::None,
                    start,
                    end,
                )?],
            )
        };
        let line_data = PreparedLine::try_new(
            0..text_len,
            LineBreakReason::End,
            10.0,
            line_height / 2.0,
            line_height,
            f64::from(font_size) * 0.75,
            f64::from(font_size) * 0.25,
        )?;
        let mut data = PreparedParagraphData::with_capacity(1, 1, glyphs.len(), units.len(), 0);
        let units_start = data.unit_count();
        for unit in units {
            let source = unit.source();
            data.push_unit(
                unit,
                slices.iter().copied().filter(|slice| {
                    let slice = slice.source();
                    source.start <= slice.start && slice.end <= source.end
                }),
            )?;
        }
        let runs_start = data.run_count();
        let glyphs_start = data.glyph_count();
        for glyph in glyphs {
            data.push_glyph(glyph)?;
        }
        data.push_run(run, 0..0, 0..0, glyphs_start..data.glyph_count())?;
        data.push_line(
            line_data,
            units_start..data.unit_count(),
            runs_start..data.run_count(),
        )?;
        let paragraph = PreparedParagraph::try_from_data(
            input.paragraph,
            text_len,
            ResolvedDirection::Ltr,
            input.features,
            data,
        )?;
        Ok(ParagraphFormationOutput::new(
            paragraph,
            FormationWork {
                analyzed: true,
                itemized: true,
                selected_clusters: 1,
                shaped_runs: 1,
                shaped_glyphs: 1,
                formed_lines: 1,
                line_shaping: LineShapingWork {
                    attempts: 2,
                    resolved_clusters: 3,
                    shaped_runs: 4,
                    shaped_glyphs: 5,
                    candidates: 6,
                    rejected_candidates: 1,
                    checkpoint_restores: 2,
                },
            },
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

    fn release(&mut self, _preparation: ParagraphPreparationId) {
        self.retained = false;
    }

    fn clear(&mut self) {
        self.retained = false;
    }

    fn retained_facts(&self) -> Option<ParagraphFormationCacheDiagnostics> {
        Some(ParagraphFormationCacheDiagnostics {
            budget_bytes: usize::MAX,
            entries: usize::from(self.retained),
            ..ParagraphFormationCacheDiagnostics::default()
        })
    }
}

#[derive(Debug)]
struct MismatchedEmptyRegionAdapter;

impl ParagraphFormation for MismatchedEmptyRegionAdapter {
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError> {
        if !input.text.is_empty() {
            return Err(PreparationError::invalid_output());
        }
        let paragraph = PreparedParagraph::try_from_data(
            input.paragraph,
            0,
            ResolvedDirection::Ltr,
            crate::SceneFeatures::EDITABLE,
            PreparedParagraphData::new(),
        )?;
        let flow = constraints
            .region_flow()
            .ok_or_else(PreparationError::invalid_output)?;
        let cursor = constraints
            .region_cursor()
            .ok_or_else(PreparationError::invalid_output)?;
        let slot = flow
            .slot(cursor)
            .ok_or_else(PreparationError::invalid_output)?;
        let wrong_height = constraints.empty_line_height() / 2.0;
        let attempt = RegionAttempt::try_new(
            input.paragraph,
            0..0,
            slot,
            wrong_height,
            RegionAttemptOutcome::Accepted,
        )
        .map_err(|_| PreparationError::invalid_output())?;
        let end = flow
            .accept(cursor, slot, wrong_height)
            .map_err(|_| PreparationError::invalid_output())?;
        let transcript = RegionTranscript::try_new(flow, cursor, end, [attempt])
            .map_err(|_| PreparationError::invalid_output())?;
        Ok(ParagraphFormationOutput::in_regions(
            paragraph,
            FormationWork::default(),
            transcript,
        ))
    }
}

#[derive(Debug)]
struct SharedEligibilityAdapter {
    calls: Rc<Cell<usize>>,
    epoch: Rc<Cell<Option<u64>>>,
}

impl ParagraphFormation for SharedEligibilityAdapter {
    fn shared_preparation_epoch(&self) -> Option<u64> {
        self.epoch.get()
    }

    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError> {
        self.calls.set(self.calls.get().saturating_add(1));
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        }
        .form(input, constraints)
    }
}

#[test]
fn backends_are_ineligible_for_cross_identity_reuse_by_default() {
    let calls = Rc::new(Cell::new(0));
    let epoch = Rc::new(Cell::new(None));
    let mut layout = LayoutEngine::new(
        SharedEligibilityAdapter {
            calls: calls.clone(),
            epoch,
        },
        CacheBudget::new(8).with_shared_preparation_bytes(1024 * 1024),
    );
    let (first, first_styles, first_paint) = one_leaf_document(*b"scene-share-0001", "same");
    let (second, second_styles, second_paint) = one_leaf_document(*b"scene-share-0002", "same");

    layout
        .prepare(
            &first.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &first_styles, &first_paint),
        )
        .expect("first document prepares");
    let second_output = layout
        .prepare(
            &second.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &second_styles, &second_paint),
        )
        .expect("second document prepares");

    assert_eq!(calls.get(), 2);
    assert_eq!(second_output.work.shared_preparations, 0);
    let diagnostics = layout.cache_diagnostics();
    assert_eq!(diagnostics.shared_preparation_entries, 0);
    assert_eq!(diagnostics.shared_preparation_hits, 0);
    assert_eq!(diagnostics.shared_preparation_misses, 0);
}

#[test]
fn eligible_backend_epoch_changes_invalidate_shared_preparation() {
    let calls = Rc::new(Cell::new(0));
    let epoch = Rc::new(Cell::new(Some(7)));
    let mut layout = LayoutEngine::new(
        SharedEligibilityAdapter {
            calls: calls.clone(),
            epoch: epoch.clone(),
        },
        CacheBudget::new(8).with_shared_preparation_bytes(1024 * 1024),
    );
    let (first, first_styles, first_paint) = one_leaf_document(*b"scene-share-0003", "same");
    let (second, second_styles, second_paint) = one_leaf_document(*b"scene-share-0004", "same");
    let (third, third_styles, third_paint) = one_leaf_document(*b"scene-share-0005", "same");

    layout
        .prepare(
            &first.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &first_styles, &first_paint),
        )
        .expect("first document prepares");
    let shared = layout
        .prepare(
            &second.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &second_styles, &second_paint),
        )
        .expect("second document shares preparation");
    assert_eq!(calls.get(), 1);
    assert_eq!(shared.work.shared_preparations, 1);

    epoch.set(Some(8));
    let invalidated = layout
        .prepare(
            &third.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &third_styles, &third_paint),
        )
        .expect("new epoch prepares fresh output");
    assert_eq!(calls.get(), 2);
    assert_eq!(invalidated.work.shared_preparations, 0);
    let diagnostics = layout.cache_diagnostics();
    assert_eq!(diagnostics.shared_preparation_entries, 1);
    assert_eq!(diagnostics.shared_preparation_hits, 1);
    assert_eq!(diagnostics.shared_preparation_misses, 2);
}

#[test]
fn shared_hit_is_revalidated_against_the_current_projection() {
    let calls = Rc::new(Cell::new(0));
    let mut layout = LayoutEngine::new(
        SharedEligibilityAdapter {
            calls,
            epoch: Rc::new(Cell::new(Some(1))),
        },
        CacheBudget::new(8).with_shared_preparation_bytes(1024 * 1024),
    );
    let (first, first_styles, first_paint) = one_leaf_document(*b"scene-share-0006", "same");
    let (second, second_styles, second_paint) = one_leaf_document(*b"scene-share-0007", "same");
    layout
        .prepare(
            &first.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &first_styles, &first_paint),
        )
        .expect("seed preparation succeeds");

    let poisoned = PreparedParagraph::try_from_data(
        first.snapshot().paragraphs()[0].id,
        0,
        ResolvedDirection::Ltr,
        crate::SceneFeatures::EDITABLE,
        PreparedParagraphData::new(),
    )
    .expect("empty prepared facts are internally valid");
    layout.replace_first_shared_facts_for_test(poisoned.shared_facts());

    let error = layout
        .prepare(
            &second.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &second_styles, &second_paint),
        )
        .expect_err("poisoned shared facts must fail current-projection validation");
    assert_eq!(error.kind(), SceneErrorKind::SourceCoverage);
    assert_eq!(layout.cache_diagnostics().shared_preparation_hits, 1);
}

#[test]
fn fingerprint_collision_never_substitutes_a_nonmatching_key() {
    let calls = Rc::new(Cell::new(0));
    let mut layout = LayoutEngine::new(
        SharedEligibilityAdapter {
            calls: calls.clone(),
            epoch: Rc::new(Cell::new(Some(1))),
        },
        CacheBudget::new(8).with_shared_preparation_bytes(1024 * 1024),
    );
    let (first, first_styles, first_paint) = one_leaf_document(*b"scene-share-0008", "aaaa");
    let (second, second_styles, second_paint) = one_leaf_document(*b"scene-share-0009", "bbbb");
    let (third, third_styles, third_paint) = one_leaf_document(*b"scene-share-0010", "bbbb");
    layout
        .prepare(
            &first.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &first_styles, &first_paint),
        )
        .expect("first key prepares");
    layout.collide_shared_bucket_for_test("aaaa", "bbbb");

    let collision_miss = layout
        .prepare(
            &second.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &second_styles, &second_paint),
        )
        .expect("nonmatching colliding key prepares");
    assert_eq!(collision_miss.work.shared_preparations, 0);
    assert_eq!(calls.get(), 2);

    let exact_hit = layout
        .prepare(
            &third.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &third_styles, &third_paint),
        )
        .expect("exact key after a colliding entry shares");
    assert_eq!(exact_hit.work.shared_preparations, 1);
    assert_eq!(calls.get(), 2);
    let diagnostics = layout.cache_diagnostics();
    assert_eq!(diagnostics.shared_preparation_misses, 2);
    assert_eq!(diagnostics.shared_preparation_hits, 1);
}

#[test]
fn invalid_first_output_releases_untracked_backend_state() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc13", "é");
    let mut layout = LayoutEngine::new(RetainingInvalidAdapter::default(), CacheBudget::new(32));
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    )
    .with_features(crate::SceneFeatures::EDITABLE);
    layout
        .prepare(&document.snapshot(), &request)
        .expect_err("mid-scalar adapter source must be rejected");

    assert_eq!(
        layout.cache_diagnostics().current_entries(),
        0,
        "invalid output must not create geometry residency"
    );
    assert_eq!(
        layout
            .cache_diagnostics()
            .adapter_facts
            .map(|diagnostics| diagnostics.entries),
        Some(0),
        "invalid output must release backend state with no geometry owner"
    );
}

#[test]
fn preparation_trace_distinguishes_reuse_invalidation_and_memory_classes() {
    let (document, styles, paint) = one_leaf_document(*b"scene-trace-0001", "trace");
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
    let request =
        SceneRequest::new(TextConstraint::MaxContent, &styles, &paint).with_preparation_trace();

    let cold = layout
        .prepare(&document.snapshot(), &request)
        .expect("cold trace fixture prepares");
    let cold_trace = cold.trace.expect("trace was requested");
    assert_eq!(cold_trace.work, cold.work);
    assert_eq!(cold_trace.reuse.paragraphs, 1);
    assert_eq!(cold_trace.reuse.cold_paragraphs, 1);
    assert_eq!(cold_trace.reuse.adapter_calls, 1);
    assert_eq!(cold_trace.reuse.exact_geometry_reuses, 0);
    assert_eq!(cold_trace.memory.cache_before.current_entries(), 0);
    assert_eq!(cold_trace.memory.cache_after.current_entries(), 1);
    assert!(cold_trace.memory.cache_after.scene_cache_accounted_bytes > 0);
    assert_eq!(
        cold_trace.memory.scene_output_capacity_bytes, 0,
        "one paragraph uses the direct spine form rather than allocating a tree node"
    );
    assert!(
        cold_trace.memory.scratch_growth_bytes() > 0,
        "cold preparation retains reusable projection capacity"
    );

    let retained = layout
        .prepare(&document.snapshot(), &request)
        .expect("retained trace fixture prepares");
    let retained_trace = retained.trace.expect("trace was requested");
    assert_eq!(retained_trace.reuse.preflight_reuses, 1);
    assert_eq!(retained_trace.reuse.exact_geometry_reuses, 1);
    assert_eq!(retained_trace.reuse.adapter_calls, 0);
    assert_eq!(retained.work.reused_paragraphs, 1);
    assert_eq!(
        retained_trace
            .memory
            .cache_before
            .scene_cache_accounted_bytes,
        retained_trace
            .memory
            .cache_after
            .scene_cache_accounted_bytes
    );

    let centered = ParagraphStyle::DEFAULT.with_alignment(TextAlignment::Center);
    let centered_styles = styles.clone().with_default_paragraph_style(centered);
    let adjusted = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &centered_styles, &paint)
                .with_preparation_trace(),
        )
        .expect("adjustment-only trace fixture prepares");
    let adjusted_trace = adjusted.trace.expect("trace was requested");
    assert_eq!(adjusted_trace.reuse.formation_invalidations, 0);
    assert_eq!(adjusted_trace.reuse.adjustment_invalidations, 1);
    assert_eq!(adjusted_trace.reuse.paint_invalidations, 0);
    let paragraph = document.snapshot().paragraphs()[0].id;
    let adjusted_geometry = layout
        .cached_geometry_for_test(paragraph)
        .expect("adjusted paragraph remains cached");

    let painted_styles =
        StyleMap::new(styles.default_style().clone().with_paint(PaintSlot::new(1)))
            .with_default_paragraph_style(centered);
    let painted_table =
        PaintTable::from_brushes([Brush::Solid(Color::BLACK), Brush::Solid(Color::WHITE)]);
    let painted = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &painted_styles, &painted_table)
                .with_preparation_trace(),
        )
        .expect("paint-only trace fixture prepares");
    let painted_trace = painted.trace.expect("trace was requested");
    assert_eq!(painted_trace.reuse.formation_invalidations, 0);
    assert_eq!(painted_trace.reuse.adjustment_invalidations, 0);
    assert_eq!(painted_trace.reuse.paint_invalidations, 1);
    let painted_geometry = layout
        .cached_geometry_for_test(paragraph)
        .expect("painted paragraph remains cached");
    assert!(
        Arc::ptr_eq(&adjusted_geometry, &painted_geometry),
        "paint-only preparation must share the complete immutable geometry"
    );
    assert_eq!(
        painted.scene.fragment(0).expect("paint fragment").paint(),
        PaintSlot::new(1),
        "the paint topology must still expose the new slot"
    );

    let mut untraced = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(1),
    );
    let ordinary = untraced
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("ordinary preparation remains available");
    assert!(
        ordinary.trace.is_none(),
        "deep diagnostics are opt-in rather than a stable-path tax"
    );
}

#[test]
fn region_request_rejects_an_adapter_that_ignores_exact_slots() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc14", "region");
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
    let flow = RegionFlow::rectangle(Rect::new(40.0, 20.0, 140.0, 100.0)).expect("region is valid");
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let error = layout
        .prepare(&document.snapshot(), &request)
        .expect_err("an adapter must return exact slots and a replayable transcript");

    assert_eq!(error.kind(), SceneErrorKind::Flow);
}

#[test]
fn empty_region_output_rejects_a_cursor_height_that_disagrees_with_geometry() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc15", "");
    let flow = RegionFlow::rectangle(Rect::new(40.0, 20.0, 140.0, 100.0)).expect("region is valid");
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let error = LayoutEngine::new(MismatchedEmptyRegionAdapter, CacheBudget::new(32))
        .prepare(&document.snapshot(), &request)
        .expect_err("flow cursor and empty geometry must consume the same height");

    assert_eq!(error.kind(), SceneErrorKind::Flow);
}

#[test]
fn explicit_direction_rejects_a_backend_with_conflicting_analysis() {
    let (document, styles, paint) = one_leaf_document(*b"scene-test-doc16", "");
    let styles = styles.with_default_paragraph_style(ParagraphStyle::new(BaseDirection::Rtl));
    let flow = RegionFlow::rectangle(Rect::new(40.0, 20.0, 140.0, 100.0)).expect("region is valid");
    let request = SceneRequest::new(
        TextConstraint::Wrap(FiniteWidth::new(100.).expect("test width is valid")),
        &styles,
        &paint,
    )
    .with_region_flow(&flow);
    let error = LayoutEngine::new(MismatchedEmptyRegionAdapter, CacheBudget::new(32))
        .prepare(&document.snapshot(), &request)
        .expect_err("explicit RTL cannot consume backend-reported LTR analysis");

    assert_eq!(error.kind(), SceneErrorKind::Preparation);
    assert_eq!(
        error.preparation(),
        Some(PreparationErrorKind::InvalidOutput)
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
    )
    .with_features(crate::SceneFeatures::EDITABLE);
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
    assert_eq!(error.source(), Some(0..1));
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
        first_scene.work.line_reshapes, 2,
        "adapter line-reshape work must survive scene reporting"
    );
    assert_eq!(
        first_scene.work.line_font_resolution.records, 3,
        "line-final retained-font resolution must survive scene reporting"
    );
    assert_eq!(
        first_scene.work.line_shape.records, 5,
        "line-final shaped glyph work must survive scene reporting"
    );
    assert_eq!(first_scene.work.line_candidates, 6);
    assert_eq!(first_scene.work.rejected_line_candidates, 1);
    assert_eq!(first_scene.work.accepted_line_candidates(), 5);
    assert_eq!(first_scene.work.line_checkpoint_restores, 2);
    let second_request =
        SceneRequest::new(TextConstraint::Wrap(width), &second_styles, &second_paint);
    let second_scene = layout
        .prepare(&second.snapshot(), &second_request)
        .expect("second scene must prepare");
    assert_ne!(
        first_scene.scene.fragment(0).expect("fragment").id(),
        second_scene.scene.fragment(0).expect("fragment").id(),
        "document identity must participate in retained fragment identity"
    );
    assert_eq!(
        first_scene
            .scene
            .fragment(0)
            .expect("fragment")
            .paint_clip(),
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
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(crate::SceneFeatures::EDITABLE);
    let error = layout
        .prepare(&first.snapshot(), &request)
        .expect_err("a foreign paragraph style must not be silently ignored");
    assert_eq!(error.kind(), SceneErrorKind::InvalidStyle);
}

#[test]
fn feature_override_from_another_document_is_rejected_on_an_exact_scene_hit() {
    let (first, styles, paint) = one_leaf_document(*b"scene-feat-doc01", "a");
    let (second, _, _) = one_leaf_document(*b"scene-feat-doc02", "b");
    let foreign = second.snapshot().paragraphs()[0].id;
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
    let valid = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint);
    layout
        .prepare(&first.snapshot(), &valid)
        .expect("the valid scene must establish an exact reusable root");

    let invalid = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_feature_policy(
            crate::SceneFeaturePolicy::default()
                .with_paragraph(foreign, crate::SceneFeatures::DISPLAY),
        );
    let error = layout
        .prepare(&first.snapshot(), &invalid)
        .expect_err("warm reuse must not bypass feature-policy validation");
    assert_eq!(error.kind(), SceneErrorKind::InvalidFeatures);
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
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(crate::SceneFeatures::EDITABLE);
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
        .scene
        .hit_test(Point::new(0.1, 1.0))
        .expect("left side must hit");
    let right = committed
        .scene
        .hit_test(Point::new(9.9, 1.0))
        .expect("right side must hit");
    let selection = committed
        .scene
        .editing()
        .expect("fixture retains editable scene data")
        .between(&left.position, &right.position, TextSelectionMode::Logical)
        .expect("fixture source must select");
    let selections = committed
        .scene
        .selection_set([selection])
        .expect("fixture selection must validate");
    let mut session = committed
        .scene
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
    let sources: Vec<_> = transient
        .scene
        .fragment(0)
        .expect("generated fragment exists")
        .sources()
        .expect("editable projection retains source provenance")
        .collect();
    let [ProjectedTextSource::Composition(range)] = sources.as_slice() else {
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
fn position_index_maps_authored_boundaries_through_whitespace_collapse() {
    let (document, mut styles, paint) = one_leaf_document(*b"collapse-pos-001", "a   b");
    let snapshot = document.snapshot();
    let paragraph = snapshot.paragraphs()[0].id;
    let text = snapshot.paragraphs()[0].leaves[0].id;
    styles.set_paragraph_style(
        paragraph,
        ParagraphStyle::DEFAULT.with_whitespace_collapse(WhitespaceCollapse::Collapse),
    );
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(crate::SceneFeatures::EDITABLE);
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(1),
    );
    let output = layout
        .prepare(&snapshot, &request)
        .expect("collapsed fixture must prepare");
    let editing = output.scene.editing().expect("fixture retains navigation");
    let start = editing
        .position_at(text, 0)
        .expect("authored start maps to the projected start");
    let end = editing
        .position_at(text, 5)
        .expect("authored end maps to the projected end");
    assert_eq!(start.byte(), 0);
    assert_eq!(start.affinity(), TextAffinity::Downstream);
    assert_eq!(end.byte(), 5);
    assert_eq!(end.affinity(), TextAffinity::Upstream);
    assert!(
        editing.position_at(text, 2).is_none(),
        "the adapter did not represent an interior collapsed-whitespace caret"
    );
    assert!(
        editing.caret(&end).is_some(),
        "caret lookup uses the same reverse source index"
    );
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
    )
    .with_features(crate::SceneFeatures::DISPLAY.with_sources());
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
    let mut fragments = output.scene.fragments();
    let first = fragments.next().expect("first split fragment");
    let second = fragments.next().expect("second split fragment");
    assert!(fragments.next().is_none());

    assert_eq!(
        first.glyphs().first().expect("first glyph").id(),
        second.glyphs().first().expect("second glyph").id()
    );
    assert_eq!(
        first.glyphs().first().expect("first glyph").instance_id(),
        second.glyphs().first().expect("second glyph").instance_id(),
        "partial-paint observations must retain one shaped-glyph identity"
    );
    assert_eq!(
        first.glyphs().first().expect("first glyph").position(),
        second.glyphs().first().expect("second glyph").position(),
        "paint splitting must not duplicate shaping or move the glyph"
    );
    assert_eq!(first.paint(), PaintSlot::new(0));
    assert_eq!(second.paint(), PaintSlot::new(1));
    assert_eq!(
        first
            .source()
            .expect("fragment retains sources")
            .expect("first source")
            .text(),
        first_text
    );
    assert_eq!(
        second
            .source()
            .expect("fragment retains sources")
            .expect("second source")
            .text(),
        second_text
    );
    assert!(first.synthesis().skew_transform().is_some());
    let origin = first.glyphs().first().expect("first glyph").position();
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
        output.scene.line(0).expect("line").fragment_range(),
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
    assert_eq!(shaping_styles, [first, second]);
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
    assert_eq!(spacious_scene.work.shape.paragraphs, 0);
    assert_eq!(spacious_scene.work.flow.paragraphs, 1);
    assert_eq!(compact_scene.scene.line(0).expect("line").bounds().y0, 10.0);
    assert_eq!(
        spacious_scene.scene.line(0).expect("line").bounds().y0,
        20.0
    );
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
    )
    .with_features(crate::SceneFeatures::EDITABLE);
    let committed = layout
        .prepare(&snapshot, &request)
        .expect("committed scene must prepare");
    let left = committed
        .scene
        .hit_test(Point::new(0.0, 1.0))
        .expect("left cluster side must hit");
    let right = committed
        .scene
        .hit_test(Point::new(9.9, 1.0))
        .expect("right cluster side must hit");
    let selection = committed
        .scene
        .editing()
        .expect("fixture retains editable scene data")
        .between(&left.position, &right.position, TextSelectionMode::Logical)
        .expect("whole-leaf selection must form");
    let selections = committed
        .scene
        .selection_set([selection])
        .expect("selection set must validate");
    let start = committed
        .scene
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
    assert_eq!(first.work.shape.paragraphs, 1);
    assert_eq!(first.scene.epoch(), first_epoch);
    assert!(first.scene.fragments().iter().all(|fragment| {
        fragment
            .sources()
            .expect("editable projection retains source provenance")
            .all(|source| {
                matches!(source, ProjectedTextSource::Composition(range)
                        if range.id() == session.id() && range.epoch() == first_epoch)
            })
    }));
    assert!(
        !first
            .scene
            .composition_selection_geometry(&session)
            .expect("preedit selection geometry must resolve")
            .is_empty()
    );
    let marked_geometry = first
        .scene
        .composition_geometry(&session)
        .expect("complete marked-text geometry must resolve");
    assert!(
        !marked_geometry.is_empty(),
        "the complete generated preedit must expose renderer-neutral geometry"
    );
    assert!(
        marked_geometry.iter().all(|rect| rect.bounds.width() > 0.0),
        "combining preedit geometry must be cluster based rather than ink based"
    );

    let repeated = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("same epoch must reuse transient work");
    assert_eq!(repeated.work.shape.paragraphs, 0);
    assert_eq!(repeated.work.reused_paragraphs, 1);
    assert!(
        Arc::ptr_eq(&first.scene.core, &repeated.scene.core),
        "an exact composition epoch must return the published scene core"
    );
    assert!(
        repeated
            .scene
            .line(0)
            .expect("the repeated projection has a line")
            .sources()
            .is_ok(),
        "a rebound view must retain projected source access"
    );

    let selection_epoch = session
        .update(
            first_epoch,
            CompositionUpdate::new("a\u{301}").with_selection(3..3),
        )
        .expect("selection-only preedit change must advance the epoch");
    let selection_only = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("selection-only epoch must rebind retained geometry");
    assert_eq!(selection_only.work.shape.paragraphs, 0);
    assert_eq!(selection_only.work.geometry.paragraphs, 0);
    assert_eq!(selection_only.work.reused_paragraphs, 1);
    assert!(selection_only.scene.fragments().iter().all(|fragment| {
        fragment
            .sources()
            .expect("editable projection retains source provenance")
            .all(|source| {
                matches!(source, ProjectedTextSource::Composition(range)
                        if range.epoch() == selection_epoch)
            })
    }));
    assert!(
        selection_only
            .scene
            .composition_selection_geometry(&session)
            .expect("rebound selected range must resolve")
            .is_empty()
    );
    assert!(
        selection_only
            .scene
            .line(0)
            .expect("the rebound projection has a line")
            .sources()
            .is_ok(),
        "a rebound view owns its source observation"
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
    assert_eq!(second.work.shape.paragraphs, 1);
    assert_eq!(snapshot.text(left.position.text()), Some("office"));
    let surface = EditableSurface::new(
        &snapshot,
        [EditableSurfaceElement::text(left.position.text())],
    )
    .expect("focused surface must flatten the selected semantic leaf");
    let host = surface
        .bind_composition(&second.scene, &session)
        .expect("host queries must bind to the exact composition epoch");
    assert_eq!(
        surface
            .bind_composition(&first.scene, &session)
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
    assert_eq!(cancelled.work.shape.paragraphs, 0);
    assert_eq!(cancelled.work.geometry.paragraphs, 0);
    assert_eq!(cancelled.work.reused_paragraphs, 1);

    let stale_session = session.clone();
    let replacement = session
        .commit(&mut document, "مرحبا")
        .expect("commit must publish one replacement");
    assert_eq!(replacement.publication().changes().paragraphs().len(), 1);
    assert_eq!(snapshot.text(left.position.text()), Some("office"));
    assert_eq!(
        replacement
            .publication()
            .snapshot()
            .text(left.position.text()),
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
fn unrelated_equal_styles_use_checked_value_preflight() {
    let (document, styles, paint) = one_leaf_document(*b"scene-preflight1", "provenance");
    let equal_styles = StyleMap::new(styles.default_style().clone());
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(8),
    );
    layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("initial preparation succeeds");

    let fallback = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &equal_styles, &paint)
                .with_preparation_trace(),
        )
        .expect("equal unrelated style state remains reusable");
    let fallback_reuse = fallback.trace.expect("trace was requested").reuse;
    assert_eq!(fallback_reuse.preflight_reuses, 1);
    assert_eq!(fallback_reuse.exact_geometry_reuses, 1);
    assert_eq!(fallback_reuse.adapter_calls, 0);

    let retained = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &equal_styles, &paint)
                .with_preparation_trace(),
        )
        .expect("equal value preflight remains reusable");
    assert_eq!(
        retained
            .trace
            .expect("trace was requested")
            .reuse
            .preflight_reuses,
        1
    );
}

#[test]
fn localized_publication_shares_scene_segments_and_binds_revisions_lazily() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-local-path"));
    let mut edit = document.edit();
    let mut texts = Vec::new();
    for value in ["before", "target", "after"] {
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph appends");
        texts.push(
            edit.append_text(paragraph, InlineRole::TEXT, value)
                .expect("fixture text appends"),
        );
    }
    let first_publication = edit.commit().expect("fixture publishes");
    let first_snapshot = first_publication.snapshot().clone();
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 10.0).expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let dark = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let light = PaintTable::from_brushes([Brush::Solid(Color::WHITE)]);
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(16),
    );
    let first = layout
        .prepare(
            &first_snapshot,
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &dark)
                .with_features(crate::SceneFeatures::DISPLAY.with_sources()),
        )
        .expect("initial scene prepares");
    let first_target_glyph = first
        .scene
        .fragments()
        .find(|fragment| {
            fragment
                .sources()
                .expect("fixture retains source provenance")
                .any(|source| source.text() == texts[1])
        })
        .and_then(|fragment| fragment.glyphs().next())
        .expect("old target glyph exists")
        .instance_id();
    let first_after_glyph = first
        .scene
        .fragments()
        .find(|fragment| {
            fragment
                .sources()
                .expect("fixture retains source provenance")
                .any(|source| source.text() == texts[2])
        })
        .and_then(|fragment| fragment.glyphs().next())
        .expect("old trailing glyph exists")
        .instance_id();
    let repainted = layout
        .prepare(
            &first_snapshot,
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &light)
                .with_features(crate::SceneFeatures::DISPLAY.with_sources()),
        )
        .expect("paint-only scene prepares");
    assert!(
        Arc::ptr_eq(&first.scene.core, &repainted.scene.core),
        "paint values must rebind one persistent geometry root"
    );

    let mut edit = document.edit();
    edit.replace_text(texts[1], "changed")
        .expect("middle paragraph edits");
    let second_publication = edit.commit().expect("localized edit publishes");
    let second = layout
        .prepare(
            second_publication.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &dark)
                .with_features(crate::SceneFeatures::DISPLAY.with_sources())
                .with_preparation_trace(),
        )
        .expect("localized scene prepares");
    assert_eq!(second.work.shape.paragraphs, 1);
    assert_eq!(second.work.reused_paragraphs, 2);
    assert_eq!(second.work.paint.paragraphs, 1);
    let localized_reuse = second.trace.expect("trace was requested").reuse;
    assert_eq!(localized_reuse.adapter_calls, 1);
    assert_eq!(localized_reuse.preflight_reuses, 2);
    assert_eq!(localized_reuse.exact_geometry_reuses, 2);
    for index in [0, 2] {
        assert!(
            Arc::ptr_eq(
                first
                    .scene
                    .core
                    .spine
                    .segment(index)
                    .expect("old sibling exists"),
                second
                    .scene
                    .core
                    .spine
                    .segment(index)
                    .expect("new sibling exists"),
            ),
            "localized publication must share unchanged paragraph segments"
        );
    }
    assert!(
        !Arc::ptr_eq(
            first
                .scene
                .core
                .spine
                .segment(1)
                .expect("old target exists"),
            second
                .scene
                .core
                .spine
                .segment(1)
                .expect("new target exists"),
        ),
        "localized publication must replace the changed paragraph segment"
    );
    let second_target_glyph = second
        .scene
        .fragments()
        .find(|fragment| {
            fragment
                .sources()
                .expect("fixture retains source provenance")
                .any(|source| source.text() == texts[1])
        })
        .and_then(|fragment| fragment.glyphs().next())
        .expect("new target glyph exists")
        .instance_id();
    let second_after_glyph = second
        .scene
        .fragments()
        .find(|fragment| {
            fragment
                .sources()
                .expect("fixture retains source provenance")
                .any(|source| source.text() == texts[2])
        })
        .and_then(|fragment| fragment.glyphs().next())
        .expect("new trailing glyph exists")
        .instance_id();
    assert_ne!(
        first_target_glyph, second_target_glyph,
        "replaced paragraph geometry must receive distinct glyph identities"
    );
    assert_eq!(
        first_after_glyph, second_after_glyph,
        "shared paragraph geometry must retain glyph identities despite changed prefixes"
    );
    assert!(
        first.scene.fragments().all(|fragment| {
            fragment
                .sources()
                .expect("fixture retains source provenance")
                .all(|source| source.revision() == first_snapshot.revision())
        }),
        "caller-retained old geometry must mint only its original revision"
    );
    assert!(
        second.scene.fragments().all(|fragment| {
            fragment
                .sources()
                .expect("fixture retains source provenance")
                .all(|source| source.revision() == second_publication.snapshot().revision())
        }),
        "new geometry must mint only the new revision"
    );
}

#[test]
fn no_op_publication_reuses_the_exact_scene_core_at_the_new_revision() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-noop-revis"));
    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("paragraph appends");
    edit.append_text(paragraph, InlineRole::TEXT, "stable")
        .expect("text appends");
    edit.commit().expect("fixture publishes");

    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 10.0).expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(4),
    );
    let first = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("initial scene prepares");

    let publication = document
        .edit()
        .commit()
        .expect("an empty transaction still publishes its revision");
    let second = layout
        .prepare(
            publication.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
                .with_preparation_trace(),
        )
        .expect("the new revision reuses its unchanged paragraph root");

    assert_ne!(first.scene.revision(), second.scene.revision());
    assert!(
        Arc::ptr_eq(&first.scene.core, &second.scene.core),
        "a metadata-only revision must retain the exact scene core"
    );
    assert_eq!(second.work.reused_paragraphs, 1);
    assert_eq!(second.work.analysis.paragraphs, 0);
    assert_eq!(second.work.shape.paragraphs, 0);
    assert_eq!(second.work.geometry.paragraphs, 0);
    assert_eq!(second.work.paint.paragraphs, 0);
    let reuse = second.trace.expect("trace was requested").reuse;
    assert_eq!(reuse.preflight_reuses, 1);
    assert_eq!(reuse.exact_geometry_reuses, 1);
    assert_eq!(reuse.adapter_calls, 0);
}

#[test]
fn localized_style_branch_prepares_only_its_paragraph() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-style-path"));
    let mut edit = document.edit();
    let mut target = None;
    for index in 0..64 {
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph appends");
        let text = edit
            .append_text(paragraph, InlineRole::TEXT, "stable")
            .expect("fixture text appends");
        if index == 31 {
            target = Some(text);
        }
    }
    let publication = edit.commit().expect("fixture publishes");
    let base = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 10.0).expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(base.clone());
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(128),
    );
    let first = layout
        .prepare(
            publication.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("initial scene prepares");

    let mut changed_styles = styles.clone();
    changed_styles.set(
        target.expect("target exists"),
        ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Test"), 12.0).expect("changed style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        ),
    );
    let changed = layout
        .prepare(
            publication.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &changed_styles, &paint)
                .with_preparation_trace(),
        )
        .expect("localized style prepares");

    assert_eq!(changed.work.shape.paragraphs, 1);
    assert_eq!(changed.work.paint.paragraphs, 1);
    assert_eq!(changed.work.reused_paragraphs, 63);
    for index in [0, 63] {
        assert!(Arc::ptr_eq(
            first
                .scene
                .core
                .spine
                .segment(index)
                .expect("old sibling exists"),
            changed
                .scene
                .core
                .spine
                .segment(index)
                .expect("new sibling exists")
        ));
    }
}

#[test]
fn appended_paragraph_extends_the_persistent_scene_path() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-appendpath"));
    let mut edit = document.edit();
    for _ in 0..64 {
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph appends");
        edit.append_text(paragraph, InlineRole::TEXT, "stable")
            .expect("fixture text appends");
    }
    let first_publication = edit.commit().expect("fixture publishes");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 10.0).expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(128),
    );
    let first = layout
        .prepare(
            first_publication.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("initial scene prepares");

    let mut edit = document.edit();
    let paragraph = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("appended paragraph is valid");
    edit.append_text(paragraph, InlineRole::TEXT, "appended")
        .expect("appended text is valid");
    let publication = edit.commit().expect("append publishes");
    let appended = layout
        .prepare(
            publication.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
                .with_preparation_trace(),
        )
        .expect("appended scene prepares");

    assert_eq!(appended.work.shape.paragraphs, 1);
    assert_eq!(appended.work.paint.paragraphs, 1);
    assert_eq!(appended.work.reused_paragraphs, 64);
    assert_eq!(appended.scene.core.spine.paragraph_count(), 65);
    assert!(Arc::ptr_eq(
        first
            .scene
            .core
            .spine
            .segment(31)
            .expect("old prefix exists"),
        appended
            .scene
            .core
            .spine
            .segment(31)
            .expect("old prefix remains")
    ));
    assert!(
        appended
            .trace
            .expect("trace exists")
            .memory
            .scene_output_capacity_bytes
            < first.scene.core.spine.accounted_node_bytes(),
        "append must publish a logarithmic spine path rather than rebuild the root"
    );
}

#[test]
fn composition_replaces_only_its_persistent_paragraph_path() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-comp-spine"));
    let mut edit = document.edit();
    let mut texts = Vec::new();
    for value in ["before", "target", "after"] {
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph appends");
        texts.push(
            edit.append_text(paragraph, InlineRole::TEXT, value)
                .expect("fixture text appends"),
        );
    }
    edit.commit().expect("fixture publishes");
    let snapshot = document.snapshot();
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 10.0).expect("fixture style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(crate::SceneFeatures::EDITABLE)
        .with_preparation_trace();
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(16),
    );
    let committed = layout
        .prepare(&snapshot, &request)
        .expect("committed fixture prepares");
    let target = committed
        .scene
        .position_at(texts[1], 0)
        .expect("target start is represented");
    let selection = committed
        .scene
        .collapsed_selection(&target)
        .expect("target caret validates");
    let selections = committed
        .scene
        .selection_set([selection])
        .expect("target selection validates");
    let mut session = committed
        .scene
        .begin_composition(&selections, CompositionId::from_bytes(*b"scene-comp-path1"))
        .expect("composition starts")
        .into_session();
    session
        .update(
            session.epoch(),
            CompositionUpdate::new("generated").with_selection(9..9),
        )
        .expect("first epoch updates");

    let first = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("first projected scene prepares");
    assert_eq!(first.work.reused_paragraphs, 2);
    assert!(
        first
            .trace
            .expect("trace was requested")
            .memory
            .scene_output_capacity_bytes
            > 0,
        "the first composition publication must account for its unshared spine path"
    );
    for index in [0, 2] {
        assert!(
            Arc::ptr_eq(
                committed
                    .scene
                    .core
                    .spine
                    .segment(index)
                    .expect("committed sibling exists"),
                first
                    .scene
                    .core
                    .spine
                    .segment(index)
                    .expect("projected sibling exists"),
            ),
            "composition must share every unchanged committed sibling"
        );
    }
    assert!(
        !Arc::ptr_eq(
            committed
                .scene
                .core
                .spine
                .segment(1)
                .expect("committed target exists"),
            first
                .scene
                .core
                .spine
                .segment(1)
                .expect("projected target exists"),
        ),
        "composition must replace its target segment"
    );

    let repeated = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("same epoch reuses its root");
    assert!(
        Arc::ptr_eq(&first.scene.core, &repeated.scene.core),
        "an exact composition request must reuse its root"
    );
    assert_eq!(
        repeated
            .trace
            .expect("trace was requested")
            .memory
            .scene_output_capacity_bytes,
        0,
        "an exact composition publication retains no new spine nodes"
    );

    let first_epoch = session.epoch();
    session
        .update(
            first_epoch,
            CompositionUpdate::new("generated").with_selection(0..0),
        )
        .expect("selection-only epoch updates");
    let second = layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("next projected epoch prepares");
    for index in [0, 2] {
        assert!(
            Arc::ptr_eq(
                first
                    .scene
                    .core
                    .spine
                    .segment(index)
                    .expect("first sibling exists"),
                second
                    .scene
                    .core
                    .spine
                    .segment(index)
                    .expect("second sibling exists"),
            ),
            "a new epoch must continue sharing unchanged siblings"
        );
    }
    assert!(
        first.scene.fragments().any(|fragment| {
            fragment
                .sources()
                .expect("projection retains source provenance")
                .any(|source| {
                    matches!(source, ProjectedTextSource::Composition(range)
                        if range.epoch() == first_epoch)
                })
        }),
        "the caller-retained old scene must keep its original generated epoch"
    );
    assert!(
        second.scene.fragments().any(|fragment| {
            fragment
                .sources()
                .expect("projection retains source provenance")
                .any(|source| {
                    matches!(source, ProjectedTextSource::Composition(range)
                        if range.epoch() == session.epoch())
                })
        }),
        "the new scene must bind generated provenance to the new epoch"
    );
}

#[test]
fn composition_residency_never_evicts_committed_geometry() {
    let (document, styles, paint) = one_leaf_document(*b"comp-budget-doc1", "committed");
    let snapshot = document.snapshot();
    let text = snapshot.paragraphs()[0].leaves[0].id;
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(crate::SceneFeatures::EDITABLE)
        .with_preparation_trace();
    let adapter = || EchoAdapter {
        split_utf8: false,
        split_paint: false,
        mismatched_paint: false,
        glyphless: false,
        interior_cursor: false,
    };
    let mut layout = LayoutEngine::new(adapter(), CacheBudget::new(1));
    let committed = layout
        .prepare(&snapshot, &request)
        .expect("committed scene prepares");
    let position = committed
        .scene
        .position_at(text, 0)
        .expect("composition target is represented");
    let selection = committed
        .scene
        .collapsed_selection(&position)
        .expect("composition selection validates");
    let selections = committed
        .scene
        .selection_set([selection])
        .expect("composition selection set validates");
    let mut session = committed
        .scene
        .begin_composition(&selections, CompositionId::from_bytes(*b"comp-budget-0001"))
        .expect("composition begins")
        .into_session();
    session
        .update(session.epoch(), CompositionUpdate::new("generated"))
        .expect("composition text updates");

    layout
        .prepare_composition(&snapshot, &request, &session)
        .expect("composition scene prepares");
    let retained = layout.cache_diagnostics();
    assert_eq!(retained.budget, 1);
    assert_eq!(retained.composition_budget, 1);
    assert_eq!(retained.committed_entries, 1);
    assert_eq!(retained.composition_entries, 1);
    assert_eq!(
        retained.current_entries(),
        2,
        "independent lanes may each retain their configured limit"
    );

    let cancelled = layout
        .prepare(&snapshot, &request)
        .expect("cancelling composition reuses committed geometry");
    assert!(
        Arc::ptr_eq(&committed.scene.core, &cancelled.scene.core),
        "composition residency must not evict the exact committed root"
    );
    assert_eq!(cancelled.work.shape.paragraphs, 0);
    assert_eq!(cancelled.work.geometry.paragraphs, 0);

    let mut unretained =
        LayoutEngine::new(adapter(), CacheBudget::new(1).with_composition_entries(0));
    let committed = unretained
        .prepare(&snapshot, &request)
        .expect("zero-composition-budget fixture prepares");
    let position = committed
        .scene
        .position_at(text, 0)
        .expect("composition target is represented");
    let selection = committed
        .scene
        .collapsed_selection(&position)
        .expect("composition selection validates");
    let selections = committed
        .scene
        .selection_set([selection])
        .expect("composition selection set validates");
    let mut session = committed
        .scene
        .begin_composition(&selections, CompositionId::from_bytes(*b"comp-budget-0002"))
        .expect("composition begins")
        .into_session();
    session
        .update(session.epoch(), CompositionUpdate::new("generated"))
        .expect("composition text updates");
    let first = unretained
        .prepare_composition(&snapshot, &request, &session)
        .expect("zero-budget composition still materializes");
    assert!(!first.scene.fragments().is_empty());
    let diagnostics = unretained.cache_diagnostics();
    assert_eq!(diagnostics.committed_entries, 1);
    assert_eq!(diagnostics.composition_entries, 0);
    assert_eq!(diagnostics.composition_budget, 0);
    let repeated = unretained
        .prepare_composition(&snapshot, &request, &session)
        .expect("unretained composition may prepare again");
    assert_eq!(
        repeated.work.shape.paragraphs, 1,
        "a zero transient budget trades residency for observable re-formation"
    );
    let cancelled = unretained
        .prepare(&snapshot, &request)
        .expect("zero transient retention preserves the committed root");
    assert!(
        Arc::ptr_eq(&committed.scene.core, &cancelled.scene.core),
        "evicting transient geometry must not invalidate committed publication"
    );
}

#[test]
fn exact_scene_hits_refresh_root_recency_lazily() {
    let calls = Rc::new(Cell::new(0));
    let mut layout = LayoutEngine::new(
        SharedEligibilityAdapter {
            calls: calls.clone(),
            epoch: Rc::new(Cell::new(None)),
        },
        CacheBudget::new(2),
    );
    let (first, first_styles, first_paint) = one_leaf_document(*b"scene-lru-doc-01", "first");
    let (second, second_styles, second_paint) = one_leaf_document(*b"scene-lru-doc-02", "second");
    let (third, third_styles, third_paint) = one_leaf_document(*b"scene-lru-doc-03", "third");
    let first_request = SceneRequest::new(TextConstraint::MaxContent, &first_styles, &first_paint);
    let second_request =
        SceneRequest::new(TextConstraint::MaxContent, &second_styles, &second_paint);
    let third_request = SceneRequest::new(TextConstraint::MaxContent, &third_styles, &third_paint);

    let first_output = layout
        .prepare(&first.snapshot(), &first_request)
        .expect("first document prepares");
    layout
        .prepare(&second.snapshot(), &second_request)
        .expect("second document prepares");
    let exact = layout
        .prepare(&first.snapshot(), &first_request)
        .expect("first exact root is reused");
    assert!(Arc::ptr_eq(&first_output.scene.core, &exact.scene.core));
    assert_eq!(calls.get(), 2);

    layout
        .prepare(&third.snapshot(), &third_request)
        .expect("third document creates cache pressure");
    assert_eq!(layout.cache_diagnostics().evictions, 1);
    let retained = layout
        .prepare(&first.snapshot(), &first_request)
        .expect("recently reused first root survives pressure");
    assert!(
        Arc::ptr_eq(&first_output.scene.core, &retained.scene.core),
        "root-level recency must be folded into stale paragraph entries during eviction"
    );
    assert_eq!(calls.get(), 3);

    layout
        .prepare(&second.snapshot(), &second_request)
        .expect("the genuinely oldest document prepares again");
    assert_eq!(
        calls.get(),
        4,
        "the untouched second root, rather than the exact-hit first root, must be evicted"
    );
}

#[test]
fn composition_root_recency_protects_only_segments_it_names() {
    let calls = Rc::new(Cell::new(0));
    let mut layout = LayoutEngine::new(
        SharedEligibilityAdapter {
            calls: calls.clone(),
            epoch: Rc::new(Cell::new(None)),
        },
        CacheBudget::new(3),
    );
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-lru-comp01"));
    let mut edit = document.edit();
    let mut texts = Vec::new();
    for text in ["target", "sibling"] {
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("test paragraph appends");
        texts.push(
            edit.append_text(paragraph, InlineRole::TEXT, text)
                .expect("test text appends"),
        );
    }
    edit.commit().expect("test document commits");
    let snapshot = document.snapshot();
    let styles = StyleMap::new(ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 16.).expect("test style is valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    ));
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(crate::SceneFeatures::EDITABLE);
    let committed = layout
        .prepare(&snapshot, &request)
        .expect("committed document prepares");
    let caret = committed
        .scene
        .position_at(texts[0], 0)
        .expect("target start is represented");
    let selection = committed
        .scene
        .collapsed_selection(&caret)
        .expect("composition selection validates");
    let selections = committed
        .scene
        .selection_set([selection])
        .expect("composition selection set validates");
    let mut composition = committed
        .scene
        .begin_composition(&selections, CompositionId::from_bytes(*b"scene-lru-comp02"))
        .expect("composition begins")
        .into_session();
    composition
        .update(composition.epoch(), CompositionUpdate::new("generated"))
        .expect("composition updates");
    let projected = layout
        .prepare_composition(&snapshot, &request, &composition)
        .expect("composition scene prepares");
    let repeated = layout
        .prepare_composition(&snapshot, &request, &composition)
        .expect("composition root is reused");
    assert!(Arc::ptr_eq(&projected.scene.core, &repeated.scene.core));

    let (second, second_styles, second_paint) =
        one_leaf_document(*b"scene-lru-comp03", "pressure one");
    let (third, third_styles, third_paint) =
        one_leaf_document(*b"scene-lru-comp04", "pressure two");
    layout
        .prepare(
            &second.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &second_styles, &second_paint),
        )
        .expect("first pressure document prepares");
    layout
        .prepare(
            &third.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &third_styles, &third_paint),
        )
        .expect("second pressure document prepares");

    let retained = layout
        .prepare_composition(&snapshot, &request, &composition)
        .expect("composition survives eviction of its superseded committed target");
    assert!(
        Arc::ptr_eq(&projected.scene.core, &retained.scene.core),
        "composition root must retain its transient target and committed sibling"
    );
    assert_eq!(
        calls.get(),
        5,
        "the exact composition reuse must not return to the adapter"
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

#[test]
fn display_scene_excludes_interaction_and_reports_requested_resident_capabilities() {
    let (document, styles, paint) = one_leaf_document(*b"scene-features01", "display only");
    let paragraph = document.snapshot().paragraphs()[0].id;
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(8),
    );
    let output = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint),
        )
        .expect("display-only scene must prepare");
    let scene = output.scene;
    assert_eq!(scene.line_count(), 1);
    assert_eq!(scene.fragment_count(), 1);

    for error in [
        scene
            .line(0)
            .expect("display scene has a line")
            .sources()
            .expect_err("display excludes sources"),
        scene
            .interaction()
            .expect_err("display excludes hit testing"),
        scene.selection().expect_err("display excludes selection"),
        scene.editing().expect_err("display excludes editing"),
    ] {
        assert_eq!(error.paragraph(), Some(paragraph));
        assert_eq!(error.requested(), crate::SceneFeatures::DISPLAY);
        assert_eq!(error.resident(), crate::SceneFeatures::DISPLAY);
        assert!(
            !crate::SceneFeatures::DISPLAY.contains(error.required()),
            "the diagnostic must name a genuinely absent closure"
        );
    }

    let geometry = layout
        .cached_geometry_for_test(paragraph)
        .expect("display geometry remains resident");
    assert!(geometry.hit_geometry.is_empty());
    assert_eq!(geometry.movement_count(), 0);
    assert!(geometry.source_map.is_none());
    assert!(geometry.semantics.is_empty());

    let accessible = layout
        .prepare(
            &document.snapshot(),
            &SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
                .with_features(crate::SceneFeatures::ACCESSIBLE),
        )
        .expect("accessibility-only scene must prepare");
    assert!(
        accessible.scene.semantics().is_ok(),
        "the accessible profile must expose semantics"
    );
    assert!(
        accessible.scene.interaction().is_err(),
        "semantic bounds must not imply retained hit testing"
    );
    let paragraph = accessible
        .scene
        .paragraph_residencies()
        .next()
        .expect("fixture has one paragraph");
    assert!(paragraph.bytes.sources > 0);
    assert!(paragraph.bytes.semantics > 0);
    assert_eq!(paragraph.bytes.hit_testing, 0);
    let geometry = layout
        .cached_geometry_for_test(paragraph.paragraph)
        .expect("accessible geometry remains resident");
    assert!(
        geometry.hit_geometry.is_empty(),
        "transient semantic-bound construction must not retain hit clusters"
    );
}

#[test]
fn source_observation_is_bound_to_each_view() {
    let (first_document, first_styles, first_paint) =
        one_leaf_document(*b"source-facade-01", "first");
    let (second_document, second_styles, second_paint) =
        one_leaf_document(*b"source-facade-02", "second");
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
    let first_request = SceneRequest::new(TextConstraint::MaxContent, &first_styles, &first_paint)
        .with_features(crate::SceneFeatures::DISPLAY.with_sources());
    let first = layout
        .prepare(&first_document.snapshot(), &first_request)
        .expect("source-capable scene must prepare");
    let second_request =
        SceneRequest::new(TextConstraint::MaxContent, &second_styles, &second_paint);
    let second = layout
        .prepare(&second_document.snapshot(), &second_request)
        .expect("display-only scene must prepare");
    let first_line = first.scene.line(0).expect("the first scene has a line");
    assert!(
        first_line.sources().is_ok(),
        "a source-capable view exposes its own provenance"
    );
    let second_line = second.scene.line(0).expect("the second scene has a line");
    let error = second_line
        .sources()
        .expect_err("a display-only view rejects source observation");
    assert_eq!(
        error.paragraph(),
        Some(second_document.snapshot().paragraphs()[0].id)
    );

    let display_fragment = second
        .scene
        .fragment(0)
        .expect("the second scene has a fragment");
    assert!(
        display_fragment.sources().is_err(),
        "a display-only fragment rejects source observation"
    );
    let display_glyph = display_fragment
        .glyphs()
        .next()
        .expect("the second scene has a glyph");
    assert!(
        display_glyph.sources().is_err(),
        "a display-only glyph rejects source observation"
    );
}

#[test]
fn sparse_editable_override_does_not_promote_a_display_sibling() {
    let mut document = Document::new(DocumentId::from_bytes(*b"scene-features02"));
    let mut edit = document.edit();
    let display = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("display paragraph must append");
    let display_text = edit
        .append_text(display, InlineRole::TEXT, "display")
        .expect("display text must append");
    let editor = edit
        .append_paragraph(ParagraphRole::BODY)
        .expect("editor paragraph must append");
    let editor_text = edit
        .append_text(editor, InlineRole::TEXT, "editor")
        .expect("editor text must append");
    edit.commit().expect("fixture must publish");
    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Test"), 16.).expect("test style must be valid"),
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let policy = crate::SceneFeaturePolicy::uniform(crate::SceneFeatures::DISPLAY)
        .with_paragraph(editor, crate::SceneFeatures::EDITABLE);
    let request =
        SceneRequest::new(TextConstraint::MaxContent, &styles, &paint).with_feature_policy(policy);
    let mut layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(8),
    );
    let snapshot = document.snapshot();
    let output = layout
        .prepare(&snapshot, &request)
        .expect("mixed-capability scene must prepare");
    let scene = output.scene;

    let editing = scene
        .editing()
        .expect("scene editing must expose the sparse editable paragraph");
    assert!(
        editing.position_at(editor_text, 0).is_some(),
        "the editable paragraph must be queryable"
    );
    assert!(
        editing.position_at(display_text, 0).is_none(),
        "the display sibling must not acquire editing facts"
    );
    assert!(
        scene.selection().is_ok(),
        "selection access must expose the sparse selectable paragraph"
    );
    assert!(
        scene.interaction().is_ok(),
        "point interaction must expose the sparse hit-testable paragraph"
    );
    assert!(
        scene
            .line(0)
            .expect("display sibling has a line")
            .sources()
            .is_err(),
        "the display sibling must reject source observation"
    );
    assert!(
        scene.semantics().is_err(),
        "editing does not imply semantic structure"
    );
    let display_geometry = layout
        .cached_geometry_for_test(display)
        .expect("display sibling remains resident");
    let editor_geometry = layout
        .cached_geometry_for_test(editor)
        .expect("editor paragraph remains resident");
    assert_eq!(display_geometry.features, crate::SceneFeatures::DISPLAY);
    assert_eq!(display_geometry.movement_count(), 0);
    assert_eq!(editor_geometry.features, crate::SceneFeatures::EDITABLE);
    assert_ne!(editor_geometry.movement_count(), 0);

    let residency = scene.residency();
    assert_eq!(residency.paragraphs, 2);
    assert!(residency.bytes.structure > 0);
    assert!(residency.bytes.layout > 0);
    assert!(residency.bytes.paint > 0);
    let paragraphs: Vec<_> = scene.paragraph_residencies().collect();
    assert_eq!(paragraphs.len(), 2);
    let display_residency = paragraphs
        .iter()
        .copied()
        .find(|entry| entry.paragraph == display)
        .expect("display residency is reported");
    assert_eq!(display_residency.requested, crate::SceneFeatures::DISPLAY);
    assert_eq!(display_residency.resident, crate::SceneFeatures::DISPLAY);
    assert_eq!(display_residency.bytes.sources, 0);
    assert_eq!(display_residency.bytes.hit_testing, 0);
    assert_eq!(display_residency.bytes.selection, 0);
    assert_eq!(display_residency.bytes.navigation, 0);

    let editor_residency = paragraphs
        .iter()
        .copied()
        .find(|entry| entry.paragraph == editor)
        .expect("editor residency is reported");
    assert_eq!(editor_residency.requested, crate::SceneFeatures::EDITABLE);
    assert_eq!(editor_residency.resident, crate::SceneFeatures::EDITABLE);
    assert!(editor_residency.bytes.sources > 0);
    assert!(editor_residency.bytes.hit_testing > 0);
    assert_eq!(editor_residency.bytes.selection, 0);
    assert_eq!(editor_residency.bytes.navigation, 0);
    assert_eq!(editor_residency.bytes.semantics, 0);
    assert_eq!(editor_residency.bytes.native_text_input, 0);

    let cache_residency = layout.cache_diagnostics().scene_cache_residency;
    assert_eq!(
        cache_residency.layout,
        display_residency
            .bytes
            .layout
            .saturating_add(editor_residency.bytes.layout)
    );
    assert_eq!(cache_residency.sources, editor_residency.bytes.sources);

    let mut complete_layout = LayoutEngine::new(
        EchoAdapter {
            split_utf8: false,
            split_paint: false,
            mismatched_paint: false,
            glyphless: false,
            interior_cursor: false,
        },
        CacheBudget::new(8),
    );
    let complete_request = SceneRequest::new(TextConstraint::MaxContent, &styles, &paint)
        .with_features(crate::SceneFeatures::EDITABLE);
    let complete = complete_layout
        .prepare(&snapshot, &complete_request)
        .expect("complete scene must prepare");
    let complete_editing = complete
        .scene
        .editing()
        .expect("complete editing must exist");
    let complete_selection = complete_editing
        .between(
            &complete_editing
                .position_at(display_text, 0)
                .expect("display start exists in the complete scene"),
            &complete_editing
                .position_at(editor_text, 6)
                .expect("editor end exists in the complete scene"),
            TextSelectionMode::Logical,
        )
        .expect("complete cross-paragraph selection must form");
    let complete_selections = complete_editing
        .set([complete_selection])
        .expect("complete selection set must validate");
    scene
        .selection()
        .expect("the sparse scene exposes its selectable paragraph")
        .geometry(&complete_selections)
        .expect_err("sparse geometry must reject an omitted selected paragraph");

    let position = editing
        .position_at(editor_text, 0)
        .expect("editor start remains represented");
    let selection = editing
        .collapsed(&position)
        .expect("editor caret forms a selection");
    let selections = editing
        .set([selection])
        .expect("editor selection set is valid");
    let mut composition = editing
        .begin_composition(&selections, CompositionId::from_bytes(*b"sparse-compose01"))
        .expect("sparse editor begins composition")
        .into_session();
    composition
        .update(composition.epoch(), CompositionUpdate::new("preedit"))
        .expect("sparse composition updates");
    let projected = layout
        .prepare_composition(&snapshot, &request, &composition)
        .expect("sparse composition prepares");
    assert!(
        projected.scene.editing().is_ok(),
        "projected editing must expose the sparse composition target"
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
