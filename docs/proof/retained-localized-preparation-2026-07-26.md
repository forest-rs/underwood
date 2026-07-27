# Retained localized-preparation proof — 2026-07-26

## Result

Normal-flow preparation after a one-byte edit no longer traverses every
paragraph. The public release allocation wind tunnel now differs between 64
and 1,000 paragraphs by three allocation calls, 504 requested bytes, and one
bounded persistent-tree level:

| Paragraphs | Allocation calls | Allocated bytes |
|---:|---:|---:|
| 64 | 612 | 158,771 |
| 1,000 | 615 | 159,275 |

The repeat run produced the same allocation counts and bytes. Before the
structural-diff path, after document COW was already active, the same
localized preparation performed 621 calls / 162,083 bytes at 64 paragraphs
and 782 calls / 223,035 bytes at 1,000. The checked-in median runner below
records CPU timing independently from allocation instrumentation.

Twenty-one isolated release samples on the same host produced:

| Paragraphs | Minimum | Median | Maximum |
|---:|---:|---:|---:|
| 64 | 62.250 µs | 64.750 µs | 84.459 µs |
| 1,000 | 67.208 µs | 70.083 µs | 82.583 µs |

The earlier matched Design-0017 progress observation was 82.583 µs at 64 and
292.542 µs at 1,000. The current 5.333 µs median difference follows bounded
tree depth rather than a 15.6× paragraph-count ratio.

The remaining roughly 159 KiB is changed-paragraph formation and today's
maximal prepared/scene output. It is not document-scale work. Capability
sidecars and source/interaction compaction in proposed Design-0018 target that
separate term.

## Mechanism

The previously published `DocumentSnapshot` and current snapshot each own a
persistent 32-way paragraph sequence. An allocation-free paired traversal:

1. skips an entire node when its `Arc` identity is shared;
2. descends only copy-on-written paths;
3. yields changed paragraph indexes in document order;
4. fails closed to the general preparation path if paragraph count or source
   structure changed.

For eligible normal flow, `LayoutEngine`:

- requires exact shared style provenance and the same constraint;
- verifies every yielded paragraph still has the same leaf identities and a
  retained cache entry;
- projects, forms, and lowers only yielded paragraphs;
- path-copies only their persistent scene-spine branches;
- records every untouched paragraph as an exact geometry reuse without
  individually visiting it;
- computes binary scene-spine capacity in O(1) from `2P - 1` nodes;
- publishes the new revision only after all changed paragraphs succeed.

The source-structure check deliberately excludes append/remove and leaf
identity changes from this fast path. Those cases use the complete validation
path. Region flow also remains excluded: a changed paragraph can alter the
cursor offered to its suffix, so region preparation requires explicit
convergence rather than pretending to be one-paragraph work.

An empty transaction produces a new document revision with the same paragraph
root. The structural diff is empty, no adapter or geometry stage runs, and the
new revision retains the exact prior `SceneCore`.

## Work-law evidence

The public benchmark asserts for both scales:

- exactly one paragraph analyzes and shapes;
- exactly `P - 1` paragraphs are reused;
- the edit-staging event is independently bounded.

Focused trace tests additionally assert for a three-paragraph localized edit:

- one adapter call;
- two preflight reuses;
- two exact geometry reuses;
- one paint paragraph;
- pointer-identical leading and trailing segments;
- a distinct changed segment;
- old and new revisions lazily bind their own source positions.

Persistent-sequence tests change indexes on six different tree paths across
the 32- and 1,024-paragraph boundaries and prove that the structural diff
returns exactly those indexes in order.

## Reproduction

```sh
cargo build --release \
  -p underwood_semantic_scene_benchmark \
  --features allocation-counting
benches/semantic-scene/profile-counted-allocations.sh
cargo build --release -p underwood_semantic_scene_benchmark
benches/semantic-scene/profile-localized-timing.sh
```

The complete repeat observation was:

| Scenario | Paragraphs | Calls | Allocated bytes | Net live calls | Net live bytes |
|---|---:|---:|---:|---:|---:|
| retained exact repeat | 64 | 0 | 0 | 0 | 0 |
| edit staging | 64 | 20 | 1,592 | 11 | 888 |
| localized prepare | 64 | 612 | 158,771 | 352 | 79,463 |
| localized edit + prepare | 64 | 632 | 160,363 | 363 | 80,351 |
| retained exact repeat | 1,000 | 0 | 0 | 0 | 0 |
| edit staging | 1,000 | 20 | 1,832 | 11 | 1,128 |
| localized prepare | 1,000 | 615 | 159,275 | 355 | 79,967 |
| localized edit + prepare | 1,000 | 635 | 161,107 | 366 | 81,095 |

These are allocator-request observations, not physical-residency estimates.
The checked-in benchmark isolates each event after constructing fonts,
document content, selection state, and the primed scene.

## Scope boundary

This proof earns localized O(change) for same-structure normal-flow document
edits. It does not claim:

- O(change) after global style, constraint, or paint-slot topology changes;
- one-paragraph region flow when downstream cursor state changes;
- low absolute changed-paragraph residency before capability sidecars;
- O(change) document structure insertion or removal, which Underwood does not
  yet expose as a general edit operation.

Those distinctions are visible in the implementation and work report rather
than hidden behind one broad “cache hit” label.
