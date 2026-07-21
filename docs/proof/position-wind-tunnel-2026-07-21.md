# Position wind-tunnel evidence — 2026-07-21

- **Capability:** document transactions and identity
- **Bead:** `und-oh0.10.1.1`
- **Trace:** private `identity-trace-v0`
- **Implementation commits:** `3734c6c`, `9d2b878`, `a6e2e59`
- **Candidate:** dependency-free canonical baseline plus persistent candidate v3
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

Eighteen wind-tunnel tests pass. The load-bearing cases are:

- insert and delete-collapse bias for sparse anchors;
- rejection of stale derived ranges;
- authored-range start/end edge transformation with no sparse-anchor
  resolution;
- immutable snapshot preservation and deterministic replay digest;
- 500 deterministic Unicode-aware edits differentially checked against
  `String`;
- 100 deterministic overlapping authored-range edit sequences differentially
  checked against the flat semantic model;
- source order across binary-counter carry cascades using 257 distinct append
  payloads, with an earlier 127-batch snapshot remaining immutable; and
- an exact one-GiB logical append run in 16,384 batches with bounded structural
  work and no unpublished tail;
- text-container split and adjacent join at a shared boundary, proving that a
  `Before` anchor remains on the left, an `After` anchor follows the right, and
  both recover their byte position and bias after join; and
- sibling move followed by delete, proving that an anchor follows its private
  node identity through the move and becomes explicitly unresolved after
  deletion.

The semantic suite is executed twice in one test and every trace id and digest
must match. The tree observation digest covers node order, node text, anchor
node, byte position, bias, and the unresolved state.

The range differential test initially failed on edit 2. Independent boundary
blocks became globally misordered after deletion collapsed spans with different
edge biases. Candidate v1 now coalesces every affected block, transforms and
globally sorts that frontier, and re-blocks it while sharing unaffected prefix
and suffix blocks. The regression sequence passes.

## Work evidence

The exact-scale work counters from the recorded debug run were:

| Pressure | Contiguous/flat baseline | Persistent candidate | Gate |
| --- | ---: | ---: | ---: |
| Million-line localized edit | 2,000,000 source bytes copied | 4,096 source bytes copied; 489 chunk records | at most 8,192 changed-chunk bytes and 4,096 index/frontier records |
| Million-span localized transform | 1,000,000 spans visited | 977 block records and 1,024 frontier spans | at most 4,096 index records plus overlaps |
| Snapshot clone | source `Arc` shared, zero source bytes copied | text/range indexes structurally shared | no source-byte copy and at most 256 newly allocated bytes |
| One-GiB logical append publication | no append strategy | 16,384 retained batch leaves; at most 30 metadata records per publication; 32,767 tree nodes created total; zero reported source-byte copies; zero unpublished batches | at most 32 metadata records per publication, zero source-byte copies, and at most two unpublished batches |

The million-span corpus contains one million ordered, non-overlapping one-byte
spans and replaces one byte intersecting one span. The earlier all-overlapping
fixture was rejected during review because visiting every overlap would not
violate the accepted gate.

The append pressure run reuses one immutable 64-KiB `Arc<str>` payload for all
16,384 publications. The final forest therefore represents one GiB of logical
text and retains 16,384 leaf records, but allocates only one distinct payload.
That controlled setup proves the publication metadata bound without confounding
it with payload creation. It does **not** prove one-GiB ingestion cost, one-GiB
retained payload memory, allocator behavior, or source-to-owned-batch copying.
The recorded publication p95 was 541 ns and remains `SCREEN`: there is still no
ratified reference machine, warmup policy, noise control, or confidence method.

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
- The append trace isolates publication metadata with a shared payload; a
  distinct-payload one-GiB ingestion and retained-memory run remains absent.
- No Loro or other collaboration-authority candidate is present.
- The tree model covers flat sibling text containers only. Recursive parent
  splices, subtree moves, schema payloads, concurrent move/delete, compaction,
  selective undo, and collaboration convergence remain unimplemented.
- Allocator sensitivity and confidence intervals are absent.
- Candidate representations are private and have not been reviewed as
  production storage.

These exclusions block candidate selection, the position bead's completion,
and any public position or storage API.
