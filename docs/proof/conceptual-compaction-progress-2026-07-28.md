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
