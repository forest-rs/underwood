# Position wind-tunnel evidence — 2026-07-21

- **Capability:** document transactions and identity
- **Bead:** `und-oh0.10.1.1`
- **Trace:** private `identity-trace-v0`
- **Implementation commit:** `3734c6c`
- **Candidate:** dependency-free canonical baseline plus persistent candidate v1
- **Proof effect:** evidence at `Specified`; no promotion to `Executable`

## Reproduction

```sh
cargo test -p underwood_position_wind_tunnel
cargo run -p underwood_position_wind_tunnel
```

The recorded run used Rust 1.96.0
`ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96` on macOS 26.5.2,
`aarch64-apple-darwin`, with the system allocator. Wall-clock observations are
screens only: the machine, allocator overhead, warmup, noise controls, and
confidence method are not yet sufficient for an accepted timing claim.

## Correctness evidence

Eleven wind-tunnel tests pass. The load-bearing cases are:

- insert and delete-collapse bias for sparse anchors;
- rejection of stale derived ranges;
- authored-range start/end edge transformation with no sparse-anchor
  resolution;
- immutable snapshot preservation and deterministic replay digest;
- 500 deterministic Unicode-aware edits differentially checked against
  `String`;
- 100 deterministic overlapping authored-range edit sequences differentially
  checked against the flat semantic model.

The range differential test initially failed on edit 2. Independent boundary
blocks became globally misordered after deletion collapsed spans with different
edge biases. Candidate v1 now coalesces every affected block, transforms and
globally sorts that frontier, and re-blocks it while sharing unaffected prefix
and suffix blocks. The regression sequence passes.

## Work evidence

The exact-scale work counters from the recorded debug run were:

| Pressure | Contiguous/flat baseline | Persistent candidate v1 | Gate |
| --- | ---: | ---: | ---: |
| Million-line localized edit | 2,000,000 source bytes copied | 4,096 source bytes copied; 489 chunk records | at most 8,192 changed-chunk bytes and 4,096 index/frontier records |
| Million-span localized transform | 1,000,000 spans visited | 977 block records and 1,024 frontier spans | at most 4,096 index records plus overlaps |
| Snapshot clone | source `Arc` shared, zero source bytes copied | text/range indexes structurally shared | no source-byte copy and at most 256 newly allocated bytes |

The million-span corpus contains one million ordered, non-overlapping one-byte
spans and replaces one byte intersecting one span. The earlier all-overlapping
fixture was rejected during review because visiting every overlap would not
violate the accepted gate.

Preliminary structural screens, not memory proof, observed:

| Record | Structural lower bound |
| --- | ---: |
| 64-byte label state represented by the flat baseline | 392 bytes |
| Sparse anchor record | 16 bytes |
| Flat authored-span record | 24 bytes |
| Derived snapshot range | 24 bytes |

These numbers exclude some allocator control blocks, capacity slack, and the
eventual production index. They remain `SCREEN`, not `PASS`, until allocator
instrumentation measures retained allocations under the accepted corpus.

## Known failures and exclusions

- The contiguous baseline fails localized source-copy economics.
- The flat range baseline fails dense localized-transform economics.
- Candidate v1 has not run the one-GiB append trace and its flat chunk-index
  metadata is not expected to meet that workload without another tree level.
- No Loro or other collaboration-authority candidate is present.
- Split/join/move/delete node identity, compaction, selective undo, and
  collaboration convergence traces are not implemented.
- Allocator sensitivity and confidence intervals are absent.
- Candidate representations are private and have not been reviewed as
  production storage.

These exclusions block candidate selection, the position bead's completion,
and any public position or storage API.
