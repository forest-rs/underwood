# Design-0018: Capability-scaled prepared scenes

- **Status:** Approved — 2026-07-26
- **Date:** 2026-07-26
- **Bead:** `und-oh0.13.17.9`
- **Extends:** Design-0011, Design-0014, and Design-0017

## Decision

Underwood will retain one immutable paragraph layout base and materialize
source, semantic, hit-testing, selection, navigation, and native-editing facts
as explicit capability sidecars. A display-only label will not retain maximal
editable-scene data.

Scene requests name required capabilities as a uniform default with optional
sparse paragraph overrides. Capability dependencies are normalized when the
request is built; unsupported queries are absent from the base display facade
rather than silently returning empty results. A retained segment whose
capabilities are a superset may satisfy a smaller request. A warm upgrade
builds only missing sidecars and does not repeat analysis, font selection,
shaping, or line formation. An upgrade after reusable adapter facts have been
explicitly released or evicted may re-form the paragraph and must report that
work.

Authored provenance will be stored once in an immutable paragraph-local source
map. Hot records retain projected paragraph byte ranges or compact indexes into
typed flat tables. Public borrowed views map those ranges to revision-bound
authored sources lazily. Persistent storage will use paragraph-owned typed
arrays, not a global general-purpose arena.

## Why this extends Design-0017

Design-0017 made paragraph-local segments the immutable sharing and
reclamation unit. Its first implementation still gives every segment:

- lines and paint fragments;
- a source `Vec` for most glyphs, clusters, movements, and semantics;
- hit-test clusters and slices;
- carets;
- complete visual and logical movement records;
- flattened semantic geometry.

That is appropriate for an editable proof, but it is not the required retained
shape of a button label, a wall of non-selectable prose, or a PDF-only export.
Making each record smaller would improve the maximal scene while preserving
the larger architectural mistake.

## Goals

- One paragraph shaping and layout result serves display, accessibility,
  links, selection, editing, and export.
- Display-only text retains only the facts needed to measure and paint it.
- Accessibility, hit testing, selection, navigation, native text input, and
  source-aware export pay only for facts they consume.
- A warm capability upgrade reuses the exact layout base and builds only the
  missing sidecars.
- A cold upgrade after reusable adapter-fact eviction has explicit,
  observable degradation rather than an impossible zero-work guarantee.
- One document can retain display-only siblings around a selectively
  interactive or editable paragraph.
- Common source provenance is stored once per paragraph rather than repeated
  as `TextId + Range` vectors throughout the scene.
- Pointer, selection, and native-text queries do not allocate merely to expose
  borrowed source observations.
- Sidecar residency and release are independently observable and budgetable.
- Core remains `no_std + alloc`, Rust 1.88 compatible, dependency neutral, and
  free of `unsafe`.

## Non-goals

- No content heuristics such as “short text means label mode.”
- No separate simple-text engine or label-specific shaping cache.
- No global slab, tracing garbage collector, stable-handle arena, or mutable
  scene graph.
- No renderer, widget, accessibility-platform, or IME policy in core.
- No promise that every legal capability combination needs a named convenience
  profile.
- No lazy interior mutation of a published scene on first query.
- No silent approximation when a scene lacks a required capability.

## Ownership fence

### LayoutEngine

`LayoutEngine` owns capability normalization, sidecar construction, retained
capability residency, upgrades, cache budgets, and immutable publication. It
may retain a capability superset but must report the resident bytes by
category.

### ParagraphFormation and underwood_parley

The adapter owns retained formation facts sufficient to produce requested
prepared sidecars. It receives the normalized required capabilities. It may
discard maximal cursor and interaction lowering for display requests.
Reusable analysis, shaping, and formed-line facts are a separately accounted
cache, not part of the published display scene. A warm upgrade reuses them. An
upgrade after their explicit release or budget eviction may re-form and is
reported as cold formation work.

### ParagraphSceneSegment

The segment remains the unit of structural sharing and reclamation. It owns one
layout base plus optional immutable sidecar handles. It does not point into a
process-global arena.

### Public scene facades

`TextScene` owns metrics, line traversal, and display traversal. Capability
facades borrow the exact sidecars they require:

```rust
let display = scene.display();
let semantics = scene.semantics();
let interaction = scene.interaction();
let selection = scene.selection();
let editing = scene.editing();
```

A fallible facade returns `Result<_, MissingSceneCapability>` when no
represented paragraph retains the required closure and names one concrete
missing request. Methods that require that facade do not remain on the
unconditional display surface. This is a programming diagnostic, not a
recoverable approximation or a request to allocate lazily.

In a mixed-capability scene the interaction, selection, and editing facades
traverse only paragraphs that physically retained their observations. A
position in an omitted paragraph is unrepresented; selection or editing
rejects it through the operation's ordinary validation result. Normal-flow
point lookup identifies the layout paragraph first and therefore does not jump
from a display-only paragraph to a distant interactive sibling. Region-flow
closest-hit behavior remains defined over the physically retained hit
geometry.

Source and semantic traversal retain their stricter whole-scene gate because
their accessors accept arbitrary public line, fragment, and glyph views.
Opening those facades for a sparse subset would either make every accessor
fallible or admit a panic when a caller supplies a display-only record. A
future paragraph-scoped structural view may relax that gate without weakening
the current contract.

This keeps the sparse case usable without introducing one forwarding facade
type per paragraph and capability:

```rust
let editing = scene
    .editing()
    .expect("the scene contains no editable paragraph");
let caret = editing.position_at(editor_text, 0);
assert!(editing.position_at(display_text, 0).is_none());
```

## Capability model

Capabilities are explicit, composable, and normalized. The initial public
vocabulary is conceptual; exact type and method names remain part of the
approval:

```rust
pub struct SceneFeatures { /* private bits */ }

impl SceneFeatures {
    pub const DISPLAY: Self;

    pub const fn with_sources(self) -> Self;
    pub const fn with_semantics(self) -> Self;
    pub const fn with_hit_testing(self) -> Self;
    pub const fn with_selection(self) -> Self;
    pub const fn with_navigation(self) -> Self;
    pub const fn with_native_text_input(self) -> Self;
}
```

No bitflags dependency is needed. Private bits preserve room to normalize
dependencies without accepting arbitrary integers.

A request carries a uniform default and may carry sparse paragraph overrides:

```rust
let features = SceneFeaturePolicy::uniform(SceneFeatures::DISPLAY)
    .with_paragraph(editor, SceneFeatures::EDITABLE);
let request =
    SceneRequest::new(constraint, &styles, &paint).with_feature_policy(features);
```

The exact sparse representation remains private. A uniform request remains
allocation-free to construct. Overrides are keyed by stable paragraph
identity, normalized once, and must not promote unrelated siblings.

The initial dependency closure is:

| Requested feature | Implied retained facts |
|---|---|
| display | layout base + paint topology |
| sources | paragraph source map |
| semantics | sources + semantic sidecar |
| hit testing | sources + hit sidecar |
| selection | hit testing + caret/selection sidecar |
| navigation | selection + movement/index sidecar |
| native text input | navigation + encoding/native-query sidecar |

Composition preparation requires the native-text-input closure for its target
paragraph. Unchanged committed siblings retain only the capabilities requested
for the committed scene.

Named convenience profiles may be provided only as aliases:

```rust
SceneFeatures::DISPLAY
SceneFeatures::ACCESSIBLE
SceneFeatures::SELECTABLE
SceneFeatures::EDITABLE
```

They do not create distinct engines or representations.

## Public request migration

### Before

Every request prepares the maximal scene:

```rust
let request = SceneRequest::new(constraint, &styles, &paint);
let output = layout.prepare(&snapshot, &request)?;
let hit = output.scene().hit_test(point);
```

### After

The calm display path stays short, while interaction is explicit:

```rust
let request = SceneRequest::new(constraint, &styles, &paint);
let output = layout.prepare(&snapshot, &request)?;
for fragment in output.scene().display().fragments() {
    render(fragment);
}
```

```rust
let features = SceneFeatures::DISPLAY
    .with_semantics()
    .with_hit_testing()
    .with_selection();
let request =
    SceneRequest::new(constraint, &styles, &paint).with_features(features);
let output = layout.prepare(&snapshot, &request)?;
let interaction = output
    .scene()
    .interaction()
    .expect("scene is missing the requested hit-testing capability");
let hit = interaction.hit_test(point);
```

Source-aware export uses its own borrowed facade instead of forcing source
owners into display glyph records:

```rust
let sources = output
    .scene()
    .sources()
    .expect("scene is missing the requested source capability");
for fragment in output.scene().display().fragments() {
    for glyph in fragment.glyphs() {
        export(glyph, sources.for_glyph(glyph)?);
    }
}
```

Source accessors are fallible because public display views can outlive their
original traversal and be presented to another source facade. Line, fragment,
and glyph views are branded with the exact prepared scene root; a mismatched
view returns `ForeignSceneView` instead of reaching another scene's source-map
invariants.

`BlockRequest` receives the same `with_features` method. Overstory can reuse a
single `ComputedInlineStyle` while choosing display, link, selectable, or
editable retention per control.

The migration note will list methods moving from `TextScene` and
`CompositionScene` to feature facades. There is no compatibility shim that
materializes missing data.

## Retained representation

Conceptually:

```text
ParagraphSceneSegment
├─ Arc<LayoutBase>
│  ├─ lines
│  ├─ positioned glyphs/runs
│  └─ metrics and line indexes
├─ Arc<PaintSidecar>
│  ├─ run-sized paint fragments
│  └─ per-line fragment ranges
├─ Option<Arc<ParagraphSourceMap>>
├─ Option<Arc<SemanticSidecar>>
├─ Option<Arc<HitSidecar>>
├─ Option<Arc<SelectionSidecar>>
├─ Option<Arc<NavigationSidecar>>
└─ Option<Arc<NativeTextSidecar>>
```

The exact sidecar split follows measured sharing. A sidecar should not exist
merely to move one small scalar into another allocation. Source and semantic
facts may share one allocation when measurements show that they are always
requested and released together.

### Layout base

The base retains only source-independent layout observations required for
metrics and painting:

- line bounds, baselines, advances, and adjustment;
- glyph id, position, advance, run/font instance, and variation/synthesis
  identity;
- compact ranges into flat run and glyph tables;
- region placement and paragraph extent summaries.

It does not retain leaf identities, selection ranges, cursor transitions, or
native encoding indexes.

### Paint sidecar

Whole-glyph paint coverage is inline in prepared glyphs. Scene paint topology
is run-sized: adjacent glyphs with identical run and paint state form one
fragment. Explicit clipped split-ligature paint remains an exceptional
multi-fragment observation over one shaped-glyph identity.

Paint-slot changes rebuild this sidecar while sharing the layout base and every
unrelated sidecar. Paint-table value changes remain a top-level O(1) binding.

## Paragraph-local source map

The current `LocalRange` repeats a full `TextId` and byte range in many
records. The replacement stores leaf identity once:

```text
ParagraphSourceMap
├─ leaves: [SourceLeaf { text_id, semantic_id, paragraph_range, ... }]
├─ projection relation runs
└─ optional spill ranges for one-to-many mappings
```

Layout and interaction records retain projected paragraph ranges or compact
`u32` indexes. A borrowed source iterator walks the relation runs and yields
revision-bound `SnapshotTextRange` values without allocating.

This preserves the hard cases:

- one grapheme crossing semantic leaves;
- collapsed whitespace retaining every authored byte;
- transformed text with non-identity source relations;
- mixed committed and generated composition provenance;
- bidi visual order distinct from logical source order;
- split ligatures whose paint portions have different authored owners.

This is not merely record compaction. One authoritative one-to-many relation
replaces several independently rebuilt source vectors, so a multi-leaf
grapheme or ligature cannot acquire inconsistent provenance in glyph,
selection, semantic, and export paths.

Hot hit results become borrowed scene observations. An owned selection uses an
explicit one-or-many representation: one range is inline; uncommon disjoint
visual or cross-leaf selections use shared packed storage. A hit or caret query
does not allocate an `Arc<[SnapshotTextRange]>`.

## Why not a general arena

A global or engine-wide arena would couple unrelated paragraph lifetimes and
make old immutable scenes, cache eviction, composition epochs, and
`release_document` harder to account for. It introduces stale-handle,
fragmentation, synchronization, and partial-reclamation questions before they
solve a measured problem.

Paragraph-local typed arrays provide the useful arena properties:

- compact indexes instead of repeated owners;
- a few allocation sites instead of one allocation per record;
- contiguous traversal;
- one immutable owner;
- exact reclamation when the segment and caller-held scenes are dropped.

Resettable engine scratch is appropriate for transient construction. It must
not become the lifetime owner of published records.

## Adapter-fact residency and degradation

Published scene residency and reusable adapter-fact residency are independent:

- The published paragraph segment owns the selected layout and scene
  capabilities and remains valid while any caller retains it.
- The adapter cache may additionally retain analysis, canonical shaping,
  formed lines, and lowering inputs that make later capability upgrades warm.
- Adapter-fact bytes have their own budget and diagnostics. A zero budget, an
  explicit trim, or ordinary budget eviction may discard them immediately
  after scene lowering, including for display-only text.
- Evicting adapter facts never invalidates a published scene or drops its
  layout base.
- A later upgrade with resident adapter facts is warm and performs no
  analysis, font selection, shaping, or line formation.
- A later upgrade without those facts is cold: it may repeat formation,
  reports the exact work counters and an adapter-cache miss, and is not an
  error.

This is the deliberate trade: a wall of static labels may minimize residency
by retaining only published display segments, while a live editor may budget
reusable adapter facts for low-latency upgrades and edits. The engine must not
silently count adapter facts as “free” merely because they are not scene
sidecars.

The public lifecycle vocabulary provides
`CacheBudget::with_adapter_facts_bytes` and
`LayoutEngine::trim_adapter_facts`. Setting the adapter budget to zero is a
supported, tested configuration rather than an internal accident.

## Cache and upgrade laws

1. Layout identity does not include requested capabilities.
2. A retained capability superset satisfies a subset request without
   recomputation.
3. A capability upgrade retains the exact layout-base handle.
4. Paint-only upgrades or slot changes retain source, interaction, selection,
   navigation, and native sidecars when their geometry identity remains valid.
5. A warm upgrade builds missing sidecars from retained adapter and layout
   facts without analysis, font selection, shaping, or line formation.
6. A smaller request does not mutate a caller-held larger scene.
7. Engine budgets may drop unpinned sidecars and reusable adapter facts
   independently, but not the base required by a resident published segment.
   A subsequent cold upgrade may re-form and must report that degradation.
8. Exact published-root reuse requires the published features to be a superset
   of the request.
9. Capability absence is observable through requested-versus-resident
   diagnostics and fallible facades.
10. Failed upgrades publish nothing and preserve the previous retained scene.
11. A sparse paragraph override cannot promote, rebuild, or expand the
    retained sidecars of an unrelated sibling.

## Accessibility, links, selection, PDF, and IME

- A static accessible label requests semantics but not hit testing,
  selection, or movement.
- A link requests semantics and hit testing; activation remains host policy.
- Selectable prose requests hit testing and selection. Bidi visual selection
  keeps its disjoint logical ranges.
- An editor requests navigation and native text input. Multiple selections and
  composition remain exact.
- Visual PDF plus text extraction requests display plus sources. Tagged
  PDF/UA also requests semantic structure. Neither needs pointer-hit,
  caret-movement, or native-editing facts.
- A raster-only decorative label may request display alone.

No mode silently supplies empty semantics, fabricated sources, or approximate
caret behavior.

## Memory and lifecycle accounting

Diagnostics report at least:

- normalized requested and actually resident capabilities per paragraph
  segment;
- layout-base resident bytes;
- paint-sidecar resident bytes;
- source-map resident bytes;
- semantic, hit, selection, navigation, and native sidecar bytes;
- reusable adapter facts and scratch bytes;
- warm and cold upgrades, hits, releases, and evictions;
- formation work caused by a cold upgrade.

Engine accounting counts each engine-owned handle once. Caller-retained old
scenes remain valid and are outside the engine-controlled eviction total.

## Required wind tunnels

Matched release measurements cover 64, 1,000, and 2,048 paragraphs or blocks:

1. cold and exact-repeat display labels;
2. identical and distinct display labels;
3. display → accessible upgrade;
4. display → hit-testable link upgrade;
5. display → selectable upgrade;
6. display → editable upgrade;
7. editable → display request without reshaping;
8. creation/destruction churn with sidecar release;
9. source-heavy collapsed whitespace and mixed-leaf graphemes;
10. mixed bidi selection and native composition;
11. pointer-hit and movement query latency;
12. retained residency compared with high-level Parley `Layout`.
13. one editable paragraph among 2,047 display-only siblings: steady-state
    typing, exact repeat, and target capability upgrade;
14. an editable-tier typing tunnel without display-only siblings.

Each warm upgrade proves zero analysis, font selection, shaping, and line
formation. A forced adapter-eviction variant proves that a cold upgrade
re-forms only the target paragraph, reports the miss and work, and leaves
2,047 siblings untouched.
Display-only residency must materially exclude maximal interaction/editing
data; an empty-vector implementation does not pass.

## Rook audit

### Real

- Design-0017 already provides immutable paragraph-local reclamation and
  persistent sharing.
- Paint-table values are already an O(1) scene binding.
- The current branch proves paint topology can be rebuilt while pointer-sharing
  all non-paint scene facts.

### Mirage risks

- A `SceneFeatures` mask is theater if the adapter still builds maximal
  `PreparedParagraph` values.
- Empty sidecar vectors are theater if their validation and construction still
  run.
- Separate feature cache entries are theater if each duplicates shaping and
  layout.
- Lazy source views are theater if every hit first allocates a mapped source
  vector.
- A global arena is theater if it reduces allocation calls while increasing
  retained bytes or preventing precise release.
- Display-only is a false claim if renderer fragments still own repeated leaf
  ranges solely because older tests inspected them.
- The memory model fails socially if hosts request `SELECTABLE` or `EDITABLE`
  everywhere merely to avoid a future `&mut LayoutEngine` upgrade. Cheap warm
  upgrades, precise missing-capability errors, and requested-versus-resident
  diagnostics must make the narrow request the easier choice.

### Most dangerous gap

The adapter currently treats interaction units and complete cursor topology
as mandatory `PreparedParagraph` output. Scene-only sidecars cannot earn the
memory law unless capability requirements cross that contract and the adapter
retains a reusable base from which missing sidecars can be built.

## Alternatives rejected

### Always retain the maximal scene

Simple and correct, but makes every label pay editor memory and construction
cost. Record compaction alone does not fix the scaling law.

### Separate label and document engines

Duplicates shaping caches and creates divergent font, fallback, and layout
behavior. It violates the one-engine invariant.

### Infer capabilities from content or widget type

Underwood does not own widgets, and short text can still be selectable,
accessible, linked, or editable. Requirements must be explicit.

### Build missing data lazily through interior mutation

Queries become unpredictably allocating and synchronization enters the scene
contract. Published scenes remain immutable instead.

### One engine-wide arena

It weakens paragraph-local lifetime, eviction, and publication ownership for
uncertain allocation benefit. Typed paragraph-local arrays and scratch are the
selected first design.

## First implementation slice after approval

1. Add `SceneFeatures`, uniform-plus-sparse feature policy, and request
   migration together with the adapter prepared-output split. The first
   mergeable slice must prove display requests do not build cursor topology;
   a maximal mask with maximal lowering is not an independently landable step.
   Implemented in the first checkpoint.
2. Add separately budgeted adapter-fact residency, warm/cold upgrade
   diagnostics, and the forced-eviction degradation proof. Implemented in the
   second checkpoint.
3. Split `ParagraphSceneSegment` into layout, run-sized paint, and interaction
   handles; move scene interaction methods behind borrowed facades.
   Implemented in the third checkpoint.
4. Introduce the paragraph-local source map and replace eager per-record
   `LocalRange` vectors in one vertical slice: glyph source traversal, hit
   result, selection ownership, PDF, and composition traps. Implemented in the
   third checkpoint together with step 3 so no temporary maximal
   representation was presented as capability-scaled.
5. Add byte accounting and matched display/selectable/editable wind tunnels.
   Implemented in the fourth checkpoint. Public diagnostics distinguish the
   request from the physically resident capability closure and report
   deterministic category bytes. The checked 64/1,000/2,048 mixed-document
   tunnel proves constant-time exact repeats, one editable paragraph's
   sidecars, and localized typing without sibling reconstruction.
6. Only then compact remaining tables and add query indexes.

## Approval gate

Approval authorizes the breaking request/facade migration and foundational
adapter/segment representation changes described above. It does not authorize
a new dependency, `unsafe`, renderer or toolkit policy, a serialized format,
or a global arena.

Bruce approved this gate on 2026-07-26.
