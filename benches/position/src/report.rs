// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::candidate::{AppendStream, BlockedRanges, ChunkedText};
use crate::model::{
    AuthoredSpan, Bias, CanonicalBaseline, EdgeBehavior, anchor_record_bytes, authored_span_bytes,
    baseline_inline_bytes, derived_range_bytes,
};
use crate::trace::{TraceOutcome, run_semantic_suite};

const SAMPLE_COUNT: usize = 200;
const DENSE_SPAN_COUNT: usize = 1_000_000;
const EDITOR_LINE_COUNT: usize = 1_000_000;
const APPEND_BATCH_BYTES: usize = 64 * 1024;
const APPEND_BATCH_COUNT: usize = (1 << 30) / APPEND_BATCH_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    Pass,
    Fail,
    NotRun,
    Screen,
}

impl Status {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::NotRun => "NOT_RUN",
            Self::Screen => "SCREEN",
        }
    }
}

#[derive(Debug)]
struct Gate {
    id: &'static str,
    status: Status,
    observed: String,
    limit: &'static str,
    note: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct AppendMeasurement {
    p95: Duration,
    logical_bytes: usize,
    batches: usize,
    retained_batches: usize,
    maximum_records: usize,
    total_nodes: usize,
    source_bytes_copied: usize,
    maximum_unpublished_batches: usize,
}

pub(crate) fn run() -> bool {
    let traces = run_semantic_suite();
    print_metadata();
    print_traces(&traces);
    print_gates(&measure_gates());
    traces.iter().all(|trace| trace.passed)
}

fn print_metadata() {
    println!("identity-trace-v0");
    println!("candidate\tcanonical-baseline+chunked-blocked-append-forest+tree-semantics-v3");
    println!(
        "machine\t{}-{}\tallocator=system\tsamples={SAMPLE_COUNT}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("status-key\tPASS=semantic-or-complete-gate\tSCREEN=preliminary-not-proof");
}

fn print_traces(traces: &[TraceOutcome]) {
    println!("semantic_case\tstatus\tdigest\tanchors\tspans\tcopied_bytes\tdetail");
    for trace in traces {
        println!(
            "{}\t{}\t{:016x}\t{}\t{}\t{}\t{}",
            trace.id,
            if trace.passed { "PASS" } else { "FAIL" },
            trace.digest,
            trace.work.anchors_visited,
            trace.work.authored_spans_visited,
            trace.work.source_bytes_copied,
            trace.detail
        );
    }
}

fn print_gates(gates: &[Gate]) {
    println!("gate\tstatus\tobserved\tlimit\tnote");
    for gate in gates {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            gate.id,
            gate.status.as_str(),
            gate.observed,
            gate.limit,
            gate.note
        );
    }
}

fn measure_gates() -> Vec<Gate> {
    let (label_heap, snapshot_clone_shares) = label_measurements();
    let p95 = localized_edit_p95();
    let (dense_visited, dense_resolved) = dense_authored_work();
    let (editor_copied, editor_visited) = editor_local_edit_work();
    let (chunked_copied, chunk_records, chunk_snapshot_shared) = chunked_editor_work();
    let (indexed_blocks, indexed_spans, range_snapshot_shared) = indexed_dense_work();
    let append = append_gib_work();

    vec![
        Gate {
            id: "label-64-fixed-heap",
            status: Status::Screen,
            observed: format!(
                "{label_heap} byte structural lower bound; below_limit={}",
                label_heap <= 4 * 1024
            ),
            limit: "<=4096 bytes",
            note: "allocator instrumentation must include control blocks and capacity slack",
        },
        Gate {
            id: "snapshot-clone",
            status: pass_if(snapshot_clone_shares),
            observed: format!("source_arc_shared={snapshot_clone_shares} copied_bytes=0"),
            limit: "no source copy; <=256 allocated bytes",
            note: "Arc clone performs no candidate allocation",
        },
        Gate {
            id: "sparse-anchor-retained",
            status: Status::Screen,
            observed: format!("{} bytes/record", anchor_record_bytes()),
            limit: "<=128 bytes/anchor",
            note: "requires allocator measurement amortized over 10000 anchors",
        },
        Gate {
            id: "dense-authored-retained",
            status: Status::Screen,
            observed: format!("{} bytes/flat record", authored_span_bytes()),
            limit: "<=48 bytes/span",
            note: "requires allocator measurement including the eventual index",
        },
        Gate {
            id: "dense-derived-retained",
            status: Status::Screen,
            observed: format!("{} bytes/flat record", derived_range_bytes()),
            limit: "<=32 bytes/range",
            note: "revision law passes; retained allocation needs instrumentation",
        },
        Gate {
            id: "form-10k-local-edit-p95",
            status: Status::Screen,
            observed: format!(
                "{} ns; below_limit={}",
                p95.as_nanos(),
                p95 < Duration::from_millis(16)
            ),
            limit: "<16000000 ns",
            note: "no confidence interval or calibrated reference machine yet",
        },
        Gate {
            id: "dense-million-transform-work",
            status: pass_if(dense_visited <= 4_096 && dense_resolved == 0),
            observed: format!("visited={dense_visited} sparse_resolutions={dense_resolved}"),
            limit: "<=4096 index records plus overlaps; no sparse resolution",
            note: "expected baseline failure exposes the need for an indexed range set",
        },
        Gate {
            id: "editor-million-local-edit-work",
            status: pass_if(editor_copied <= 2 * 4_096 && editor_visited <= 4_096),
            observed: format!("copied_bytes={editor_copied} records={editor_visited}"),
            limit: "<=8192 changed-chunk bytes; <=4096 index records plus frontier",
            note: "expected baseline failure exposes the need for persistent chunking",
        },
        Gate {
            id: "chunked-editor-million-local-edit-work",
            status: pass_if(chunked_copied <= 2 * 4_096 && chunk_records <= 4_096),
            observed: format!(
                "copied_bytes={chunked_copied} chunk_records={chunk_records} snapshot_shared={chunk_snapshot_shared}"
            ),
            limit: "<=8192 changed-chunk bytes; <=4096 index records plus frontier",
            note: "candidate v1 structurally shares unchanged source chunks",
        },
        Gate {
            id: "indexed-dense-million-transform-work",
            status: pass_if(indexed_blocks + indexed_spans <= 4_096 && range_snapshot_shared),
            observed: format!(
                "block_records={indexed_blocks} spans={indexed_spans} snapshot_shared={range_snapshot_shared}"
            ),
            limit: "<=4096 index records plus overlaps; structurally shared snapshot",
            note: "candidate v1 shifts suffix blocks without visiting their spans",
        },
        Gate {
            id: "append-gib-publication-work",
            status: pass_if(
                append.logical_bytes == 1 << 30
                    && append.batches == APPEND_BATCH_COUNT
                    && append.retained_batches == APPEND_BATCH_COUNT
                    && append.maximum_records <= 32
                    && append.source_bytes_copied == 0,
            ),
            observed: format!(
                "logical_bytes={} batches={} retained_batches={} max_records={} total_nodes={} copied_bytes={}",
                append.logical_bytes,
                append.batches,
                append.retained_batches,
                append.maximum_records,
                append.total_nodes,
                append.source_bytes_copied,
            ),
            limit: "1 GiB in 16384 batches; <=32 metadata records/publication; zero source copy",
            note: "persistent binomial forest clones or merges one logarithmic metadata path",
        },
        Gate {
            id: "append-gib-retained-tail",
            status: pass_if(append.maximum_unpublished_batches <= 2),
            observed: format!(
                "maximum_unpublished_batches={}",
                append.maximum_unpublished_batches
            ),
            limit: "<=2 configured batches",
            note: "candidate publishes every immutable append batch immediately",
        },
        Gate {
            id: "append-gib-publication-p95",
            status: Status::Screen,
            observed: format!(
                "{} ns; below_limit={}",
                append.p95.as_nanos(),
                append.p95 < Duration::from_millis(16)
            ),
            limit: "<16000000 ns",
            note: "shared payload isolates publication metadata; reference machine and confidence method remain unratified",
        },
        Gate {
            id: "collab-rich-tree-history",
            status: Status::NotRun,
            observed: String::from("no collaboration candidate dependency approved"),
            limit: "semantic equivalence and merge/publication budgets",
            note: "Loro or another authority candidate remains a human dependency gate",
        },
    ]
}

fn label_measurements() -> (usize, bool) {
    let text = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut model = CanonicalBaseline::new(text);
    for offset in [0, 16, 32, 64] {
        model
            .create_anchor(offset, Bias::Before)
            .expect("label trace boundary is valid");
    }
    model.replace_authored(
        (0_u32..8)
            .map(|value| {
                let start = usize::try_from(value).expect("small value") * 8;
                AuthoredSpan {
                    range: start..start + 8,
                    edges: EdgeBehavior {
                        start: Bias::Before,
                        end: Bias::After,
                    },
                    value,
                }
            })
            .collect(),
    );
    let estimated_heap = baseline_inline_bytes()
        + model.text_len()
        + 4 * anchor_record_bytes()
        + 8 * authored_span_bytes();
    let (snapshot, _) = model.snapshot();
    let clone = snapshot.clone();
    (estimated_heap, snapshot.shares_text_with(&clone))
}

fn localized_edit_p95() -> Duration {
    let source = "x".repeat(10 * 1024);
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let mut model = CanonicalBaseline::new(&source);
        let at = 5_000 + sample % 100;
        let started = Instant::now();
        model
            .replace(at..at + 1, "y")
            .expect("ASCII edit boundary is valid");
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    samples[(SAMPLE_COUNT * 95).div_ceil(100) - 1]
}

fn dense_authored_work() -> (usize, usize) {
    let source = "x".repeat(DENSE_SPAN_COUNT);
    let mut model = CanonicalBaseline::new(&source);
    model.replace_authored(
        (0..DENSE_SPAN_COUNT)
            .map(|value| AuthoredSpan {
                range: value..value + 1,
                edges: EdgeBehavior {
                    start: Bias::Before,
                    end: Bias::After,
                },
                value: u32::try_from(value).expect("one million fits in u32"),
            })
            .collect(),
    );
    let middle = DENSE_SPAN_COUNT / 2;
    let edit = model
        .replace(middle..middle + 1, "y")
        .expect("ASCII edit boundary is valid");
    (edit.work.authored_spans_visited, edit.work.anchors_resolved)
}

fn editor_local_edit_work() -> (usize, usize) {
    let source = "x\n".repeat(EDITOR_LINE_COUNT);
    let mut model = CanonicalBaseline::new(&source);
    let middle = source.len() / 2;
    let edit = model
        .replace(middle..middle + 1, "y")
        .expect("ASCII edit boundary is valid");
    (
        edit.work.source_bytes_copied,
        edit.work.snapshot_records_visited,
    )
}

fn chunked_editor_work() -> (usize, usize, bool) {
    let source = "x\n".repeat(EDITOR_LINE_COUNT);
    let candidate = ChunkedText::new(&source);
    let snapshot = candidate.clone();
    let middle = source.len() / 2;
    let (edited, work) = candidate
        .replace(middle..middle + 1, "y")
        .expect("ASCII edit boundary is valid");
    assert_eq!(edited.len(), source.len(), "replacement preserves length");
    (
        work.source_bytes_copied,
        work.chunk_records_visited,
        candidate.shares_index_with(&snapshot),
    )
}

fn indexed_dense_work() -> (usize, usize, bool) {
    let spans = (0..DENSE_SPAN_COUNT)
        .map(|value| AuthoredSpan {
            range: value..value + 1,
            edges: EdgeBehavior {
                start: Bias::Before,
                end: Bias::After,
            },
            value: u32::try_from(value).expect("one million fits in u32"),
        })
        .collect();
    let candidate = BlockedRanges::new(spans);
    let snapshot = candidate.clone();
    let middle = DENSE_SPAN_COUNT / 2;
    let (edited, work) = candidate.transform(middle..middle + 1, 1);
    assert_eq!(
        edited.len(),
        DENSE_SPAN_COUNT,
        "transform preserves span count"
    );
    (
        work.block_records_visited,
        work.spans_visited,
        candidate.shares_index_with(&snapshot),
    )
}

fn append_gib_work() -> AppendMeasurement {
    let payload = Arc::<str>::from("x".repeat(APPEND_BATCH_BYTES));
    let mut stream = AppendStream::default();
    let mut samples = Vec::with_capacity(APPEND_BATCH_COUNT);
    let mut maximum_records = 0;
    let mut total_nodes = 0;
    let mut source_bytes_copied = 0;
    let mut maximum_unpublished_batches = 0;

    for _ in 0..APPEND_BATCH_COUNT {
        let started = Instant::now();
        let (next, work) = stream.append(Arc::clone(&payload));
        samples.push(started.elapsed());
        maximum_records =
            maximum_records.max(work.level_records_visited + work.node_records_created);
        total_nodes += work.node_records_created;
        source_bytes_copied += work.source_bytes_copied;
        maximum_unpublished_batches = maximum_unpublished_batches.max(work.unpublished_batches);
        stream = next;
    }
    samples.sort_unstable();

    AppendMeasurement {
        p95: samples[(APPEND_BATCH_COUNT * 95).div_ceil(100) - 1],
        logical_bytes: stream.len(),
        batches: stream.batches(),
        retained_batches: stream.retained_batches(),
        maximum_records,
        total_nodes,
        source_bytes_copied,
        maximum_unpublished_batches,
    }
}

const fn pass_if(condition: bool) -> Status {
    if condition {
        Status::Pass
    } else {
        Status::Fail
    }
}

#[cfg(test)]
mod tests {
    use super::{APPEND_BATCH_COUNT, append_gib_work, localized_edit_p95};

    #[test]
    fn p95_runner_produces_a_nonzero_observation() {
        assert!(
            !localized_edit_p95().is_zero(),
            "timer must observe the edit"
        );
    }

    #[test]
    fn append_runner_reaches_one_gib_with_bounded_structural_work() {
        let measurement = append_gib_work();
        assert_eq!(measurement.logical_bytes, 1 << 30);
        assert_eq!(measurement.batches, APPEND_BATCH_COUNT);
        assert_eq!(measurement.retained_batches, APPEND_BATCH_COUNT);
        assert!(measurement.maximum_records <= 32);
        assert_eq!(measurement.source_bytes_copied, 0);
        assert_eq!(measurement.maximum_unpublished_batches, 0);
    }
}
