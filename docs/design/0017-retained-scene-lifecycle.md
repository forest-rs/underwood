# Design-0017: Retained scene lifecycle

- **Status:** Approved — 2026-07-25
- **Date:** 2026-07-25
- **Bead:** `und-oh0.13.17`
- **Extends:** Design-0012, Design-0014, and Design-0016
- **Evidence:** `docs/proof/retained-lifecycle-baseline-2026-07-25.md`

## Decision

Underwood will retain immutable paragraph-local scene segments in a persistent
summary tree. `TextScene` will become a cheap revision-and-paint binding over
that shared scene core. Public scene traversal will use positioned views and
iterators rather than flat borrowed slices of already-translated,
revision-stamped records.

Cache eligibility will be proven first by immutable provenance identities and
facet-specific paragraph keys. A numeric generation without its originating
immutable state is not a key.

Document publication will use a persistent paragraph sequence and transaction
overlays so one changed paragraph does not clone the document. The Parley
adapter will retain its validated lowered result and scratch workspaces, but it
will not acquire document or scene ownership.

## Goal

Make the retained public path proportional to required change:

- an exact repeated snapshot and request returns an already-published immutable
  scene in O(1);
- a same-height localized edit rebuilds one paragraph and O(log paragraphs)
  scene-spine nodes;
- a changed-height localized edit shares every unchanged paragraph record and
  changes only O(log paragraphs) prefix summaries;
- region flow recomputes only the affected suffix until the incoming region
  cursor converges;
- global constraints and defaults visit every paragraph they actually affect;
- complete rendering remains proportional to visible output, as it must.

## Non-goals

- No mutable scene graph, observer network, or renderer-owned invalidation
  policy.
- No stable serialized scene-node format or distributed patch protocol.
- No attempt to make a global width, default style, or region-flow change
  sublinear when it changes every paragraph's legal output.
- No cache hit based on a pointer address after its owner has been dropped.
- No public unchecked constructor and no weakening of adapter-output
  validation.
- No new production dependency and no `unsafe`.
- No requirement that independently constructed but value-equal style maps,
  paint tables, or region flows receive the O(1) identity hit. They remain
  correct and may take the slower value-validation path.

## Measured problem

The real release public path currently reports:

| Event | Paragraphs | Allocation calls | Requested bytes | Median time |
|---|---:|---:|---:|---:|
| exact retained repeat | 64 | 42,607 | 10,265,160 | 1.17 ms |
| exact retained repeat | 1,000 | 666,167 | 163,571,752 | 20.33 ms |
| one-byte edit staging | 1,000 | 1,015 | 105,088 | 0.021 ms |
| localized prepare | 1,000 | 666,807 | 165,767,603 | 21.98 ms |

The preparation caches do avoid repeated analysis and shaping. The remaining
cost comes from three repetitions of the same lifecycle error:

1. edit staging proves isolation by cloning the complete document value;
2. cache lookup proves freshness by rebuilding projection values and
   deep-comparing them;
3. scene publication proves ownership by deep-copying retained paragraph
   geometry into flat document-space vectors.

## Ownership fence

### Document

`Document` owns immutable semantic publication, persistent paragraph order,
transaction overlays, and copy-on-write mutation of touched paragraph and text
storage.

It does not own projection, computed-style caches, line layout, or scene
placement.

### StyleMap and immutable request resources

`StyleMap` owns shareable immutable computed-style state and precise deltas for
local overrides and defaults. `PaintTable` and `RegionFlow` remain immutable
shared values.

They do not own paragraph preparation or cache lifetime.

### LayoutEngine

`LayoutEngine` owns:

- request provenance and invalidation;
- paragraph-local projection and geometry caches;
- the persistent scene spine and its summaries;
- committed and composition scene publication;
- coordinated retention and release.

It does not own shaping internals, widget retention, renderer timing, or
process-allocation accounting.

### ParagraphFormation and underwood_parley

The adapter owns retained analysis, shaping, formation, validated lowering,
font-resource views, and mutable scratch.

It does not own document identity, semantic identity, final scene placement,
whole-document invalidation, or publication.

### Renderer and host

Renderers traverse positioned scene views and resolve paint slots. Hosts retain
the scene handles they present.

They do not reconstruct source geometry, rebase revisions, or decide text
cache invalidation.

## Required invariants

1. Published scenes are immutable. Old scenes remain exact while newer
   revisions share their unchanged nodes.
2. Paragraph segments contain paragraph-local coordinates, local record
   indexes, stable text and semantic identities, and unstamped local source
   ranges and positions.
3. Document revision, composition epoch, paint values, paragraph origin, and
   global line/fragment ordinals are bindings or traversal context; they are
   not rewritten into every cached record.
4. A persistent spine node summarizes paragraph count, block extent, line and
   record counts, maximum inline extent, paint-slot requirement, and first/last
   baseline facts.
5. Updating one leaf replaces only that leaf and the nodes on its root path.
   No update clones a flat segment vector.
6. Normal-flow paragraph origins are prefix sums of subtree block extents.
   A changed height updates ancestor summaries; downstream records remain
   shared.
7. Region-flow segments retain absolute offered slots and start/end cursors.
   A changed paragraph invalidates successors only until the next segment's
   incoming cursor and all other keys match.
8. A scene iterator applies paragraph origin, global record bases, revision,
   and optional composition binding exactly once when exposing each view.
9. Snapshot ranges and positions minted by a committed scene always carry that
   scene's revision. Composition views preserve authored and generated
   provenance without mutating shared committed segments.
10. Exact scene reuse requires strong immutable provenance for snapshot,
    styles, paint, region flow, constraint, and composition epoch. Stored keys
    keep their provenance owners alive, so address reuse cannot create a hit.
11. Branches from one cloned `StyleMap` cannot collide after independent
    mutations. Each immutable state or affected paragraph bucket has a unique
    shared identity.
12. Setting a style to its existing value is a no-op and preserves provenance.
13. A provenance miss is only a performance miss. The checked projection and
    validation path remains the correctness fallback for unrelated but
    value-equal inputs.
14. Private trusted construction is permitted only immediately after the same
    module proves the invariant. Public input and adapter-output boundaries
    remain checked.
15. Cache budgets count all engine-retained scene roots, nodes, segments,
    paragraph entries, shared preparations, adapter output, and scratch by
    explicit category. Caller-retained old scenes cannot be evicted by the
    engine and are never misreported as engine-controlled residency.
16. `release_document` drops every strong engine-owned scene and
    identity-bound segment for that document. Caller-held scenes remain valid.
17. Core crates remain `no_std + alloc`, Rust 1.88 compatible, dependency
    neutral, and free of `unsafe`.

## Work laws

Let `P` be paragraph count, `R` records in changed paragraphs, `A` the
region-flow suffix whose incoming cursor changes, and `V` records a consumer
actually visits.

| Operation | Required preparation/publication work |
|---|---:|
| exact snapshot + exact request | O(1) |
| paint-table value change, same slots | O(1) |
| same-height local text/style edit | O(log P + R) |
| changed-height local edit, normal flow | O(log P + R) |
| local edit in region flow | O(log P + R + A) |
| append a prepared paragraph | O(log P + R) |
| global width/default/region change | O(P + changed records) |
| full scene traversal/render | O(log P + V), with no record materialization |
| normal-flow point-to-paragraph lookup | O(log P) plus line-local search |
| exact text/caret lookup | O(log P) plus paragraph-local search |

A flat `Vec<Arc<ParagraphSceneSegment>>` removes record copies but still takes
O(P) handle clones and eagerly recomputes origins and global ordinals. It does
not satisfy these laws.

## Immutable request provenance

### DocumentSnapshot

The snapshot's strong immutable state handle is its exact provenance. Once the
document uses a persistent paragraph sequence, structurally shared nodes also
identify unchanged ranges between revisions.

### StyleMap

`StyleMap` becomes a cheap clone over immutable state. Its internal state
contains:

- shared default inline and paragraph styles;
- a persistent ordered map of source-ordered override buckets grouped by
  paragraph;
- a unique immutable identity for each default and paragraph bucket;
- cached maximum referenced paint slot and validation summaries.

Mutation creates new state and replaces only the affected bucket identity.
Independent branches therefore cannot share a `(lineage, generation)` pair by
accident; no global atomic identity source is needed in `no_std`.

The persistent map skips shared subtrees by `Arc` identity when comparing two
style snapshots. `StyleMap::set` performs O(log overrides) path copying plus
copy-on-write of the affected paragraph's normally-small leaf bucket. There is
no retained predecessor chain and therefore no hidden unbounded style history.

### PaintTable

`PaintTable` already owns immutable `Arc` backing. Shared backing plus slot
count is exact provenance. A new table with the same values is a safe false
miss. A paint-only request can bind a new table to an existing core when the
core's maximum referenced slot is present.

### RegionFlow

`RegionFlow` already owns immutable compiled `Arc` backing. Shared backing plus
the exact starting cursor is the fast identity. An independently compiled
equal flow takes the checked fallback.

### Paragraph preflight key

Before constructing `Projection`, each retained paragraph compares a compact
key:

- paragraph immutable identity or current paragraph version during migration;
- default and paragraph-local style bucket identities;
- exact constraint;
- region-flow identity and incoming cursor;
- composition identity, epoch, target, and text backing when transient.

Paint-table values are a scene binding. The projected paint-slot partition
comes from the style bucket and remains in the paragraph key.

On an exact request hit, the scene root is returned before document/style
validation or paragraph traversal. On a changed snapshot or style state,
LayoutEngine structurally diffs the previous and current persistent roots:
shared subtrees are skipped and only changed paragraphs are preflighted. On a
paragraph preflight hit, no projected string, source-key vector, style/run
vector, or adapter input is rebuilt. On a miss, the normal checked path
constructs the projection and records a new key. Deep value equality can rescue
safe reuse after an unrelated-provenance miss, but it is never required on the
hit path.

## Persistent scene core

Conceptually:

```text
TextScene
├── document + revision
├── paint table
└── Arc<SceneCore>
    ├── request geometry identity
    ├── persistent SceneSpine root
    └── optional persistent region transcript

SceneSpine
├── Branch(summary, left, right)
└── Leaf(summary, Arc<ParagraphSceneSegment>)
```

The implementation may use a weight-balanced tree, AVL tree, or another small
deterministic persistent tree. It must support O(log P) lookup and replacement,
bounded depth, linear initial construction, and iterative traversal without a
production dependency. Because paragraph identity is `u32`-bounded, iterators
can use a fixed bounded traversal stack; they must not allocate once per
paragraph or perform O(P log P) repeated indexed lookup.

`ParagraphSceneSegment` owns the current `CachedGeometry` facts after those
facts are renamed and shaped for publication. Its records remain local:

- line fragment ranges and cluster line indexes are segment-local;
- coordinates are paragraph-local in ordinary flow;
- region coordinates remain in flow space;
- source ranges and positions are local and unstamped;
- glyph-instance identity is paragraph-local plus stable paragraph identity,
  not a flat fragment-vector offset.

The summary tree supplies prefix block origin, global line and fragment bases,
metrics, and paint-slot validation. Changing one paragraph height changes
summary values on its root path; it does not rewrite downstream geometry.

## Public scene traversal

The current methods return flat slices:

```rust
scene.lines() -> &[SceneLine]
scene.fragments() -> &[SceneFragment]
```

Those signatures require all records to exist contiguously with final
coordinates and stamped sources. They are incompatible with persistent sharing
and will change.

The target shape is conceptually:

```rust
for paragraph in scene.paragraphs() {
    let origin = paragraph.origin();
    for line in paragraph.lines() {
        render_line(line);
    }
    for fragment in paragraph.fragments() {
        render_fragment(fragment);
    }
}

let first = scene.line(0);
let count = scene.line_count();
```

Positioned line, fragment, glyph, cluster, caret, and semantic views expose the
same observations as today while applying origin and revision lazily. They are
borrowed or cheap value handles; they do not allocate a translated source
vector merely to answer one query.

Flat collection helpers, if a real consumer still needs them, will be explicit
`collect_*` operations whose O(V) allocation is visible at the call site. They
will not be the renderer or interaction default.

Interaction methods remain on `TextScene`. They descend by paragraph or block
summary and use paragraph-local sorted indexes. Selection geometry traverses
only paragraphs and clusters overlapped by its source ranges.

## Composition

Committed and transient scenes share the same local segment representation.
Local provenance already distinguishes snapshot and composition sources.

A composition scene reuses every unchanged committed segment and replaces only
the target paragraph segment. Its top-level binding adds composition identity
and epoch. Starting a new epoch never rewrites composition identity into an old
cached segment; it creates or reuses a segment whose key names that exact
epoch.

If preedit height changes, the same prefix-summary rule moves following
paragraphs lazily. Region-flow composition follows cursor convergence.

## Region transcripts

The current document transcript is flattened and revalidated every prepare.
Each paragraph segment will retain its already-validated start cursor, end
cursor, and immutable attempt block. The scene root summarizes the complete
start/end chain.

Exact repeats clone one transcript view. A localized region change replaces
attempt blocks only through cursor convergence. Public attempt traversal becomes
an exact-size iterator; an explicit collection helper replaces the current
assumption that every transcript must be one contiguous slice.

When several consecutive region segments change, the tree performs a batched
range replacement or split/concatenate operation. Replacing `A` leaves one at
a time for O(A log P) does not earn the stated convergence law.

Deep replay remains available as an explicit verification operation and in
debug/proof gates. A transcript assembled from validated blocks with matching
cursor seams uses a private trusted path.

## Adapter retention and scratch

`ParagraphFormation::form` currently returns owned `PreparedParagraph` output,
and the Parley adapter reconstructs validated lowered lines and movement even
when its private preparation facts did not change.

The adapter will retain:

- immutable validated canonical and formed facts;
- the exact lowered `PreparedParagraph` facts for matching output;
- a byte-to-character prefix table for source/shaping contribution queries;
- shaping, candidate, line-range, and lowering scratch buffers.

LayoutEngine will not call the adapter on a paragraph preflight hit. On calls
that require reformation, the adapter may return a shared immutable lowered
handle when its exact output key matches. Paint-table value changes never call
it. Paint-slot partition changes may re-slot retained glyph coverage only when
that transformation is independently validated.

Per-facet numeric revisions may be added to the adapter contract only with an
engine/entry provenance token that makes lying or cross-engine collision
impossible. The first slice does not need them: a retained prepared-output
handle plus explicit invalidation from LayoutEngine is sufficient.

## Document publication

`Document::edit()` will stop cloning `DocumentState`.

The target transaction owns:

- the immutable base-state handle;
- a replacement workspace;
- a keyed overlay of touched staged paragraphs;
- a batched appended tail;
- the exact changed-paragraph set.

Published paragraph order is a persistent sequence. Editing one paragraph
copies one root path and one paragraph value. A touched leaf uses
`StagedText::{Shared(Arc<str>), Owned(String)}` and converts to `String` once,
regardless of how many carets edit it during the transaction.

Old snapshots retain old roots. Commit builds one new state and one exact
change summary; failure publishes nothing.

The stronger paragraph-owned text buffer with leaf `(role, range)` records
remains a possible later representation. It is not required to eliminate
whole-document staging or repeated same-leaf copies.

## Cache lifetime and memory

Scene publication and paragraph geometry can no longer pretend to be separate
retention when they share the same segment records.

The coordinated cache will account for:

- engine-owned published scene roots and persistent spine nodes;
- unique paragraph segments;
- paragraph projection/preparation keys;
- shared cross-identity preparation;
- adapter-retained lowered output;
- scratch capacities, separately from immutable residency.

One latest committed root and relevant composition roots may be retained per
document only while their complete segment set fits its coordinated lane
budget. Committed and transient-composition limits are independent so
transient work cannot evict the committed scene needed when composition is
cancelled.
Paragraph cache entries and retained roots share the same `Arc` segments; the
engine registry charges each unique segment once and charges persistent root
metadata separately. Before evicting a segment, the engine drops every
engine-owned exact-root entry that contains it. It must not claim the segment
memory was released while another engine root still pins it.

Caller-held scenes are outside the eviction budget: they intentionally keep old
immutable roots alive. Engine diagnostics report weak-root liveness or exported
root counts only if that observation can be implemented without per-frame
scanning or a portability regression; otherwise they state that external
retention is unmeasured. They never present engine-controlled residency as
whole-process memory.

Exact scene hits update scene-level recency in O(1). They do not walk every
paragraph merely to refresh individual LRU timestamps. Paragraph eviction
lazily folds a newer root timestamp into stale candidate entries while it is
already enforcing a budget. A composition root refresh protects its transient
target and the committed sibling segments that root actually names, but not
the superseded committed target segment.

The present entry-count budget is retained during migration, then supplemented
or replaced by explicit byte categories only with measured accounting. Cache
policy changes receive their own migration note.

## Options considered

### A. Add scratch and optimize the existing flat materialization

This would reduce allocator traffic while retaining O(P) projection checks,
copies, stamping, and coordinate translation.

Reject. It improves constants around the wrong lifecycle.

### B. Publish `Vec<Arc<ParagraphSceneSegment>>`

This removes deep record copies and is a useful transitional representation.
It still clones P handles, rebuilds a flat vector, and eagerly computes
downstream origins and global indexes.

Reject as the destination. It may appear inside the first implementation only
if its benchmark and naming state explicitly that localized publication
remains O(P).

### C. Keep flat slices through lazy whole-scene materialization

Preparation would become cheap, but the first `lines()` or `fragments()` call
would allocate and copy the complete document. Almost every renderer would pay
the old cost under a less honest name.

Reject as the primary API. Explicit collection compatibility helpers are fine.

### D. Publish scene deltas and make every consumer own the retained graph

This can be efficient, but it pushes origin, revision, cache, and atomic
publication invariants into every renderer and host.

Not the first contract. A future distributed-scene protocol can derive deltas
from immutable roots without changing core ownership.

### E. Persistent summary tree plus positioned views

Choose E.

It is the only option that satisfies exact-repeat, changed-height, old-snapshot,
interaction, and full-traversal laws together without mutable shared state or a
new dependency.

## Migration note

This design intentionally changes public traversal while preserving semantic
capability.

- `TextScene` and `CompositionScene` clones become cheap shared handles.
- Flat slice getters for lines and fragments become iterator/view APIs.
  Indexing migrates from `scene.lines()[i]` to `scene.line(i)` and from
  `scene.fragments()[i]` to `scene.fragment(i)`.
- Call sites using `.lines().iter()` or `.fragments().iter()` iterate the view
  directly.
- APIs needing contiguous owned records call explicit `collect_lines` or
  `collect_fragments`; these make O(V) work visible.
- `SceneLine`, `SceneFragment`, `SceneGlyph`, caret, cluster, and semantic
  observations may become positioned view types. Their accessors retain
  current meanings and return scene-space coordinates and current-revision
  sources.
- Region transcript attempts become an iterator/view with an explicit
  collection helper.
- `StyleMap` construction and mutation call shapes remain source-compatible.
  Cloning becomes cheap and mutations become copy-on-write.
- `LayoutEngine::prepare` and `prepare_block` retain their result-oriented
  call shape.
- `CacheBudget::new(entries)` applies `entries` independently to committed and
  transient-composition geometry so composition cannot evict committed work.
  `with_composition_entries` selects a different transient limit, including
  zero for caller-owned composition output without engine retention.
  Consequently `CacheDiagnostics::current_entries()` may be the sum of two
  full lanes; `budget()` reports the committed limit and
  `composition_budget()` reports the transient limit.
- `ParagraphFormation::release(ParagraphId)` becomes
  `release(ParagraphPreparationId)`. Backends key retained work by
  `ParagraphInput::preparation()` so committed and transient composition lanes
  cannot displace one another.
- `ParagraphInput::change()` reports the validated formation facets changed
  since that exact preparation lane last ran. A backend may skip
  deep-comparison only when it still owns the matching cache entry; a missing
  entry remains cold regardless of the change record.
- `ParagraphInput::reusable_preparation()` is an optional exact-output reuse
  opportunity across lanes. Backends may ignore it, but must not infer broader
  semantic identity from the shared `ParagraphId`.
- Paint fragments are run-sized in the common case. Consumers that previously
  assumed one fragment per glyph must iterate `fragment.glyphs()`. A fragment
  may span several authored owners, so glyph-specific provenance comes from
  the glyph view while `fragment.sources()` describes the whole fragment.

All repository examples, PDF export, showcase rendering, tests, and benchmarks
will migrate in the same coherent change. There will be no compatibility
adapter on the hot renderer path.

## Execution slices

1. **Measured baseline:** complete in
   `docs/proof/retained-lifecycle-baseline-2026-07-25.md`.
2. **Provenance preflight:** immutable style/request provenance, compact
   paragraph preflight keys, no projection/source-key construction on hits.
3. **Persistent scene:** local immutable segments, summary tree, exact root
   reuse, positioned traversal, lazy revision/origin binding, and migration of
   every consumer.
4. **Adapter output:** retained lowered handle, linear byte/character indexing,
   and scratch reuse.
5. **Document COW:** persistent paragraph order, transaction overlay, and
   once-mutable touched text.
6. **Data shape and indexes:** one-or-many sources, whole-or-split paint,
   fragment coalescing, sorted local interaction indexes.
7. **Trace and product proof:** deterministic invalidation, scratch, residency,
   and region-convergence trace over the corrected lifecycle.

Each slice records the 64- and 1,000-paragraph timing/allocation delta and lands
green before the next begins.

## Proof matrix

| Law | Required proof |
|---|---|
| exact repeat O(1) | zero projection/adapter/geometry work; allocation count independent of P |
| local edit O(change) | 64/1,000 counts differ only by logarithmic spine work |
| changed height | unchanged segment pointer identities survive; every downstream coordinate is exact |
| stale revisions | old and new scenes mint distinct revisions over shared unchanged segments |
| style provenance | clone branches, equal no-op, unrelated equal values, default and local changes |
| paint-only | scene core pointer retained; new table resolves every slot |
| region convergence | only cursor-dependent suffix replaced; replay result unchanged |
| composition | committed siblings shared; generated identity and epoch remain exact |
| bidi interaction | visual/logical movement and multi-range selection unchanged |
| renderer/PDF | visual and extraction snapshots unchanged except intentional API migration |
| retention | release, eviction, caller-held old scenes, and budget categories |
| portability | Rust 1.88, `no_std + alloc`, wasm, bare-metal, all hosts |

Arbitrary region layouts can place several columns at the same block
coordinate, so the normal-flow prefix tree alone does not prove logarithmic
point lookup there. Region hit testing must use region/slot spatial summaries
or state its candidate-dependent complexity; the proof matrix will not infer a
2D index from the existence of a 1D persistent spine.

## Human gate

Approved by Bruce on 2026-07-25. The approval authorizes:

- the public scene traversal migration above;
- internal copy-on-write `StyleMap` and `Document` representations;
- the persistent scene spine and private trusted-provenance constructors;
- additive deterministic diagnostics needed to prove the work laws.

It does not authorize a new production dependency, `unsafe`, a serialized scene
format, or renderer/toolkit policy in core.
