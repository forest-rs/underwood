# Conceptual compaction engineering ledger

- **Campaign:** `und-0re`
- **Design:** Design-0022
- **Baseline commit:** `3fe34114acafa630c58151e29d795359e00154b7`
- **Status:** implementation in progress

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
