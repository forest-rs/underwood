// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use parley_engine::{Boundary, shape::Whitespace};
use underwood::{FlowRegion, Rect, RegionFlow};

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
        allows_soft_wrap: true,
        allows_emergency_wrap: false,
        emergency_affects_min_content: false,
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
        CommitOutcome::Accepted
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
        CommitOutcome::Accepted
    );
    assert_eq!(
        former.work(),
        LineFormerWork {
            proposed: 2,
            rejected: 1,
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
fn height_rejection_retries_the_same_text_in_the_next_region_slot() {
    let clusters = [
        cluster(0, Boundary::None, 'a'),
        cluster(1, Boundary::None, ' '),
        cluster(2, Boundary::Line, 'b'),
    ];
    let flow = RegionFlow::new([
        FlowRegion::new(Rect::new(0.0, 0.0, 8.0, 5.0)).expect("first region is valid"),
        FlowRegion::new(Rect::new(20.0, 0.0, 28.0, 20.0)).expect("second region is valid"),
    ])
    .expect("region flow is valid");
    let mut cursor = flow.cursor();
    let first_slot = flow.slot(cursor).expect("first slot exists");
    let mut former = LineFormer::new(
        &clusters,
        FormationConstraint::Wrap(first_slot.inline_size()),
    )
    .expect("fixture facts are valid");
    let mut output = vec!["earlier"];
    let checkpoint = former.checkpoint(output.len());
    let candidate = former
        .candidate()
        .expect("candidate selection succeeds")
        .expect("candidate exists");
    output.push("provisional");
    assert_eq!(
        former
            .commit(
                candidate,
                LineMeasurements {
                    advance: 6.0,
                    height: 10.0,
                },
                LineLimits {
                    max_advance: Some(first_slot.inline_size()),
                    max_height: Some(first_slot.block_size()),
                },
            )
            .expect("height evaluation succeeds"),
        CommitOutcome::SlotRejected
    );
    former
        .restore(checkpoint, &mut output)
        .expect("text and provisional output restore together");
    cursor = flow
        .reject(cursor, first_slot)
        .expect("rejected slot advances region geometry");

    let second_slot = flow.slot(cursor).expect("second slot exists");
    former
        .set_constraint(FormationConstraint::Wrap(second_slot.inline_size()))
        .expect("slot width is a valid constraint");
    assert_eq!(
        former
            .candidate()
            .expect("retry selection succeeds")
            .expect("retry candidate exists"),
        candidate
    );
    assert_eq!(
        former
            .commit(
                candidate,
                LineMeasurements {
                    advance: 6.0,
                    height: 10.0,
                },
                LineLimits {
                    max_advance: Some(second_slot.inline_size()),
                    max_height: Some(second_slot.block_size()),
                },
            )
            .expect("retry evaluation succeeds"),
        CommitOutcome::Accepted
    );
    assert_eq!(output, ["earlier"]);
    assert!(former.is_done());
}

#[test]
fn wrap_policy_distinguishes_soft_emergency_and_intrinsic_breaks() {
    let mut no_wrap = [
        cluster(0, Boundary::None, 'a'),
        cluster(1, Boundary::Line, 'b'),
        cluster(2, Boundary::Line, 'c'),
    ];
    for cluster in &mut no_wrap {
        cluster.allows_soft_wrap = false;
    }
    let mut former =
        LineFormer::new(&no_wrap, FormationConstraint::Wrap(3.0)).expect("no-wrap facts are valid");
    assert_eq!(
        former
            .candidate()
            .expect("candidate selection succeeds")
            .expect("candidate exists")
            .clusters(),
        0..3
    );

    let mut anywhere = [
        cluster(0, Boundary::None, 'a'),
        cluster(1, Boundary::None, 'b'),
        cluster(2, Boundary::None, 'c'),
    ];
    for cluster in &mut anywhere {
        cluster.allows_emergency_wrap = true;
        cluster.emergency_affects_min_content = true;
    }
    let mut former = LineFormer::new(&anywhere, FormationConstraint::Wrap(3.0))
        .expect("emergency-wrap facts are valid");
    assert_eq!(
        former
            .candidate()
            .expect("candidate selection succeeds")
            .expect("candidate exists")
            .clusters(),
        0..1
    );
    let mut former = LineFormer::new(&anywhere, FormationConstraint::MinContent)
        .expect("anywhere min-content facts are valid");
    assert_eq!(
        former
            .candidate()
            .expect("candidate selection succeeds")
            .expect("candidate exists")
            .clusters(),
        0..1
    );

    for cluster in &mut anywhere {
        cluster.emergency_affects_min_content = false;
    }
    let mut former = LineFormer::new(&anywhere, FormationConstraint::MinContent)
        .expect("break-word min-content facts are valid");
    assert_eq!(
        former
            .candidate()
            .expect("candidate selection succeeds")
            .expect("candidate exists")
            .clusters(),
        0..3
    );
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
        CommitOutcome::Accepted
    );
    assert!(former.needs_terminal_empty_line());
}
