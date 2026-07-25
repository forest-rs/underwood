// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use parley_engine::{Boundary, shape::Whitespace};

use crate::line_former::*;

fn cluster(index: usize, boundary: Boundary, source_char: char) -> LogicalCluster {
    LogicalCluster {
        run: 0,
        index,
        source: index..index + 1,
        boundary,
        source_char,
        whitespace: match source_char {
            '\r' | '\n' => Whitespace::Newline,
            ' ' => Whitespace::Space,
            _ => Whitespace::None,
        },
        ligature_component: false,
        advance: 2.0,
    }
}

#[test]
fn checkpoint_restores_traversal_and_provisional_output_together() {
    let clusters = [
        cluster(0, Boundary::None, 'a'),
        cluster(1, Boundary::None, ' '),
        cluster(2, Boundary::Line, 'b'),
        cluster(3, Boundary::None, ' '),
        cluster(4, Boundary::Line, 'c'),
    ];
    let mut former = LineFormer::new(&clusters, FormationConstraint::Wrap(5.0))
        .expect("fixture facts are valid");
    let mut output = vec!["earlier"];
    let checkpoint = former.checkpoint(output.len());
    let candidate = former
        .candidate()
        .expect("candidate selection succeeds")
        .expect("candidate exists");
    assert_eq!(candidate.clusters(), 0..2);
    assert_eq!(candidate.source(), 0..2);
    assert_eq!(candidate.trailing_whitespace_clusters(), 1..2);
    assert_eq!(candidate.trailing_whitespace_advance(), 2.0);
    assert_eq!(
        former
            .commit(
                candidate,
                LineMeasurements {
                    advance: 4.0,
                    height: 10.0,
                },
                LineLimits {
                    max_advance: Some(5.0),
                    max_height: Some(12.0),
                },
            )
            .expect("candidate commit succeeds"),
        CommitOutcome::Accepted(CandidateOverflow::None)
    );
    output.push("provisional");

    former
        .restore(checkpoint, &mut output)
        .expect("checkpoint restores");
    assert_eq!(output, ["earlier"]);
    assert_eq!(
        former
            .candidate()
            .expect("candidate selection succeeds")
            .expect("candidate exists"),
        candidate
    );
    assert_eq!(former.work().restores, 1);
}

#[test]
fn line_final_expansion_retries_the_previous_legal_boundary() {
    let clusters = [
        cluster(0, Boundary::None, 'a'),
        cluster(1, Boundary::None, ' '),
        cluster(2, Boundary::Line, 'b'),
        cluster(3, Boundary::None, ' '),
        cluster(4, Boundary::Line, 'c'),
    ];
    let mut former = LineFormer::new(&clusters, FormationConstraint::Wrap(9.0))
        .expect("fixture facts are valid");
    let candidate = former
        .candidate()
        .expect("candidate selection succeeds")
        .expect("candidate exists");
    assert_eq!(candidate.clusters(), 0..4);
    let retry = match former
        .commit(
            candidate,
            LineMeasurements {
                advance: 10.0,
                height: 10.0,
            },
            LineLimits {
                max_advance: Some(9.0),
                max_height: Some(12.0),
            },
        )
        .expect("fit evaluation succeeds")
    {
        CommitOutcome::Retry(retry) => retry,
        outcome => panic!("expected retry, got {outcome:?}"),
    };
    assert_eq!(retry.clusters(), 0..2);
    assert_eq!(
        former
            .commit(
                retry,
                LineMeasurements {
                    advance: 4.0,
                    height: 10.0,
                },
                LineLimits {
                    max_advance: Some(9.0),
                    max_height: Some(12.0),
                },
            )
            .expect("retry commit succeeds"),
        CommitOutcome::Accepted(CandidateOverflow::None)
    );
    assert_eq!(
        former.work(),
        LineFormerWork {
            proposed: 2,
            rejected: 1,
            accepted: 1,
            restores: 0,
        }
    );
}

#[test]
fn a_too_short_slot_rejects_without_advancing() {
    let clusters = [cluster(0, Boundary::None, 'a')];
    let mut former = LineFormer::new(&clusters, FormationConstraint::MaxContent)
        .expect("fixture facts are valid");
    let candidate = former
        .candidate()
        .expect("candidate selection succeeds")
        .expect("candidate exists");
    assert_eq!(
        former
            .commit(
                candidate,
                LineMeasurements {
                    advance: 2.0,
                    height: 13.0,
                },
                LineLimits {
                    max_advance: None,
                    max_height: Some(12.0),
                },
            )
            .expect("fit evaluation succeeds"),
        CommitOutcome::SlotRejected
    );
    assert!(!former.is_done());
    assert_eq!(former.work().rejected, 1);
}

#[test]
fn crlf_is_one_mandatory_candidate_and_requests_an_empty_terminal_line() {
    let clusters = [
        cluster(0, Boundary::None, '\r'),
        cluster(1, Boundary::Mandatory, '\n'),
    ];
    let mut former = LineFormer::new(&clusters, FormationConstraint::MaxContent)
        .expect("fixture facts are valid");
    let candidate = former
        .candidate()
        .expect("candidate selection succeeds")
        .expect("candidate exists");
    assert_eq!(candidate.clusters(), 0..2);
    assert_eq!(candidate.source(), 0..2);
    assert_eq!(candidate.reason(), CandidateBreak::Mandatory);
    assert_eq!(candidate.trailing_whitespace_clusters(), 0..2);
    assert_eq!(candidate.trailing_whitespace_advance(), 4.0);
    assert_eq!(
        former
            .commit(
                candidate,
                LineMeasurements {
                    advance: 4.0,
                    height: 10.0,
                },
                LineLimits::default(),
            )
            .expect("candidate commit succeeds"),
        CommitOutcome::Accepted(CandidateOverflow::None)
    );
    assert!(former.needs_terminal_empty_line());
}
