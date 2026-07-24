// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use peniko::Blob;

use super::{
    ClusterBoundary, ClusterWhitespace, FontSynthesis, GlyphPaintCoverage, GlyphPaintSegment,
    LineBreakReason, PreparationErrorKind, PreparedCaret, PreparedClusterSide,
    PreparedCursorMovement, PreparedCursorStep, PreparedGlyph, PreparedInteractionSlice,
    PreparedInteractionUnit, PreparedLine, PreparedParagraph, PreparedRun, TextAffinity,
};
use crate::{DocumentId, FontData, FontVariation, PaintSlot, ParagraphId, Rect, Tag, Vec2};

#[test]
fn synthesis_evidence_is_validated_canonical_and_last_wins() {
    let wght = Tag::new(b"wght");
    let wdth = Tag::new(b"wdth");
    let synthesis = FontSynthesis::try_new(
        [
            FontVariation::new(wght, 400.0),
            FontVariation::new(wdth, 75.0),
            FontVariation::new(wght, 700.0),
        ],
        true,
        Some(0.0),
    )
    .expect("finite synthesis evidence is valid");
    assert_eq!(
        synthesis.variations(),
        &[
            FontVariation::new(wdth, 75.0),
            FontVariation::new(wght, 700.0),
        ],
        "synthesis axes must be tag ordered with duplicate-last-wins semantics"
    );
    assert!(synthesis.embolden(), "embolden evidence must be retained");
    assert_eq!(
        synthesis.skew_degrees(),
        None,
        "zero skew must have the canonical absent representation"
    );
    let oblique =
        FontSynthesis::try_new([], false, Some(14.0)).expect("a finite non-zero skew is valid");
    let transform = oblique
        .skew_transform()
        .expect("a non-zero skew must produce a transform");
    assert!(
        transform.as_coeffs()[2].is_finite() && transform.as_coeffs()[2] > 0.0,
        "the shared skew transform must contain a finite horizontal shear"
    );
    assert!(
        FontSynthesis::try_new([FontVariation::new(wght, f32::NAN)], false, None).is_err(),
        "non-finite synthesis evidence must fail at the adapter boundary"
    );
}

#[test]
fn whole_glyph_paint_is_exactly_one_unclipped_segment() {
    let coverage = GlyphPaintCoverage::whole(2..5, PaintSlot::new(3))
        .expect("whole-glyph coverage must be valid");
    let [segment] = coverage.segments() else {
        panic!("whole-glyph coverage must contain exactly one segment");
    };
    assert_eq!(segment.source(), 2..5);
    assert_eq!(segment.slot(), PaintSlot::new(3));
    assert_eq!(segment.clip(), None);
}

#[test]
fn split_glyph_paint_requires_explicit_clips_for_every_segment() {
    let left = Rect::new(-1.0, -8.0, 4.0, 2.0);
    let right = Rect::new(4.0, -8.0, 11.0, 2.0);
    let coverage = GlyphPaintCoverage::try_from_segments([
        GlyphPaintSegment::clipped(0..1, PaintSlot::new(0), left)
            .expect("left split must be valid"),
        GlyphPaintSegment::clipped(1..3, PaintSlot::new(1), right)
            .expect("right split must be valid"),
    ])
    .expect("contiguous explicitly clipped coverage must be valid");
    let glyph = PreparedGlyph::try_new(17, 0..3, Vec2::new(10.0, 0.0), Vec2::ZERO, coverage)
        .expect("split coverage must preserve one shaped glyph");
    assert_eq!(glyph.paint().segments().len(), 2);
    assert_eq!(glyph.paint().segments()[0].clip(), Some(left));
    assert_eq!(glyph.paint().segments()[1].clip(), Some(right));
}

#[test]
fn glyph_paint_rejects_mixed_unclipped_and_clipped_segments() {
    let error = GlyphPaintCoverage::try_from_segments([
        GlyphPaintSegment::whole(0..1, PaintSlot::new(0))
            .expect("whole segment must be valid alone"),
        GlyphPaintSegment::clipped(1..2, PaintSlot::new(1), Rect::new(5.0, -8.0, 10.0, 2.0))
            .expect("clipped segment must be valid alone"),
    ])
    .expect_err("mixed full and partial paint would make clipping ambiguous");
    assert_eq!(error.kind(), PreparationErrorKind::UnsupportedPaintCoverage);
}

#[test]
fn glyph_paint_rejects_source_gaps_and_single_partial_segments() {
    let gap = GlyphPaintCoverage::try_from_segments([
        GlyphPaintSegment::clipped(0..1, PaintSlot::new(0), Rect::new(0.0, -8.0, 4.0, 2.0))
            .expect("first clipped segment must be valid"),
        GlyphPaintSegment::clipped(2..3, PaintSlot::new(1), Rect::new(6.0, -8.0, 10.0, 2.0))
            .expect("second clipped segment must be valid"),
    ])
    .expect_err("source gaps cannot describe complete glyph paint");
    assert_eq!(gap.kind(), PreparationErrorKind::UnsupportedPaintCoverage);

    let partial = GlyphPaintCoverage::try_from_segments([GlyphPaintSegment::clipped(
        0..1,
        PaintSlot::new(0),
        Rect::new(0.0, -8.0, 4.0, 2.0),
    )
    .expect("the segment geometry itself is valid")])
    .expect_err("one complete paint owner must use the unclipped whole-glyph form");
    assert_eq!(
        partial.kind(),
        PreparationErrorKind::UnsupportedPaintCoverage
    );
}

#[test]
fn prepared_paragraph_rejects_a_gap_between_lines() {
    let paragraph = ParagraphId {
        document: DocumentId::from_bytes(*b"adapter-test-001"),
        index: 0,
    };
    let first = line(0..1);
    let second = line(2..3);
    let error = PreparedParagraph::try_new(paragraph, 3, [first, second], [])
        .expect_err("source gaps must be rejected at the adapter boundary");
    assert_eq!(
        error.kind(),
        PreparationErrorKind::InvalidOutput,
        "a source gap is invalid adapter output"
    );
}

#[test]
fn prepared_paragraph_rejects_incomplete_cursor_facts() {
    let paragraph = ParagraphId {
        document: DocumentId::from_bytes(*b"adapter-test-002"),
        index: 0,
    };
    let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
    let end = PreparedClusterSide::new(1, TextAffinity::Upstream);
    let unknown = PreparedClusterSide::new(0, TextAffinity::Upstream);
    let caret = PreparedCaret::try_new(0, 0.0).expect("test caret is valid");
    let start_movement = PreparedCursorMovement::new(
        start,
        caret,
        None,
        Some(PreparedCursorStep::new(unknown, Some(0..1))),
        None,
        Some(PreparedCursorStep::new(end, Some(0..1))),
    );
    let end_movement = PreparedCursorMovement::new(
        end,
        caret,
        Some(PreparedCursorStep::new(start, Some(0..1))),
        None,
        Some(PreparedCursorStep::new(start, Some(0..1))),
        None,
    );
    let error =
        PreparedParagraph::try_new(paragraph, 1, [line(0..1)], [start_movement, end_movement])
            .expect_err("every cursor target must have its own movement record");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_paragraph_rejects_a_caret_on_an_unknown_line() {
    let paragraph = ParagraphId {
        document: DocumentId::from_bytes(*b"adapter-test-003"),
        index: 0,
    };
    let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
    let end = PreparedClusterSide::new(1, TextAffinity::Upstream);
    let invalid_caret = PreparedCaret::try_new(1, 0.0).expect("coordinates are finite");
    let start_movement = PreparedCursorMovement::new(
        start,
        invalid_caret,
        None,
        Some(PreparedCursorStep::new(end, Some(0..1))),
        None,
        Some(PreparedCursorStep::new(end, Some(0..1))),
    );
    let end_movement = PreparedCursorMovement::new(
        end,
        invalid_caret,
        Some(PreparedCursorStep::new(start, Some(0..1))),
        None,
        Some(PreparedCursorStep::new(start, Some(0..1))),
        None,
    );
    let error =
        PreparedParagraph::try_new(paragraph, 1, [line(0..1)], [start_movement, end_movement])
            .expect_err("caret line identities must resolve inside the paragraph");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_paragraph_rejects_a_step_source_that_is_not_an_interaction_unit() {
    let paragraph = ParagraphId {
        document: DocumentId::from_bytes(*b"adapter-test-004"),
        index: 0,
    };
    let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
    let end = PreparedClusterSide::new(2, TextAffinity::Upstream);
    let start_movement = PreparedCursorMovement::new(
        start,
        PreparedCaret::try_new(0, 0.0).expect("test caret is valid"),
        None,
        Some(PreparedCursorStep::new(end, Some(0..1))),
        None,
        Some(PreparedCursorStep::new(end, Some(0..1))),
    );
    let end_movement = PreparedCursorMovement::new(
        end,
        PreparedCaret::try_new(0, 1.0).expect("test caret is valid"),
        Some(PreparedCursorStep::new(start, Some(0..1))),
        None,
        Some(PreparedCursorStep::new(start, Some(0..1))),
        None,
    );
    let error =
        PreparedParagraph::try_new(paragraph, 2, [line(0..2)], [start_movement, end_movement])
            .expect_err("a cursor step must cross one actual prepared interaction unit");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_line_rejects_missing_run_source() {
    let error = PreparedLine::try_new(
        0..2,
        LineBreakReason::End,
        1.0,
        0.8,
        1.0,
        0.8,
        0.2,
        [unit(0..2, 1.0)],
        [run(0..1)],
    )
    .expect_err("visual runs must cover the complete non-empty line source");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_line_rejects_missing_interaction_unit_source() {
    let error = PreparedLine::try_new(
        0..2,
        LineBreakReason::End,
        1.0,
        0.8,
        1.0,
        0.8,
        0.2,
        [unit(0..1, 1.0)],
        [run(0..2)],
    )
    .expect_err("interaction units must cover the complete line source");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_interaction_unit_rejects_a_side_outside_its_source() {
    let error = PreparedInteractionUnit::try_new(
        1..2,
        [PreparedInteractionSlice::try_new(1..2, 1.0).expect("the interaction slice is valid")],
        0,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(0, TextAffinity::Downstream),
        PreparedClusterSide::new(2, TextAffinity::Upstream),
    )
    .expect_err("interaction-unit sides must name one of the source boundaries");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_interaction_unit_retains_visual_slices_and_checks_canonical_coverage() {
    let unit = PreparedInteractionUnit::try_new(
        0..3,
        [
            PreparedInteractionSlice::try_new(1..3, 0.0).expect("zero-advance mark slice is valid"),
            PreparedInteractionSlice::try_new(0..1, 5.0).expect("base slice is valid"),
        ],
        1,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(3, TextAffinity::Upstream),
        PreparedClusterSide::new(0, TextAffinity::Downstream),
    )
    .expect("visual slice order may differ from canonical source order");
    assert_eq!(unit.source(), 0..3);
    assert_eq!(unit.advance(), 5.0);
    assert_eq!(unit.slices()[0].source(), 1..3);
    assert_eq!(unit.slices()[1].source(), 0..1);

    let error = PreparedInteractionUnit::try_new(
        0..3,
        [PreparedInteractionSlice::try_new(0..1, 5.0).expect("the individual slice is valid")],
        0,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(0, TextAffinity::Downstream),
        PreparedClusterSide::new(3, TextAffinity::Upstream),
    )
    .expect_err("missing mark source must fail at the adapter boundary");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

fn line(source: core::ops::Range<u32>) -> PreparedLine {
    PreparedLine::try_new(
        source.clone(),
        LineBreakReason::End,
        1.0,
        0.8,
        1.0,
        0.8,
        0.2,
        [unit(source.clone(), 1.0)],
        [run(source)],
    )
    .expect("test line is valid")
}

fn unit(source: core::ops::Range<u32>, advance: f64) -> PreparedInteractionUnit {
    PreparedInteractionUnit::try_new(
        source.clone(),
        [PreparedInteractionSlice::try_new(source.clone(), advance)
            .expect("test interaction slice is valid")],
        0,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(source.start, TextAffinity::Downstream),
        PreparedClusterSide::new(source.end, TextAffinity::Upstream),
    )
    .expect("test interaction unit is valid")
}

#[test]
fn prepared_run_accepts_control_only_source_without_a_phantom_glyph() {
    let run = PreparedRun::try_new(
        0..1,
        0,
        *b"Zyyy",
        FontData::new(Blob::from(vec![0_u8]), 0),
        16.,
        FontSynthesis::default(),
        [],
        core::iter::once(0..1),
        [],
    )
    .expect("control-only source does not require a fabricated glyph");
    assert!(
        run.glyphs().is_empty(),
        "control-only runs must retain an honest empty glyph sequence"
    );
}

fn run(source: core::ops::Range<u32>) -> PreparedRun {
    let paint = GlyphPaintCoverage::whole(source.clone(), PaintSlot::new(0))
        .expect("whole-glyph paint is valid");
    let glyph = PreparedGlyph::try_new(1, source.clone(), Vec2::new(1., 0.), Vec2::ZERO, paint)
        .expect("test glyph is valid");
    PreparedRun::try_new(
        source,
        0,
        *b"Latn",
        FontData::new(Blob::from(vec![0_u8]), 0),
        16.,
        FontSynthesis::default(),
        [],
        [],
        [glyph],
    )
    .expect("test run is internally valid")
}
