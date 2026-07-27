# Preparation trace and memory accounting review — 2026-07-25

## Status

The preparation trace is implemented through the public document, composition,
and `TextBlock` paths and is measured in the label-scale wind tunnel. It is an
opt-in deterministic diagnostic, not an always-on profiler.

This proof closes `und-oh0.13.11`.

## Public contract

`WorkReport` remains available on every successful output and continues to
report exact stage work without host timing or allocator instrumentation.
Callers that need the fuller explanation opt in:

```rust
let output = layout.prepare_block(
    &block.snapshot(),
    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint)
        .with_preparation_trace(),
)?;
let trace = output.trace().expect("trace was requested");
```

`PreparationTrace` adds:

- paragraphs considered, cold identities, exact geometry reuse, shared
  preparation reuse, and adapter calls;
- formation, accepted-line adjustment, and paint invalidations;
- the complete existing `WorkReport`, including analysis, shaping, candidate
  formation, rejected candidates, checkpoint restoration, adjustment,
  geometry, and paint work;
- region-attempt and height-rejection totals, with every exact offered slot
  remaining in the replayable `RegionTranscript`;
- cache state before and after preparation;
- the capacity charge for the newly published scene;
- reusable layout scratch capacity before and after preparation.

Invalidation counters intentionally overlap. A paragraph whose alignment and
paint both change reports both facts. Exact geometry reuse, shared preparation,
and an adapter call are mutually exclusive outcomes.

An ordinary untraced request returns `None` from `SceneOutput::trace()` and
does not run the deep scene-output capacity pass or reserve the trace payload
inside the output. The output stores only a nullable shared handle; a traced
request allocates the immutable trace out of line. The always-on reuse counters
are scalar increments folded into existing cache decisions.

## Memory vocabulary

The API keeps three quantities distinct:

1. **Process allocations** are allocation calls and requested bytes observed by
   external host tooling. Underwood core does not claim to know them.
2. **Reusable scratch** is capacity owned by `LayoutEngine` for temporary work.
   The first concrete scratch buffer retains exact region attempts between
   calls. The region-flow regression proves first-use growth and zero growth
   on an equal retained request.
3. **Cache residency** is a deterministic accounting charge. Scene-cache bytes
   are maintained incrementally when entries are inserted, replaced, evicted,
   or released. Shared-preparation bytes retain their separately budgeted
   accounting. Shared font blobs, backend-private storage, allocator metadata,
   and process RSS are not silently folded into either number.

The first attempted implementation recomputed scene-cache bytes by traversing
all retained entries for every traced request. The 2,048-label wind tunnel
immediately exposed the resulting quadratic diagnostic path. The checked-in
implementation maintains the cache charge incrementally, so observing current
residency is O(1) and changing one entry charges only that entry.

## Deterministic traps

The focused corpus proves:

- cold, exact-reuse, adjustment-only, and paint-only decisions have distinct
  trace counters;
- `trace.work()` equals the always-available output work;
- cache residency grows, shrinks after explicit release, and returns to zero
  after clearing or zero-budget eviction;
- the first region request grows reusable attempt scratch;
- an equal retained region request reports the same scratch capacity and zero
  growth;
- height rejection counts agree with the exact replayable transcript;
- untraced preparation publishes no detailed trace;
- document and transient-composition output use the same trace machinery.

## Release wind tunnel

Reproduction:

```sh
cargo build --release -p underwood_label_benchmark
./target/release/underwood_label_benchmark retained-identical 100 2048
./target/release/underwood_label_benchmark traced-retained 100 2048
```

This host prepared 204,800 stable one-paragraph labels per scenario.

| Run | Untraced retained | Traced retained | Difference |
| --- | ---: | ---: | ---: |
| 1 | 6,002 ns/label | 6,015 ns/label | +13 ns, +0.22% |
| 2 | 5,901 ns/label | 5,909 ns/label | +8 ns, +0.14% |

An isolated worktree at pre-trace commit `b587e56` also checked the cost paid by
ordinary untraced callers. Alternating runs put the current retained path
0.7–1.0% above that baseline and the cold path 0.47% above it on this host.
Those small differences include incrementally maintaining the new cache-byte
charge on ordinary cache access; they are retained as a regression budget
rather than described as zero.

The final traced record reported:

- 16,584 bytes of scene-output vector capacity for one label;
- 80,005,120 bytes of accounted scene-cache residency for 2,048 labels;
- 39,065 bytes of accounted scene-cache residency for the one-label run;
- zero region-attempt scratch for the non-region label workload.

These are capacity charges, not process allocation or live-byte claims. The
existing `profile-allocations.sh` and `malloc_history` path remains the
allocation authority.

Design-0017 subsequently replaced flat output vectors with a persistent scene
spine. `scene_output_capacity_bytes` now reports only the spine-node payload
newly retained relative to a reusable prior publication. A one-paragraph scene
uses the direct segment form and reports zero tree bytes; multi-paragraph
scenes report only their persistent tree nodes. Exact shared roots also report
zero, and paragraph geometry remains in scene-cache accounting. The follow-on
law and its composition correction are recorded in
`retained-lifecycle-progress-2026-07-25.md`.

## Host timing and product path

The native showcase already measures preparation and rendering with separate
host `Instant` intervals. It now requests the detailed trace and, while a
diagnostic mode is active, shows invalidation causes, region attempts,
scene-output capacity, scene-cache residency, and scratch growth beside those
separate prepare/render times. No clock entered the `no_std` core.

## Spoor decision

Spoor does not become a production dependency in this slice.

The deterministic counters and existing macOS allocator tooling answer the
current questions without dependency or feature growth. A useful Spoor
integration would need attributed live-byte ownership across scene output,
scratch, both cache layers, backend-private state, and shared font blobs; a
generic process-memory number would add presentation without resolving that
model.

The independent P2 follow-up `und-oh0.14` owns a tooling-first prototype and
dependency review. Any production adoption remains a human dependency gate.

## Public migration

This is additive pre-stable API work:

- use `SceneRequest::with_preparation_trace()` or
  `BlockRequest::with_preparation_trace()` to opt in;
- read `SceneOutput::trace()` or `CompositionSceneOutput::trace()`;
- continue using `work()` unchanged when detailed tracing is not needed;
- use `CacheDiagnostics::scene_cache_accounted_bytes()` only as the documented
  deterministic charge, not allocator telemetry.

Existing callers require no source change and remain untraced by default.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo +1.88.0 check --workspace --all-targets --all-features --locked \
  --exclude underwood_showcase \
  --exclude underwood_visual_proof \
  --exclude underwood_pdf \
  --exclude underwood_pdf_proof
cargo doc --workspace --all-features --no-deps
cargo xtask check
bd dep cycles
```

All commands passed on 2026-07-25. The MSRV gate uses the repository's CI
exclusions because the renderer-backed showcase, visual proof, PDF adapter, and
PDF proof inherit Rust 1.92 from their published dependencies; stable CI checks
those members in the full-workspace gates.
