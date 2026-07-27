// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{vec, vec::Vec};
use core::mem::size_of;

use peniko::Blob;

use super::{
    ClusterBoundary, ClusterWhitespace, FontSynthesis, GlyphPaintCoverage, GlyphPaintSegment,
    LineBreakReason, PreparationErrorKind, PreparedClusterSide, PreparedGlyph,
    PreparedInteractionSlice, PreparedInteractionSliceSpill, PreparedInteractionUnit,
    PreparedInteractionUnitRecord, PreparedLine, PreparedParagraph, PreparedParagraphBuilder,
    PreparedRun, TextAffinity,
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
    let error = build_paragraph(paragraph, 3, ResolvedDirection::Ltr, [first, second])
        .expect_err("source gaps must be rejected at the adapter boundary");
    assert_eq!(
        error.kind(),
        PreparationErrorKind::InvalidOutput,
        "a source gap is invalid adapter output"
    );
}

#[test]
fn dropping_an_unfinished_line_poisons_the_paragraph_builder() {
    let mut paragraph =
        PreparedParagraphBuilder::new(test_paragraph(20), 0, ResolvedDirection::Ltr);
    {
        let _unfinished = paragraph
            .begin_line(
                PreparedLine::try_new(0..0, LineBreakReason::End, 0.0, 8.0, 10.0, 8.0, 2.0)
                    .expect("empty line metadata is valid"),
            )
            .expect("the line begins");
    }
    let error = paragraph
        .finish()
        .expect_err("a partially streamed line must never publish");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_line_rejects_missing_run_source() {
    let (slices, units) = interaction(0..2, 1.0);
    let line = test_line(0..2, 1.0, slices, units, [run(0..1)]);
    let error = build_paragraph(test_paragraph(10), 2, ResolvedDirection::Ltr, [line])
        .expect_err("visual runs must cover the complete non-empty line source");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_line_rejects_overlapping_extra_run_source() {
    let (slices, units) = interaction(0..2, 1.0);
    let line = test_line(0..2, 1.0, slices, units, [run(0..2), run(1..2)]);
    let error = build_paragraph(test_paragraph(21), 2, ResolvedDirection::Ltr, [line])
        .expect_err("visual runs must cover the line exactly once");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_line_rejects_missing_interaction_unit_source() {
    let (slices, units) = interaction(0..1, 1.0);
    let line = test_line(0..2, 1.0, slices, units, [run(0..2)]);
    let error = build_paragraph(test_paragraph(11), 2, ResolvedDirection::Ltr, [line])
        .expect_err("interaction units must cover the complete line source");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

#[test]
fn prepared_interaction_unit_rejects_a_side_outside_its_source() {
    let error = PreparedInteractionUnit::try_new(
        1..2,
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
        5.0,
        1,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(3, TextAffinity::Upstream),
        PreparedClusterSide::new(0, TextAffinity::Downstream),
    )
    .expect("the packed interaction record is locally valid");
    let line = test_line(0..3, 5.0, slices, [unit], [run(0..3)]);
    let paragraph = build_paragraph(
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
        .next()
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
        5.0,
        0,
        ClusterBoundary::None,
        ClusterWhitespace::None,
        PreparedClusterSide::new(0, TextAffinity::Downstream),
        PreparedClusterSide::new(3, TextAffinity::Upstream),
    )
    .expect("the record is validated against its table by the line");
    let line = test_line(
        0..3,
        5.0,
        [PreparedInteractionSlice::try_new(0..1, 5.0).expect("the individual slice is valid")],
        [incomplete],
        [run(0..3)],
    );
    let error = build_paragraph(test_paragraph(12), 3, ResolvedDirection::Ltr, [line])
        .expect_err("missing mark source must fail at the adapter boundary");
    assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
}

fn line(source: core::ops::Range<u32>) -> TestLine {
    let (slices, units) = interaction(source.clone(), 1.0);
    test_line(source.clone(), 1.0, slices, units, [run(source)])
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
    )
    .expect("control-only source does not require a fabricated glyph");
    let unrendered_source = core::iter::once(0..1).collect();
    let run = TestRun {
        run,
        normalized_coords: Vec::new(),
        unrendered_source,
        glyphs: Vec::new(),
    };
    let (slices, units) = interaction(0..1, 0.0);
    let line = test_line(0..1, 0.0, slices, units, [run]);
    let paragraph = build_paragraph(
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
            .next()
            .expect("the paragraph has one line")
            .runs()
            .next()
            .expect("the line has one run")
            .glyphs()
            .len()
            == 0,
        "control-only runs must retain an honest empty glyph sequence"
    );
}

fn run(source: core::ops::Range<u32>) -> TestRun {
    let paint = GlyphPaintCoverage::whole();
    let glyph = PreparedGlyph::try_new(1, source.clone(), Vec2::new(1., 0.), Vec2::ZERO, paint)
        .expect("test glyph is valid");
    let run = PreparedRun::try_new(
        source,
        0,
        *b"Latn",
        FontData::new(Blob::from(vec![0_u8]), 0),
        16.,
        FontSynthesis::default(),
    )
    .expect("test run is internally valid");
    TestRun {
        run,
        normalized_coords: Vec::new(),
        unrendered_source: Vec::new(),
        glyphs: vec![glyph],
    }
}

struct TestRun {
    run: PreparedRun,
    normalized_coords: Vec<i16>,
    unrendered_source: Vec<core::ops::Range<u32>>,
    glyphs: Vec<PreparedGlyph>,
}

struct TestLine {
    line: PreparedLine,
    slices: Vec<PreparedInteractionSlice>,
    units: Vec<PreparedInteractionUnit>,
    runs: Vec<TestRun>,
}

fn test_paragraph(index: u32) -> ParagraphId {
    ParagraphId {
        document: DocumentId::from_bytes(*b"adapter-test-004"),
        index,
    }
}

fn test_line(
    source: core::ops::Range<u32>,
    advance: f64,
    slices: impl IntoIterator<Item = PreparedInteractionSlice>,
    units: impl IntoIterator<Item = PreparedInteractionUnit>,
    runs: impl IntoIterator<Item = TestRun>,
) -> TestLine {
    TestLine {
        line: PreparedLine::try_new(source, LineBreakReason::End, advance, 0.8, 1.0, 0.8, 0.2)
            .expect("test line metrics are valid"),
        slices: slices.into_iter().collect(),
        units: units.into_iter().collect(),
        runs: runs.into_iter().collect(),
    }
}

fn build_paragraph(
    paragraph: ParagraphId,
    text_len: u32,
    direction: ResolvedDirection,
    lines: impl IntoIterator<Item = TestLine>,
) -> Result<PreparedParagraph, super::PreparationError> {
    let mut builder = PreparedParagraphBuilder::new(paragraph, text_len, direction);
    for test_line in lines {
        let mut line = builder.begin_line(test_line.line)?;
        for unit in test_line.units {
            let source = unit.source();
            line.push_unit(
                unit,
                test_line.slices.iter().copied().filter(|slice| {
                    let slice = slice.source();
                    source.start <= slice.start && slice.end <= source.end
                }),
            )?;
        }
        for test_run in test_line.runs {
            let mut run = line.begin_run(test_run.run);
            run.extend_normalized_coords(test_run.normalized_coords);
            for glyph in test_run.glyphs {
                run.push_glyph(glyph)?;
            }
            for source in test_run.unrendered_source {
                run.push_unrendered_source(source)?;
            }
            run.finish()?;
        }
        line.finish()?;
    }
    builder.finish()
}
