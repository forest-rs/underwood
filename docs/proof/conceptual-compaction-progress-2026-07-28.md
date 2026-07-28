# Conceptual compaction engineering ledger

- **Campaign:** `und-0re`
- **Design:** Design-0022
- **Baseline commit:** `3fe34114acafa630c58151e29d795359e00154b7`
- **Status:** complete; reviewed landing artifact in PR #33

This is an external engineering ledger for a text framework, not runtime proof
state. It records complete concepts deleted, API migrations, source movement,
and product gates so source reduction cannot disguise a performance,
residency, or correctness regression.

## Slice 1: direct scene traversal

The first slice deletes the non-operational scene-facade taxonomy:

- `SceneDisplay`;
- `ProjectedSceneDisplay`;
- `SceneSourceAccess`;
- `ProjectedSceneSourceAccess`;
- `SceneSemanticAccess`;
- `ProjectedSceneSemanticAccess`;
- `ForeignSceneView`;
- the complete `scene/facades.rs` module.

Display was already available directly on `TextScene` and `CompositionScene`.
Semantics now returns its iterator directly. Lines, fragments, and glyphs
answer their own fallible source query, so it is impossible to combine a view
with a source facade from another scene. The old pointer/revision identity
check and its public error disappear with that invalid state.

Only operation closures remain. `SceneInteraction`, `SceneSelection`,
`SceneEditing`, `ProjectedSceneInteraction`, and `ProjectedSceneEditing` moved
to `scene/sessions.rs`; they amortize a capability check across a real
pointer, selection, keyboard, or IME operation.

Successful session acquisition is now O(1). The published scene root stores
the union and intersection of resident paragraph capabilities. Persistent
spine nodes remain unchanged; the aggregate is deliberately not charged to
every paragraph or tree node. A failure may still scan to name the first
paragraph missing the requested capability.

### Public migration

```rust,ignore
// Before
for line in scene.display().lines() { /* ... */ }
let sources = scene.sources()?.for_line(line)?;
for semantic in scene.semantics()?.iter() { /* ... */ }

// After
for line in scene.lines() { /* ... */ }
let sources = line.sources()?;
for semantic in scene.semantics()? { /* ... */ }
```

Every workspace caller, including the PDF adapter, visual proof, headless
example, and live showcase, uses the direct surface.

### Source ratchet

`tokei` and `scc` independently report the same affected-tree counts:

| State | Physical lines | Rust code lines |
|---|---:|---:|
| Baseline | 21,366 | 18,407 |
| After slice 1 | 21,013 | 18,157 |
| Deleted | 353 | 250 |

This meets the slice-level requirement to delete a complete concept. The
campaign-level `<= 17,000` Rust-code gate remains open.

### Product gates

- workspace tests: green;
- strict all-target/all-feature Clippy: green;
- workspace formatting: green;
- repository policy: green;
- no dependency or `unsafe` added;
- source, bidi, region, PDF, showcase, and visual snapshot callers remain
  executable through the migrated API.

The final campaign gate will rerun the release allocation, residency, edit,
repeat, and query wind tunnels after the adapter-construction deletion, rather
than presenting an intermediate compile/test checkpoint as a performance win.

## Slice 2: ordinary prepared traversal

The second slice deletes the four named traversal containers:

- `PreparedLines`;
- `PreparedRuns`;
- `PreparedGlyphs`;
- `PreparedInteractionUnits`.

The line, run, glyph, and interaction-unit views remain because they perform
real joins between compact records and shared paragraph tables. Their
containers did not: each reimplemented slice iteration, indexing, length,
first, and last over a contiguous table range.

Public traversal now returns opaque exact-size, double-ended iterators.
Explicit `line`, `run`, `glyph`, and `unit` methods serve indexed queries;
matching count methods avoid materializing an iterator only to ask its length.
The cursor implementation keeps its one useful binary search directly over
the canonical line table. No compatibility wrapper or replacement container
was introduced.

### Public migration

```rust,ignore
// Before
let line = paragraph.lines().get(index)?;
for run in line.runs().iter() { /* ... */ }

// After
let line = paragraph.line(index)?;
for run in line.runs() { /* ... */ }
```

Every workspace consumer now uses the direct accessors or standard iterator
operations.

### Source ratchet

| State | Physical lines | Rust code lines |
|---|---:|---:|
| Baseline | 21,366 | 18,407 |
| After slice 1 | 21,013 | 18,157 |
| After slice 2 | 20,728 | 17,927 |
| Deleted since baseline | 638 | 480 |

The affected tree still contains the same 31 production files. `tokei` and
`scc` agree on the Rust-code count. The campaign-level `<= 17,000` gate remains
open; flat adapter construction is the next deletion.

### Product gates

- workspace tests: green;
- strict all-target/all-feature Clippy: green;
- workspace formatting: green;
- repository policy: green;
- no dependency or `unsafe` added;
- adapter traversal, cursor derivation, source mapping, PDF, showcase, and
  visual snapshot callers remain executable through the migrated API.

## Slice 3: flat prepared-paragraph ingestion

The third slice deletes the nested prepared-output construction protocol:

- `PreparedParagraphBuilder`;
- `PreparedLineBuilder`;
- `PreparedRunBuilder`;
- their `failed`, `finished`, `Option`, and `Drop` poisoning states;
- `PreparedParagraphCapacity`;
- `prepared_capacity` and `lowered_glyph_count`;
- `PreparedLineRecord`;
- `PreparedRunRecord`;
- `PreparedInteractionUnitRecord`.

`PreparedLine`, `PreparedRun`, and `PreparedInteractionUnit` are now the
canonical compact records. A backend appends them and their sparse spill data
to one flat `PreparedParagraphData`, then crosses
`PreparedParagraph::try_from_data` once. That boundary checks table
partitions, source coverage, ordering, sparse indexes, and capability closure.
Internal traversal trusts the resulting immutable `PreparedParagraph`.

Underwood Parley writes the flat tables directly from formed Parley Engine
facts. It no longer walks every line, run, cluster, and glyph in a second
capacity-counting lowering pass. Cheap existing run, coordinate, character,
and exceptional reshaping counts provide capacity estimates without creating
a second output model.

### Public migration

```rust,ignore
// Before
let mut paragraph = PreparedParagraphBuilder::with_features(/* ... */);
let mut line = paragraph.begin_line(line)?;
let mut run = line.begin_run(run);
run.push_glyph(glyph)?;
run.finish()?;
line.finish()?;
let paragraph = paragraph.finish()?;

// After
let mut data = PreparedParagraphData::with_capacity(/* ... */);
let glyphs_start = data.glyph_count();
data.push_glyph(glyph)?;
data.push_run(run, coords, unrendered, glyphs_start..data.glyph_count())?;
data.push_line(line, units, runs)?;
let paragraph = PreparedParagraph::try_from_data(/* ... */, data)?;
```

The flat batch deliberately exposes no `begin`, `finish`, or failure-poisoning
state. Failed scalar appends roll back their own sparse spill entries; a
partially populated batch cannot become a prepared paragraph.

### Source ratchet

| State | Physical lines | Rust code lines |
|---|---:|---:|
| Baseline | 21,366 | 18,407 |
| After slice 1 | 21,013 | 18,157 |
| After slice 2 | 20,728 | 17,927 |
| After slice 3 | 19,457 | 17,823 |
| Deleted since baseline | 1,909 | 584 |

The small code-line movement is honest: the slice replaces three public state
machines and three runtime mirrors with one documented third-party ingestion
batch and its single validator. It is a real conceptual deletion, but it does
not satisfy the campaign source gate by itself. Committed and composition
query duplication is the next large target.

### Product gates

- workspace tests and doc-tests: green;
- strict all-target/all-feature Clippy: green;
- workspace formatting and repository policy: green;
- no dependency or `unsafe` added;
- actual localized-edit allocation: 15 calls / 2,886 bytes, unchanged from
  the immediately preceding slice;
- exact repeat, exact hit, closest hit, and represented-position queries:
  zero allocations;
- 1,000-label cold display: 13,075 calls / 3,284,912 bytes, the same call
  count and 32 bytes of extra estimated table capacity per paragraph;
- seven-sample median localized edit: 5,726 ns at 64 paragraphs and 5,709 ns
  at 1,000 paragraphs, both below the campaign baseline;
- 64-unit exact/closest/position query medians: 69/101/68 ns. Closest is
  unchanged, position improves materially, and exact remains within
  nanosecond-scale host noise.

The current 64-label churn median is 11,254 ns against the 10,059 ns campaign
baseline. This slice does not touch churn-side algorithms and the operation is
short enough to be host-noisy, but the final campaign must reproduce or beat
the baseline with the full sample count; the gate remains open.

## Slice 4: one typed scene vocabulary

The fourth slice converges committed and composition display traversal behind
one generic implementation while preserving distinct authored and projected
source types:

- `Scene<T, Identity>` owns the common scene root; `TextScene` and
  `CompositionScene` remain public semantic aliases;
- `SceneLines`, `SceneLineView`, `SceneFragments`, `SceneFragmentView`,
  `SceneGlyphs`, `SceneGlyphView`, `TextSources`, and `TextUnitView` provide
  the common traversal algorithms;
- the projected names remain aliases that make generated provenance explicit
  at call sites;
- selection contains interaction, and editing contains selection, so the
  capability lattice no longer repeats inherited operation forwarding.

Diagnostic and output records now expose their documented fields directly.
The migration removes trivial getter and positional-constructor thickets from
`FormationWork`, `LineShapingWork`, cache diagnostics, preparation traces,
residency observations, scene records, and scene outputs. Named struct fields
make counter wiring visible instead of accepting long runs of adjacent
booleans and integers.

A reproducible random-source sample then found two more complete deletion
targets:

- `underwood_parley::validation` repeated UTF-8 coverage and style-index
  checks over projection tables constructed and validated by `LayoutEngine`.
  `ParagraphInput` is now `#[non_exhaustive]` and documents that backend
  contract; the redundant 123-line adapter module is gone.
- `SceneRegionAttempts` and `SceneParagraphResidencies` merely forwarded
  existing exact-size iterators. Their methods now return opaque standard
  iterators directly. Redundant iterator convenience methods likewise defer
  to ordinary `next`, `nth`, and `last`.

Common preparation publication now passes through one `finish_scene` boundary
for summary, region binding, trace construction, and scene-core construction.
This is a control-flow convergence, not a second scene representation.

### Public migration

```rust,ignore
// Observation records are ordinary data.
let shaped = output.work.shape.paragraphs;
let scene = &output.scene;

// Capability sessions inherit their prerequisite operations.
let editing = scene.editing()?;
let hit = editing.hit_test(point);
let selection = editing.between(anchor, extent)?;

// Forwarding iterator containers are ordinary opaque iterators.
let first_attempt = output.region_transcript().and_then(|trace| trace.attempts().next());
let second_source = hit.source.sources().nth(1);
```

The old getter methods, positional diagnostic constructors,
`SceneRegionAttempts`, and `SceneParagraphResidencies` have no compatibility
aliases. `ParagraphInput` fields remain readable by third-party formation
backends, but external code cannot construct the validated record with a
struct literal.

### Source ratchet

| State | Physical lines | Rust code lines |
|---|---:|---:|
| Baseline | 21,366 | 18,407 |
| After slice 3 | 19,457 | 17,823 |
| After slice 4 | 18,849 | 16,486 |
| Deleted since baseline | 2,517 | 1,921 |

`tokei` and `scc` agree on 16,486 Rust code lines. The implementation is below
the accepted design's 17,000 requirement and 16,500 stretch gate. The
aspirational 16,000 target remains a review prompt rather than a reason to
compress meaningful algorithms.

### Product gates

- all workspace tests and doc-tests pass;
- strict all-target/all-feature Clippy and formatting pass;
- no dependency or `unsafe` was added;
- the committed/projected public distinction, sparse capability residency,
  source completeness, bidi interaction, regions, PDF, showcase, and visual
  snapshots remain covered by the unchanged executable suites.

## Slice 5: unreachable presentation and duplicate-value deletion

An external line-by-line scalpel review found a presentation path that looked
supported in the adapter contract but could not be produced by the only
production backend. Underwood Parley rejects a paint boundary inside one
shaped glyph as `UnsupportedPaintCoverage`; only hand-built test adapters
could populate split-glyph paint records.

This slice deletes that mirage end to end:

- `GlyphPaintCoverage`, `GlyphPaintSegment`, and the split-paint table;
- fragment paint clips and the scene/PDF/rendering split branch;
- per-glyph scene instance identity used only to deduplicate that split path;
- the constant identity fragment transform;
- the complete `adapter/paint.rs` module and split-only test adapters.

Ordinary paint is now the only successful contract: one glyph has one paint
slot and run-sized scene fragments. A paint boundary inside one glyph still
fails explicitly instead of being approximated.

The same pass deletes values that were computed or retained without a product
consumer:

- `PreparationErrorKind::{Cancelled, MissingCapability}`;
- font diagnostic names, a full-file FNV digest, and stored units-per-em used
  only by `Debug`;
- adapter peak-byte, checkpoint-restore, and accepted-candidate counters;
- the line-former overflow payload and error taxonomy that collapsed at its
  only match;
- a shaped-glyph “cache” field that never survived one call;
- the paint invalidation facet, whose only effect was to select a validation
  that is now simply performed where required;
- named forwarding containers for projection segments and prepared
  interaction slices.

A second scene/cache pass removes:

- per-key backend epochs from a cache that is emptied whenever the epoch
  changes;
- unused cluster/semantic spine offsets and the summary counters that existed
  only to advance them;
- zero-valued selection, navigation, and native-input residency categories
  after those operations became allocation-free derivations;
- incremental scene-cache byte accounting beside the existing recomputed
  category accounting;
- duplicate constraint-key types and option-comparison helpers;
- a second exact-reuse predicate, so preflight and post-projection reuse now
  consume the same facet decision;
- a second region-transcript replay for retained and shared preparations.

Paint runs no longer cross the paragraph-formation seam or participate in
shared-preparation identity. Prepared facts contain no paint data; scene paint
topology is the single place that resolves current paint slots and rejects a
paint boundary through one glyph.

Two duplicate-value contracts also disappear. The Parley producer no longer
builds a coverage bitmap for units that the public final-ingestion boundary
must check for third-party adapters anyway. More importantly,
`PreparedInteractionUnit` and `PreparedLine` no longer accept advances that
their builders immediately recompute and compare with tolerances. The
canonical builder derives unit advance from retained shaping slices and line
advance from retained units exactly once.

The review's three explicit non-targets remain:

- `validate_topology` is the final defense of panic-free borrowed views;
- run validation protects a subsequent hard shaping index;
- `reshape_scratch` preserves rejected-candidate correctness, not merely
  allocation capacity.

Region fit is checked by the public `RegionAttempt` transcript record and
matched back to prepared lines when a scene consumes adapter output. The
duplicate height comparison in `PreparedLine` is gone.

### Public migration

```rust,ignore
// Advances are derived from the retained parts.
let unit = PreparedInteractionUnit::try_new(
    source,
    bidi_level,
    boundary,
    whitespace,
    left,
    right,
)?;
data.push_unit_parts(unit, slices)?;

let line = PreparedLine::try_new(
    source,
    reason,
    baseline,
    block_extent,
    content_ascent,
    content_descent,
)?;
data.push_line(line, units, runs)?;

// Font construction no longer computes or retains a diagnostic-only digest.
let font = Font::from_bytes(bytes)?;
```

`PreparedGlyph::try_new` no longer accepts paint coverage. Fragment
`paint_clip`, constant `transform`, and glyph `instance_id` observations are
removed. `ProjectionSegments` and `PreparedInteractionSlices` are replaced by
opaque exact-size iterators with ordinary iterator operations.
`ParagraphInput` no longer exposes paint runs. `SceneResidencyBytes` reports
only categories that can retain bytes.

### Source ratchet

| State | Physical lines | Rust code lines |
|---|---:|---:|
| Baseline | 21,366 | 18,407 |
| After slice 4 | 18,849 | 16,486 |
| After slice 5 | 18,203 | 15,951 |
| Deleted since baseline | 3,163 | 2,456 |

`tokei` and `scc` agree on the 15,951 Rust-code result. Three production
modules have disappeared from the calibrated tree. The result is 1,049 lines
below the required gate, 549 below the stretch gate, and 49 below the aspirational
16,000-line target, without replacing the deleted features with compressed
generic machinery.

### Correctness checkpoint

- all workspace tests and doc-tests pass;
- all workspace targets and features compile;
- no production dependency or `unsafe` was added;
- `CR/LF + combining mark` and `SPACE + ZWJ + Arabic` regressions found by the
  external conformance ledger are pinned as product tests; logical line source
  now covers non-shaping bytes in complete UAX #29 interaction units;
- cross-script and mixed-level graphemes, source-complete units, bidi
  interaction, region flow, CJK, PDF, showcase, and both CPU snapshots remain
  covered by executable tests.

The `TextBlock` style memo is the review's one challenged survivor. Removing
it added 62 allocation calls and 9,240 requested bytes across 64 cold
identical labels—almost exactly one allocation per label and an 8.3% call
regression. It remains as measured label-path machinery rather than an
unexplained cache.

Strict lint, portability, allocation, latency, and live-residency gates are
rerun below before this slice can land.

## Closure: measured product result

The compaction closes with the same product behavior and a materially smaller
implementation. It does not claim that Underwood is now irreducible. In
particular, Parley's simpler retained representation remains the right
external challenge: every additional Underwood table and cache should
continue to earn a capability or a measured result.

### Release latency

Seven matched release samples produced these medians:

| Scale | Operation | Underwood | Parley | Ratio |
|---:|---|---:|---:|---:|
| 64 | exact repeat | 71 ns | 185 ns | 0.38× |
| 64 | localized edit | 5,528 ns | 2,885 ns | 1.92× |
| 64 | warm localized edit | 5,562 ns | — | — |
| 64 | churn | 9,743 ns | 6,389 ns | 1.52× |
| 1,000 | exact repeat | 117 ns | 186 ns | 0.63× |
| 1,000 | localized edit | 5,877 ns | 2,974 ns | 1.98× |
| 1,000 | warm localized edit | 5,810 ns | — | — |
| 1,000 | churn | 8,338 ns | 4,601 ns | 1.81× |

The edit and churn gates pass at both scales. Exact repeat remains faster than
the matched Parley path.

Small-query timings are sensitive to host noise at this scale, so the closure
also built the baseline commit and current commit in separate worktrees and
ran 31 samples of 100,000 operations under the same conditions:

| 64-unit Underwood query | Baseline | Compacted |
|---|---:|---:|
| exact hit | 66 ns | 69 ns |
| closest hit | 105 ns | 102 ns |
| represented byte position | 89 ns | 67 ns |

Exact hit is unchanged within nanosecond-scale noise; closest hit and
represented-position lookup improve. At 1,000 units, the seven-sample current
medians are 81/113/95 ns for exact/closest/position, versus the campaign
baseline's 80/121/124 ns. Query gates therefore pass without relying on the
long-text result to hide a small-text regression.

### Allocation and residency

The optional counting allocator reports:

- stable repeat: zero calls;
- edit publication: 2 calls / 104 requested bytes;
- edited preparation: 16 calls / 2,888 requested bytes, with 13 calls /
  2,664 bytes peak-live growth;
- paint-only preparation: zero calls.

The result exactly meets both parts of the counting-allocator gate. The
matched `malloc_history` tunnel, which excludes the counting allocator's own
instrumentation effect, records 15 calls / 2,758 bytes for the same edited
preparation. Exact repeat, exact hit, closest hit, and represented-position
lookup allocate nothing in that profiler.

The last eight requested bytes were recovered by deleting `SceneCore`'s
duplicate paragraph count and reading the existing O(1) scene-spine root.
That leaves the O(1) capability union/intersection in six bytes of existing
structure padding; no compressed bit trick or second policy cache was added.

Three macOS live-heap samples were byte-identical:

| 1,000 retained labels | Bytes above own font baseline | Parley ratio |
|---|---:|---:|
| Underwood display | 3,139,344 | 0.929× |
| Underwood editable | 3,483,344 | 1.031× |
| Parley | 3,378,240 | 1.000× |

Bounded churn retains 303,072 bytes above Underwood's font baseline versus
291,008 bytes for Parley, or 1.041×. All are below the 1.25× gate.

The display and editable deltas are 160,752 bytes below the campaign baseline.
The external audit's unreachable split-paint deletion more than recovers the
small ordinary amortized flat-table reserve introduced when the exact
capacity-counting lowering pass disappeared.

### Correctness and portability

The final locked matrix is green:

- workspace tests and doc-tests;
- strict all-target/all-feature Clippy;
- rustdoc with warnings denied;
- Rust 1.88 across supported workspace targets;
- `x86_64-unknown-none` for `underwood` and `underwood_parley`;
- `wasm32-unknown-unknown` for both core crates;
- formatting, Taplo, spelling, repository policy, dependency duplication,
  Beads lint, and Beads cycle checks.

No production dependency or `unsafe` was added.

### Final Rook judgment

**Real:** the branch deletes complete state machines, wrapper families,
duplicate traversal implementations, and redundant validation. The flat
adapter boundary is checked once; internal traversal trusts it. Sparse scene
capabilities, source-complete mapping, regions, bidi editing, PDF export, and
composition remain executable behavior rather than architecture prose.

**Mirage retired:** the old facade taxonomy, foreign-scene defense, nested
poisoning protocol, exact-capacity proof pass, adapter validation replay, and
iterator-container vocabulary no longer exist under replacement names. The
follow-up Parley scalpel audit also deletes the unreachable split-paint
vertical, debug-only font identity work, permanently zero residency categories,
duplicate advance and slot proofs, pseudo-cache fields, and unread accounting.

**Remaining risk:** `PreparationErrorKind::InvalidOutput` remains a coarse
third-party-adapter diagnostic even though malformed-table tests isolate every
current cross-table invariant. Improving error location should not add retained
proof records or restore staged builders. More broadly, this result proves the
remaining representation meets current behavior and product gates; it does not
declare every surviving table permanently necessary.

### Final architecture and API audits

- **Alder:** the fence remains intact. `underwood_parley` owns Parley analysis,
  shaping, and line formation; `underwood` owns one backend-neutral checked
  artifact, scene placement, paint/source binding, sparse capabilities, and
  document interaction. No Parley type enters the core contract.
- **Cedar:** every workspace call site uses the direct scene and flat adapter
  surfaces. The design and changelog enumerate the breaking migration,
  including paint-independent inputs, derived advances, removed diagnostics,
  and `Font::from_bytes(bytes)`. Workspace rustdoc is warning-free.
- **Stoat:** the final matched measurements above preserve every latency,
  allocation, and residency gate. The audit caught and replaced one generic
  iterator formulation that was shorter but measurably slower.
- **Rook/Lynx:** no deleted state machine, facade, split-paint sidecar, or
  iterator container survives under a renamed wrapper. The two final review
  corrections make paint-only artifact reuse genuinely adapter-free and give
  equal-source logical clusters a deterministic order.
