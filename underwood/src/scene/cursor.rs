// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Allocation-free cursor derivation over retained interaction units.
//!
//! This module owns navigation policy over prepared paragraph facts; it
//! explicitly does not own another cursor graph or scene-space placement.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CursorCluster {
    pub(super) line: usize,
    pub(super) unit: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CursorCaretAnchor {
    Unit {
        cluster: CursorCluster,
        at_end: bool,
    },
    LastLineStart,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DerivedCursorStep {
    pub(super) target: PreparedClusterSide,
    pub(super) source: Option<Range<u32>>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ParagraphCursor<'a> {
    facts: &'a PreparedParagraphFacts,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CursorMovement<'a> {
    cursor: ParagraphCursor<'a>,
    position: PreparedClusterSide,
}

impl<'a> ParagraphCursor<'a> {
    pub(super) const fn new(facts: &'a PreparedParagraphFacts) -> Self {
        Self { facts }
    }

    pub(super) fn count(self) -> usize {
        if !self.facts.features().has_selection() {
            return 0;
        }
        let units = self
            .facts
            .lines()
            .iter()
            .map(|line| line.units().len())
            .sum::<usize>();
        if units == 0 {
            1
        } else {
            units.saturating_mul(2)
        }
    }

    pub(super) fn get(self, position: PreparedClusterSide) -> Option<CursorMovement<'a>> {
        if !self.facts.features().has_selection()
            || self.normalize(position.offset(), position.affinity()) != position
        {
            return None;
        }
        Some(CursorMovement {
            cursor: self,
            position,
        })
    }

    pub(super) fn start(self) -> Option<CursorMovement<'a>> {
        self.get(PreparedClusterSide::new(0, TextAffinity::Downstream))
    }

    pub(super) fn end(self) -> Option<CursorMovement<'a>> {
        if self.first_cluster().is_none() {
            return self.start();
        }
        self.get(PreparedClusterSide::new(
            self.facts.text_len(),
            TextAffinity::Upstream,
        ))
    }

    pub(super) fn first_visual(self) -> Option<CursorMovement<'a>> {
        let position = self
            .first_cluster()
            .and_then(|cluster| self.unit(cluster))
            .map_or(
                PreparedClusterSide::new(0, TextAffinity::Downstream),
                |unit| unit.left(),
            );
        self.get(position)
    }

    pub(super) fn last_visual(self) -> Option<CursorMovement<'a>> {
        let position = self
            .last_cluster()
            .and_then(|cluster| self.unit(cluster))
            .map_or(
                PreparedClusterSide::new(self.facts.text_len(), TextAffinity::Upstream),
                |unit| unit.right(),
            );
        self.get(position)
    }

    fn normalize(self, offset: u32, affinity: TextAffinity) -> PreparedClusterSide {
        if offset > self.facts.text_len() {
            return PreparedClusterSide::new(u32::MAX, affinity);
        }
        if self.first_cluster().is_none() {
            return PreparedClusterSide::new(
                self.facts.text_len(),
                if self.facts.text_len() == 0 {
                    TextAffinity::Downstream
                } else {
                    TextAffinity::Upstream
                },
            );
        }
        if let Some(cluster) = self.downstream_cluster(offset)
            && let Some(unit) = self.unit(cluster)
        {
            let offset = unit.source().start;
            return PreparedClusterSide::new(
                offset,
                if offset == 0 {
                    TextAffinity::Downstream
                } else {
                    affinity
                },
            );
        }
        PreparedClusterSide::new(self.facts.text_len(), TextAffinity::Upstream)
    }

    fn visual_clusters(self, position: PreparedClusterSide) -> [Option<CursorCluster>; 2] {
        let upstream = self.upstream_cluster(position.offset());
        let downstream = self.downstream_cluster(position.offset());
        if position.affinity() == TextAffinity::Upstream {
            if let Some(cluster) = upstream {
                if self.is_rtl(cluster) {
                    [self.previous_cluster(cluster), Some(cluster)]
                } else {
                    [Some(cluster), self.next_cluster(cluster)]
                }
            } else if let Some(cluster) = downstream {
                if self.is_rtl(cluster) {
                    [None, Some(cluster)]
                } else {
                    [Some(cluster), None]
                }
            } else {
                [None, None]
            }
        } else if let Some(cluster) = downstream {
            if self.is_rtl(cluster) {
                [Some(cluster), self.next_cluster(cluster)]
            } else {
                [self.previous_cluster(cluster), Some(cluster)]
            }
        } else if let Some(cluster) = upstream {
            if self.is_rtl(cluster) {
                [None, Some(cluster)]
            } else {
                [Some(cluster), None]
            }
        } else {
            [None, None]
        }
    }

    fn upstream_cluster(self, offset: u32) -> Option<CursorCluster> {
        let lines = self.facts.lines();
        let line_index = lines.partition_point(|line| line.source().end < offset);
        let line = lines.get(line_index)?;
        (line.source().start < offset && offset <= line.source().end)
            .then(|| self.find_unit(line, line_index, offset, true))
            .flatten()
    }

    fn downstream_cluster(self, offset: u32) -> Option<CursorCluster> {
        let lines = self.facts.lines();
        let line_index = lines.partition_point(|line| line.source().end <= offset);
        let line = lines.get(line_index)?;
        (line.source().start <= offset && offset < line.source().end)
            .then(|| self.find_unit(line, line_index, offset, false))
            .flatten()
    }

    fn find_unit(
        self,
        line: &PreparedLine,
        line_index: usize,
        offset: u32,
        upstream: bool,
    ) -> Option<CursorCluster> {
        let mut start = 0;
        let mut end = line.units().len();
        while start < end {
            let middle = start + (end - start) / 2;
            let source = line.unit_at_source_rank(middle)?.1.source();
            let before = if upstream {
                source.end < offset
            } else {
                source.end <= offset
            };
            if before {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        let (unit, prepared) = line.unit_at_source_rank(start)?;
        let source = prepared.source();
        let contains = if upstream {
            source.start < offset && offset <= source.end
        } else {
            source.start <= offset && offset < source.end
        };
        if !contains {
            return None;
        }
        Some(CursorCluster {
            line: line_index,
            unit,
        })
    }

    fn first_cluster(self) -> Option<CursorCluster> {
        self.facts
            .lines()
            .iter()
            .enumerate()
            .find(|(_, line)| !line.units().is_empty())
            .map(|(line, _)| CursorCluster { line, unit: 0 })
    }

    fn last_cluster(self) -> Option<CursorCluster> {
        self.facts
            .lines()
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| !line.units().is_empty())
            .map(|(line, prepared)| CursorCluster {
                line,
                unit: prepared.units().len() - 1,
            })
    }

    fn previous_cluster(self, cluster: CursorCluster) -> Option<CursorCluster> {
        if let Some(unit) = cluster.unit.checked_sub(1) {
            return Some(CursorCluster {
                line: cluster.line,
                unit,
            });
        }
        self.facts.lines()[..cluster.line]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| !line.units().is_empty())
            .map(|(line, prepared)| CursorCluster {
                line,
                unit: prepared.units().len() - 1,
            })
    }

    fn next_cluster(self, cluster: CursorCluster) -> Option<CursorCluster> {
        let line = self.facts.lines().get(cluster.line)?;
        if cluster.unit + 1 < line.units().len() {
            return Some(CursorCluster {
                line: cluster.line,
                unit: cluster.unit + 1,
            });
        }
        self.facts
            .lines()
            .iter()
            .enumerate()
            .skip(cluster.line + 1)
            .find(|(_, line)| !line.units().is_empty())
            .map(|(line, _)| CursorCluster { line, unit: 0 })
    }

    fn unit(self, cluster: CursorCluster) -> Option<PreparedInteractionUnitView<'a>> {
        self.facts
            .lines()
            .get(cluster.line)?
            .units()
            .nth(cluster.unit)
    }

    fn is_rtl(self, cluster: CursorCluster) -> bool {
        self.unit(cluster)
            .is_some_and(|unit| unit.bidi_level() & 1 == 1)
    }

    fn is_end_of_line(self, cluster: CursorCluster) -> bool {
        self.facts
            .lines()
            .get(cluster.line)
            .is_some_and(|line| cluster.unit + 1 == line.units().len())
    }

    fn is_soft_line_end(self, cluster: CursorCluster) -> bool {
        self.is_end_of_line(cluster)
            && self.facts.lines()[cluster.line].break_reason() == LineBreakReason::Regular
    }

    fn is_hard_line_end(self, cluster: CursorCluster) -> bool {
        self.is_end_of_line(cluster)
            && self.facts.lines()[cluster.line].break_reason() == LineBreakReason::Mandatory
    }
}

impl CursorMovement<'_> {
    pub(super) const fn position(self) -> PreparedClusterSide {
        self.position
    }

    pub(super) fn caret_anchor(self) -> CursorCaretAnchor {
        let [left, right] = self.cursor.visual_clusters(self.position);
        match (left, right) {
            (Some(left), Some(right)) => {
                if self.cursor.is_end_of_line(left) {
                    if self.cursor.is_soft_line_end(left) {
                        if self.cursor.is_rtl(left)
                            && self.position.affinity() == TextAffinity::Downstream
                            || !self.cursor.is_rtl(left)
                                && self.position.affinity() == TextAffinity::Upstream
                        {
                            CursorCaretAnchor::Unit {
                                cluster: left,
                                at_end: true,
                            }
                        } else {
                            CursorCaretAnchor::Unit {
                                cluster: right,
                                at_end: false,
                            }
                        }
                    } else {
                        CursorCaretAnchor::Unit {
                            cluster: right,
                            at_end: false,
                        }
                    }
                } else {
                    CursorCaretAnchor::Unit {
                        cluster: left,
                        at_end: true,
                    }
                }
            }
            (Some(left), None) if self.cursor.is_hard_line_end(left) => {
                CursorCaretAnchor::LastLineStart
            }
            (Some(left), _) => CursorCaretAnchor::Unit {
                cluster: left,
                at_end: true,
            },
            (_, Some(right)) => CursorCaretAnchor::Unit {
                cluster: right,
                at_end: false,
            },
            _ => CursorCaretAnchor::LastLineStart,
        }
    }

    pub(super) fn previous_visual(self) -> Option<DerivedCursorStep> {
        let [left, right] = self.cursor.visual_clusters(self.position);
        if let (Some(left), Some(right)) = (left, right)
            && self.cursor.is_soft_line_end(left)
        {
            if self.cursor.is_rtl(left) && self.position.affinity() == TextAffinity::Upstream {
                let index = if self.cursor.is_rtl(right) {
                    self.cursor.unit(left)?.source().start
                } else {
                    self.cursor.unit(left)?.source().end
                };
                return Some(DerivedCursorStep {
                    target: self.cursor.normalize(index, TextAffinity::Downstream),
                    source: None,
                });
            } else if !self.cursor.is_rtl(left)
                && self.position.affinity() == TextAffinity::Downstream
            {
                let index = if self.cursor.is_rtl(right) {
                    self.cursor.unit(right)?.source().end
                } else {
                    self.cursor.unit(right)?.source().start
                };
                return Some(DerivedCursorStep {
                    target: self.cursor.normalize(index, TextAffinity::Upstream),
                    source: None,
                });
            }
        }
        let left = left?;
        let unit = self.cursor.unit(left)?;
        let source = unit.source();
        let index = if self.cursor.is_rtl(left) {
            source.end
        } else {
            source.start
        };
        Some(DerivedCursorStep {
            target: self.cursor.normalize(
                index,
                affinity_for_visual_direction(self.cursor.is_rtl(left), false),
            ),
            source: Some(source),
        })
    }

    pub(super) fn next_visual(self) -> Option<DerivedCursorStep> {
        let [left, right] = self.cursor.visual_clusters(self.position);
        if let (Some(left), Some(right)) = (left, right) {
            if self.cursor.is_soft_line_end(left) {
                if self.cursor.is_rtl(left) && self.position.affinity() == TextAffinity::Downstream
                {
                    let index = if self.cursor.is_rtl(right) {
                        self.cursor.unit(right)?.source().end
                    } else {
                        self.cursor.unit(right)?.source().start
                    };
                    return Some(DerivedCursorStep {
                        target: self.cursor.normalize(index, TextAffinity::Upstream),
                        source: None,
                    });
                } else if !self.cursor.is_rtl(left)
                    && self.position.affinity() == TextAffinity::Upstream
                {
                    let index = if self.cursor.is_rtl(right) {
                        self.cursor.unit(right)?.source().end
                    } else {
                        self.cursor.unit(right)?.source().start
                    };
                    return Some(DerivedCursorStep {
                        target: self.cursor.normalize(index, TextAffinity::Downstream),
                        source: None,
                    });
                }
            }
            return self.step_after_visual(right);
        }
        right.and_then(|right| self.step_after_visual(right))
    }

    pub(super) fn previous_logical(self) -> Option<DerivedCursorStep> {
        let cluster = self.cursor.upstream_cluster(self.position.offset())?;
        let source = self.cursor.unit(cluster)?.source();
        Some(DerivedCursorStep {
            target: self
                .cursor
                .normalize(source.start, TextAffinity::Downstream),
            source: Some(source),
        })
    }

    pub(super) fn next_logical(self) -> Option<DerivedCursorStep> {
        let cluster = self.cursor.downstream_cluster(self.position.offset())?;
        let source = self.cursor.unit(cluster)?.source();
        Some(DerivedCursorStep {
            target: self.cursor.normalize(source.end, TextAffinity::Upstream),
            source: Some(source),
        })
    }

    fn step_after_visual(self, cluster: CursorCluster) -> Option<DerivedCursorStep> {
        let unit = self.cursor.unit(cluster)?;
        let source = unit.source();
        let offset = if self.cursor.is_rtl(cluster) {
            source.start
        } else {
            source.end
        };
        Some(DerivedCursorStep {
            target: self.cursor.normalize(
                offset,
                affinity_for_visual_direction(self.cursor.is_rtl(cluster), true),
            ),
            source: Some(source),
        })
    }
}

const fn affinity_for_visual_direction(rtl: bool, moving_right: bool) -> TextAffinity {
    match (rtl, moving_right) {
        (true, true) | (false, false) => TextAffinity::Downstream,
        _ => TextAffinity::Upstream,
    }
}
