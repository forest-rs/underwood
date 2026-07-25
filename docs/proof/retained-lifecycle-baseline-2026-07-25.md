# Retained lifecycle baseline — 2026-07-25

## Claim

Underwood's paragraph preparation caches avoid repeated analysis, shaping, and
formation, but the committed public document-to-scene path is not yet
incremental in total work. Exact repeats and one-paragraph edits still rebuild
per-paragraph comparison inputs and allocate fresh document output records.
Document edit staging also copies storage proportional to the number of
paragraphs.

This is a measured baseline for `und-oh0.13.17`; it is not a performance
acceptance claim.

## Workloads

`benches/semantic-scene` now exposes four isolated public-path events over the
same real mixed Latin/Arabic fixture used by its product benchmark:

- `retained`: prepare the exact same snapshot and request after priming;
- `edit-staging`: insert one ASCII byte through a scene-validated collapsed
  selection and publish, without preparing the new revision;
- `localized-prepare`: publish that one-byte edit before measurement, then
  prepare its new revision;
- `localized-edit`: measure publication and preparation together.

Every preparation assertion requires zero analysis, shaping, and formation for
an exact repeat. The localized cases require exactly one shaped paragraph and
reuse of every unchanged sibling.

The allocation build uses the optional `allocation-counting` feature of the
top-level benchmark crate. `allocation-counter` replaces the allocator only in
that benchmark binary and measures the event closure on its current thread.
The workload is single-threaded. No core crate, production feature, or public
API depends on it.

## Reproduction

CPU timing, without the instrumenting allocator:

```sh
cargo build --release -p underwood_semantic_scene_benchmark --locked
for paragraphs in 64 1000; do
  for scenario in retained edit-staging localized-prepare localized-edit; do
    target/release/underwood_semantic_scene_benchmark "$scenario" "$paragraphs"
  done
done
```

Scoped allocation counts, requested bytes, and peak/net live storage:

```sh
cargo build --release -p underwood_semantic_scene_benchmark \
  --features allocation-counting --locked
benches/semantic-scene/profile-counted-allocations.sh
```

An independent macOS cross-check remains available:

```sh
cargo build --release -p underwood_semantic_scene_benchmark --locked
benches/semantic-scene/profile-allocations.sh \
  target/release/underwood_semantic_scene_benchmark 64
```

The `malloc_history -allEvents` cross-check is deliberately not the primary
large-document instrument: exporting complete process histories becomes
prohibitively slow as the primed scene grows.

## Environment

- Apple arm64 host
- macOS 26.5.2 (25F84)
- `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Cargo `release` profile
- commit ancestry headed by `e5b8bd2`; the benchmark patch itself was
  uncommitted during measurement

Timing is machine-local. Allocation event counts and requested sizes were
identical across two consecutive instrumented runs.

## Timing

The table reports the median of three fresh-process runs in nanoseconds.

| Event | 64 paragraphs | 1,000 paragraphs |
|---|---:|---:|
| retained exact repeat | 1,170,750 | 20,332,084 |
| edit staging only | 3,333 | 21,375 |
| localized prepare only | 1,362,875 | 21,979,500 |
| localized edit + prepare | 1,311,542 | 21,022,541 |

The 1,000-paragraph exact repeat and localized prepare both cost about 20–22
milliseconds despite the latter reshaping only one paragraph. Timing variance
can reverse their ordering; the meaningful result is that both remain
document-scale.

## Allocations

The table reports exact event-scoped counts from two identical runs.
`allocated bytes` is cumulative requested storage during the event. `peak live`
and `net live` describe storage outstanding relative to the start of the
measured closure; the returned publication or scene output remains alive at the
measurement boundary.

| Event | Paragraphs | Calls | Allocated bytes | Peak live bytes | Net live bytes |
|---|---:|---:|---:|---:|---:|
| retained exact repeat | 64 | 42,607 | 10,265,160 | 6,420,320 | 5,471,112 |
| edit staging only | 64 | 79 | 7,744 | 7,432 | 7,040 |
| localized prepare only | 64 | 43,247 | 10,556,371 | 6,526,411 | 5,537,675 |
| localized edit + prepare | 64 | 43,326 | 10,564,115 | 6,533,451 | 5,544,715 |
| retained exact repeat | 1,000 | 666,167 | 163,571,752 | 102,698,816 | 86,770,440 |
| edit staging only | 1,000 | 1,015 | 105,088 | 104,776 | 104,384 |
| localized prepare only | 1,000 | 666,807 | 165,767,603 | 104,319,563 | 87,789,323 |
| localized edit + prepare | 1,000 | 667,822 | 165,872,691 | 104,423,947 | 87,893,707 |

For the 64-paragraph retained case, matched
`MallocStackLogging=full`/`malloc_history` processes reported 42,542 calls and
10,262,552 bytes. The scoped counter differs by 65 calls and 2,608 bytes
(approximately 0.15% and 0.03%), consistent with its own thread-local
measurement setup. This is close enough to validate the faster instrument
without pretending the instruments have identical overhead.

## What the numbers prove

1. **Exact-repeat publication is O(paragraphs), not O(1).** Paragraph count
   grows by 15.625× from 64 to 1,000. Retained allocation calls grow by 15.635×
   and time grows by roughly 17×.
2. **Document staging copies the paragraph spine.** One-byte edit staging grows
   from 79 to 1,015 calls, almost exactly one additional allocation per added
   paragraph. This is independent of layout.
3. **Localized preparation does not localize scene work.** At 1,000 paragraphs
   it performs 666,807 calls, only 640 more than an exact repeat. The one
   reshaped paragraph is lost in document-wide projection comparison and scene
   materialization.
4. **The output copy is not transient noise.** An exact repeat leaves about
   86.8 MB of newly allocated output storage live at the measurement boundary.
   Cache reuse is real for expensive text preparation, but the public result is
   still deep-published.
5. **The allocation audit was conservative for this corpus.** Its architectural
   diagnosis is confirmed; this richer public scene creates substantially more
   records than its headline estimate.

## Consequences

Preparation tracing is paused behind `und-oh0.13.17`. Publishing trace
vocabulary against the current rebuild/deep-compare/deep-copy lifecycle would
make an accidental implementation shape look permanent.

The next design must satisfy these separate laws:

- exact repeated snapshot and request: O(1), returning the prior immutable scene
  handle;
- same-height localized edit: changed paragraph work plus a sublinear
  persistent-scene-spine update;
- changed-height localized edit: unchanged paragraph records remain shared
  while downstream origins resolve correctly;
- global width, region, or computed-policy changes: explicit O(affected
  paragraphs) work with stage-owned reasons;
- one-byte document edit: storage proportional to touched paragraphs and
  leaves, not total document size.

A flat `Vec<Arc<ParagraphSceneSegment>>` can remove deep record copies, but it
still walks the document and clones one handle per paragraph. It is therefore a
useful representation component, not proof of O(change). Likewise, numeric
generation stamps are valid only when qualified by stable input provenance or
an immutable compiled-snapshot identity.

## Proof status

**Measured baseline.** The workload, work assertions, scoped allocation
instrument, repeatability check, and macOS cross-check are executable. No
optimization has yet earned the O(change) laws.
