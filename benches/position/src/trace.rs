// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::model::{AuthoredSpan, Bias, CanonicalBaseline, EdgeBehavior, ModelError, WorkCounters};
use crate::tree::{NodeId, TreeBaseline, TreeError};

const TRACE_SCHEMA: &str = "identity-trace-v0";
const TRACE_SEED: u64 = 0x5EED_0000_0000_0001;

#[derive(Debug)]
pub(crate) struct TraceOutcome {
    pub(crate) id: &'static str,
    pub(crate) passed: bool,
    pub(crate) digest: u64,
    pub(crate) work: WorkCounters,
    pub(crate) detail: String,
}

pub(crate) fn run_semantic_suite() -> Vec<TraceOutcome> {
    vec![
        insertion_bias(),
        deletion_then_insertion_bias(),
        immutable_snapshot(),
        stale_derived_range(),
        authored_edge_behavior(),
        tree_split_join_anchor_lifecycle(),
        tree_move_delete_anchor_lifecycle(),
        deterministic_replay(),
    ]
}

fn insertion_bias() -> TraceOutcome {
    let mut model = CanonicalBaseline::new("ab");
    let before = model
        .create_anchor(1, Bias::Before)
        .expect("trace anchor is valid");
    let after = model
        .create_anchor(1, Bias::After)
        .expect("trace anchor is valid");
    let edit = model.replace(1..1, "X").expect("trace edit is valid");
    let before_at = model
        .resolve_anchor(before)
        .expect("anchor remains resolved");
    let after_at = model
        .resolve_anchor(after)
        .expect("anchor remains resolved");
    let passed = model.text_len() == 3 && before_at == 1 && after_at == 2;
    TraceOutcome {
        id: "anchor-insert-bias",
        passed,
        digest: digest_observation(&model, &[before_at, after_at]),
        work: edit.work,
        detail: format!("text=3 bytes before={before_at} after={after_at}"),
    }
}

fn deletion_then_insertion_bias() -> TraceOutcome {
    let mut model = CanonicalBaseline::new("abcdef");
    let before = model
        .create_anchor(3, Bias::Before)
        .expect("trace anchor is valid");
    let after = model
        .create_anchor(3, Bias::After)
        .expect("trace anchor is valid");
    let delete = model.replace(2..5, "").expect("trace deletion is valid");
    let collapsed_before = model
        .resolve_anchor(before)
        .expect("anchor remains resolved");
    let collapsed_after = model
        .resolve_anchor(after)
        .expect("anchor remains resolved");
    let insert = model.replace(2..2, "Z").expect("trace insertion is valid");
    let final_before = model
        .resolve_anchor(before)
        .expect("anchor remains resolved");
    let final_after = model
        .resolve_anchor(after)
        .expect("anchor remains resolved");
    let passed =
        collapsed_before == 2 && collapsed_after == 2 && final_before == 2 && final_after == 3;
    TraceOutcome {
        id: "anchor-delete-collapse-bias",
        passed,
        digest: digest_observation(&model, &[final_before, final_after]),
        work: add_work(delete.work, insert.work),
        detail: format!(
            "collapsed=({collapsed_before},{collapsed_after}) final=({final_before},{final_after})"
        ),
    }
}

fn immutable_snapshot() -> TraceOutcome {
    let mut model = CanonicalBaseline::new("before");
    let (old, publish_work) = model.snapshot();
    let clone = old.clone();
    let edit = model
        .replace(0..6, "after")
        .expect("trace replacement is valid");
    let (new, _) = model.snapshot();
    let passed = old.text() == "before"
        && clone.text() == "before"
        && new.text() == "after"
        && old.shares_text_with(&clone)
        && !old.shares_text_with(&new);
    TraceOutcome {
        id: "immutable-snapshot",
        passed,
        digest: digest_bytes(old.text().as_bytes()) ^ digest_bytes(new.text().as_bytes()),
        work: add_work(publish_work, edit.work),
        detail: format!(
            "old={} new={} clone-shares={} next-shares={}",
            old.text(),
            new.text(),
            old.shares_text_with(&clone),
            old.shares_text_with(&new)
        ),
    }
}

fn stale_derived_range() -> TraceOutcome {
    let mut model = CanonicalBaseline::new("abcdef");
    let range = model.snapshot_range(1..3);
    let before = model
        .resolve_snapshot_range(&range)
        .expect("current range resolves")
        .to_owned();
    let edit = model.replace(0..0, "Z").expect("trace edit is valid");
    let stale = model.resolve_snapshot_range(&range);
    let passed = before == "bc" && matches!(stale, Err(ModelError::StaleRange { .. }));
    TraceOutcome {
        id: "derived-range-revision",
        passed,
        digest: digest_bytes(before.as_bytes()),
        work: edit.work,
        detail: format!("initial={before} stale={stale:?}"),
    }
}

fn authored_edge_behavior() -> TraceOutcome {
    let mut model = CanonicalBaseline::new("abcdef");
    model.replace_authored(vec![
        AuthoredSpan {
            range: 2..4,
            edges: EdgeBehavior {
                start: Bias::Before,
                end: Bias::Before,
            },
            value: 1,
        },
        AuthoredSpan {
            range: 2..4,
            edges: EdgeBehavior {
                start: Bias::After,
                end: Bias::After,
            },
            value: 2,
        },
    ]);
    let edit = model.replace(2..2, "X").expect("trace edit is valid");
    let spans = model.authored();
    let passed = spans
        == [
            AuthoredSpan {
                range: 2..5,
                edges: EdgeBehavior {
                    start: Bias::Before,
                    end: Bias::Before,
                },
                value: 1,
            },
            AuthoredSpan {
                range: 3..5,
                edges: EdgeBehavior {
                    start: Bias::After,
                    end: Bias::After,
                },
                value: 2,
            },
        ]
        && edit.work.anchors_resolved == 0;
    TraceOutcome {
        id: "authored-edge-transform",
        passed,
        digest: digest_spans(spans),
        work: edit.work,
        detail: format!(
            "spans={spans:?} sparse-resolutions={}",
            edit.work.anchors_resolved
        ),
    }
}

fn tree_split_join_anchor_lifecycle() -> TraceOutcome {
    let left = NodeId::new(10);
    let tail = NodeId::new(20);
    let right = NodeId::new(30);
    let mut tree = TreeBaseline::new([(left, "abcd"), (tail, "ef")]);
    let before = tree
        .create_anchor(left, 2, Bias::Before)
        .expect("trace tree anchor is valid");
    let after = tree
        .create_anchor(left, 2, Bias::After)
        .expect("trace tree anchor is valid");
    let suffix = tree
        .create_anchor(left, 3, Bias::Before)
        .expect("trace tree anchor is valid");

    tree.split(left, 2, right).expect("trace split is valid");
    let split_before = tree
        .resolve_anchor(before)
        .expect("before anchor remains on the left");
    let split_after = tree
        .resolve_anchor(after)
        .expect("after anchor follows the right");
    let split_suffix = tree
        .resolve_anchor(suffix)
        .expect("suffix anchor follows the right");
    let split_order = tree.node_order();
    let split_digest = digest_tree_observation(
        &tree,
        &[
            Some((split_before.node, split_before.offset, split_before.bias)),
            Some((split_after.node, split_after.offset, split_after.bias)),
            Some((split_suffix.node, split_suffix.offset, split_suffix.bias)),
        ],
    );

    tree.join(left, right).expect("trace join is valid");
    let joined_before = tree
        .resolve_anchor(before)
        .expect("before anchor survives the join");
    let joined_after = tree
        .resolve_anchor(after)
        .expect("after anchor survives the join");
    let joined_suffix = tree
        .resolve_anchor(suffix)
        .expect("suffix anchor survives the join");
    let joined_order = tree.node_order();

    let passed = split_before.node == left
        && split_before.offset == 2
        && split_before.bias == Bias::Before
        && split_after.node == right
        && split_after.offset == 0
        && split_after.bias == Bias::After
        && split_suffix.node == right
        && split_suffix.offset == 1
        && split_order == [left, right, tail]
        && joined_before.node == left
        && joined_before.offset == 2
        && joined_before.bias == Bias::Before
        && joined_after.node == left
        && joined_after.offset == 2
        && joined_after.bias == Bias::After
        && joined_suffix.node == left
        && joined_suffix.offset == 3
        && tree.text(left) == Ok("abcd")
        && joined_order == [left, tail];
    let joined_digest = digest_tree_observation(
        &tree,
        &[
            Some((joined_before.node, joined_before.offset, joined_before.bias)),
            Some((joined_after.node, joined_after.offset, joined_after.bias)),
            Some((joined_suffix.node, joined_suffix.offset, joined_suffix.bias)),
        ],
    );
    TraceOutcome {
        id: "tree-split-join-anchor",
        passed,
        digest: mix(split_digest, joined_digest),
        work: WorkCounters::default(),
        detail: format!(
            "split_order={split_order:?} split=({split_before:?},{split_after:?},{split_suffix:?}) joined_order={joined_order:?} joined=({joined_before:?},{joined_after:?},{joined_suffix:?})"
        ),
    }
}

fn tree_move_delete_anchor_lifecycle() -> TraceOutcome {
    let first = NodeId::new(10);
    let moved = NodeId::new(20);
    let last = NodeId::new(30);
    let mut tree = TreeBaseline::new([(first, "a"), (moved, "β"), (last, "c")]);
    let anchor = tree
        .create_anchor(moved, "β".len(), Bias::After)
        .expect("trace tree anchor is valid");

    tree.move_before(moved, first).expect("trace move is valid");
    let after_move = tree
        .resolve_anchor(anchor)
        .expect("anchor follows the moved node");
    let moved_order = tree.node_order();
    let moved_digest = digest_tree_observation(
        &tree,
        &[Some((after_move.node, after_move.offset, after_move.bias))],
    );
    tree.delete(moved).expect("trace delete is valid");
    let after_delete = tree.resolve_anchor(anchor);
    let deleted_order = tree.node_order();
    let deleted_digest = digest_tree_observation(&tree, &[None]);

    let passed = moved_order == [moved, first, last]
        && after_move.node == moved
        && after_move.offset == "β".len()
        && after_move.bias == Bias::After
        && matches!(after_delete, Err(TreeError::UnresolvedAnchor(token)) if token == anchor)
        && deleted_order == [first, last];
    TraceOutcome {
        id: "tree-move-delete-anchor",
        passed,
        digest: mix(moved_digest, deleted_digest),
        work: WorkCounters::default(),
        detail: format!(
            "moved_order={moved_order:?} moved={after_move:?} deleted_order={deleted_order:?} deleted={after_delete:?}"
        ),
    }
}

fn deterministic_replay() -> TraceOutcome {
    let left = replay_digest();
    let right = replay_digest();
    TraceOutcome {
        id: "deterministic-replay",
        passed: left == right,
        digest: left,
        work: WorkCounters::default(),
        detail: format!(
            "schema={TRACE_SCHEMA} seed={TRACE_SEED:#018x} digests={left:016x}/{right:016x}"
        ),
    }
}

fn digest_tree_observation(tree: &TreeBaseline, anchors: &[Option<(NodeId, usize, Bias)>]) -> u64 {
    let mut hash = digest_bytes(TRACE_SCHEMA.as_bytes()) ^ TRACE_SEED;
    for node in tree.node_order() {
        hash = mix(hash, u64::from(node.get()));
        hash = mix(
            hash,
            digest_bytes(
                tree.text(node)
                    .expect("tree observation contains a known node")
                    .as_bytes(),
            ),
        );
    }
    for anchor in anchors {
        match anchor {
            Some((node, offset, bias)) => {
                hash = mix(hash, u64::from(node.get()));
                hash = mix(hash, usize_tag(*offset));
                hash = mix(hash, bias_tag(*bias));
            }
            None => hash = mix(hash, u64::MAX),
        }
    }
    hash
}

fn replay_digest() -> u64 {
    let mut model = CanonicalBaseline::new("alpha beta");
    let before = model
        .create_anchor(5, Bias::Before)
        .expect("trace anchor is valid");
    let after = model
        .create_anchor(5, Bias::After)
        .expect("trace anchor is valid");
    model.replace_authored(vec![AuthoredSpan {
        range: 0..5,
        edges: EdgeBehavior {
            start: Bias::Before,
            end: Bias::After,
        },
        value: 7,
    }]);
    model
        .replace(5..5, " retained")
        .expect("trace edit is valid");
    let positions = [
        model
            .resolve_anchor(before)
            .expect("anchor remains resolved"),
        model
            .resolve_anchor(after)
            .expect("anchor remains resolved"),
    ];
    digest_observation(&model, &positions)
}

fn digest_observation(model: &CanonicalBaseline, positions: &[usize]) -> u64 {
    let (snapshot, _) = model.snapshot();
    let mut hash = digest_bytes(TRACE_SCHEMA.as_bytes()) ^ TRACE_SEED;
    hash = mix(hash, digest_bytes(snapshot.text().as_bytes()));
    hash = mix(hash, digest_spans(snapshot.authored()));
    hash = mix(hash, revision_tag(snapshot.revision()));
    for position in positions {
        hash = mix(hash, usize_tag(*position));
    }
    hash
}

fn digest_spans(spans: &[AuthoredSpan]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for span in spans {
        hash = mix(hash, usize_tag(span.range.start));
        hash = mix(hash, usize_tag(span.range.end));
        hash = mix(hash, u64::from(span.value));
        hash = mix(hash, bias_tag(span.edges.start));
        hash = mix(hash, bias_tag(span.edges.end));
    }
    hash
}

fn revision_tag(revision: crate::model::Revision) -> u64 {
    revision.get()
}

fn usize_tag(value: usize) -> u64 {
    u64::try_from(value).expect("trace offsets fit in u64")
}

const fn bias_tag(bias: Bias) -> u64 {
    match bias {
        Bias::Before => 0,
        Bias::After => 1,
    }
}

fn digest_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn mix(left: u64, right: u64) -> u64 {
    left.rotate_left(5) ^ right.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

const fn add_work(left: WorkCounters, right: WorkCounters) -> WorkCounters {
    WorkCounters {
        anchors_visited: left.anchors_visited + right.anchors_visited,
        anchors_resolved: left.anchors_resolved + right.anchors_resolved,
        authored_spans_visited: left.authored_spans_visited + right.authored_spans_visited,
        source_bytes_copied: left.source_bytes_copied + right.source_bytes_copied,
        snapshot_records_visited: left.snapshot_records_visited + right.snapshot_records_visited,
    }
}

#[cfg(test)]
mod tests {
    use super::run_semantic_suite;

    #[test]
    fn dependency_free_semantic_suite_is_deterministic_and_passes() {
        let first = run_semantic_suite();
        let second = run_semantic_suite();
        assert!(!first.is_empty(), "semantic suite must contain traces");
        assert_eq!(first.len(), second.len());
        for (left, right) in first.iter().zip(&second) {
            assert!(left.passed, "trace {} failed: {}", left.id, left.detail);
            assert_eq!(left.id, right.id);
            assert_eq!(
                left.digest, right.digest,
                "trace {} produced a nondeterministic digest",
                left.id
            );
        }
    }
}
