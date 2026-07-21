// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Knowledge {
    Estimated,
    Measured,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Segment {
    blocks: Range<u32>,
    knowledge: Knowledge,
    lower: u64,
    current: u64,
    upper: u64,
    estimator: u64,
    sample_revision: u64,
    measured_fragment: Option<Range<u32>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtentMap {
    source_revision: u64,
    segments: Vec<Segment>,
    cumulative: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Navigation {
    pub(crate) segment: usize,
    pub(crate) blocks: Range<u32>,
    pub(crate) knowledge: Knowledge,
    pub(crate) comparisons: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Correction {
    pub(crate) affected: Range<u32>,
    pub(crate) old_extent: u64,
    pub(crate) new_extent: u64,
    pub(crate) old_prefix_before: u64,
    pub(crate) old_prefix_after: u64,
    pub(crate) new_prefix_before: u64,
    pub(crate) new_prefix_after: u64,
    pub(crate) delta_above: i64,
    pub(crate) delta_through: i64,
    pub(crate) delta_below: i64,
    pub(crate) estimator_violation: bool,
    pub(crate) anchor_survived: bool,
    pub(crate) segments_reaggregated: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeometryKnowledge {
    pub(crate) knowledge: Knowledge,
    pub(crate) lower: u64,
    pub(crate) current: u64,
    pub(crate) upper: u64,
}

impl ExtentMap {
    pub(crate) fn estimated(blocks: u32, segment_blocks: u32, extent_per_block: u64) -> Self {
        let segments = (0..blocks.div_ceil(segment_blocks))
            .map(|index| {
                let start = index * segment_blocks;
                let end = (start + segment_blocks).min(blocks);
                let count = u64::from(end - start);
                Segment {
                    blocks: start..end,
                    knowledge: Knowledge::Estimated,
                    lower: count * (extent_per_block * 7 / 8),
                    current: count * extent_per_block,
                    upper: count * (extent_per_block * 9 / 8),
                    estimator: 0x6578_7465_6e74_0001,
                    sample_revision: 1,
                    measured_fragment: None,
                }
            })
            .collect::<Vec<_>>();
        let cumulative = cumulative(&segments);
        Self {
            source_revision: 1,
            segments,
            cumulative,
        }
    }

    pub(crate) fn total_extent(&self) -> u64 {
        self.cumulative.last().copied().unwrap_or(0)
    }

    pub(crate) fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub(crate) fn navigate(&self, coordinate: u64) -> Option<Navigation> {
        if self.segments.is_empty() || coordinate >= self.total_extent() {
            return None;
        }
        let mut low = 0;
        let mut high = self.cumulative.len();
        let mut comparisons = 0;
        while low < high {
            comparisons += 1;
            let middle = low + (high - low) / 2;
            if coordinate < self.cumulative[middle] {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        let segment = &self.segments[low];
        Some(Navigation {
            segment: low,
            blocks: segment.blocks.clone(),
            knowledge: segment.knowledge,
            comparisons,
        })
    }

    pub(crate) fn measure(
        &mut self,
        segment_index: usize,
        measured_extent: u64,
        measurement_revision: u64,
        anchor: Option<u32>,
    ) -> Option<Correction> {
        let segment = self.segments.get(segment_index)?.clone();
        let old_prefix_before = segment_index
            .checked_sub(1)
            .map_or(0, |previous| self.cumulative[previous]);
        let old_prefix_after = self.cumulative[segment_index];
        let estimator_violation =
            measured_extent < segment.lower || measured_extent > segment.upper;
        let target = &mut self.segments[segment_index];
        target.knowledge = Knowledge::Measured;
        target.current = measured_extent;
        target.lower = measured_extent;
        target.upper = measured_extent;
        target.sample_revision = measurement_revision;
        target.measured_fragment = Some(segment.blocks.clone());

        let mut prefix = old_prefix_before;
        for (index, entry) in self.segments[segment_index..].iter().enumerate() {
            prefix += entry.current;
            self.cumulative[segment_index + index] = prefix;
        }
        let new_prefix_after = self.cumulative[segment_index];
        let delta = signed_difference(measured_extent, segment.current);
        Some(Correction {
            affected: segment.blocks.clone(),
            old_extent: segment.current,
            new_extent: measured_extent,
            old_prefix_before,
            old_prefix_after,
            new_prefix_before: old_prefix_before,
            new_prefix_after,
            delta_above: 0,
            delta_through: delta,
            delta_below: delta,
            estimator_violation,
            anchor_survived: anchor.is_none_or(|block| {
                self.segments
                    .last()
                    .is_some_and(|last| block < last.blocks.end)
            }),
            segments_reaggregated: self.segments.len() - segment_index,
        })
    }

    pub(crate) fn geometry(&self, block: u32) -> GeometryKnowledge {
        let Some(segment) = self
            .segments
            .iter()
            .find(|segment| segment.blocks.contains(&block))
        else {
            return GeometryKnowledge {
                knowledge: Knowledge::Unavailable,
                lower: 0,
                current: 0,
                upper: 0,
            };
        };
        GeometryKnowledge {
            knowledge: segment.knowledge,
            lower: segment.lower,
            current: segment.current,
            upper: segment.upper,
        }
    }

    pub(crate) fn semantic_child_exists(&self, block: u32) -> bool {
        self.segments
            .last()
            .is_some_and(|last| block < last.blocks.end)
    }
}

fn cumulative(segments: &[Segment]) -> Vec<u64> {
    let mut total = 0_u64;
    segments
        .iter()
        .map(|segment| {
            total += segment.current;
            total
        })
        .collect()
}

fn signed_difference(left: u64, right: u64) -> i64 {
    if left >= right {
        i64::try_from(left - right).expect("synthetic correction fits i64")
    } else {
        -i64::try_from(right - left).expect("synthetic correction fits i64")
    }
}

#[cfg(test)]
mod tests {
    use super::{ExtentMap, Knowledge};

    #[test]
    fn random_navigation_uses_prefix_search_without_realizing_blocks() {
        let map = ExtentMap::estimated(1_000_000, 1024, 16 * 1024);
        let navigation = map
            .navigate(map.total_extent() * 3 / 4)
            .expect("coordinate is inside virtual extent");
        assert_eq!(navigation.knowledge, Knowledge::Estimated);
        assert!(navigation.comparisons <= 10);
        assert!(navigation.blocks.len() <= 1024);
    }

    #[test]
    fn measurement_reports_exact_correction_and_knowledge() {
        let mut map = ExtentMap::estimated(1_000_000, 1024, 16 * 1024);
        let old = map.geometry(2048);
        assert_eq!(old.knowledge, Knowledge::Estimated);
        let measured = old.current + 1024;
        let correction = map
            .measure(2, measured, 2, Some(2048))
            .expect("segment exists");
        assert_eq!(correction.delta_above, 0);
        assert_eq!(correction.delta_through, 1024);
        assert_eq!(correction.delta_below, 1024);
        assert!(!correction.estimator_violation);
        assert!(correction.anchor_survived);
        assert_eq!(map.geometry(2048).knowledge, Knowledge::Measured);
        assert!(map.semantic_child_exists(999_999));
    }

    #[test]
    fn out_of_bounds_measurement_emits_estimator_violation() {
        let mut map = ExtentMap::estimated(4096, 1024, 16 * 1024);
        let correction = map.measure(0, 1, 2, None).expect("segment exists");
        assert!(correction.estimator_violation);
    }
}
