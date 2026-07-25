# Retained lifecycle design review — 2026-07-25

## Scope

Rook review of proposed
`docs/design/0017-retained-scene-lifecycle.md` against the committed
implementation and the measured 64/1,000-paragraph baseline.

This review assesses whether the proposal can earn O(change) preparation and
publication. It does not claim that the implementation exists.

## Summary judgment

The selected direction is structurally capable of fixing Claude Fable's five
headline findings. A persistent summary tree of paragraph-local segments plus
lazy revision/origin binding addresses the actual ownership problem; a flat
vector of shared handles does not.

The design is ready for a human architecture gate after six corrections below.
It remains one review deep until the first provenance slice and persistent
scene slice each receive implementation review and measured proof.

## Mirage risks found and corrected

### 1. “Shared segments” could still mean O(paragraphs)

**Mirage:** `Vec<Arc<ParagraphSceneSegment>>` sounds retained while cloning one
handle and recomputing one origin/index prefix per paragraph on every prepare.

**Correction:** Design-0017 rejects that representation as the destination and
requires a persistent summary tree, O(1) exact root return, and O(log P) leaf
replacement.

### 2. O(change) had no source of change information

**Mirage:** a persistent scene cannot find one changed paragraph cheaply if
`prepare(&DocumentSnapshot, ...)` still scans a flat snapshot and mutable
`StyleMap`.

**Correction:** document and style states are themselves persistent. The engine
structurally diffs roots and skips `Arc`-identical subtrees; exact request
identity returns before validation or traversal.

### 3. Style deltas could retain unbounded predecessor history

**Mirage:** recording an immediate predecessor and delta makes one mutation
cheap but either loses multi-mutation history or retains every old complete
style state.

**Correction:** paragraph-grouped overrides use a persistent ordered map.
Structural identity provides both branch-safe provenance and bounded path
copying without a predecessor chain.

### 4. A tree iterator could hide O(P log P) or fresh allocation

**Mirage:** repeatedly implementing iteration as `line(index)` would turn a
full traversal into O(P log P); a heap stack on every iterator would also make
“zero materialization” less meaningful.

**Correction:** the design requires bounded-depth iterative traversal with a
fixed stack. Full traversal is O(log P + V) and allocates no record collection.

### 5. Region convergence could cost O(A log P)

**Mirage:** replacing every affected region paragraph through an independent
tree update does not satisfy O(log P + A).

**Correction:** affected contiguous suffixes use batched range replacement or
split/concatenate. Cursor seams are the convergence proof.

### 6. A block-prefix tree is not automatically a 2D spatial index

**Mirage:** arbitrary columns can overlap in the block coordinate. A subtree's
union rectangle may fail to prune point lookup, so “O(log P) hit testing” was
not earned for all region flows.

**Correction:** the strict logarithmic claim now applies to normal flow.
Region-flow hit testing must add region/slot spatial summaries or report
candidate-dependent complexity.

### 7. Shared roots could defeat eviction while diagnostics claimed success

**Mirage:** removing a paragraph-cache map entry does not free its segment if
an engine-owned published root still holds it. Counting cache entries would
understate real engine residency.

**Correction:** unique segments and root metadata receive separate charges.
Before segment eviction the engine drops every exact-root entry containing it.
Caller-held scenes remain intentionally outside engine eviction and are never
reported as engine-controlled memory.

## Real strengths

- The measured baseline is real and reproducible. It separates edit staging,
  localized preparation, and exact-repeat publication rather than inferring
  one from another.
- Cached geometry is already paragraph-local and revision-free. The proposed
  segment boundary builds on an existing real invariant rather than inventing
  a second geometry model.
- `PaintTable` and `RegionFlow` already have immutable shared backing, and
  `DocumentSnapshot` already has immutable state backing. These are useful
  provenance foundations.
- Source positions are already represented locally inside cached geometry,
  which makes lazy revision stamping feasible.
- Old snapshot correctness is already a tested public contract. Persistent
  roots strengthen its implementation instead of changing the semantic law.
- Region transcripts already expose exact start/end cursor seams, making
  convergence testable.

## Most dangerous gap

The public view migration is load-bearing. If implementation preserves flat
slice getters through implicit lazy flattening, the architecture will become a
mirage even if preparation microbenchmarks improve.

The renderer, PDF exporter, showcase, interaction paths, tests, and benchmarks
must consume positioned views directly. Any explicit compatibility collection
must be visibly named, measured, and absent from those hot product paths.

## Required implementation tests

1. Exact repeats at 64 and 1,000 paragraphs perform the same allocation count,
   with zero projection, adapter, geometry, and record work.
2. One changed paragraph produces a new segment and O(log P) spine nodes while
   pointer identity for every unchanged segment survives.
3. Changing the first paragraph's height moves the last paragraph correctly
   without changing its segment identity.
4. An old scene and a new revision stamp different positions over one shared
   unchanged segment.
5. Two clones of one `StyleMap`, independently mutated to different values,
   never collide; an equal no-op preserves the fast identity.
6. An independently constructed equal style map is correct on the slow path.
7. Paint-table replacement preserves the scene core and validates slots in
   O(1).
8. Region reflow stops at the first matching incoming cursor and replaces a
   contiguous range in O(log P + A).
9. Composition replaces only its target segment and does not mutate committed
   local provenance.
10. Dropping or evicting an engine root changes diagnostics honestly while a
    caller-held old scene remains usable.
11. All renderer and PDF proof call sites traverse views without invoking a
    flat collection helper.
12. Mixed bidi visual selection, caret movement, hit testing, and source
    extraction remain exact across segment boundaries.

## Status

**Design reviewed; implementation not started.** No unresolved design-level
Must remains after the corrections above. Representation details still require
ordinary implementation review, benchmark proof, and the public architecture
approval named by Design-0017.
