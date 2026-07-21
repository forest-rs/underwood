# Flow wind-tunnel evidence — 2026-07-21

- **Capability:** layout and scene
- **Bead:** `und-oh0.10.1.2`
- **Trace:** private `flow-trace-v0`
- **Implementation commit:** `138a092`
- **Candidate:** synthetic continuation and virtual-extent model v0
- **Proof effect:** evidence at `Specified`; no promotion to `Executable`

## Reproduction

```sh
cargo test -p underwood_flow_wind_tunnel
cargo run -p underwood_flow_wind_tunnel
```

The recorded run used Rust 1.96.0
`ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96` on macOS 26.5.2,
`aarch64-apple-darwin`, with the system allocator. Wall-clock latency and
retained-memory claims remain screens because the reference machine, allocator
instrumentation, warmup/noise controls, and confidence method are not ratified.

## Checkpoint density curve

The deterministic corpus contains 100,000 synthetic blocks, mandatory region
boundaries every 512 blocks, stable per-block work, occasional carried-state
effects, and 10,000 seeded invalidation positions.

| Policy | Checkpoints | Serialized bytes | p95 restart | Maximum restart |
| --- | ---: | ---: | ---: | ---: |
| Region boundary | 196 | 33,124 | 487 blocks | 511 blocks |
| Fixed 16 | 6,250 | 1,056,250 | 15 blocks | 15 blocks |
| Fixed 64 | 1,563 | 264,147 | 60 blocks | 63 blocks |
| Fixed 256 | 391 | 66,079 | 243 blocks | 255 blocks |
| Adaptive, 640 work units / 1,024 hard maximum | 977 | 165,113 | 100 blocks | 108 blocks |

The curve demonstrates why one favorable density is insufficient. Region-only
and fixed-256 miss the accepted p95 restart gate. Fixed-16 meets restart
distance but its serialized checkpoint bytes exceed the structural
`max(2% of synthetic fragment size, 8 bytes per source block)` screen.
Adaptive and fixed-64 pass the deterministic restart-work gate. Serialized
bytes are not retained-memory proof, so no policy receives a checkpoint-memory
`PASS`.

## Correctness and work evidence

Ten flow tests pass. The recorded executable observations are:

- a 169-byte private checkpoint decodes to full state and re-encodes
  byte-for-byte;
- truncated, trailing, and non-canonical boolean encodings are rejected;
- restart selects a valid checkpoint with a predecessor strictly before the
  invalidated block;
- a metric-preserving edit at block 50,000 restarts at 49,985, emits no prefix,
  and converges at 50,176 after visiting 191 blocks;
- an extent-changing edit at block 50,000 restarts at 49,985, emits no prefix,
  and converges after the region reset at 50,281 after visiting 296 blocks;
- deterministic cancellation publishes zero fragments and zero checkpoints;
- a one-million-block virtual map contains 977 segments; 10,000 seeded
  coordinate queries use at most 10 binary-search comparisons and return at
  most one 1,024-block candidate segment;
- replacing an estimate with a measurement reports zero correction above,
  +1,024 through and below the segment, preserves the named semantic anchor,
  and changes geometry knowledge from `Estimated` to `Measured`;
- an out-of-range measurement emits an estimator-violation diagnostic;
- semantic child existence is queryable independently of geometry, and an
  outside block reports `Unavailable` rather than fabricated bounds.

## What the synthetic model proves

The model proves Underwood-owned state-machine laws: explicit continuation
state, canonical private encoding behavior, strict predecessor validity,
successor-state convergence, deterministic checkpoint placement,
cancellation-publication atomicity, prefix aggregation, knowledge tagging, and
host-neutral correction reporting.

Those are real laws, but they are only one review deep and run over synthetic
blocks. The private encoding deliberately does not select a public wire format.

## Known failures and exclusions

- Floats, exclusions, footnotes, keeps, widows/orphans, counters, running
  headers, fragmented tables, width-dependent objects, and nested flows are
  represented only by synthetic work/carried-state digests.
- The 100,000-block corpus is not the accepted feature-complete long book.
- Cancellation has not yet been injected at every checkpointable state.
- The virtual map does not model fold toggles, massive paste, or deletion of a
  host-selected anchor.
- Memory numbers use serialized length and `size_of` screens, not allocator
  instrumentation.
- Latency distributions and 1/1024-layout-unit geometry tolerance are not
  measured.
- No Parley paragraph-breaking state participates.

These exclusions block checkpoint-policy selection, a public checkpoint
encoding, completion of the flow bead, and proof promotion.
