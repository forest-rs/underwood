// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Immutable region geometry and resumable line-slot traversal.
//!
//! This module owns deterministic decomposition of rectangles, exclusions,
//! floats, and columns into exact line slots. It explicitly does not own line
//! breaking, shaping, paragraph policy, painting, or rendering.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::Range;

use crate::{ParagraphId, Rect, SceneError, SceneErrorKind, Size};

/// Physical side used to place a floated exclusion inside a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatSide {
    /// Place the float against the region's left edge.
    Left,
    /// Place the float against the region's right edge.
    Right,
}

/// A rectangular float positioned relative to one region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionFloat {
    side: FloatSide,
    block_offset: f64,
    size: Size,
}

impl RegionFloat {
    /// Creates a float with finite positive size and a non-negative block offset.
    pub fn new(side: FloatSide, block_offset: f64, size: Size) -> Result<Self, SceneError> {
        if !block_offset.is_finite()
            || block_offset < 0.0
            || !size.width.is_finite()
            || size.width <= 0.0
            || !size.height.is_finite()
            || size.height <= 0.0
        {
            return Err(invalid_region());
        }
        Ok(Self {
            side,
            block_offset,
            size,
        })
    }

    /// Returns the physical placement side.
    #[must_use]
    pub const fn side(self) -> FloatSide {
        self.side
    }

    /// Returns the offset from the region's block start.
    #[must_use]
    pub const fn block_offset(self) -> f64 {
        self.block_offset
    }

    /// Returns the float's rectangular size.
    #[must_use]
    pub const fn size(self) -> Size {
        self.size
    }
}

/// One rectangular flow region with caller-authored exclusions and floats.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowRegion {
    bounds: Rect,
    exclusions: Arc<[Rect]>,
    floats: Arc<[RegionFloat]>,
}

impl FlowRegion {
    /// Creates a finite region with strictly positive width and height.
    pub fn new(bounds: Rect) -> Result<Self, SceneError> {
        validate_rect(bounds)?;
        Ok(Self {
            bounds,
            exclusions: Arc::from([]),
            floats: Arc::from([]),
        })
    }

    /// Returns a copy with rectangular exclusions.
    ///
    /// Exclusions may extend outside the region; decomposition clips them to
    /// the region bounds.
    pub fn with_exclusions(
        mut self,
        exclusions: impl IntoIterator<Item = Rect>,
    ) -> Result<Self, SceneError> {
        let exclusions: Vec<_> = exclusions.into_iter().collect();
        for exclusion in &exclusions {
            validate_rect(*exclusion)?;
        }
        self.exclusions = exclusions.into();
        Ok(self)
    }

    /// Returns a copy with left- or right-placed floated exclusions.
    pub fn with_floats(
        mut self,
        floats: impl IntoIterator<Item = RegionFloat>,
    ) -> Result<Self, SceneError> {
        let floats: Vec<_> = floats.into_iter().collect();
        if floats.iter().any(|float| {
            float.size.width > self.bounds.width()
                || float.block_offset >= self.bounds.height()
                || float.block_offset + float.size.height > self.bounds.height()
        }) {
            return Err(invalid_region());
        }
        self.floats = floats.into();
        Ok(self)
    }

    /// Returns the region rectangle in flow coordinates.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Returns caller-authored rectangular exclusions.
    #[must_use]
    pub fn exclusions(&self) -> &[Rect] {
        &self.exclusions
    }

    /// Returns floats positioned within this region.
    #[must_use]
    pub fn floats(&self) -> &[RegionFloat] {
        &self.floats
    }

    fn obstacles(&self) -> Vec<Rect> {
        let mut obstacles = self.exclusions.to_vec();
        obstacles.extend(self.floats.iter().map(|float| {
            let x0 = match float.side {
                FloatSide::Left => self.bounds.x0,
                FloatSide::Right => self.bounds.x1 - float.size.width,
            };
            let y0 = self.bounds.y0 + float.block_offset;
            Rect::new(x0, y0, x0 + float.size.width, y0 + float.size.height)
        }));
        obstacles
    }
}

/// Immutable ordered regions traversed as columns or fragmentainers.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionFlow {
    regions: Arc<[CompiledRegion]>,
    max_inline_size: f64,
}

impl RegionFlow {
    /// Compiles ordered regions into allocation-free line-slot lookup tables.
    pub fn new(regions: impl IntoIterator<Item = FlowRegion>) -> Result<Self, SceneError> {
        let source: Vec<_> = regions.into_iter().collect();
        if source.is_empty() {
            return Err(invalid_region());
        }
        let mut compiled = Vec::with_capacity(source.len());
        let mut max_inline_size = 0.0_f64;
        for region in source {
            max_inline_size = max_inline_size.max(region.bounds.width());
            compiled.push(CompiledRegion::new(region)?);
        }
        Ok(Self {
            regions: compiled.into(),
            max_inline_size,
        })
    }

    /// Creates a one-rectangle flow.
    pub fn rectangle(bounds: Rect) -> Result<Self, SceneError> {
        Self::new([FlowRegion::new(bounds)?])
    }

    /// Returns the first resumable cursor.
    #[must_use]
    pub fn cursor(&self) -> RegionCursor {
        RegionCursor {
            region: 0,
            block_start: self.regions[0].source.bounds.y0,
            interval: 0,
            row_height: 0.0,
        }
    }

    /// Returns the ordered source regions.
    pub fn regions(&self) -> impl ExactSizeIterator<Item = &FlowRegion> {
        self.regions.iter().map(|region| &region.source)
    }

    /// Returns the largest region width.
    #[must_use]
    pub const fn max_inline_size(&self) -> f64 {
        self.max_inline_size
    }

    pub(crate) fn shares_backing_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.regions, &other.regions)
    }

    /// Offers the exact slot at `cursor`, skipping fully excluded bands.
    #[must_use]
    pub fn slot(&self, cursor: RegionCursor) -> Option<LineSlot> {
        let cursor = self.normalize(cursor)?;
        let region = self.regions.get(cursor.region as usize)?;
        let band_index = region.band_at(cursor.block_start)?;
        let band = region.bands.get(band_index)?;
        let interval_index = band.intervals.start + cursor.interval as usize;
        if interval_index >= band.intervals.end {
            return None;
        }
        let interval = region.intervals.get(interval_index)?;
        Some(LineSlot {
            region: cursor.region,
            interval: cursor.interval,
            inline_start: interval.start,
            block_start: cursor.block_start,
            inline_size: interval.size,
            block_size: band.end - cursor.block_start,
        })
    }

    /// Advances after accepting a line of the given block size.
    pub fn accept(
        &self,
        cursor: RegionCursor,
        slot: LineSlot,
        line_height: f64,
    ) -> Result<RegionCursor, SceneError> {
        let cursor = self.checked_cursor_slot(cursor, slot)?;
        if !line_height.is_finite() || line_height <= 0.0 || line_height > slot.block_size {
            return Err(invalid_region());
        }
        let region = self
            .regions
            .get(cursor.region as usize)
            .ok_or_else(invalid_region)?;
        let band = region
            .bands
            .get(
                region
                    .band_at(cursor.block_start)
                    .ok_or_else(invalid_region)?,
            )
            .ok_or_else(invalid_region)?;
        let next_interval = usize::try_from(cursor.interval)
            .ok()
            .and_then(|interval| interval.checked_add(1))
            .ok_or_else(invalid_region)?;
        let interval_count = band.intervals.end - band.intervals.start;
        let row_height = cursor.row_height.max(line_height);
        let next = if next_interval < interval_count {
            RegionCursor {
                interval: u32::try_from(next_interval).map_err(|_| invalid_region())?,
                row_height,
                ..cursor
            }
        } else {
            RegionCursor {
                block_start: cursor.block_start + row_height,
                interval: 0,
                row_height: 0.0,
                ..cursor
            }
        };
        Ok(self.normalize_or_end(next))
    }

    /// Advances past a slot whose vertical allowance cannot contain the line.
    ///
    /// Text traversal is intentionally not part of this operation.
    pub fn reject(&self, cursor: RegionCursor, slot: LineSlot) -> Result<RegionCursor, SceneError> {
        let cursor = self.checked_cursor_slot(cursor, slot)?;
        Ok(self.normalize_or_end(RegionCursor {
            block_start: cursor.block_start + slot.block_size,
            interval: 0,
            row_height: 0.0,
            ..cursor
        }))
    }

    fn checked_cursor_slot(
        &self,
        cursor: RegionCursor,
        slot: LineSlot,
    ) -> Result<RegionCursor, SceneError> {
        let normalized = self.normalize(cursor).ok_or_else(invalid_region)?;
        if self.slot(normalized) != Some(slot) {
            return Err(invalid_region());
        }
        Ok(normalized)
    }

    fn normalize(&self, mut cursor: RegionCursor) -> Option<RegionCursor> {
        loop {
            let region = self.regions.get(cursor.region as usize)?;
            if cursor.block_start < region.source.bounds.y0 {
                cursor.block_start = region.source.bounds.y0;
            }
            if cursor.block_start >= region.source.bounds.y1 {
                cursor.region = cursor.region.checked_add(1)?;
                let next = self.regions.get(cursor.region as usize)?;
                cursor.block_start = next.source.bounds.y0;
                cursor.interval = 0;
                cursor.row_height = 0.0;
                continue;
            }
            let band_index = region.band_at(cursor.block_start)?;
            let band = region.bands.get(band_index)?;
            let count = band.intervals.end - band.intervals.start;
            if count == 0 {
                cursor.block_start = band.end;
                cursor.interval = 0;
                cursor.row_height = 0.0;
                continue;
            }
            if cursor.interval as usize >= count {
                cursor.block_start = band.end;
                cursor.interval = 0;
                cursor.row_height = 0.0;
                continue;
            }
            return Some(cursor);
        }
    }

    fn normalize_or_end(&self, cursor: RegionCursor) -> RegionCursor {
        self.normalize(cursor).unwrap_or_else(|| RegionCursor {
            region: u32::try_from(self.regions.len()).unwrap_or(u32::MAX),
            block_start: self
                .regions
                .last()
                .map_or(0.0, |region| region.source.bounds.y1),
            interval: 0,
            row_height: 0.0,
        })
    }
}

/// Resumable position in an immutable [`RegionFlow`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionCursor {
    region: u32,
    block_start: f64,
    interval: u32,
    row_height: f64,
}

impl RegionCursor {
    /// Returns the zero-based region index.
    #[must_use]
    pub const fn region(self) -> usize {
        self.region as usize
    }

    /// Returns the current block coordinate.
    #[must_use]
    pub const fn block_start(self) -> f64 {
        self.block_start
    }

    /// Returns the zero-based interval within the current block row.
    #[must_use]
    pub const fn interval(self) -> usize {
        self.interval as usize
    }
}

/// One exact inline interval and block-axis allowance offered for a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSlot {
    region: u32,
    interval: u32,
    inline_start: f64,
    block_start: f64,
    inline_size: f64,
    block_size: f64,
}

impl LineSlot {
    /// Returns the zero-based region index.
    #[must_use]
    pub const fn region(self) -> usize {
        self.region as usize
    }

    /// Returns the zero-based interval within the current block row.
    #[must_use]
    pub const fn interval(self) -> usize {
        self.interval as usize
    }

    /// Returns the physical inline start.
    #[must_use]
    pub const fn inline_start(self) -> f64 {
        self.inline_start
    }

    /// Returns the physical block start.
    #[must_use]
    pub const fn block_start(self) -> f64 {
        self.block_start
    }

    /// Returns the finite, strictly positive inline allowance.
    #[must_use]
    pub const fn inline_size(self) -> f64 {
        self.inline_size
    }

    /// Returns the finite, strictly positive block allowance.
    #[must_use]
    pub const fn block_size(self) -> f64 {
        self.block_size
    }

    /// Returns the slot rectangle.
    #[must_use]
    pub fn bounds(self) -> Rect {
        Rect::new(
            self.inline_start,
            self.block_start,
            self.inline_start + self.inline_size,
            self.block_start + self.block_size,
        )
    }
}

/// Result of one measured line's attempt to consume a region slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionAttemptOutcome {
    /// The line consumed its source and advanced the region cursor.
    Accepted,
    /// The slot was too short; text traversal was restored before advancing.
    HeightRejected,
}

/// Deterministic record of one measured line and exact offered slot.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionAttempt {
    paragraph: ParagraphId,
    source: Range<u32>,
    slot: LineSlot,
    line_height: f64,
    outcome: RegionAttemptOutcome,
}

impl RegionAttempt {
    /// Validates one region attempt.
    pub fn try_new(
        paragraph: ParagraphId,
        source: Range<u32>,
        slot: LineSlot,
        line_height: f64,
        outcome: RegionAttemptOutcome,
    ) -> Result<Self, SceneError> {
        let height_matches = match outcome {
            RegionAttemptOutcome::Accepted => line_height <= slot.block_size,
            RegionAttemptOutcome::HeightRejected => line_height > slot.block_size,
        };
        if source.start > source.end
            || !line_height.is_finite()
            || line_height <= 0.0
            || !height_matches
        {
            return Err(invalid_region());
        }
        Ok(Self {
            paragraph,
            source,
            slot,
            line_height,
            outcome,
        })
    }

    /// Returns the paragraph identity.
    #[must_use]
    pub const fn paragraph(&self) -> ParagraphId {
        self.paragraph
    }

    /// Returns the paragraph-local source attempted in this slot.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns the exact offered slot.
    #[must_use]
    pub const fn slot(&self) -> LineSlot {
        self.slot
    }

    /// Returns the measured line-box height.
    #[must_use]
    pub const fn line_height(&self) -> f64 {
        self.line_height
    }

    /// Returns whether the attempt consumed text or rejected only geometry.
    #[must_use]
    pub const fn outcome(&self) -> RegionAttemptOutcome {
        self.outcome
    }
}

/// Replayable sequence of region attempts and cursor transitions.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionTranscript {
    start: RegionCursor,
    end: RegionCursor,
    attempts: Arc<[RegionAttempt]>,
}

impl RegionTranscript {
    /// Validates a transcript by replaying every attempt through `flow`.
    pub fn try_new(
        flow: &RegionFlow,
        start: RegionCursor,
        end: RegionCursor,
        attempts: impl IntoIterator<Item = RegionAttempt>,
    ) -> Result<Self, SceneError> {
        let attempts: Arc<[RegionAttempt]> = attempts.into_iter().collect::<Vec<_>>().into();
        let transcript = Self {
            start,
            end,
            attempts,
        };
        if transcript.replay(flow)? != end {
            return Err(invalid_region());
        }
        Ok(transcript)
    }

    /// Returns the cursor before the first attempt.
    #[must_use]
    pub const fn start(&self) -> RegionCursor {
        self.start
    }

    /// Returns the cursor after the last attempt.
    #[must_use]
    pub const fn end(&self) -> RegionCursor {
        self.end
    }

    /// Returns attempts in deterministic execution order.
    #[must_use]
    pub fn attempts(&self) -> &[RegionAttempt] {
        &self.attempts
    }

    /// Replays the transcript without shaping or scene state.
    pub fn replay(&self, flow: &RegionFlow) -> Result<RegionCursor, SceneError> {
        let mut cursor = self.start;
        for attempt in self.attempts.iter() {
            if flow.slot(cursor) != Some(attempt.slot) {
                return Err(invalid_region());
            }
            cursor = match attempt.outcome {
                RegionAttemptOutcome::Accepted => {
                    flow.accept(cursor, attempt.slot, attempt.line_height)?
                }
                RegionAttemptOutcome::HeightRejected => flow.reject(cursor, attempt.slot)?,
            };
        }
        Ok(cursor)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledRegion {
    source: FlowRegion,
    bands: Arc<[RegionBand]>,
    intervals: Arc<[RegionInterval]>,
}

impl CompiledRegion {
    fn new(source: FlowRegion) -> Result<Self, SceneError> {
        let bounds = source.bounds;
        let obstacles: Vec<_> = source
            .obstacles()
            .into_iter()
            .filter_map(|obstacle| intersect(bounds, obstacle))
            .collect();
        let mut boundaries = Vec::with_capacity(obstacles.len() * 2 + 2);
        boundaries.push(bounds.y0);
        boundaries.push(bounds.y1);
        for obstacle in &obstacles {
            boundaries.push(obstacle.y0);
            boundaries.push(obstacle.y1);
        }
        boundaries.sort_by(total_cmp);
        boundaries.dedup_by(|left, right| left.to_bits() == right.to_bits());

        let mut bands = Vec::with_capacity(boundaries.len().saturating_sub(1));
        let mut intervals = Vec::new();
        for window in boundaries.windows(2) {
            let start = window[0];
            let end = window[1];
            if end <= start {
                continue;
            }
            let interval_start = intervals.len();
            let mut blocked: Vec<_> = obstacles
                .iter()
                .filter(|obstacle| obstacle.y0 < end && obstacle.y1 > start)
                .map(|obstacle| (obstacle.x0.max(bounds.x0), obstacle.x1.min(bounds.x1)))
                .filter(|(start, end)| end > start)
                .collect();
            blocked.sort_by(|left, right| total_cmp(&left.0, &right.0));
            let mut inline = bounds.x0;
            for (blocked_start, blocked_end) in blocked {
                if blocked_start > inline {
                    intervals.push(RegionInterval {
                        start: inline,
                        size: blocked_start - inline,
                    });
                }
                inline = inline.max(blocked_end);
            }
            if inline < bounds.x1 {
                intervals.push(RegionInterval {
                    start: inline,
                    size: bounds.x1 - inline,
                });
            }
            bands.push(RegionBand {
                start,
                end,
                intervals: interval_start..intervals.len(),
            });
        }
        if bands.is_empty() {
            return Err(invalid_region());
        }
        Ok(Self {
            source,
            bands: bands.into(),
            intervals: intervals.into(),
        })
    }

    fn band_at(&self, block_start: f64) -> Option<usize> {
        let index = self.bands.partition_point(|band| band.end <= block_start);
        self.bands
            .get(index)
            .filter(|band| band.start <= block_start && block_start < band.end)
            .map(|_| index)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RegionBand {
    start: f64,
    end: f64,
    intervals: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RegionInterval {
    start: f64,
    size: f64,
}

fn intersect(left: Rect, right: Rect) -> Option<Rect> {
    let result = Rect::new(
        left.x0.max(right.x0),
        left.y0.max(right.y0),
        left.x1.min(right.x1),
        left.y1.min(right.y1),
    );
    (result.width() > 0.0 && result.height() > 0.0).then_some(result)
}

fn validate_rect(rect: Rect) -> Result<(), SceneError> {
    if !rect.x0.is_finite()
        || !rect.y0.is_finite()
        || !rect.x1.is_finite()
        || !rect.y1.is_finite()
        || rect.width() <= 0.0
        || rect.height() <= 0.0
    {
        return Err(invalid_region());
    }
    Ok(())
}

fn invalid_region() -> SceneError {
    SceneError::new(SceneErrorKind::InvalidRegion)
}

fn total_cmp(left: &f64, right: &f64) -> Ordering {
    left.total_cmp(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(flow: &RegionFlow, cursor: RegionCursor) -> LineSlot {
        flow.slot(cursor).expect("fixture must offer a slot")
    }

    #[test]
    fn rectangle_advances_by_accepted_line_height() {
        let flow =
            RegionFlow::rectangle(Rect::new(10.0, 20.0, 110.0, 80.0)).expect("rectangle is valid");
        let cursor = flow.cursor();
        let first = slot(&flow, cursor);
        assert_eq!(first.bounds(), Rect::new(10.0, 20.0, 110.0, 80.0));
        let cursor = flow
            .accept(cursor, first, 12.0)
            .expect("line fits the rectangle");
        assert_eq!(
            slot(&flow, cursor).bounds(),
            Rect::new(10.0, 32.0, 110.0, 80.0)
        );
    }

    #[test]
    fn central_exclusion_offers_both_intervals_before_advancing_the_row() {
        let region = FlowRegion::new(Rect::new(0.0, 0.0, 100.0, 100.0))
            .expect("region is valid")
            .with_exclusions([Rect::new(40.0, 0.0, 60.0, 30.0)])
            .expect("exclusion is valid");
        let flow = RegionFlow::new([region]).expect("flow is valid");
        let cursor = flow.cursor();
        let left = slot(&flow, cursor);
        assert_eq!(left.bounds(), Rect::new(0.0, 0.0, 40.0, 30.0));
        let cursor = flow
            .accept(cursor, left, 10.0)
            .expect("left interval accepts");
        let right = slot(&flow, cursor);
        assert_eq!(right.bounds(), Rect::new(60.0, 0.0, 100.0, 30.0));
        let cursor = flow
            .accept(cursor, right, 14.0)
            .expect("right interval accepts");
        assert_eq!(
            slot(&flow, cursor).bounds(),
            Rect::new(0.0, 14.0, 40.0, 30.0)
        );
    }

    #[test]
    fn left_and_right_floats_compile_to_exact_bands() {
        let region = FlowRegion::new(Rect::new(0.0, 0.0, 120.0, 100.0))
            .expect("region is valid")
            .with_floats([
                RegionFloat::new(FloatSide::Left, 0.0, Size::new(30.0, 20.0))
                    .expect("left float is valid"),
                RegionFloat::new(FloatSide::Right, 20.0, Size::new(40.0, 30.0))
                    .expect("right float is valid"),
            ])
            .expect("floats fit");
        let flow = RegionFlow::new([region]).expect("flow is valid");
        let cursor = flow.cursor();
        let first = slot(&flow, cursor);
        assert_eq!(first.bounds(), Rect::new(30.0, 0.0, 120.0, 20.0));
        let cursor = flow
            .reject(cursor, first)
            .expect("height rejection advances to the next band");
        let second = slot(&flow, cursor);
        assert_eq!(second.bounds(), Rect::new(0.0, 20.0, 80.0, 50.0));
        let cursor = flow
            .reject(cursor, second)
            .expect("second band can be skipped");
        assert_eq!(
            slot(&flow, cursor).bounds(),
            Rect::new(0.0, 50.0, 120.0, 100.0)
        );
    }

    #[test]
    fn ordered_regions_continue_as_columns() {
        let flow = RegionFlow::new([
            FlowRegion::new(Rect::new(0.0, 0.0, 40.0, 20.0)).expect("first column is valid"),
            FlowRegion::new(Rect::new(50.0, 0.0, 90.0, 30.0)).expect("second column is valid"),
        ])
        .expect("column flow is valid");
        let cursor = flow.cursor();
        let first = slot(&flow, cursor);
        let cursor = flow
            .accept(cursor, first, 20.0)
            .expect("first column accepts");
        let second = slot(&flow, cursor);
        assert_eq!(second.region(), 1);
        assert_eq!(second.bounds(), Rect::new(50.0, 0.0, 90.0, 30.0));
    }

    #[test]
    fn a_fully_excluded_region_resumes_at_the_next_column() {
        let blocked = FlowRegion::new(Rect::new(0.0, 0.0, 40.0, 20.0))
            .expect("first region is valid")
            .with_exclusions([Rect::new(0.0, 0.0, 40.0, 20.0)])
            .expect("full exclusion is valid");
        let flow = RegionFlow::new([
            blocked,
            FlowRegion::new(Rect::new(50.0, 0.0, 90.0, 30.0)).expect("second column is valid"),
        ])
        .expect("column flow is valid");

        let first = slot(&flow, flow.cursor());
        assert_eq!(first.region(), 1);
        assert_eq!(first.bounds(), Rect::new(50.0, 0.0, 90.0, 30.0));
    }

    #[test]
    fn transcript_replays_accepted_and_height_rejected_slots() {
        let flow = RegionFlow::new([
            FlowRegion::new(Rect::new(0.0, 0.0, 40.0, 5.0)).expect("first region is valid"),
            FlowRegion::new(Rect::new(50.0, 0.0, 90.0, 30.0)).expect("second region is valid"),
        ])
        .expect("column flow is valid");
        let paragraph = ParagraphId {
            document: crate::DocumentId::from_bytes(*b"region-transcrip"),
            index: 0,
        };
        let start = flow.cursor();
        let first = slot(&flow, start);
        let rejected = RegionAttempt::try_new(
            paragraph,
            0..4,
            first,
            10.0,
            RegionAttemptOutcome::HeightRejected,
        )
        .expect("rejected attempt is valid");
        let second_cursor = flow.reject(start, first).expect("first slot rejects");
        let second = slot(&flow, second_cursor);
        let accepted = RegionAttempt::try_new(
            paragraph,
            0..4,
            second,
            10.0,
            RegionAttemptOutcome::Accepted,
        )
        .expect("accepted attempt is valid");
        let end = flow
            .accept(second_cursor, second, 10.0)
            .expect("second slot accepts");
        let transcript = RegionTranscript::try_new(&flow, start, end, [rejected, accepted])
            .expect("transcript replays");
        assert_eq!(transcript.replay(&flow).expect("replay succeeds"), end);
    }

    #[test]
    fn clones_share_the_compiled_slot_tables() {
        let flow = RegionFlow::new([
            FlowRegion::new(Rect::new(0.0, 0.0, 40.0, 20.0)).expect("first region is valid"),
            FlowRegion::new(Rect::new(50.0, 0.0, 90.0, 30.0)).expect("second region is valid"),
        ])
        .expect("column flow is valid");
        let cloned = flow.clone();

        assert!(Arc::ptr_eq(&flow.regions, &cloned.regions));
        assert_eq!(flow.cursor(), cloned.cursor());
        assert_eq!(flow.slot(flow.cursor()), cloned.slot(cloned.cursor()));
    }
}
