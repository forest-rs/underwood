# Design-0022: Conceptual compaction

- **Status:** Accepted
- **Date:** 2026-07-27
- **Bead:** `und-0re.2`
- **Builds on:** Design-0021
- **Evidence:** `docs/proof/conceptual-compaction-baseline-2026-07-27.md`

## Decision

Underwood will keep the compact one-artifact runtime model from Design-0021
and replace its construction and traversal ceremony.

The migration has four required changes:

1. Replace the nested poisoned paragraph/line/run builders with one flat
   paragraph-data accumulator and one final checked ingestion boundary.
2. Merge temporary line, run, and interaction metadata mirrors into the
   canonical compact records wherever sparse storage does not require a
   distinct input form.
3. Delete non-operational scene facades. Display stays directly on scenes,
   source stays directly on the view that owns it, and semantics returns its
   iterator directly. Capability sessions remain only for interaction,
   selection, and editing operations that genuinely amortize a capability
   check and define an operation set.
4. Delete line/run/glyph/unit iterator-container families in favor of direct
   indexed lookup and opaque exact-size iterators over the retained views.

Committed and composition traversal will then be converged behind a private
position/source mode where that deletes code without weakening their public
type distinction.

This is a breaking migration with no compatibility shim.

## Product, not prover

Underwood is a high-performance, compact text framework. It is not a proof
system.

Correctness evidence lives in tests, conformance corpora, benchmarks, design
records, and optional diagnostics. The runtime keeps facts needed to render,
query, edit, reflow, export, or reuse expensive work. It does not keep a value
merely to certify another value, and it does not repeatedly validate canonical
tables after ingestion.

The checked adapter boundary remains important because a backend can be
implemented outside this repository. “Checked once” means:

- scalar conversion validates values when converting them to the compact
  representation;
- final ingestion validates cross-table ranges, ordering, source coverage, and
  capability closure exactly once;
- internal scene and query code trusts those canonical invariants;
- debug tests may run a slower exhaustive verifier, but release traversal does
  not.

## Fences

### Portable artifact

The adapter module owns the compact backend-neutral paragraph artifact and its
single untrusted-ingestion check; it explicitly does not own nested
construction state machines, scene placement, document identity, or records
whose purpose is to attest to other records.

### Parley adapter

`underwood_parley` owns conversion from Parley Engine analysis, shaping, and
formed-line facts into the flat portable artifact; it explicitly does not
construct a temporary paragraph object graph or run a capacity-counting copy
of the lowering pass.

### Scene

The scene owns document-space placement, paint/source binding, sparse
capability residency, and user-facing queries; it explicitly does not wrap one
scene reference in a separate type for every noun in the capability lattice.

### Authoring

`und-oh0.18` owns rich labels, document builders, parser feeds, and structural
editing ergonomics. This design uses those golden call sites to reject hostile
seams; it explicitly does not add rich authoring to the compaction campaign.

## Invariants

1. One immutable `PreparedParagraph` artifact remains the only retained owner
   of portable formed layout and requested interaction facts.
2. No adapter-final-output cache, copied scene glyph model, cursor graph, or
   clone-based repaint path returns.
3. Sparse display, source, semantics, hit, selection, navigation, and native
   capabilities remain physically omittable.
4. Source-complete multi-leaf and multi-slice mapping, mixed bidi interaction,
   exact regions, alignment, and justification remain behaviorally identical.
5. Successful hot capability checks are O(1). Constructing a detailed error
   may walk to the first missing paragraph.
6. Exact repeat, repaint, and existing query workloads allocate zero.
7. The new canonical records use logical inline/block vocabulary. Physical
   `kurbo` geometry remains the public scene/output boundary.
8. The core remains Rust 1.88 and `no_std + alloc`, with no new dependency or
   `unsafe`.

## Options

### A. Polish the existing layers

Keep `PreparedParagraphBuilder`, `PreparedLineBuilder`,
`PreparedRunBuilder`, their mirrored metadata values, and the facade families,
but improve names and factor common helpers.

This is rejected. It preserves the same states and invalid combinations and
would be source cosmetics rather than conceptual compaction.

### B. Flat checked batch plus direct borrowed traversal

Build compact flat records directly, validate cross-table topology once, expose
direct indexed views and opaque iterators, and retain only operational
capability sessions.

This is chosen. It preserves the backend seam and compact runtime while
deleting complete protocols.

### C. Let `LayoutEngine` provide a callback sink to the backend

Change `ParagraphFormation::form` into `form_into(&mut dyn ParagraphSink)`.
This could let the engine own output allocation, but it recreates a stateful
streaming protocol at the trait boundary, complicates cancellation/error
recovery, and makes the backend/core call direction less obvious.

This is rejected for the first compaction. A batch is simpler and can still
use accurate cheap capacity estimates.

### D. Merge Parley formation and scene publication

Allow `underwood_parley` to produce scene geometry directly.

This deletes types by destroying the backend-neutral boundary. It would make
Parley-specific types and invalidation policy load-bearing in core and is
rejected.

## Chosen adapter shape

The canonical path becomes:

```text
ParagraphInput + ParagraphConstraints
        ↓
Parley analysis / shaping / line formation
        ↓
flat PreparedParagraphData
        ↓ one cross-table validation
PreparedParagraph
        ↓ trusted borrowed traversal
scene placement + paint/source binding
```

`PreparedParagraphData` is an owned flat table bundle. It is not a nested
builder and has no `failed`, `finished`, `Drop` poisoning, or line/run lifetime
state.

Conceptually:

```rust,ignore
pub struct PreparedParagraphData {
    lines: Vec<PreparedLine>,
    runs: Vec<PreparedRun>,
    glyphs: Vec<PreparedGlyphRecord>,
    glyph_placements: Vec<PreparedGlyphPlacement>,
    split_glyph_paints: Vec<PreparedSplitGlyphPaint>,
    units: Vec<PreparedInteractionUnit>,
    interaction_slices: Vec<PreparedInteractionSlice>,
    interaction_slice_spills: Vec<PreparedInteractionSliceSpill>,
    source_order: Vec<u32>,
    normalized_coords: Vec<i16>,
    unrendered_source: Vec<SourceSpan>,
}
```

The exact fields may remain private behind ordinary `push_*`/count methods.
Those methods append one compact record; they do not create nested states or
perform cross-table validation. A third-party adapter cannot obtain a
`PreparedParagraph` without:

```rust,ignore
PreparedParagraph::try_from_data(
    paragraph,
    text_len,
    resolved_direction,
    features,
    data,
)
```

That function checks:

- line source order and complete paragraph coverage;
- line-to-unit and line-to-run table ranges;
- unit source coverage and optional visual source-order permutation;
- run source coverage and run-to-glyph/coordinate/unrendered ranges;
- glyph and unrendered-source containment;
- rare placement, paint, and slice spill indexes;
- feature-dependent table presence.

It does not rebuild a second set of tables, sort a copy merely to compare it,
or revalidate scalar properties already established by compact conversion.

### Underwood Parley call site

Today:

```rust,ignore
let mut paragraph = PreparedParagraphBuilder::with_features(
    input.paragraph(),
    text_len,
    direction,
    input.features(),
);
paragraph.reserve_exact(prepared_capacity(input.text(), preparation)?);

for formed in &preparation.formed_lines {
    let mut line = paragraph.begin_line(PreparedLine::try_new_in_slot(/* ... */)?)?;
    lower_visual_units(/* ... */, &mut line)?;

    for piece in &run_pieces {
        let mut run = line.begin_run(PreparedRun::try_new(/* ... */)?);
        run.extend_normalized_coords(coords);
        lower_glyphs_into(/* ... */, &mut run)?;
        append_unrendered_source(/* ... */, &mut run)?;
        run.finish()?;
    }
    line.finish()?;
}

let paragraph = paragraph.finish()?;
```

After:

```rust,ignore
let mut data = PreparedParagraphData::with_capacity(
    preparation.formed_lines.len(),
    preparation.shaped_text.runs().len(),
    preparation.shaped_text.glyphs().len(),
    preparation.interaction_units.len(),
);

for formed in &preparation.formed_lines {
    let units = lower_visual_units(/* ... */, &mut data)?;
    let runs = lower_runs_and_glyphs(/* ... */, &mut data)?;
    data.push_line(PreparedLine::new(/* metrics */, units, runs));
}

let paragraph = PreparedParagraph::try_from_data(
    input.paragraph(),
    text_len,
    direction,
    input.features(),
    data,
)?;
```

`with_capacity` uses cheap existing counts as estimates. It does not walk
every cluster and glyph in a separate `prepared_capacity` /
`lowered_glyph_count` pass. The allocation tunnel decides whether estimates,
amortized growth, or one additional reserve is best.

### Record convergence

- `PreparedLine` becomes the canonical retained line record by gaining compact
  unit/run ranges. `PreparedLineRecord` disappears.
- `PreparedRun` becomes the canonical retained run record by gaining compact
  glyph/coordinate/unrendered ranges. `PreparedRunRecord` disappears.
- `PreparedInteractionUnit` stores the compact advance and side flags directly.
  `PreparedInteractionUnitRecord` disappears.
- Glyphs retain the current common compact record plus rare placement/paint
  spills. A transient glyph input may remain only if scalar conversion is
  materially clearer than a direct `push_glyph` call.
- `PreparedParagraphCapacity`, `PreparedLineBuilder`, and
  `PreparedRunBuilder` disappear. `PreparedParagraphBuilder` disappears rather
  than becoming a renamed wrapper around `PreparedParagraphData`.

## Chosen traversal shape

The joined record views remain useful:

- `PreparedLineView`;
- `PreparedRunView`;
- `PreparedGlyphView`;
- `PreparedInteractionUnitView`.

The container wrappers do not:

- `PreparedLines`;
- `PreparedRuns`;
- `PreparedGlyphs`;
- `PreparedInteractionUnits`.

The replacement is direct lookup plus opaque exact-size traversal:

```rust,ignore
let first = paragraph.line(0);
for line in paragraph.lines() {
    for run in line.runs() {
        for glyph in run.glyphs() {
            render(glyph);
        }
    }
}
```

`lines()`, `runs()`, `glyphs()`, and `units()` return
`impl DoubleEndedIterator + ExactSizeIterator + Clone`. They map compact index
ranges to the existing borrowed views and allocate nothing.

This is intentionally ordinary Rust iteration. The replacement must not be a
generic table framework larger or harder to understand than the wrappers it
deletes.

## Chosen scene shape

### Display

Display is unconditional and already exists directly on both scene types.

Before:

```rust,ignore
let display = scene.display();
for line in display.lines() { /* ... */ }
for fragment in display.fragments() { /* ... */ }
```

After:

```rust,ignore
for line in scene.lines() { /* ... */ }
for fragment in scene.fragments() { /* ... */ }
```

`SceneDisplay` and `ProjectedSceneDisplay` disappear.

### Source

Source belongs to the line, fragment, glyph, or interaction unit being
observed.

Before:

```rust,ignore
let access = scene.sources()?;
let sources = access.for_line(line)?;
```

After:

```rust,ignore
let sources = line.sources()?;
```

Views carry enough scene/request identity to return
`MissingSceneCapability` if provenance was omitted. They cannot be combined
with another scene's source facade, so `SceneSourceAccess`,
`ProjectedSceneSourceAccess`, and `ForeignSceneView` disappear.

### Semantics

Before:

```rust,ignore
for fragment in scene.semantics()?.iter() { /* ... */ }
```

After:

```rust,ignore
for fragment in scene.semantics()? { /* ... */ }
```

`SceneSemanticAccess` and `ProjectedSceneSemanticAccess` disappear.

### Operational capability sessions

The following remain because they define real operation closures and amortize
capability checks across a pointer/key/IME event:

- hit interaction;
- selection;
- editing/native input;
- projected hit interaction;
- projected editing/native input.

They move beside the operations they expose rather than living in a general
facade taxonomy. Common methods may call private shared functions; a public
generic capability framework is not introduced.

`scene/facades.rs` is deleted as a module.

### Capability summaries

`SceneSummary` gains the union and intersection of resident
`SceneFeatures`. Successful “any paragraph” and “every paragraph” checks use
those summaries in O(1). A failed check may traverse to name the first missing
paragraph in `MissingSceneCapability`.

This both supports the simpler access surface and removes an O(paragraph)
operation currently paid whenever a host reacquires an interaction facade.

## Committed and composition traversal

Authored snapshot positions and projected composition positions remain
different public types. That distinction prevents native protocols from
confusing generated preedit bytes with committed document bytes.

Their line/fragment/glyph placement and iteration are the same algorithm.
After the required deletions above, a private sealed mode may parameterize:

- source observation type;
- position type;
- scene-root identity.

Public aliases preserve the existing semantic distinction. This convergence is
required only if it deletes at least 250 Rust code lines without increasing
monomorphized hot code or making rustdoc harder to read. Otherwise the
duplication is recorded as deliberate and left for the vertical-scene design.

## Logical-axis readiness

The new batch contract must not freeze horizontal assumptions into the compact
tables:

- advances and offsets are named `inline` and `block`, not `x` and `y`;
- line extent is named `block_extent`, not generic `height`;
- the current baseline is named `alphabetic_baseline`;
- interaction sides become `line_left` and `line_right`, whose future vertical
  physical mapping is top/bottom;
- scene placement performs the one logical-to-physical conversion;
- public scene geometry remains physical `kurbo` geometry.

This does not add vertical layout, writing-mode inputs, orientation itemization,
vertical metrics, or top-to-bottom shaping. It prevents a new public compact
record format from making those changes harder.

## TextBlock and authoring call sites

The plain retained block path remains calm:

```rust,ignore
let mut block = TextBlock::plain(id, "Save")?;
let output = layout.prepare_block(
    &block.snapshot(),
    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
)?;
block.set_text("Open")?;
```

No adapter table type appears in application authoring.

The authoring epic's richer target remains a stress test:

```rust,ignore
let page = Document::build(id, |doc| {
    doc.heading_1("TYPE, ALIVE.");
    doc.paragraph(|paragraph| {
        paragraph.text("One document bends around a float. ");
        paragraph.emphasis("مرحبا بالعالم");
        paragraph.action(action, "Source on GitHub");
    });
})?;
```

Design-0022 is acceptable only if the compact artifact remains an internal
preparation result below that call site. It does not implement this builder.

## Public migration

Removed adapter types:

- `PreparedParagraphBuilder`;
- `PreparedParagraphCapacity`;
- `PreparedLineBuilder`;
- `PreparedRunBuilder`;
- `PreparedLineRecord` and `PreparedRunRecord` as distinct private mirrors;
- `PreparedInteractionUnitRecord` as a distinct private mirror;
- `PreparedLines`;
- `PreparedRuns`;
- `PreparedGlyphs`;
- `PreparedInteractionUnits`.

Added or changed adapter surface:

- `PreparedParagraphData`;
- `PreparedParagraph::try_from_data`;
- direct compact table append/count methods;
- `PreparedParagraph::{line, lines}`;
- `PreparedLineView::{run, runs, unit, units}`;
- `PreparedRunView::{glyph, glyphs}`;
- logical-axis record names.

Removed scene types:

- `SceneDisplay`;
- `ProjectedSceneDisplay`;
- `SceneSourceAccess`;
- `ProjectedSceneSourceAccess`;
- `SceneSemanticAccess`;
- `ProjectedSceneSemanticAccess`;
- `ForeignSceneView`.

Changed scene surface:

- callers use existing direct display methods;
- source traversal starts from the observed view;
- `semantics()` returns the iterator directly;
- interaction/selection/editing capability failures remain explicit
  `Result`s;
- capability-session hot operations keep their current nonallocating result
  shapes.

The implemented convergence also makes result and diagnostic records ordinary
documented data:

- `SceneOutput`, `CompositionSceneOutput`, `WorkReport`, preparation traces,
  cache diagnostics, residency observations, and scene records expose named
  fields instead of duplicating them with trivial getters;
- positional constructors for work and cache-counter records disappear;
- editing inherits selection and interaction operations from the capability
  lattice rather than forwarding them again;
- `SceneRegionAttempts` and `SceneParagraphResidencies` disappear in favor of
  opaque standard iterators returned by their owning observations;
- standard iterator operations (`next`, `nth`, and `last`) replace redundant
  convenience methods where no indexed data structure is being exposed;
- `ParagraphInput` is a non-exhaustive, readable record whose ordered UTF-8
  coverage and style-index invariants are established by `LayoutEngine`.

There is no deprecated alias or hidden compatibility implementation.

## Deletion gates

The calibrated start is 18,407 Rust code lines and 21,366 physical lines over
the 31 affected files.

Required:

- at most 17,000 `tokei` / `scc` Rust code lines after the migration;
- delete `scene/facades.rs`;
- delete all three nested builder state machines and their poisoning protocol;
- delete at least the four line/run/glyph/unit iterator-container types named
  above;
- delete the line/run/interaction input-record mirrors;
- no equivalent renamed layers.

Stretch:

- at most 16,500 Rust code lines;
- one shared committed/projected traversal implementation;
- a smaller public re-export list and shorter adapter migration example.

Tests and external evidence are expected to grow and are excluded from the
code-line target only when they live outside the measured production files.

## Performance gates

The implementation must keep:

- display residency at or below 1.25× matched Parley;
- editable residency at or below 1.25× matched Parley;
- localized edit latency at or below 2× Parley;
- changed preparation at or below 16 allocation calls / 3,200 requested bytes
  in the counting allocator;
- zero allocations for exact repeat, repaint, exact hit, closest hit, and
  represented-position lookup;
- no regression in the 1,000-unit query medians;
- no regression in 64-label churn.

The current 64-unit exact/closest/position query medians are 61/101/84 ns
versus Parley's 15/15/13 ns. The implementation profiles that fixed overhead
and must improve at least one of the three without making another worse. Long
text wins do not excuse label-sized ceremony.

## Correctness and portability gates

- custom malformed-adapter tests for every final cross-table invariant;
- source-heavy, multi-leaf, mixed-script grapheme, ligature, bidi hard/soft
  break, region, alignment, justification, and composition cases unchanged;
- sparse capability omission and upgrade tests;
- foreign-scene misuse deleted with the API, not converted into a panic;
- horizontal geometry and work reports unchanged through logical renames;
- Rust 1.88;
- `no_std` bare-metal target;
- wasm target;
- rustdoc, fmt, strict Clippy, workspace tests, repository policy, and Beads
  graph green.

## Sequence

1. Delete non-operational scene facades and add O(1) capability summaries.
2. Replace nested builders with flat data and one final validator across
   `underwood` and `underwood_parley` in one coherent commit.
3. Delete iterator-container families and migrate all consumers.
4. Attempt committed/projected convergence; keep it only if its explicit gate
   passes.
5. Run the complete numeric, correctness, portability, and source-deletion
   matrix.
6. Perform final Cedar, Rook, Alder, and Stoat audits before landing.

Each slice leaves the workspace green. No slice keeps an old normal path beside
the new one.

## Rook audit

### Real strengths

- The design preserves the measured one-artifact runtime instead of using
  source deletion as an excuse to restore duplication.
- It deletes named state machines and impossible-state defenses.
- It keeps a checked third-party adapter boundary while ending repeated
  internal validation.
- It treats small-query overhead, residency, and allocation as product gates.
- It does not fold rich authoring, vertical layout, or new Parley APIs into the
  compaction.

### Mirage risks

- `PreparedParagraphData` is theater if it hides the same paragraph/line/run
  begin/finish protocol.
- “Trusted internally” is theater if scene construction replays the final
  validator or retains evidence records for debug comfort.
- Deleting source facades is theater if views gain another scene-access wrapper
  with the same foreign-view checks.
- Opaque iterators are theater if a generic table framework replaces a few
  obvious range maps.
- A private committed/projected mode is theater if monomorphized duplication
  remains while source becomes harder to follow.
- The 17,000-line gate can be gamed by dense files or macros. Named concept
  deletion and call-site clarity remain blocking.

### Most dangerous gap

The flat batch can accidentally become an unstructured bag of indexes that is
shorter but hostile to custom adapters. The migration example and malformed
adapter tests must show that ranges are explicit, errors are diagnosable, and
one final failure identifies the bad table without resurrecting a staged
runtime prover.

### Required review questions

1. Is every retained field consumed by product behavior?
2. Is every invariant checked once and then trusted?
3. Did a deleted invalid state become unrepresentable, or merely panic later?
4. Did the in-tree Parley adapter lose a pass or only call renamed helpers?
5. Are source and capability failures still understandable at the call site?
6. Did small text get faster or merely shorter source?

## Approval

Approved by the project owner on 2026-07-27. Implementation may break the
foundational adapter and scene APIs described by this design, and may delete
additional redundant concepts when measurement and review show that they do
not serve runtime behavior.
