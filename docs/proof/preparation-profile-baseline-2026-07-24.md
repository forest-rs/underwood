# Preparation CPU and memory baseline — 2026-07-24

## Status

This is a machine-local baseline of Underwood's real public `TextBlock` path,
not a cross-machine performance claim. It establishes isolated workloads and
records the first CPU and residency evidence before source projection,
cross-identity reuse, or scratch behavior changes.

Allocation call and byte evidence is now executable through macOS full malloc
stack logging and `malloc_history`. The checked-in benchmark script measures
one isolated operation against an exactly matched setup process, using
equal-length scenario codes so process argument storage cancels from the
difference. Two consecutive complete runs produced byte-for-byte identical
results. No allocator wrapper, dependency, or Underwood-owned `unsafe`
instrumentation was introduced.

## Metric and hypothesis

Primary metrics:

- release nanoseconds per prepared label;
- exact stage-work laws reported by the public output;
- sampled owned CPU stacks;
- maximum resident set size and peak memory footprint;
- retained cache entries and evictions.

Hypotheses:

1. Distinct identities containing identical text repeat analysis, font
   selection, shaping, interaction construction, and geometry work.
2. Stable identity reuse is materially cheaper but still materializes owned
   scene output on every call.
3. Width churn correctly retains canonical shaping but line-final shaping and
   geometry remain significant.
4. Explicit cache budgets bound identity churn even before cross-identity reuse
   exists.

## Reproduction

Build:

```sh
cargo build --release -p underwood_label_benchmark
```

Full correctness and timing suite:

```sh
cargo run --release -p underwood_label_benchmark
```

Isolated scenarios:

```sh
./target/release/underwood_label_benchmark cold-identical 500
./target/release/underwood_label_benchmark retained-identical 500
./target/release/underwood_label_benchmark paint-change 500
./target/release/underwood_label_benchmark localized-edit 100
./target/release/underwood_label_benchmark interaction-materialization 100
./target/release/underwood_label_benchmark width-churn 200
./target/release/underwood_label_benchmark region-ready 200
./target/release/underwood_label_benchmark identity-churn 200
```

The optional third argument limits labels per round. That makes one-operation
external allocation traces practical without changing the ordinary 2,048-label
timing workloads.

Allocation calls and allocated bytes on macOS:

```sh
cargo build --release -p underwood_label_benchmark
benches/labels/profile-allocations.sh
```

The optional `UNDERWOOD_PROFILE_HOLD_SECS` environment variable holds the
completed process outside the measured interval for process-attached tools.
`setup-identical` constructs fonts, the engine, and 2,048 blocks without
preparing them, providing a residency baseline.

CPU sampling:

```sh
./target/release/underwood_label_benchmark cold-identical 500
sample PID 5 -file /tmp/underwood-cold-identical.sample.txt
```

Peak residency:

```sh
/usr/bin/time -l \
  ./target/release/underwood_label_benchmark cold-identical 1
```

## Host

- Architecture: Apple silicon ARM64
- Operating system: macOS 26.5.2
- Rust profile: repository `release`
- Font policy: checked-in Roboto Flex and Noto Kufi Arabic, system discovery
  disabled
- Labels per round: 2,048
- Churn cache budget: 64 paragraphs

## Timing baseline

| Workload | Rounds | Operations | ns/label |
| --- | ---: | ---: | ---: |
| Cold identical | 500 | 1,024,000 | 22,979 |
| Retained identical | 500 | 1,024,000 | 5,956 |
| Paint change | 500 | 1,024,000 | 5,929 |
| Localized edit | 100 | 204,800 | 29,456 |
| Interaction materialization | 100 | 204,800 | 22,474 |
| Width churn | 200 | 409,600 | 24,650 |
| Region-ready width reformation | 200 | 409,600 | 24,435 |
| Identity churn | 200 | 409,600 | 43,457 |

The cold-identical path is 3.86 times the retained-identical path on this run.
That ratio does not predict the final cross-identity speedup: distinct labels
must still rebind semantic identity and materialize their own interaction and
scene records.

The original mixed suite on the same checkout reported:

| Workload | ns/label |
| --- | ---: |
| Cold unique | 27,571 |
| Retained unique | 8,004 |
| Constrained unique | 24,761 |
| Explicit release | 5,502 |
| Cold identical | 22,515 |
| Retained identical | 6,493 |
| Budget churn | 33,843 |

## Work laws

The benchmark asserts:

- cold distinct identities analyze and shape exactly one paragraph each;
- retained identities report zero analysis, itemization, font selection,
  canonical shaping, line shaping, flow, and geometry work;
- width-only changes report zero analysis, itemization, font selection, and
  canonical shaping while reforming exactly one paragraph;
- one edited block reanalyzes and reshapes exactly one paragraph;
- cache residency never exceeds the configured budget after enforcement;
- 2,048 transient identities under a 64-entry budget produce 1,984 evictions,
  a transient peak of 65, and zero final entries after clearing.

These assertions are more portable than machine-local time.

## CPU sample

The five-second cold-identical sample captured 4,176 main-thread samples.
Material owned by or directly selected by Underwood included:

- `LayoutEngine::prepare` and `prepare_paragraph_geometry`;
- `ParleyParagraphEngine::form`;
- `prepared_cursor_movements` and visual cursor-cluster construction;
- `build_geometry`, cached cursor steps, and source-position projection;
- prepared-output validation and visual-unit lowering;
- Parley Engine analysis, font matching, HarfRust shaping, and shaped-run
  construction;
- allocator growth, reallocation, free, zeroing, and memory movement beneath
  several of those paths.

The top-of-stack view included 96 samples in `build_geometry`, 69 in
`cached_cursor_step`, 55 in projected position resolution, 50 in the paragraph
adapter, 48 in cursor-step materialization, 34 in Parley Engine shaping, and
multiple allocator and vector-growth sites.

The sample supports both leading hypotheses: repeated text physics is real, and
identity-bound interaction/geometry work is large enough that a shared shaping
cache alone will not make distinct labels equivalent to retained calls.

## Residency baseline

| Workload | Maximum resident set | Peak memory footprint |
| --- | ---: | ---: |
| Setup only, 2,048 blocks | 7,012,352 bytes | 4,211,000 bytes |
| Cold identical, 2,048 retained entries | 102,989,824 bytes | 99,598,696 bytes |
| Retained identical, including priming | 102,924,288 bytes | 99,533,160 bytes |
| Identity churn, 64-entry budget | 13,565,952 bytes | 10,207,544 bytes |

This is process residency, not retained object size. It includes fonts, allocator
behavior, code, and profiler-visible runtime state. The bounded-churn result is
nevertheless useful: coordinated eviction prevents residency from approaching
the 2,048-entry retained case.

## Allocation baseline

| Workload | Allocation calls | Allocated bytes |
| --- | ---: | ---: |
| Cold identical | 542 | 130,464 |
| Retained identical | 247 | 26,640 |
| Paint change | 247 | 26,640 |
| Localized edit | 739 | 158,317 |
| Interaction materialization | 542 | 130,464 |
| Width churn | 181 | 25,592 |
| Region-ready width reformation | 181 | 25,592 |
| Identity churn including block creation | 1,009 | 225,921 |

The script launches a full-logging setup and workload process for one label,
aggregates `ALLOC`, `CALLOC`, and `REALLOC` event sizes, then subtracts the
matched setup. It excludes VM allocation events and does not claim portable
allocator behavior. `MallocStackLogging=1` alone selected compact/live mode on
this host and was rejected; `MallocStackLogging=full` is required.

The `region-ready` workload is deliberately the current finite-width
reformation path. It establishes the pre-region baseline for the same
shape/formation/geometry work that region slots will later drive; it does not
pretend that exclusion or column policy exists already.

The implemented exclusion, float, and column result retains this historical
baseline and compares real `region-churn` against it in
`region-flow-review-2026-07-25.md`.

The counts expose an important baseline rather than a success: even retained
and paint-only calls allocate 247 times because they rematerialize an owned
scene. Cold interaction construction allocates more than twice as often, and
identity churn crosses one thousand allocation calls per created/prepared
block. No reusable public scratch-capacity diagnostic exists yet. The repeated
growth is observable here through allocation events and in the CPU stacks;
stage-owned scratch capacity and growth reporting remain the explicit
`und-oh0.13.11` deliverable.

## Ranked candidate work

1. **Identity-free preparation reuse.** Share only immutable projection,
   analysis, font selection, shaping, and eligible formation facts, then rebind
   all document, revision, semantic, interaction, paint, and placement state.
2. **Selective non-editable materialization.** Measure whether static labels can
   omit cursor-movement and selection structures while retaining the same
   prepared physics and scene vocabulary.
3. **Geometry scratch and exact reservation.** Reuse or reserve the vectors
   implicated by cursor and geometry construction after allocation counts are
   available.
4. **Cache indexing.** Compare current ordered maps with alternatives only after
   owned stage cost falls enough for lookup structure to matter.

No optimization lands from this ranking without a before/after workload and
unchanged correctness laws.
