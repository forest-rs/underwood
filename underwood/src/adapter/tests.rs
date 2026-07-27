// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{vec, vec::Vec};
use core::mem::size_of;

use peniko::Blob;

use super::{
    ClusterBoundary, ClusterWhitespace, FontSynthesis, GlyphPaintCoverage, GlyphPaintSegment,
    LineBreakReason, PreparationErrorKind, PreparedClusterSide, PreparedGlyph,
    PreparedInteractionSlice, PreparedInteractionSliceSpill, PreparedInteractionUnit,
    PreparedInteractionUnitRecord, PreparedLine, PreparedParagraph, PreparedRun, TextAffinity,
};
use crate::{
    DocumentId, FontData, FontVariation, PaintSlot, ParagraphId, Rect, ResolvedDirection, Tag, Vec2,
};

#[test]
fn ordinary_interaction_units_do_not_retain_slice_ranges() {
    assert_eq!(size_of::<PreparedInteractionUnitRecord>(), 16);
    assert_eq!(size_of::<PreparedInteractionSliceSpill>(), 12);
}

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
fn whole_glyph_paint_retains_no_duplicate_source_or_slot() {
    let coverage = GlyphPaintCoverage::whole();
    assert!(coverage.is_whole());
    assert!(coverage.split_segments().is_none());
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
    let segments = glyph
        .paint()
        .split_segments()
        .expect("split glyph retains exceptional segments");
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].clip(), Some(left));
    assert_eq!(segments[1].clip(), Some(right));
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
    let error = PreparedParagraph::try_new(paragraph, 3, ResolvedDirection::Ltr, [first, second])
        .expect_err("source gaps must be rejected at the adapter boundary");
    assert_eq!(
        error.kind(),
        PreparationErrorKind::InvalidOutput,
        "a source gap is invalid adapter output"
    );
}

#[test]
fn prepared_line_rejects_missing_run_source() {
    let (slices, units) = interaction(0..2, 1.0);
    let error = PreparedLine::try_new(
        0..2,
        LineBreakReason::End,
        1.0,
        0.8,
        1.0,
        0.8,
        0.2,
        slices,
        units,
        [run(0..1)],
    )
    .expect_err("visual runs must cover the complete non-empty line source");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_line_rejects_missing_interaction_unit_source() {
    let (slices, units) = interaction(0..1, 1.0);
    let error = PreparedLine::try_new(
        0..2,
        LineBreakReason::End,
        1.0,
        0.8,
        1.0,
        0.8,
        0.2,
        slices,
        units,
        [run(0..2)],
    )
    .expect_err("interaction units must cover the complete line source");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_interaction_unit_rejects_a_side_outside_its_source() {
    let error = PreparedInteractionUnit::try_new(
        1..2,
        0..1,
        1.0,
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
    let slices = [
        PreparedInteractionSlice::try_new(1..3, 0.0).expect("zero-advance mark slice is valid"),
        PreparedInteractionSlice::try_new(0..1, 5.0).expect("base slice is valid"),
    ];
    let unit = PreparedInteractionUnit::try_new(
        0..3,
        0..2,
        5.0,
        1,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(3, TextAffinity::Upstream),
        PreparedClusterSide::new(0, TextAffinity::Downstream),
    )
    .expect("the packed interaction record is locally valid");
    let line = PreparedLine::try_new(
        0..3,
        LineBreakReason::End,
        5.0,
        0.8,
        1.0,
        0.8,
        0.2,
        slices,
        [unit],
        [run(0..3)],
    )
    .expect("visual slice order may differ from canonical source order");
    let paragraph = PreparedParagraph::try_new(
        ParagraphId {
            document: DocumentId::from_bytes(*b"adapter-test-002"),
            index: 0,
        },
        3,
        ResolvedDirection::Ltr,
        [line],
    )
    .expect("the line flattens into a canonical paragraph artifact");
    let unit = paragraph
        .lines()
        .first()
        .expect("the paragraph has one line")
        .units()
        .next()
        .expect("the line has one unit");
    assert_eq!(unit.source(), 0..3);
    assert_eq!(unit.advance(), 5.0);
    assert_eq!(unit.slices().get(0).expect("mark slice").source(), 1..3);
    assert_eq!(unit.slices().get(1).expect("base slice").source(), 0..1);

    let incomplete = PreparedInteractionUnit::try_new(
        0..3,
        0..1,
        5.0,
        0,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(0, TextAffinity::Downstream),
        PreparedClusterSide::new(3, TextAffinity::Upstream),
    )
    .expect("the record is validated against its table by the line");
    let error = PreparedLine::try_new(
        0..3,
        LineBreakReason::End,
        5.0,
        0.8,
        1.0,
        0.8,
        0.2,
        [PreparedInteractionSlice::try_new(0..1, 5.0).expect("the individual slice is valid")],
        [incomplete],
        [run(0..3)],
    )
    .expect_err("missing mark source must fail at the adapter boundary");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

fn line(source: core::ops::Range<u32>) -> PreparedLine {
    let (slices, units) = interaction(source.clone(), 1.0);
    PreparedLine::try_new(
        source.clone(),
        LineBreakReason::End,
        1.0,
        0.8,
        1.0,
        0.8,
        0.2,
        slices,
        units,
        [run(source)],
    )
    .expect("test line is valid")
}

fn interaction(
    source: core::ops::Range<u32>,
    advance: f64,
) -> (Vec<PreparedInteractionSlice>, Vec<PreparedInteractionUnit>) {
    let slices = vec![
        PreparedInteractionSlice::try_new(source.clone(), advance)
            .expect("test interaction slice is valid"),
    ];
    let unit = PreparedInteractionUnit::try_new(
        source.clone(),
        0..1,
        advance,
        0,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(source.start, TextAffinity::Downstream),
        PreparedClusterSide::new(source.end, TextAffinity::Upstream),
    )
    .expect("test interaction unit is valid");
    (slices, vec![unit])
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
    let (slices, units) = interaction(0..1, 0.0);
    let line = PreparedLine::try_new(
        0..1,
        LineBreakReason::End,
        0.0,
        0.8,
        1.0,
        0.8,
        0.2,
        slices,
        units,
        [run],
    )
    .expect("control-only run forms an honest line");
    let paragraph = PreparedParagraph::try_new(
        ParagraphId {
            document: DocumentId::from_bytes(*b"adapter-test-003"),
            index: 0,
        },
        1,
        ResolvedDirection::Ltr,
        [line],
    )
    .expect("control-only line flattens into the canonical artifact");
    assert!(
        paragraph
            .lines()
            .first()
            .expect("the paragraph has one line")
            .runs()
            .next()
            .expect("the line has one run")
            .glyphs()
            .is_empty(),
        "control-only runs must retain an honest empty glyph sequence"
    );
}

fn run(source: core::ops::Range<u32>) -> PreparedRun {
    let paint = GlyphPaintCoverage::whole();
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
