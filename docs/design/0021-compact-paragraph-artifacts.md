# Design-0021: Compact paragraph artifacts

- **Status:** Approved — 2026-07-27
- **Date:** 2026-07-27
- **Bead:** `und-oh0.13.17.11`
- **Supersedes the retained shape, not the capability model, of:** Design-0018

## Decision

Underwood will publish one compact paragraph artifact instead of retaining
separate maximal prepared, scene-hit, caret, and cursor-movement forms.

The artifact has a flat portable layout base and optional compact capability
tables. Adapter output and published scene views share that exact artifact.
The scene layer adds paragraph placement, line adjustment, paint binding,
authored-source binding, and document revision; it does not copy the artifact
into another owned glyph or interaction model.

Editable interaction retains the smallest authoritative facts:

- one paragraph-local interaction-unit table;
- exact source, bidi, boundary, whitespace, and visual-side facts per unit;
- line-local visual order and formed-line break reasons;
- rare spill tables for multi-source or multi-semantic geometry.

Cursor positions, caret anchors, and logical/visual adjacency are
allocation-free borrowed derivations over those facts. They are not retained
as separate hit clusters, position tables, carets, or four complete
source-aware transitions per position.

`TextBlock` remains a facade over the same paragraph preparation and scene
engine, but its source owner becomes a compact one-paragraph state rather than
a one-node persistent document tree.

Reusable adapter formation state remains optional and independently budgeted.
It may retain analysis, canonical shaping, and line-formation inputs, but it
does not retain another final `PreparedParagraph`. The default zero budget
remains real. Warm retention and the default published-only path are measured
separately.

## Simplification thesis

This is an opportunity to remove concepts, not merely pack the current object
graph.

Underwood's system story is one sentence:

> A source snapshot is prepared once into one immutable paragraph artifact,
> and every scene capability is a borrowed interpretation of that artifact.

The canonical path is:

```text
Document paragraph or TextBlock snapshot
        → shared projection input
        → ParagraphFormation
        → one validated paragraph artifact
        → scene placement + paint/source binding
        → borrowed display/interaction/export views
```

Documents and blocks differ only in source ownership. Warm and cold formation
differ only in whether adapter working state survives. They do not create
alternative prepared or scene representations.

The scene-level paragraph artifact becomes the authoritative retained data
store. Rendering, source traversal, semantics, hit testing, carets, selection,
navigation, native input, and export are borrowed views that join its typed
tables by index. They are not separately owned models with copied proof data.

The desired direction is:

```text
one validated paragraph artifact
        ├─ borrowed display view
        ├─ borrowed source view
        ├─ borrowed hit view
        ├─ borrowed selection geometry
        ├─ borrowed movement view
        └─ borrowed native/export view
```

This permits deliberate asymmetry. The minimal artifact may retain a small
projected span or topology index that one display operation does not need when
that field deletes a much larger prepared-output cache, repaint copy, or
source-aware sidecar. Capability scaling remains, but it is not applied so
mechanically that it creates six owners for six overlapping views.

### Likely deletions

The final implementation should make these concepts unnecessary or materially
smaller:

- adapter-retained final `PreparedParagraph`;
- `prepared_paint_runs` and the clone-based `try_map_glyph_paint` path;
- complete `PreparedCursorMovement` records with four owned transitions;
- separate `CachedCaret` and `CachedCursorMovement` tables;
- repeated semantic identity in every hit slice;
- a second scene-owned glyph form copied from prepared glyphs;
- temporary nested line/run/glyph vectors built only to be flattened;
- one-node `Document` construction inside plain `TextBlock`;
- release-mode deep validation of values produced by an already validated
  internal builder;
- cache hits that rebuild projection/source/style values only to compare and
  drop them.

The migration does not preserve obsolete types or add a compatibility mirror.
If the new artifact lands while those forms remain live in the ordinary path,
the slice is incomplete.

### Deletion ledger

The proof records a before/after symbol inventory. The intended removals
include:

- `PreparedCursorMovement`, `PreparedCursorStep`, and
  `prepared_cursor_movements`;
- `CachedCaret`, `CachedCursorMovement`, and `CachedCursorStep`;
- adapter `RetainedOutput`, its `outputs` map, and final prepared-output
  retention inside `PreparationCache`;
- `prepared_paint_runs` and `PreparedParagraph::try_map_glyph_paint`;
- nested temporary `PreparedLine → PreparedRun → PreparedGlyph` collection
  where the flat artifact can be filled directly;
- the `Document` field and edit/append/commit construction path in
  plain `TextBlock`;
- deep membership/dedup validation helpers used only because the old graph
  repeated positions and source ranges;
- scene geometry tables that duplicate fields already authoritative in the
  artifact.

Names may change during implementation, but equivalent old responsibilities
must not survive under aliases.

Production source line count across `underwood/src/adapter`,
`underwood/src/scene`, `underwood/src/block.rs`, and `underwood_parley/src`
should decrease after the complete migration despite the new flat builder. If
it grows, implementation pauses for a coherence review. Tests, diagnostics,
and migration documentation are excluded from that crude count and are
expected to grow.

## Retain, derive, or recalculate

Memory and CPU are selected per fact instead of treating “retained” as
automatically virtuous.

### Retain once

Retain facts whose reconstruction is expensive or whose exact identity is the
published result:

- selected font instances and glyph identities;
- formed line boundaries and paragraph-local placement;
- compact glyph source-coverage topology;
- resolved interaction units and line-local visual order requested by the
  scene;
- the authored/projected source relation requested by the scene;
- immutable revision and capability identity at the scene root.

### Derive through borrowed joins

Do not retain values that are cheap joins of authoritative tables:

- absolute glyph and caret coordinates from paragraph origin, line placement,
  and local coordinates;
- caret rectangles from visual unit sides, line placement, and line metrics;
- movement targets from neighboring logical or visual units;
- traversed source from the crossed unit and the source map;
- semantic identity from a compact source/leaf index;
- selection rectangles from selected unit ranges and line geometry;
- public fragments, hits, movements, and source observations.

These derivations must be allocation-free and must not redo Unicode analysis,
font selection, shaping, or line breaking.

### Recalculate after eviction

It is acceptable to repeat expensive preparation when the application chose
not to retain its reusable inputs:

- a static label may discard analysis and canonical shaping after publication;
- a later width change or capability upgrade may therefore be cold;
- an editor or animated/resizable surface may budget warm formation state;
- diagnostics report the cold work and the exact warm residency price.

The current design retained 21.7 MB of warm adapter state across 1,000 labels
without improving matched typing. Recalculation is preferable to residency
that does not serve the measured workload.

This is not permission for hidden per-query shaping. Recalculation occurs only
at explicit mutable preparation boundaries.

## Safety without retained proof copies

The current system often expresses confidence by reconstructing, validating,
and retaining another owned value. Design-0021 instead uses safe Rust and
proof by construction:

- one checked public builder validates every table and freezes one artifact;
- private builders may produce trusted internal parts because the same module
  established their invariants;
- table indexes and ranges are checked once before publication;
- owner-qualified generation stamps prove cache freshness without deep
  comparison;
- the scene root stamps revision and epoch once; borrowed public views bind
  them when observed;
- `debug_assert!` and an explicit exhaustive `verify()` path preserve deep
  audit pressure outside release hot paths;
- immutable ownership prevents a validated artifact from changing underneath
  a borrowed view.

This remains memory-safe, panic-resistant at public boundaries, and
fail-closed for third-party adapters. What disappears is redundant runtime
evidence—not safety.

## Why this is a correction, not another optimization pass

The matched high-level Parley comparison falsified the current trade.

All live-heap values below subtract the engine's matched font baseline:

| 1,000 retained items | Underwood | Parley | Ratio |
|---|---:|---:|---:|
| display labels | 7,994,272 B | 3,378,240 B | 2.37× |
| editable labels, adapter facts evicted | 20,266,272 B | 3,378,240 B | 6.00× |
| editable labels, warm adapter facts | 41,973,152 B | 3,378,240 B | 12.42× |

The optional warm adapter layer does not improve the matched typing path:
one 1,000-item run measured 22.35 microseconds per edit without adapter facts
and 22.27 microseconds with them. It buys warm width, style, and capability
reuse, not text-edit latency.

Twenty-one-sample medians for the committed shape were:

| Operation | Scale | Underwood | Parley |
|---|---:|---:|---:|
| exact repeat | 64 | 125 ns/item | 192 ns/item |
| exact repeat | 1,000 | 165 ns/item | 189 ns/item |
| localized edit, default | 64 | 22,679 ns | 3,045 ns |
| localized edit, default | 1,000 | 22,520 ns | 3,026 ns |
| localized edit, warm | 64 | 22,103 ns | — |
| localized edit, warm | 1,000 | 22,213 ns | — |
| cold churn | 64 | 18,708 ns/item | 6,716 ns/item |
| cold churn | 1,000 | 14,261 ns/item | 4,842 ns/item |

The current edit is O(change), but its fixed cost is not acceptable. A matched
`malloc_history` observation reports 132–135 Underwood allocation calls and
roughly 42–50 KiB requested per edit, versus 3 calls and about 1.1 KiB for
Parley.

The allocation stacks identify structural duplication rather than allocator
noise. In the 1,000-label warm editable process:

- adapter cursor-movement construction retains a 6.27 MB allocation class;
- scene movement lowering retains another 5.38 MB allocation class;
- adapter glyph lowering retains 2.30 MB;
- scene geometry retains further glyph and interaction tables;
- each one-paragraph output constructs document, scene-spine, cache-key, and
  sidecar owners independently.

The model earns two claims:

- exact repeat is allocation-free and faster than Parley in this fixture;
- after the indexed-query correction, 1,000-unit exact, closest, and byte
  queries are 61/77/38 ns versus Parley's 152/204/176 ns.

Those results preserve the retained lifecycle and sublinear lookup strategy.
They do not excuse the editable representation.

## Goals

- One final paragraph form is owned once and traversed by adapter, scene,
  rendering, hit testing, selection, editing, export, and diagnostics.
- Default published editable residency is within 2× matched Parley `Layout` at
  1,000 labels.
- Display-label residency is within 1.5× matched Parley `Layout`.
- Localized edit latency is within 2× matched Parley and does not scale from
  64 to 1,000 unchanged siblings.
- One edit performs at most 16 allocation calls and requests at most 8 KiB in
  the checked macOS wind tunnel.
- Exact repeats remain allocation-free.
- Indexed hit and position queries remain sublinear and competitive.
- Warm width, style, and capability reuse has an explicit, separately measured
  residency price.
- Mixed bidi, multi-selection, composition, collapsed source mapping, region
  flow, and PDF extraction remain exact.
- Core remains `no_std + alloc`, Rust 1.88 compatible, dependency neutral, and
  free of `unsafe`.

## Non-goals

- No separate label shaping or line-layout engine.
- No global arena, tracing collector, or process-wide mutable scene graph.
- No content-length heuristic that silently chooses retention.
- No attempt to reproduce Parley's private representation byte-for-byte.
- No compatibility layer that builds the old maximal form beside the new one.
- No weakening of adapter validation at the public trust boundary.
- No new production dependency.
- No coordinate-axis redesign hidden inside a memory patch. Design-0020 owns
  writing-mode readiness; this design uses logical names and does not add new
  horizontal assumptions.

## Laws

### 1. One published paragraph form

After successful publication, one paragraph's portable final layout and
interaction facts exist in one artifact. An adapter cache and a scene cache
must not each own equivalent final glyph, unit, caret, or movement tables.

### 2. Transfer or share; never re-lower by copying

`ParagraphFormation` returns a validated artifact owner. `LayoutEngine` either
takes that owner or shares it with a cache. It does not visit the artifact to
build a second owned portable representation.

### 3. Every fact has one authoritative table

- Glyph source coverage lives with glyph topology.
- Authored/projected correspondence lives in the paragraph source map.
- Cursor positions, caret anchors, and movement are derived from the
  interaction-unit and line tables.
- Paint fragments refer to glyph ranges and paint slots.

Public views may join those tables, but retained records do not repeat their
contents.

The artifact is a paragraph-local typed arena in the useful sense: contiguous
tables, compact indexes, one owner, and exact reclamation. It is not a
general-purpose arena with independent allocation, fragmentation, or stale
handle policy.

### 4. Common cases are inline; complexity spills

One source span, one semantic owner, whole-glyph paint, and one interaction
unit are represented inline. Cross-leaf graphemes, split ligatures, collapsed
source, and disjoint mappings use exact paragraph-local spill tables.

The rare representation may cost more. The ordinary representation may not
pay for the rare one.

### 5. Validation happens once per trust transition

Public adapter constructors validate ranges, table ordering, index coverage,
interaction-unit sides, and finite geometry. Scene binding validates UTF-8
boundaries against the projected text. Internal construction
then carries validated provenance. Scene publication checks identity and
revision compatibility without reconstructing and deep-comparing the artifact.

Debug and explicit verification paths may repeat exhaustive checks; release
queries and exact cache hits do not.

### 6. Optional formation state must earn residency

Adapter-retained analysis and shaping are charged separately and may be
evicted without invalidating a published artifact. The adapter does not retain
a final artifact solely to answer exact output reuse; scene and shared-output
caches own that job.

Warm retained state must demonstrate a measured width, style, or capability
reuse benefit. It is not described as a typing optimization when it does not
improve typing.

### 7. A compact block is still the same engine

`TextBlock` may use specialized single-paragraph source storage. It must still
feed the same projection, paragraph formation, source map, scene artifact,
capability facades, and cache laws as a paragraph inside `Document`.

### 8. Published sharing is coarse and explicit

An `Arc` is appropriate for one immutable paragraph artifact or independently
reusable sidecar shared by an engine cache, a current output, and an older
caller-held scene. It is not appropriate per glyph, cluster, caret, edge, or
small vector.

Borrowing is the ordinary observation mechanism. Owning an `Arc` or `Vec` in a
public result must correspond to a real lifetime crossing, not convenience for
the implementation.

### 9. Eviction changes future work, not current meaning

Dropping adapter formation state may make a later upgrade or reflow cold. It
cannot change the geometry, source observations, selection behavior, or native
queries of a published scene.

### 10. Failure is measured, not narrated away

If the residency, edit, allocation, query, or churn gates do not pass, the
design remains incomplete. Richer semantics may explain some overhead; they do
not waive the gates.

## Retained shape

Conceptually:

```text
ParagraphSceneSegment
├─ Arc<PreparedLayoutBase>
│  ├─ line table
│  ├─ run/font-instance table
│  ├─ flat glyph table
│  ├─ compact projected glyph coverage
│  └─ rare coverage spills
├─ line placement/adjustment table
├─ paint binding
├─ Option<Arc<ParagraphSourceMap>>
├─ Option<Arc<PreparedInteractionTopology>>
│  ├─ unit geometry and projected source spans
│  ├─ rare semantic/visual-slice spills
│  ├─ unique position + caret table
│  ├─ visual adjacency indexes
│  └─ logical adjacency indexes
└─ optional semantic/native indexes that measurements justify
```

The exact sidecar allocation split follows measured sharing. One artifact with
several flat vectors may be cheaper than several `Arc` allocations. The
important boundary is independent reuse and release, not a diagram-driven
allocation count.

### Compact indexes

Paragraph-local indexes and ranges use checked `u32` storage. The paragraph
contract already rejects projected text larger than `u32::MAX`, so `usize`
ranges waste space without representing additional valid input.

Sentinel values are private and validated. Public APIs expose typed borrowed
views rather than raw integers.

### Geometry scalars

This design does not silently convert every coordinate to `f32`.
Paragraph-local glyph geometry is a candidate because it originates in
font-scale floating-point data and is later translated by a document-space
origin. That choice requires error-bound tests over long lines, extreme sizes,
region placement, fractional transforms, and vertical-ready logical axes.

Until those tests earn a narrower scalar, compaction comes from ownership,
indexes, and deleting duplicate forms.

## Portable adapter contract

The public adapter vocabulary becomes flat and table-oriented. Conceptually:

```rust
pub struct PreparedParagraph {
    layout: Arc<PreparedLayoutBase>,
    interaction: Option<Arc<PreparedInteractionTopology>>,
}
```

The exact public constructors may use builders so malformed third-party
adapters still fail closed. The important properties are:

- lines name contiguous run ranges;
- runs name contiguous glyph ranges;
- glyphs carry source-coverage topology independent of paint-slot values;
- interaction positions are unique and sorted/indexed once;
- movement edges name position and unit indexes;
- capabilities below selection do not build position or movement tables;
- the complete validated owner moves through `ParagraphFormationOutput`.

Underwood's Parley adapter constructs the flat tables directly from formed
lines. It does not first build nested `PreparedLine → PreparedRun → Vec` values
and then flatten them in core.

## Paint and source ownership

The adapter owns source-to-glyph coverage because shaping establishes
ligatures, components, marks, and unrendered controls. It does not own
application paint slots.

The portable glyph topology therefore records:

- one projected source span for the ordinary whole-glyph case;
- ordered fractional or clipped source coverage only for a split glyph;
- explicit unrendered projected ranges.

`LayoutEngine` binds those spans to `PaintRun`s when constructing the compact
paint table. A paint-slot change does not clone prepared lines or call back
into shaping.

Authored `TextId`, semantic identity, and document revision remain in the
source map and source snapshot. Hot glyph and interaction records keep compact
projected spans or leaf indexes.

## Derived interaction navigation

The adapter remains responsible for Unicode analysis, extended-grapheme
grouping, resolved bidi levels, and line-local visual unit order. Core does
not infer interaction from glyph order.

The compact artifact already expresses the authoritative inputs:

```text
InteractionUnit {
    projected source range,
    resolved bidi level,
    left and right source positions,
    boundary and whitespace facts
}

PreparedLine {
    source range,
    break reason,
    interaction units in visual order
}
```

Logical lookup binary-searches source-ordered lines and units, with a fallback
inside genuinely visually reordered bidi lines. Visual movement follows
line-local unit order and formed-line boundaries. Soft-wrap affinity and
mandatory-break caret placement are derived from the same rules as Parley's
cluster cursor. Hit placement supplies adjusted inline coordinates, so
selection obtains exact carets without a retained caret table. Movement
returns source observations by joining the crossed unit to the paragraph
source map. All are borrowed and allocation-free.

The existing exact, closest, and byte-position binary searches remain. Packing
must not restore line-local scans over long unwrapped text.

## Lightweight `TextBlock` source state

The current `TextBlock::plain` allocates an empty `Document`, starts an edit,
appends one paragraph, appends one leaf, commits a persistent 32-way sequence,
and wraps the result in a document snapshot. That is correct but not
lightweight.

The replacement is conceptually:

```text
TextBlock
└─ Arc<BlockState>
   ├─ DocumentId + revision + paragraph version
   ├─ fixed paragraph/text/semantic identities
   └─ Arc<str>
```

An edit copies or reuses one string and publishes one new `BlockState`.
`TextBlockSnapshot` shares it cheaply.

Projection consumes an internal borrowed paragraph-source view implemented by
both `DocumentSnapshot` paragraphs and `TextBlockSnapshot`. This is not a new
shaping cache or a second scene path. Tests compare block and document
artifacts for identical text, style, paint, width, direction, source mapping,
and capabilities.

Rich multi-leaf blocks remain document-backed until a real compact run model
is designed. The plain-label fast path must not constrain document semantics.

## Adapter formation state

`PreparationCache` is split by reusable stage instead of retaining every
intermediate and final form in one record:

- analysis and itemization;
- canonical shaping and spacing base;
- line-formation state for one constraint;
- engine-owned scratch.

The implementation removes `prepared: Option<PreparedParagraph>` and
`prepared_paint_runs`. Exact prepared-output reuse is already a scene/shared
artifact responsibility. Paint binding moves to core.

Tables retained only to rebuild another retained table are challenged
individually. Base advances, logical clusters, character indexes, scripts,
formed lines, and style-index arrays remain only when their measured
invalidation path needs them. Scratch capacity is recycled at engine scope
when persistent ownership is unnecessary.

Diagnostics report stage bytes, not one opaque adapter total.

## Invalidation

Style, paint, source, constraints, and feature policy carry monotonic
generations or immutable identity stamps at their owning boundary.

The preflight key is composed from:

- paragraph/source version;
- analysis, shaping, flow, paint, and paragraph-style generations;
- normalized constraint and region identity;
- normalized requested capabilities;
- adapter/source epoch where required.

An O(1) preflight hit does not build projected text, style vectors, or source
keys merely to compare and drop them. A miss builds and validates the exact
changed input once.

Generation stamps prove identity within their owner. They are not hashes and
cannot collide across unrelated documents or style owners.

## Edit path

The steady-state one-block edit should be:

1. mutate one reusable `String` or allocate one replacement string;
2. publish one compact source state;
3. miss one paragraph generation key;
4. reuse engine-owned analysis, shaping, lowering, and projection scratch;
5. form and validate one flat paragraph artifact;
6. bind one compact source/paint/interaction view;
7. path-copy only the affected scene root;
8. release or reuse the previous paragraph artifact.

No step clones an unchanged document tree, constructs four cursor transitions
as owned source values, builds nested run vectors only to flatten them, or
validates an internally produced value a second time.

The target is a shorter path with fewer participating types. Scratch buffers
may remove residual allocation only after the persistent representation has
been simplified; they are not a justification for keeping the old pipeline.

## Relationship to Design-0020

Design-0020's logical-axis readiness work remains a prerequisite for any
coordinate-field redesign. This design requires its packed records to use
logical `inline`/`block` terminology internally and physical geometry only at
the public scene boundary.

Compaction must not freeze:

- `x` as inline;
- `y` as block;
- scalar `paragraph_y` as the only scene placement;
- horizontal-only line lookup or caret rectangles.

The first compaction may preserve existing scalar widths and physical public
types while moving them behind one private logical-axis choke point.

## Public migration

This is a deliberate breaking adapter migration:

- nested `PreparedLine`, `PreparedRun`, `PreparedGlyph`, and complete
  `PreparedCursorMovement` construction move to flat artifact builders/views;
- `PreparedParagraph::{try_new, try_new_with_features}` no longer accept a
  cursor-movement iterator;
- `PreparedCaret`, `PreparedCursorMovement`, `PreparedCursorStep`, and their
  borrowed topology views are removed;
- ordinary paint-slot and source coverage leave adapter-prepared glyph values
  in favor of core binding from authoritative paragraph paint runs;
- `ParagraphFormationOutput` transfers or shares one validated artifact;
- custom adapters migrate from repeated nested constructors to one checked
  paragraph builder;
- public scene and `TextBlock` behavior remain source- and revision-correct,
  but no representation or pointer-identity compatibility is promised.

There will be one migration note with before/after custom-adapter examples.
There is no compatibility shim retaining both forms.

The cursor portion of that migration is already concrete. A custom adapter
previously constructed every position, caret, and four-way transition after
constructing its lines:

```rust,ignore
PreparedParagraph::try_new_with_features(
    paragraph,
    text_len,
    direction,
    features,
    lines,
    movements,
)
```

It now supplies only validated lines whose interaction units carry exact
source, bidi, and visual-side facts:

```rust,ignore
PreparedParagraph::try_new_with_features(
    paragraph,
    text_len,
    direction,
    features,
    lines,
)
```

Scene navigation and carets are derived from those units. Adapters must not
rebuild the removed graph privately.

Ordinary glyph paint construction also changes:

```rust,ignore
// Before: every glyph retained another source range and paint slot.
GlyphPaintCoverage::whole(source.clone(), slot)?

// After: PreparedGlyph already owns source; core owns projected paint runs.
GlyphPaintCoverage::whole()
```

Only a glyph genuinely split across paint boundaries retains
`GlyphPaintSegment` records, and every such segment remains explicitly
clipped. Adapters must not copy ordinary paint runs into glyph records.

## Required wind tunnels

All runs use the matched source, font, style, width, and scale fixture in
`benches/residency-compare`.

1. Display, editable/default, and editable/warm labels at 64 and 1,000.
2. One mixed editable paragraph among 63/999/2,047 display siblings, with and
   without warm adapter state.
3. Exact repeat and localized edit, 21 samples.
4. Cold creation/destruction churn with a 64-item retained window.
5. Exact point hit, closest point hit, byte-position lookup, visual movement,
   logical movement, and selection geometry at 64/1,000/8,192 units.
6. Width churn and display-to-editable warm/cold upgrades.
7. Mixed LTR/RTL, Arabic marks, cross-script graphemes, ligatures, collapsed
   whitespace, mandatory breaks, multi-selection, and composition.
8. One block versus one equivalent document paragraph.
9. Deterministic category bytes, live heap, allocation histories, and process
   footprint kept as distinct observations.

The blocking numeric gates are:

| Gate | Required result |
|---|---:|
| 1,000 display-label live-heap delta | ≤ 1.5× Parley |
| 1,000 default editable-label live-heap delta | ≤ 2× Parley |
| localized-edit 21-sample median | ≤ 2× Parley |
| localized-edit allocation calls | ≤ 16 |
| localized-edit requested bytes | ≤ 8 KiB |
| exact repeat allocations | 0 |
| 1,000-unit hit/position latency | ≤ 2× matched Parley |
| sibling scaling from 64 to 1,000 | no material edit-work growth |
| obsolete retained forms | deleted, not wrapped |
| affected production source | net decrease or explicit coherence re-review |

If a gate proves physically impossible because Underwood exposes an operation
Parley does not represent, the comparison must add a matched capability rather
than waive the gate. Any changed threshold requires a separate human decision
with raw evidence.

## Sequencing

1. Check in the matched failure proof and keep it reproducible.
2. Replace the portable nested adapter output and scene copy together. A flat
   adapter artifact beside copied scene geometry is not landable.
3. Delete movement/caret topology and derive every query from authoritative
   line and interaction-unit facts in one vertical slice.
4. Move paint binding out of adapter output and remove retained final prepared
   output.
5. Introduce compact `TextBlock` source state through the common paragraph
   source view.
6. Add generation preflight and engine-owned scratch after the new ownership
   shape makes the remaining allocations visible.
7. Rerun the complete wind tunnel after every slice; revert changes that add
   complexity without moving a blocking metric.
8. Delete every superseded record, constructor, validation pass, and cache
   path before calling the migration complete.
9. Run full Rust 1.88, `no_std`, docs, fmt, Clippy, tests, repository, and
   protected-remote gates.

## Rook audit

### Real

- Exact scene-root reuse is already allocation-free.
- Persistent document and scene spines already make sibling work sublinear.
- Capability-scaled sidecars are physically absent when not requested.
- Paragraph-local source mapping already centralizes one-to-many provenance.
- Indexed hit and byte-position queries already beat Parley at the 1,000-unit
  scale in the matched fixture.

### Mirage risks

- A flat artifact is theater if the adapter keeps the old nested final output.
- Compact movement indexes are theater if public queries reconstruct and
  allocate complete steps on every call.
- A lightweight block is theater if `prepare_block` immediately rebuilds a
  temporary `DocumentSnapshot`.
- Generation keys are unsafe theater if they are process-global counters
  without owner identity.
- Scratch is theater if retained outputs still clone its contents into several
  equivalent tables.
- An `Arc` sidecar split is theater if every capability is always requested and
  every small table receives its own allocation.
- Better deterministic accounting is theater if live heap, allocation count,
  latency, and churn do not improve.
- A borrowed facade is theater if it allocates an owned joined record before
  returning.
- “Safety” is theater if the second proof copy can disagree with the first
  authoritative table.

### Most dangerous gap

The public adapter contract currently encodes the expensive representation:
nested owned lines/runs/glyphs plus a complete cursor graph. Merely optimizing
private scene records leaves the adapter constructing and retaining the same
cost before publication. The adapter artifact and scene traversal must change
together.

## Alternatives rejected

### Explain the overhead as richer editing

Underwood is richer, but the matched edit is about 7× slower while default
editable residency is about 6× larger. The capabilities do not justify those
ratios.

### Keep maximal data only for editors

Capability scaling already does this. The maximal editor itself remains too
large, and labels still pay too much source/cache ceremony.

### Keep the graph and use smaller fields

Smaller fields help, but four complete transitions per position repeat
topology. Indexing deletes information duplication and makes the relationship
explicit.

### Use a global arena

It can reduce allocation count while weakening immutable publication,
paragraph reclamation, old-scene lifetime, and cache accounting. One paragraph
artifact already provides the useful arena boundary.

### Cache high-level Parley `Layout`

It would couple the core to high-level Parley, surrender Underwood's region,
source, capability, and publication contracts, and create a second path for
non-Parley adapters. The comparison is a performance floor, not an ownership
transfer.

### Drop revision-safe and source-complete behavior

That would make the benchmark smaller by measuring a different product.
Underwood instead stores those facts once and refers to them compactly.

## Approval gate

Approval authorizes:

- the breaking portable-adapter migration;
- replacement of nested prepared and scene records with one flat artifact;
- indexed editable topology;
- moving paint binding from adapter output to core;
- compact single-paragraph `TextBlock` source storage;
- generation-keyed preflight and trusted internal construction;
- removal of compatibility code made obsolete by the new shape.

It does not authorize a new production dependency, `unsafe`, a serialized
artifact format, renderer/toolkit policy, a global arena, or weakening
source/bidi/IME correctness.
