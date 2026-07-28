// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resumable line formation over shaped-text facts.
//!
//! This module deliberately knows nothing about Underwood documents, scenes,
//! shaping orchestration, or rendering. It turns an immutable sequence of
//! Parley-derived cluster facts into reversible line candidates. Callers own
//! line-final shaping and any provisional output.
//! Inputs are crate-private products of validated collection and constraint
//! types; this kernel does not re-prove those invariants.

use alloc::vec::Vec;
use core::ops::Range;

use parley_engine::{Boundary, shape::Whitespace};

#[derive(Clone, Debug)]
pub(crate) struct LogicalCluster {
    pub(crate) run: usize,
    pub(crate) index: usize,
    pub(crate) source: Range<usize>,
    pub(crate) boundary: Boundary,
    pub(crate) source_char: char,
    pub(crate) whitespace: Whitespace,
    pub(crate) ligature_component: bool,
    pub(crate) allows_soft_wrap: bool,
    pub(crate) allows_emergency_wrap: bool,
    pub(crate) emergency_affects_min_content: bool,
    pub(crate) advance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FormationConstraint {
    MinContent,
    MaxContent,
    Wrap(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CandidateBreak {
    Regular,
    Mandatory,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineCandidate {
    start: usize,
    end: usize,
    source_start: usize,
    source_end: usize,
    reason: CandidateBreak,
    canonical_advance: f64,
}

impl LineCandidate {
    pub(crate) const fn clusters(self) -> Range<usize> {
        self.start..self.end
    }

    pub(crate) const fn end(self) -> usize {
        self.end
    }

    pub(crate) const fn source(self) -> Range<usize> {
        self.source_start..self.source_end
    }

    pub(crate) const fn reason(self) -> CandidateBreak {
        self.reason
    }

    pub(crate) const fn canonical_advance(self) -> f64 {
        self.canonical_advance
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineMeasurements {
    pub(crate) advance: f64,
    pub(crate) height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LineLimits {
    pub(crate) max_advance: Option<f64>,
    pub(crate) max_height: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CommitOutcome {
    Accepted,
    Retry(LineCandidate),
    SlotRejected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LineFormerWork {
    pub(crate) proposed: u32,
    pub(crate) rejected: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineCheckpoint {
    cursor: usize,
    last_break: Option<CandidateBreak>,
    output_len: usize,
}

#[derive(Debug)]
pub(crate) struct LineFormer<'a> {
    clusters: &'a [LogicalCluster],
    constraint: FormationConstraint,
    cursor: usize,
    last_break: Option<CandidateBreak>,
    work: LineFormerWork,
}

impl<'a> LineFormer<'a> {
    pub(crate) fn new(clusters: &'a [LogicalCluster], constraint: FormationConstraint) -> Self {
        Self::at(clusters, constraint, 0)
    }

    pub(crate) fn at(
        clusters: &'a [LogicalCluster],
        constraint: FormationConstraint,
        cursor: usize,
    ) -> Self {
        Self {
            clusters,
            constraint,
            cursor,
            last_break: None,
            work: LineFormerWork::default(),
        }
    }

    pub(crate) const fn is_done(&self) -> bool {
        self.cursor == self.clusters.len()
    }

    pub(crate) const fn needs_terminal_empty_line(&self) -> bool {
        self.is_done() && matches!(self.last_break, Some(CandidateBreak::Mandatory))
    }

    pub(crate) const fn work(&self) -> LineFormerWork {
        self.work
    }

    /// Changes only the limits used to choose the next candidate.
    ///
    /// Traversal, prior-break state, and accumulated work remain intact so a
    /// caller can retry the same text in a different region slot.
    pub(crate) fn set_constraint(&mut self, constraint: FormationConstraint) {
        self.constraint = constraint;
    }

    pub(crate) const fn checkpoint(&self, output_len: usize) -> LineCheckpoint {
        LineCheckpoint {
            cursor: self.cursor,
            last_break: self.last_break,
            output_len,
        }
    }

    pub(crate) fn restore<T>(&mut self, checkpoint: LineCheckpoint, output: &mut Vec<T>) {
        self.cursor = checkpoint.cursor;
        self.last_break = checkpoint.last_break;
        output.truncate(checkpoint.output_len);
    }

    pub(crate) fn candidate(&mut self) -> Option<LineCandidate> {
        if self.is_done() {
            return None;
        }
        let candidate = choose_candidate(self.clusters, self.cursor, self.constraint);
        self.work.proposed = self.work.proposed.saturating_add(1);
        Some(candidate)
    }

    pub(crate) fn commit(
        &mut self,
        candidate: LineCandidate,
        measurements: LineMeasurements,
        limits: LineLimits,
    ) -> CommitOutcome {
        if limits
            .max_height
            .is_some_and(|height| measurements.height > height)
        {
            self.work.rejected = self.work.rejected.saturating_add(1);
            return CommitOutcome::SlotRejected;
        }

        let inline_overflow = limits
            .max_advance
            .is_some_and(|advance| measurements.advance > advance);
        if candidate.reason == CandidateBreak::Regular
            && inline_overflow
            && let Some(retry) = self.retry_before(candidate)
        {
            self.work.rejected = self.work.rejected.saturating_add(1);
            self.work.proposed = self.work.proposed.saturating_add(1);
            return CommitOutcome::Retry(retry);
        }

        self.cursor = candidate.end;
        self.last_break = Some(candidate.reason);
        CommitOutcome::Accepted
    }

    fn retry_before(&self, candidate: LineCandidate) -> Option<LineCandidate> {
        let end = (candidate.start + 1..candidate.end).rev().find(|&index| {
            is_soft_boundary(&self.clusters[index])
                || is_emergency_boundary(&self.clusters[index], self.constraint)
        })?;
        let canonical_advance = self.clusters[candidate.start..end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum();
        Some(make_candidate(
            self.clusters,
            candidate.start,
            end,
            CandidateBreak::Regular,
            canonical_advance,
        ))
    }
}

fn is_soft_boundary(cluster: &LogicalCluster) -> bool {
    cluster.allows_soft_wrap && cluster.boundary == Boundary::Line && !cluster.ligature_component
}

fn is_emergency_boundary(cluster: &LogicalCluster, constraint: FormationConstraint) -> bool {
    !cluster.ligature_component
        && cluster.allows_emergency_wrap
        && match constraint {
            FormationConstraint::Wrap(_) => true,
            FormationConstraint::MinContent => cluster.emergency_affects_min_content,
            FormationConstraint::MaxContent => false,
        }
}

fn choose_candidate(
    clusters: &[LogicalCluster],
    start: usize,
    constraint: FormationConstraint,
) -> LineCandidate {
    let mut index = start;
    let mut advance = 0.0_f64;
    let mut last_opportunity: Option<(usize, f64)> = None;
    while index < clusters.len() {
        let cluster = &clusters[index];
        if index > start {
            let soft = is_soft_boundary(cluster);
            let emergency = is_emergency_boundary(cluster, constraint);
            if constraint == FormationConstraint::MinContent && (soft || emergency) {
                return make_candidate(clusters, start, index, CandidateBreak::Regular, advance);
            }
            if matches!(constraint, FormationConstraint::Wrap(_)) && soft {
                last_opportunity = Some((index, advance));
            }
        }

        let next_advance = advance + cluster.advance;
        if matches!(constraint, FormationConstraint::Wrap(width) if next_advance > width)
            && index > start
        {
            if let Some((end, opportunity_advance)) = last_opportunity {
                return make_candidate(
                    clusters,
                    start,
                    end,
                    CandidateBreak::Regular,
                    opportunity_advance,
                );
            }
            if is_emergency_boundary(cluster, constraint) {
                return make_candidate(clusters, start, index, CandidateBreak::Regular, advance);
            }
        }
        if cluster.whitespace == Whitespace::Newline {
            advance = next_advance;
            index += 1;
            let cr_before_lf = cluster.source_char == '\r'
                && clusters
                    .get(index)
                    .is_some_and(|next| next.source_char == '\n');
            if cr_before_lf {
                continue;
            }
            return make_candidate(clusters, start, index, CandidateBreak::Mandatory, advance);
        }

        advance = next_advance;
        index += 1;
    }
    make_candidate(
        clusters,
        start,
        clusters.len(),
        CandidateBreak::End,
        advance,
    )
}

fn make_candidate(
    clusters: &[LogicalCluster],
    start: usize,
    end: usize,
    reason: CandidateBreak,
    canonical_advance: f64,
) -> LineCandidate {
    LineCandidate {
        start,
        end,
        source_start: clusters[start].source.start,
        source_end: clusters[end - 1].source.end,
        reason,
        canonical_advance,
    }
}
