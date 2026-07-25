# Underwood execution plans

## Reusable text preparation and region-aware line layout

**Status:** Active — Design-0014 and ADR-0005 approved 2026-07-24

**Beads:** `und-oh0.13` and its dependency-ordered children

### Goal

Build a source-complete, reusable preparation pipeline over Parley Engine
facts, then use it to deliver whitespace processing, resumable line
formation, region filling, paragraph style fidelity, alignment,
justification, preparation tracing, allocation proof, and the region-aware
living page without creating another all-owning text layout object.

### Fence

Reusable kernels own explicit projected-text, shaped-fact, line-candidate,
region-slot, and line-adjustment transformations. Underwood owns their
composition with immutable documents, retained invalidation, semantic
identity, editing, portable scenes, and product proof. Neither layer owns
toolkit behavior or pixel production.

### Non-goals

- No high-level Parley production dependency in foundational crates.
- No CSS parser, cascade, browser formatting context, or universal
  script-justification claim. CSS Text computed semantics remain a positive
  conformance source.
- No Overstory widget or prepared-output retention policy.
- No production dependency, `unsafe`, or foundational API choice without its
  human gate.
- No placeholder transformation or benchmark-only substitute for the public
  path.

### Steps

1. Use Underwood `faa19ead16054d52d4d921de469a8f28993b6767` and Overstory
   `75e22e5d0c4141767d131d237e781bc5ee1ac16f` as executable consumer
   checkpoints, ratify Design-0014 and ADR-0005, and freeze only the stage
   ownership and evidence requirements. Treat companion patches as evidence
   rather than an API to copy.
2. Extend the existing public-path wind tunnels with isolated release CPU
   profiles, allocation counts and bytes, retained-capacity reporting, and
   region churn, plus identical/distinct label and creation/destruction
   workloads before optimizing.
3. Implement and prove compact projected text with composed authored-source
   mappings, real preserved/collapsed whitespace, and one-to-many replacement
   coverage across semantic style boundaries.
4. Replace the current private formation loop with an extraction-ready,
   resumable line-candidate kernel over public Parley Engine facts while
   preserving line-final shaping and fit-changing retry.
5. Carry `LineHeight::{MetricsRelative, FontSizeRelative, Absolute}`, letter
   spacing, word spacing, `WordBreak`, `OverflowWrap`, and wrap policy through
   their owning stages with exact invalidation.
6. Design and prove explicitly budgeted cross-identity reuse for immutable
   preparation facts. Rebind document, revision, semantic, interaction, paint,
   and placement identity for every consumer.
7. Add empty and system-only font-catalog construction, stable family-name
   observation, and shared-backing clone proof without creating another
   application font universe.
8. Introduce the concrete line-slot and region-cursor protocol; prove
   rectangles, exclusions, floats, columns, height retry, and deterministic
   replay.
9. Implement direction-aware start/end plus left/right/center alignment and
   explicit Western justification adjustments within accepted slots. Prove
   that glyphs, carets, hits, selections, semantics, and line bounds move
   together.
10. Add calm editable-block scene endpoints, represented-caret resolution,
   logical word movement from retained analysis, and revision-rebound
   replacement. Gate a single-line editor façade on remaining call-site
   ceremony.
11. Prove CJK line breaking and document the exact boundary between current
   Unicode support, dictionary data, word-break policy, and future CJK
   justification.
12. Publish the first-class preparation trace, integrate measured scratch and
   cache counters, and decide separately whether Spoor earns a production
   dependency.
13. Product-prove the complete path through a compelling region-aware living
   page and guided diagnostic modes.
14. Retire the nonstandard catch-all preparation metaphor from live code and
   normative architecture. Name analysis, shaping, prepared facts, formation,
   adjustment, geometry, and caches according to their actual stage.
15. Run adversarial architecture, correctness, performance, accessibility,
   PDF, portability, and real-vs-mirage review; land coherent protected
   changes with complete proof records.

### Progress

- Steps 1–4 and the independent shared-font-catalog part of step 7 are landed
  and protected.
- Step 5 is implemented through the real public scene path. Underwood consumes
  Unicode `Joining_Type` from the small Parley Engine integration commit
  `97b874719f810c375025f3fa727b245530a87f9f`; no script table or new production
  dependency was added.
- The exact Overstory computed-style and editable-control lowering type-checks
  after its disposable consumer proof uses one Parlance type universe and
  applies two stale presentation-pattern fixes. The branch's 25 non-alignment
  TextInput tests pass against this Underwood worktree. Its three original
  integration failures remain in Overstory lowering and retention policy; they
  no longer expose missing Underwood editing primitives.
- Chromium-recorded first-line cases are recorded for the step 11 CJK and
  CSS-profile corpus. They remain browser-compatibility evidence rather than a
  Unicode oracle.
- Step 8 is implemented through the public document, composition, and
  `TextBlock` paths. Exact exclusion, float, and column slots are replayable;
  height rejection restores text traversal; mixed-bidi interaction geometry
  and retained invalidation are product-proven. The measured local cost is
  recorded in `docs/proof/region-flow-review-2026-07-25.md`.
- Step 9 is implemented through the same public paths. Logical and physical
  alignment consume accepted slots and retained analysis direction; Western
  U+0020 expansion moves paint, hits, carets, selections, semantics,
  compositions, visual output, and PDF geometry together without repeating
  analysis, shaping, or formation. Measurements and exact limits are recorded in
  `docs/proof/line-adjustment-review-2026-07-25.md`.
- Step 6 has a matched before-state wind tunnel and the approved cache from
  `docs/design/0016-cross-identity-preparation-cache.md` is implemented.
  Design-0016 was approved on 2026-07-25. The owner is `LayoutEngine`; the value
  is immutable paragraph-local
  prepared facts; every consumer rebuilds identity-bound geometry; retention
  is a separately opt-in byte-budgeted LRU; backend participation is
  default-off and epoch-scoped. Exact identity, invalidation, collision,
  composition, region-rebinding, byte-budget, eviction, oversized-entry, and
  release laws are protected. The measured result is recorded in
  `docs/proof/cross-identity-preparation-cache-2026-07-25.md`.
- Step 14 is complete. Live identifiers, rustdoc, examples, benchmarks, and
  normative architecture now name analysis, shaping, prepared facts,
  formation, adjustment, geometry, or cache state according to ownership.
  The deliberately preserved historical symbol references are recorded in
  `docs/proof/preparation-terminology-review-2026-07-25.md`.
- Step 10 is complete. `TextScene` exposes complete-scene logical endpoints,
  represented leaf-local caret resolution, and logical word movement from
  retained adapter analysis. `TextBlock` exposes its stable leaf identity,
  current text, and atomic selection replacement with post-publication
  selections. The parked Overstory editor call site compiles without duplicate
  navigation or transaction logic; the exact traps and remaining consumer
  work are recorded in
  `docs/proof/editable-text-block-operations-2026-07-25.md`.
- Step 11 is complete for line breaking. The deterministic corpus covers
  Japanese kinsoku punctuation, kana, iteration marks, ideographic space,
  Chinese, Korean, mixed Latin/CJK, emoji ZWJ sequences, mandatory breaks, and
  all three authored `WordBreak` values through analysis and reusable
  formation. A native-fallback product trap reaches the public scene path.
  Parley Engine's compiled dictionary data is exposed as an explicit
  non-default feature with its binary cost measured. Locale tailoring, CJK
  justification, and dictionary-quality CJK word navigation remain separate
  claims; the precedence-merged Parley boundary fact blocking the latter is
  tracked by `und-oh0.2.11`. Exact results are recorded in
  `docs/proof/cjk-line-breaking-review-2026-07-25.md`.
- Step 12 is complete. `WorkReport` remains the always-on stage ledger while
  opt-in `PreparationTrace` explains cold, exact, shared, adapter,
  invalidation, candidate, region, output-capacity, scratch-growth, and cache
  residency facts. Host tooling owns separate prepare/render time and process
  allocation evidence. The matched label wind tunnel measures tracing at
  0.14–0.22% over the retained path on this host after cache-byte accounting
  was made incremental. Spoor remains the independently gated `und-oh0.14`;
  exact results are recorded in
  `docs/proof/preparation-trace-review-2026-07-25.md`.
- The committed-tree allocation audit exposed a separate retained-lifecycle
  problem: unchanged values are rebuilt and deep-compared, then retained
  records are deep-copied into each output. Its matched 1,000-paragraph
  baseline and proposed correction now live in the sibling campaign
  `und-oh0.13.17`; they do not extend this campaign's finish line. The
  preparation trace remains neutral before-state evidence for that later work.

### Risks and controls

- **Architecture without product:** whitespace collapse and the living page are
  required real consumers.
- **Premature public vocabulary:** schematic records remain private until the
  representation wind tunnel and human API gate pass.
- **Second shaping engine:** kernels consume only public Parley Engine facts
  and never copy HarfRust behavior.
- **Line-layout regression:** Arabic, ligature, bidi, CRLF, intrinsic,
  fit-changing, and grapheme traps remain blocking.
- **Region monolith:** slot providers own geometry and float policy; the former
  owns candidates and checkpoints only.
- **Memory optimism:** scratch changes require allocation and wall-time
  before/after evidence on the same workloads.
- **Style mismatch:** the final Overstory analysis is an explicit input before
  paragraph styles freeze.
- **Prototype anchoring:** Overstory companion code contributes traps and call
  sites, not a representation to copy.
- **Identity-poisoned reuse:** shared cache entries contain only immutable
  identity-free facts; churn tests enforce an explicit memory budget.
- **Partial alignment:** every spatial record and interaction path is compared
  after offsets and adjustment.
- **Duplicate navigation:** word movement consumes retained Parley Engine
  analysis facts rather than another Unicode segmenter.
- **Duplicate font discovery:** catalog construction and cloning prove shared
  backing and one discovery pass.
- **Conformance overclaim:** CJK, Western justification, and Arabic
  justification keep separate proof status.

### Completion

The campaign is complete when one real public path demonstrates every stage,
source mappings remain exact under real collapse, line formation is reusable
and reversible, regions and alignment consume accepted slots, known style gaps
are closed, identical labels reuse eligible preparation without sharing
identity, editable blocks and font catalogs satisfy the exact Overstory
call-site invariants, CPU and memory costs are observable and bounded, CJK
limits are executable rather than anecdotal, the living page depends on the
work, and all local and protected remote gates are green.

## Retained O(change) preparation and scene lifecycle

**Status:** Proposed — sibling campaign; Design-0017 awaits architecture approval

**Beads:** `und-oh0.13.17` and its dependency-ordered children

### Goal

Replace rebuild-and-deep-compare cache validation, deep-copy scene
publication, repeated adapter lowering, whole-document edit staging, and
allocation-heavy common-case records with a coherent retained lifecycle.
Exact repeats should be O(1), and localized edits should touch changed
paragraph facts plus a sublinear scene-spine update.

### Fence

This campaign is informed by the preparation trace but is not part of the
reusable text-tools completion gate. It must not begin foundational
implementation until Design-0017 and its public traversal migration are
approved.

### Measured baseline

At 1,000 paragraphs an exact retained prepare performs 666,167 allocation
calls and requests 163,571,752 bytes; one-byte edit staging performs 1,015
calls; and preparation of that localized edit performs 666,807 calls. Exact
results and instrument limits are recorded in
`docs/proof/retained-lifecycle-baseline-2026-07-25.md`.

### Work laws

- An exact repeated `(snapshot, request)` may return the previously published
  immutable scene handle in O(1), with no projection, adapter, geometry, or
  record allocation.
- A localized text edit performs document staging, projection, preparation,
  geometry, and publication work proportional to the changed paragraph plus a
  sublinear persistent-scene-spine update. It does not touch unchanged sibling
  paragraph records.
- A paint-only change rebinds paint without repeating text projection,
  analysis, shaping, formation, or identity-bound interaction geometry.
- A global width, region, or style-policy change may visit every affected
  paragraph; the trace must say why and distinguish required formation work
  from avoidable value reconstruction.
- Cache eligibility uses stable provenance plus facet generations or immutable
  compiled-snapshot identity. A naked generation counter shared by unrelated
  values is not a valid key.
- Revision and epoch identity live at the scene/publication boundary. Cached
  paragraph-local records do not rewrite document identity on every hit;
  checked public queries mint or validate stamped positions at the boundary.
- Allocation counts and bytes are external wind-tunnel evidence. Core
  diagnostics report deterministic work, capacities, cache residency, and
  growth without pretending to observe process allocations or wall time.

### Ownership

`LayoutEngine` owns request invalidation, paragraph-segment reuse, the
persistent scene spine, and immutable scene publication. The Parley adapter
owns retained analyzed, shaped, formed, and lowered facts plus its scratch
workspaces. `Document` owns immutable publication and copy-on-write edit
staging.

### Execution order

1. Ratify the cache-key and persistent-scene representation against the work
   laws, including downstream-origin changes when paragraph height changes.
2. Add provenance-qualified preflight keys and skip projection construction
   on retained hits.
3. Return the exact prior scene on exact repeats, then introduce shared
   paragraph-local segments and a persistent scene spine.
4. Retain adapter lowering output and add engine-owned scratch.
5. Make document staging copy-on-write and mutate each touched leaf once.
6. Compact common-case source, paint, and interaction records with matched
   before/after allocation and query evidence.

The proposed representation and public migration are recorded in
`docs/design/0017-retained-scene-lifecycle.md`.

### Risks and controls

- **Arc optimism:** shared paragraph records are not sufficient if every
  prepare still rebuilds a flat document vector or eagerly rewrites downstream
  origins.
- **Generation collision:** every fast key is provenance-qualified or comes
  from an immutable compiled snapshot.
- **Cache pinning:** shared publication must remain explicitly budgeted and
  observable.
- **Hidden materialization:** flat convenience collection must remain an
  explicit cold-path operation rather than the default traversal API.

### Completion

Matched 64- and 1,000-paragraph public-path wind tunnels prove exact-repeat and
localized-edit work laws; unchanged records are shared rather than copied;
adapter hits do not re-lower; one-byte edits do not clone untouched paragraph
or leaf storage; common records avoid per-glyph allocation; and the full Rust
1.88, `no_std`, formatting, lint, test, documentation, repository, and
protected-remote gates pass.

## Module boundaries and Parley Engine convergence

**Status:** Complete — protected implementation and proof landed

**Beads:** `und-oh0.5.5`, `und-oh0.5.5.1`, `und-oh0.5.5.2`,
`und-oh0.5.5.3`

### Goal

Make the implementation structure express the architecture: calm crate roots,
private modules with one owner each, one current Parley Engine type universe,
and a replaceable Underwood line former whose correctness and cost are explicit.

### Fence

Underwood owns portable paragraph-formation policy, source interaction, cache
lifetime, and scene semantics. Parley Engine owns Unicode analysis,
itemization, font-backed shaping, and shaped text. Crate roots own
documentation, module declarations, and public re-exports; they explicitly do
not own implementation algorithms.

### Steps

1. Split `underwood_parley/src/lib.rs` into font, engine, shaping,
   line-breaking, lowering, interaction, validation, and focused test modules
   without changing behavior, dependencies, features, or public paths.
2. Prove the structural checkpoint through all local, portability, product, and
   protected remote gates before changing text behavior.
3. Split oversized Underwood scene and adapter implementation files by cache,
   projection, geometry, interaction, prepared-record, transaction, and host
   mapping ownership while preserving the 59-line public crate facade.
4. Stop at the human gate before changing dependency pins, then move
   `parley_engine`, Fontique, and Parlance together to approved immutable
   revision `9c41a4d0b9aa1aae7b8fdad8cf31728c9c3476bb`.
5. Replace fork-only in-place break mutation with a private line former that
   re-itemizes retained paragraph analysis at committed line boundaries and
   shapes final line ranges through public Parley Engine APIs.
6. Add a paragraph-level explicit base-direction value, keep it out of inline
   shaping style, and pass it to Parley Engine analysis with exact cache
   invalidation.
7. Prove Arabic joins, ligatures, fit-changing backtracking, intrinsic modes,
   auto and explicit paragraph direction, mixed bidi, source-complete
   interaction, and cache behavior; measure and report line-reshape work rather
   than hiding it inside width-only formation.
8. Run adversarial review, every local gate, and protected remote landing for
   each independently coherent slice.

### Risks and controls

- **Cosmetic decomposition:** no monolith may simply move under a new filename;
  each module receives a one-sentence ownership fence and focused tests.
- **Public-path drift:** crate roots re-export the exact existing public
  vocabulary; rustdoc and downstream examples are gates.
- **Private second shaper:** the local line former may call only public Parley
  Engine analysis, itemization, and shaping APIs; it may not copy HarfRust
  internals or add a direct shaping dependency.
- **Style-layer confusion:** paragraph base direction is an analysis input, not
  an inline shaping value; it receives its own computed paragraph-style
  partition and invalidates analysis for only the affected paragraph.
- **Correctness lost for convenience:** the existing Arabic, ligature,
  backtracking, bidi, interaction, PDF, and visual traps remain blocking.
- **Hidden performance regression:** a multiline formation wind tunnel records
  cold, unchanged, width-churn, safe-break, and unsafe-break costs plus exact
  line-reshape work.
- **Portability drift:** all foundational modules remain `no_std + alloc`,
  Rust 1.88, dependency-neutral except for replacing the existing Parley pin.

### Completion

The campaign is complete when crate roots are calm facades, oversized files
are decomposed by invariant, the temporary bounded-break fork is absent,
Underwood and its consumers can share current Parley/Fontique types, all
line-breaking and interaction correctness proofs remain executable, measured
costs are checked in, and every local and protected remote gate is green.

### Result

Completed on 2026-07-24 through protected PRs #25, #26, and #27. The adapter
and core roots are calm facades over explicitly owned modules. Underwood uses
one exact current Parley Engine, Fontique, and Parlance revision, carries no
bounded-break fork API, and reports its conservative public-Engine line
shaping separately from canonical shaping. Paragraph base direction is an
explicit computed value. Architecture, correctness, interaction, intrinsic
layout, performance, visual, PDF, Rust 1.88, `no_std`, and full remote proofs
are green. `und-oh0.2.10` isolates a possible future upstream performance seam
without weakening this completed boundary.

## Retained TextBlock and intrinsic-layout campaign

**Status:** Complete — implementation and protected remote proof complete

**Beads:** `und-oh0.5.3`, `und-oh0.5.3.1`, `und-oh0.5.3.2`,
`und-oh0.5.3.3`, `und-oh0.5.3.4`, `und-oh0.5.3.5`

### Goal

Make small retained text genuinely inexpensive without creating a second text
engine: add a one-paragraph `TextBlock` façade, real min/max/constrained
formation, exact metrics, coordinated bounded caches, and a label-scale wind
tunnel.

### Fence

This campaign owns retained block ergonomics, shared intrinsic constraints and
metrics, paragraph-cache lifetime, construction cleanup, and public-path
measurements. It explicitly does not own widgets, host callbacks,
accessibility policy, a separate label shaper, a global style registry, or
font-resource integration policy.

### Steps

1. Accept and execute
   `docs/design/0012-retained-text-block-and-intrinsic-layout.md`.
2. Add the separate label wind-tunnel crate and its deterministic workload
   skeleton before optimizing production caches.
3. Implement `TextBlock`, borrowed shared-style requests, explicit intrinsic
   constraints, and exact scene metrics through the existing paragraph path.
4. Replace linear retained lookup and add coordinated release, configurable
   LRU budgeting, and cache diagnostics across core and Parley adapter layers.
5. Remove the empty `TextData` placeholder and infallible constructor
   `Result`; migrate every public caller with recorded guidance.
6. Run the stable/identical/edit/resize/churn workloads, adversarial review,
   complete local gates, and protected remote landing.

### Risks and controls

- **Façade-only performance:** deterministic work and cache counters plus
  same-machine measurements must prove the cost rather than infer it from a
  shorter call site.
- **Second text model:** the block remains one internal document paragraph and
  produces the existing `TextScene`.
- **Fake intrinsic sizing:** max-content and min-content are explicit adapter
  modes and execute mandatory-break and break-reshape laws.
- **Cache split-brain:** `LayoutEngine` coordinates eviction and propagates
  release into `ParagraphFormation`; resident counts are observable.
- **Style registry creep:** requests borrow immutable common styles whose
  variable-sized members are already shared; further interning requires wind-
  tunnel evidence.
- **Core contamination:** use only `alloc` collections, add no production
  dependency, and preserve both no_std targets.

### Completion

The campaign is complete when thousands of blocks execute through the public
path with exact intrinsic metrics, stable work repeats no text preparation,
width changes perform only required formation, edit work stays local,
destroyed/budget-evicted blocks release both cache layers, measured evidence is
checked in, all callers migrate, and every local and protected remote gate is
green.

### Result

Implemented locally on 2026-07-24. The public path, intrinsic and multilingual laws,
failure cleanup, indexed lifecycle, zero/small-budget behavior, MSRV, no-std
targets, full workspace, rustdoc, and both product wind tunnels pass. The
checked-in label proof covers 2,048 stable and identical blocks plus edit,
constraint, explicit-release, and create/destroy churn.

The real-vs-mirage audit records one integration-dependent limit: a toolkit
should retain unchanged owned outputs rather than rematerialize thousands of
full scenes each frame. `und-oh0.5.4` carries that Overstory proof and the
evidence gate for any future selective non-editable materialization.

## Prepared-scene PDF slice

**Status:** Complete

**Bead:** `und-oh0.9.1`

### Goal

Lower one real prepared `TextScene` through Krilla into a deterministic,
human-inspectable PDF while preserving Underwood's ownership of shaping,
fallback, bidi, geometry, paint partitioning, and Unicode provenance.

### Fence

This slice owns a replaceable `underwood_pdf` adapter, a mixed-script proof
crate, and the generated proof artifact. It does not add PDF concepts to core,
re-shape text, approximate unsupported variable instances or synthesis, claim
tagged-PDF accessibility, or broaden the first slice beyond one page.

### Steps

1. Translate supported public scene observations into Krilla fonts, glyphs,
   transforms, solid paint, and explicit paint clips.
2. Resolve each glyph's immutable source ranges through the matching document
   snapshot and retain that Unicode in the PDF text mapping.
3. Reject unsupported paint, non-default variation coordinates, synthesis, bad
   resources, and unrepresentable geometry before serialization.
4. Build and run a deterministic mixed Latin/Arabic proof through the public
   Underwood path; retain and visually inspect the resulting PDF.
5. Give lines direct fragment ranges and partial-painted observations a shared
   shaped-glyph identity, so renderer adapters need no geometric inference.
6. Run the complete workspace policy, lint, test, and documentation gates.

### Risks and controls

- **Second text engine:** accept only prepared scene glyphs and never invoke
  Krilla's shaping API.
- **Quiet approximation:** preflight every representable scene property and
  return a fragment-local error for unsupported inputs.
- **Duplicate font storage:** transfer a clone of the scene's shared font
  backing into Krilla rather than copying font bytes.
- **Extraction overclaim:** preserve glyph Unicode, but defer logical-order
  bidi extraction and tagged PDF to a separate evidence-backed slice; record
  macOS Preview behavior against Underwood, Chrome, and Quartz controls.
- **Core dependency creep:** keep Krilla and its Rust 1.92 floor in the external
  renderer-host adapter.

### Completion

The slice is complete when the proof PDF is generated from a mixed-script real
scene, its output and rejection paths are tested, the artifact is visually
credible, and the repository is green.

## Foundation decision-support campaign

**Status:** Complete — all five records ratified 2026-07-21

**Beads:** `und-oh0.11.1.1`, `und-oh0.2.1.1`, `und-oh0.7.1.1`,
`und-oh0.5.1.1`, `und-oh0.3.1.1`

### Goal

Make Charter-000 and ADR-0001 through ADR-0004 decision-ready using explicit
local evidence, executable trace designs, alternatives, thresholds, and
unresolved human choices.

### Fence

This campaign owns evidence and recommendations for the mandatory foundation
decisions; it explicitly does not ratify those decisions, create product
crates, establish foundational public APIs, or select permanent representations
on the human's behalf.

### Non-goals

- No production dependencies, dependency pins, or feature changes.
- No `unsafe`.
- No public product APIs or prototype implementations.
- No Parley fork, patch, or upstream communication without explicit authority.
- No GitHub repository creation, remote mutation, commit, or push.

### Steps

1. Audit the checked-out Parley revision and map retained-preparation seams,
   data entry points, conformance needs, and evidence-backed gaps.
2. Specify canonical-storage and position traces for the three credible
   authority models.
3. Specify resumable-flow traces, checkpoint laws, virtual-extent corrections,
   and prototype decision thresholds.
4. Turn Charter-000 into a ratification packet with explicit proposed answers,
   alternatives, owners, and unresolved commitments.
5. Run repository, text, Beads, and dependency-cycle validation; export the
   scrubbed Beads graph; leave a durable handoff.

### Risks and controls

- **Local source drift:** record exact source revisions and distinguish observed
  facts from proposed contracts.
- **Paper architecture:** require each recommendation to name a trace,
  measurement, conformance law, or upstream seam.
- **Premature permanence:** keep representations private and decisions open
  until explicit human ratification.
- **Scope expansion:** stop before adding dependencies, crates, public APIs,
  `unsafe`, remotes, or external messages.

### Completion

The campaign is complete when all five records are ready for a human decision,
their support beads contain validation and unresolved-choice notes, and the
repository is green. The records remain Open until explicitly ratified.

All five records are Accepted. Their private proof obligations continue as
`und-oh0.10.1.1` through `und-oh0.10.1.4`; the first permanent-slice design is
`und-oh0.10.1.5`.

## First semantic-to-scene campaign

**Status:** Complete — executable proof landed through PR #6

**Beads:** `und-oh0.10.1`, `und-oh0.10.1.1`, `und-oh0.10.1.2`,
`und-oh0.10.1.3`, `und-oh0.10.1.4`, `und-oh0.10.1.5`

### Goal

Carry one permanent, headless path from an immutable semantic document through
retained Parley preparation into renderer-neutral scene geometry, paint slots,
and semantic mapping. The path must be useful to the living agent document and
must prove local-edit and paint-only reuse rather than merely rendering a
string.

### Fence

This campaign owns the first public vertical slice and the four private proof
obligations selected by the accepted ADRs. It does not stabilize the complete
document model, create a second shaping engine, promise general flow from a
single rectangle, or treat a wind-tunnel representation as a public contract.

### Non-goals

- No `unsafe`.
- No production dependency before its explicit gate.
- No Loro dependency before the collaboration-authority experiment gate.
- No speculative split into every crate named by the five-year topology.
- No public stable position type before the identity traces earn it.
- No claim above the evidence recorded in the proof ledger.

### Steps

1. Ratify the first-slice packet in
   `docs/design/0001-first-semantic-to-scene-slice.md`.
2. Execute the dependency-free canonical baseline of `identity-trace-v0` in a
   separate position wind-tunnel crate; record failures as well as passes.
3. Add the initial production crate boundary and draft public path only after
   the packet's crate and API gate.
4. Add the exact Parley pin and adapter only after its production-dependency
   gate and refreshed upstream audit.
5. Prove immutable snapshot publication, paragraph-local edit invalidation,
   paint-only reuse, semantic-to-geometry mapping, and deterministic headless
   scene output.
6. Measure position/storage, resumable flow, text-data footprint, and retained
   Parley seams against their accepted gates before broadening the façade.

### Risks and controls

- **Surface outruns hands:** begin with one production façade and one adapter
  fence; split only when dependency or ownership pressure is real.
- **Convincing but fake scene:** the first proof includes shaped glyph identity,
  source mapping, hit/caret evidence, and reuse counters; placeholder glyphs do
  not qualify.
- **Benchmark theater:** deterministic work counters are primary; wall time
  names machine, allocator, samples, and confidence.
- **Upstream drift:** pin an immutable Parley revision and rerun the seam audit
  before adding it.
- **Premature API permanence:** draft APIs carry a migration note and remain
  pre-stable until the first product trace exercises them.

### Completion

The campaign reaches Executable when the public vertical path runs headlessly
with no private product shortcut and its repository checks pass. It reaches
Measured only when all four accepted experiment beads contain checked-in
evidence. Higher proof stages remain unavailable until their named corpora and
owners exist.

## CPU visual-proof slice

**Status:** Complete

**Bead:** `und-oh0.10.1.8`

### Goal

Turn the executable semantic-to-scene spine into a compelling, inspectable
image. A downstream example must lower the real `TextScene` through `imaging`,
render it with `imaging_vello_cpu`, and retain a deterministic poster snapshot
that makes the hard text and invalidation evidence visible.

### Fence

This slice owns an external renderer adapter, visual composition, PNG output,
and snapshot verification. It does not move rendering into `underwood` or
`underwood_parley`, add production dependencies, broaden the draft public API,
or substitute decoration for real shaped output.

### Steps

1. Add one unpublished top-level example crate with released `imaging` and
   `imaging_vello_cpu` dependencies and an explicit governance fence.
2. Lower public scene fragments into clipped imaging glyph runs, preserving
   font instance data, glyph positions, transforms, and paint brushes.
3. Compose a poster from real Latin ligature, Arabic RTL, source, hit/caret,
   line, semantic, edit, reuse, and paint-only evidence.
4. Render with Vello CPU, inspect the output, and iterate until the composition
   is legible and compelling rather than merely technically non-empty.
5. Commit the accepted PNG, exact pixel snapshot test, and evidence notes; run
   the full stable, MSRV, policy, text, and Beads gates.

### Risks and controls

- **Pretty mirage:** all text, proof values, and diagnostics derive from real
  Underwood output; imaging-only primitives are limited to presentation.
- **Core contamination:** renderer and PNG dependencies stay in the external
  example crate, and the no-std production targets remain unchanged.
- **Snapshot gremlins:** use the CPU backend's stable render mode and require an
  exact RGBA match across the repository's Linux, macOS, and Windows CI jobs.
- **Font drift:** reuse the checked-in licensed font bytes rather than host
  font discovery.

### Completion

The slice is complete when a human-inspectable PNG is checked in, its pixels
are regenerated exclusively through the public Underwood path, the visual
evidence assertions and exact snapshot test pass, and the repository is green.

## Computed inline-style campaign

**Status:** Complete — executable proof landed through PR #6

**Beads:** `und-oh0.4.1`, `und-oh0.4.2`

### Goal

Replace the monolithic first-slice text style with the permanent computed
shaping/inline-flow/paint partitions, execute heterogeneous styles through the
real Parley and scene path, and prove the result with one compelling variable-
type specimen.

### Fence

This campaign owns complete computed inline values, their paragraph-local run
projection, stage-specific invalidation, migration of the public callsite, and
the executable specimen. It does not own cascade, font matching, paragraph-
break policy, block layout, decorations, tracking behavior, or rendering.

### Steps

1. Ratify `docs/design/0003-computed-inline-style-spine.md` and the shared
   Parlance vocabulary edge.
2. Implement validated shaping and inline-flow values plus complete per-leaf
   style assignment in `underwood`.
3. Project contiguous partitioned runs and make analysis, shaping, flow, and
   paint cache identities honest.
4. Adapt `underwood_parley` to execute language, feature, variation, and
   heterogeneous-size runs without making paint a shaping input.
5. Migrate all public callers and turn the CPU proof into a real variable-font
   and feature specimen built through the new path.
6. Run adversarial API/invalidation review, the complete local validation
   matrix, and remote CI before closing the beads.

### Risks and controls

- **Monolithic style identity:** compare and retain each partition separately;
  assert negative work at every boundary.
- **Parley invasion:** expose only Parlance vocabulary; contain engine types in
  `underwood_parley`.
- **Pretty but fake variable type:** assert normalized coordinates and glyph
  substitution before painting the specimen.
- **Placeholder property creep:** expose only values that execute end to end in
  this campaign.
- **Run-boundary ambiguity:** canonicalize complete leaf styles into contiguous
  paragraph runs and test mixed UTF-8, bidi, feature, paint, and empty-leaf
  boundaries.

### Completion

The campaign is complete when one heterogeneous document exercises the public
style path, each property has the intended invalidation evidence, the accepted
CPU snapshot is regenerated from real shaped output, all checked-in callers
use the canonical workflow, and the repository and remote CI are green.

## Fontique-backed font-request campaign

**Status:** Complete — executable proof validated in CI run 29854892508

**Bead:** `und-oh0.4.3`

### Goal

Replace the provisional ordered-font shortcut with real Fontique family,
attribute, fallback, coverage, and synthesis selection while preserving
Underwood's backend-neutral computed-style and scene boundaries.

### Fence

This campaign owns computed font requests, their projection into Fontique, a
deterministic caller-supplied catalog, selection work reporting, exact resolved
font/synthesis evidence, and the executable proof. It does not move matching
into Underwood, expose engine types, enable system font discovery, add unsafe,
or claim renderer effects that the proof backend does not execute.

### Steps

1. Ratify `docs/design/0004-fontique-backed-font-requests.md` and migrate
   `ShapingStyle` to the shared Parlance request vocabulary.
2. Turn `underwood_parley::FontSet` into a Fontique catalog with explicit
   generic and script/language fallback configuration.
3. Query Fontique per item/cluster, pass the selected instance and synthesis to
   Parley Core, and retain portable synthesis evidence in the scene.
4. Make selection work and request invalidation observable, including
   paragraph-local negative-work proofs.
5. Migrate the headless example and benchmark, then extend the CPU poster with
   named families, variable instances, fallback, and supported synthetic style.
6. Run API, real-vs-mirage, and workspace-green reviews; commit coherent slices,
   publish a draft PR, and land only after local and remote gates pass.

### Risks and controls

- **Matcher duplication:** adapter code is limited to Fontique query setup and
  the cluster callback required by Parley Core.
- **Unstable style identity:** family sources are parsed into one structural
  form once; every numeric request is validated before entering a cache key.
- **Hidden selection cost:** selection receives its own deterministic work
  counter and cache-hit assertions.
- **Synthesis theater:** final normalized coordinates and raw synthesis evidence
  are asserted; the visual proof uses only effects its renderer executes.
- **Machine-dependent output:** system fonts stay disabled and all proof fonts
  remain checked in.

### Completion

The campaign is complete when all Design-0004 proofs run through one public
workflow, the poster visibly demonstrates the real resolved instances and
fallback, all public callers are migrated, and the full local and remote
validation matrices pass.

## Retained Parley ShapedText uptake

**Status:** Complete — local and remote proof green in PR #8

**Beads:** `und-oh0.2.5`, `und-oh0.10.1.4`

### Goal

Replace the callback-era copied shaped-run cache with Parley Core's upstream
owned `ShapedText`, preserve Underwood's portable adapter boundary, and leave
paragraph breaking with one retained cluster and metric truth.

### Fence

This campaign owns the immutable Parley revision uptake, private retained
shaping storage, source-correct lowering, migration evidence, and conformance
review. It does not expose Parley types, implement line breaking, change font
matching ownership, or fold hit testing and paint-coverage follow-ups into the
same change.

### Steps

1. Ratify `docs/design/0005-retained-parley-shaped-text.md` against exact
   Parley main and update the existing dependency pins together.
2. Replace the callback-owned run/glyph copies with one reusable `ShapedText`
   plus the minimal script sidecar.
3. Lower Parley's cluster storage into portable prepared glyphs with explicit
   ligature, RTL, UTF-8, and control-only source laws.
4. Preserve public invalidation/work behavior and migrate the private seam
   experiment from "current gap" to executable retained-result evidence.
5. Compare correctness and retained-path performance against the previous pin,
   run Lynx review, and land only after all local and remote gates pass.

### Risks and controls

- **Representation mirage:** delete the local shaped-run and glyph structs;
  merely copying into renamed equivalents does not complete the campaign.
- **Ligature source loss:** union start and continuation cluster ranges and
  assert the exact `ffi` range through the public output.
- **Control phantom glyphs:** permit honest glyphless prepared runs instead of
  manufacturing output that Parley intentionally removed.
- **Upstream drift:** record and pin exact `6c81e1d`; all Parley workspace
  dependencies resolve from one revision.
- **Breaking scope creep:** record Parley's boundary and metrics now, but leave
  line formation to `und-oh0.2.2`.

### Completion

The campaign is complete when Underwood retains Parley's native `ShapedText`,
the duplicate callback representation is absent, public conformance and
invalidation proofs pass on the new pin, the measured cost is recorded, and
local plus remote validation are green.

GitHub Actions run `29864780006` passed all eight jobs on Linux, macOS, and
Windows, including MSRV, rustdoc, repository/text policy, bare-metal, and
WebAssembly gates. Paragraph breaking continues independently as
`und-oh0.2.2`.

## Parley-backed paragraph formation

**Status:** Complete — bounded break reshaping executes in the product path

**Bead:** `und-oh0.2.2`

### Goal

Delete scene construction's glyph-by-glyph wrapping and hard-coded 80/20 line
box split. Form lines from Parley Core's Unicode boundaries and font metrics,
retain analysis and shaping across width-only changes, and prove mandatory,
legal, bidi, and break-sensitive behavior through the public scene path.

### Fence

Parley owns Unicode analysis, break opportunities, font metrics, and bounded
break reshaping. `underwood_parley` owns the narrow adapter from those retained
results to portable formed-line records. Underwood owns document/region flow,
paragraph stacking, scene lowering, and future resumable checkpoints. This
campaign does not adopt high-level `parley::Layout`, duplicate its private
layout engine, add pagination, or claim that an absent upstream reshaping seam
exists.

### Steps

1. Audit exact Parley revision `6c81e1d`, high-level breaker behavior, current
   Underwood geometry, and draft PR #634's break/concat contract.
2. Ratify Design-0006: move width and inline-flow inputs across a paragraph-
   formation contract and return source-valid, visual-order lines with real
   metrics; include the draft-API migration.
3. Add a high-level Parley oracle to the private seam experiment for legal and
   mandatory breaks, CRLF, overflow, bidi line ranges, and line metrics.
4. Implement retained Core-backed line formation in `underwood_parley`, remove
   text preparation from scene construction, and prove width-only reuse.
5. Exercise and record the break-sensitive case against the upstream bounded
   reshape seam; do not close or land the campaign with a counterfeit local
   substitute.
6. Update the visual proof, run Cedar/Lynx/Rook review, measure the product
   path, and pass all local and remote gates before landing.

### Executable implementation

Commit `023c777` completes the safe-break portion through the public product
path: legal and mandatory boundaries, real metrics, mixed bidi, complete
cross-leaf source, explicit unrendered controls, width/line-height retained
shaping, work evidence, and the two-line CPU poster. The focused local gates and
same-machine benchmark are recorded in
`docs/proof/parley-formation-review-2026-07-22.md` and
`docs/proof/parley-formation-benchmark-2026-07-22.md`.

The final implementation now pins public Parley candidate `44d155e`, rebased
directly onto current main revision `38809fb` without the superseded glyph-ink
experiment. Parley Core retains the inputs
needed for bounded break/concat reshaping; `underwood_parley` applies that seam
at selected legal boundaries, backtracks if reshaping changes fit, and lowers
only the committed formed result. The product corpus proves a legal Arabic
zero-width break changes real glyph output without rerunning analysis or
initial shaping, leaves no glyph crossing the line seam, and reports exactly
one bounded reshape. A second trap makes that reshape exceed the width, then
proves concat restores the exact canonical shape before the adapter commits the
earlier legal boundary.

The full local stable, MSRV, denied-warning rustdoc, repository/text policy,
Beads, bare-metal, and WebAssembly matrix passes on the candidate pin. GitHub
Actions run `29894614476` passes all eight jobs across Linux, macOS, and
Windows. Upstream adoption and removal of the temporary fork URL are isolated
in `und-oh0.2.7`; that lifecycle work does not erase or postpone the executable
product result in this completed campaign.

### Risks and controls

- **Second text engine:** use Core boundary/metric truth and a small explicit
  greedy policy; do not copy high-level Parley's `LayoutData` or 1,500-line
  breaker.
- **Portable-record theater:** the final scene must consume formed-line output;
  retaining unused line records does not count.
- **Bidi corruption:** choose breaks in logical source order, then apply UAX #9
  L2 run reordering per line and preserve visual glyph order within each run.
- **Break reshape mirage:** a committed unsafe boundary must change Arabic
  joining or split a ligature and concat must restore the original output.
- **Invalidation collapse:** width and line height enter formation identity,
  never analysis or shaping identity; tests assert zero analyzer/itemizer/
  shaper work on width-only and line-height-only changes.
- **Upstream drift:** the oracle, production adapter, and documentation name one
  immutable revision; any pin change reruns the seam matrix.

### Completion

The campaign is complete only when legal and mandatory breaks, real line
metrics, mixed bidi, width-only retained shaping, and break/concat reshaping all
execute through the product path; the provisional scene breaker is deleted;
the migration and upstream lifecycle are recorded; the visual specimen exposes
the improvement; and local plus remote validation are green.

## Renderer-owned glyph paint extent

**Status:** Complete — local and remote proof green in PR #15

**Beads:** `und-oh0.2.4`, `und-oh0.2.9`

### Goal

Keep the Arabic-dot and no-invented-ligature lessons from the original paint
coverage work while removing outline bounds as a prerequisite for ordinary
glyph rendering. Support synthesized and non-outline glyph paint without
pretending that an outline rectangle describes all backend output.

### Fence

Parley Core and Fontique own shaping, font selection, and synthesis.
`underwood_parley` owns complete source-to-paint assignment. Underwood owns the
portable retained scene and explicit partial-paint clips. The renderer owns
actual painted extent. Hit testing and editing use cluster geometry. No layer
substitutes advance bounds for ink or requires outline metadata to draw a whole
glyph.

### Steps

1. Keep the native Chinese IME commit as a failing product-path regression; it
   must resolve a real Han system fallback and pass preparation.
2. Make ordinary paint coverage source-complete and unclipped. Preserve the
   future conformant split seam as explicit, validated clipped segments.
3. Remove outline-metric and synthetic-embolden rejection from
   `underwood_parley` while retaining cross-paint rejection.
4. Migrate render adapters, diagnostics, and pointer tests away from mandatory
   glyph clips. Prove Arabic pixels remain present.
5. Supersede the stale design and proof claims, remove the abandoned Parley ink
   candidate from the pin, and run all local and remote gates.

### Risks and controls

- **Unknown extent treated as empty:** ordinary glyphs draw without a clip;
  future damage and culling stay conservative.
- **Advance renamed as ink:** no universal ink-bounds API is introduced.
- **Split-paint mirage:** only exact, complete segment clips are accepted;
  current cross-paint Parley glyphs remain explicitly unsupported.
- **Interaction regression:** hits, carets, selections, and IME rectangles use
  cluster and line geometry, never painted extent.
- **Backend synthesis gap:** preparation retains Fontique's synthesis request;
  renderer fidelity is reported independently and not overstated.

### Completion

The campaign is complete when the exact Chinese IME regression, synthetic
emboldening, ordinary unclipped paint, explicit split validation, Arabic pixel
proof, documentation migration, clean Parley bounded-reshape pin, and all local
plus remote gates pass. Design-0010 is authoritative; Design-0007 remains only
the historical record of the earlier investigation.

## Live document showcase

**Status:** Complete — live review and remote proof green in PR #11

### Goal

Open one genuine semantic document in a resizable native window and make the
retained Underwood pipeline visible in real time. Resizing must reflow mixed
LTR/RTL paragraphs; a local edit, paint-only change, and animated variable-font
axis must report the work they actually cause.

### Fence

The external showcase crate owns native events, presentation, demo composition,
and narration. It explicitly does not own document semantics, font selection,
shaping, line formation, renderer-neutral scene geometry, or rasterization.
`winit` and `softbuffer` are example-only host dependencies; production crates
remain toolkit- and renderer-independent.

### Integration

```text
winit resize / keys / timer
             |
             v
Underwood document -> retained TextScene -> imaging -> imaging_vello_cpu
                                                          |
                                                          v
                                               RGBA -> softbuffer
```

### Steps

1. Add the isolated native showcase crate and additive semantic heading roles.
2. Compose one heading/deck/body document with variable Latin type, mixed
   English/Arabic text, real fallback, and multiple paragraphs.
3. Re-prepare on resize and present CPU-rendered pixels without moving host or
   rendering dependencies into production crates.
4. Add local-edit, paint-only, reset, and optional variable-axis animation
   controls with visible `WorkReport` evidence.
5. Run the app, inspect the live output at narrow and wide sizes, then run the
   workspace Definition of Done and preserve a reliable static fallback.

### Risks and controls

- **Static poster in a window:** width changes must alter real line formation.
- **Fake document:** the content is one snapshot with semantic heading and body
  blocks, not separately positioned labels masquerading as document flow.
- **Animation theater:** axis animation changes an executed shaping value and
  reports shaping work; it is optional and off by default.
- **Toolkit invasion:** all native and presentation dependencies stay in the
  unpublished external example.
- **Meeting fragility:** retain the landed deterministic poster and keep the
  showcase event-driven when animation is disabled.

### Completion

The slice is complete: the native app visibly and smoothly reflows the real
document, its controls exercise honest invalidation paths, the core dependency
fence remains unchanged, and the full local and remote workspace gates pass.
The live meeting demonstration succeeded, and GitHub Actions run `29911682384`
proved the eight-job matrix across Linux, macOS, Windows, MSRV, rustdoc,
repository policy, formatting, bare metal, and WebAssembly.

## Interactive semantic document campaign

**Status:** Active — native editor and semantic activation landed through PR
#17; accepted Design-0011 specifies the extended-grapheme completion

**Design:** Design-0009 and accepted Design-0011

### Goal

Replace diagnostic interaction approximations with real paragraph-engine
cluster/caret mechanics, then make the live semantic document directly
selectable and editable through validated transactions and a separate IME
composition epoch. The IME boundary must serve both Winit-like event feeds and
host-driven native protocols that query selection, text, ranges, geometry, and
hit testing synchronously.

### Fence

Parley owns paragraph-local cluster, bidi, caret-affinity, and cursor facts;
Underwood owns revision-bound semantic positions, selection geometry,
transactions, source projection, revisioned editable surfaces, composition
epochs, and retained work; the showcase owns native gestures, keyboard/IME
translation, focus, blinking, action routing, and presentation. Platform
adapters own offset encodings, synchronous protocol callbacks, coordinate
transforms, locking, and lifecycle notifications. Durable collaborative
anchors, history, clipboard policy, permanent link schema, and block flow are
outside this campaign.

### Integration

```text
Parley shaped/formatted clusters
              |
              v
portable paragraph interaction map
              |
              v
snapshot positions -> selections / transaction / composition
              |
              v
retained TextScene -> imaging -> live native proof
```

### Steps

1. Complete `und-oh0.2.3` with a conformance-first portable cluster and caret
   map; delete fragment-bounds hit testing and query-point caret geometry.
2. Add revision-bound collapsed positions, selection sets, bidi-correct visual
   selections that may contain multiple logical ranges, owned selection
   rectangles, visual/logical cluster movement, and validated same-leaf-per-
   selection replacement with returned selections in the new revision.
3. Specify the shared editable-surface contract for both Winit-like event feeds
   and AppKit/UIKit/Android/TSF-like host conversations. Add an explicit
   generated-source composition projection and a cache that retains committed
   paragraph formation across preedit churn and cancel.
4. Translate native pointer, keyboard, and Winit IME events in the showcase;
   render selection, caret, and composition overlays without changing text
   preparation or moving host policy into core.
5. Add exact semantic hover, press cancellation, and activation through a
   showcase-owned `SemanticId` action registry; hand a URL-shaped request to the
   host without stabilizing a permanent core link schema or launching a browser.
6. Run the mixed-script/ligature/combining/break/empty-text corpus, event-feed
   and host-query IME traces, native visual review, full local/remote gates, and
   land coherent PR slices.
7. Replace shaping-record deletion with Parley-analysis-derived extended-
   grapheme interaction units, preserve multi-leaf and generated provenance,
   and publish same-paragraph multi-leaf selections without structural edits.

### Risks and controls

- **Byte offsets renamed positions:** every position names a snapshot revision,
  validated UTF-8 boundary, semantic leaf, and affinity; stale use fails.
- **Underwood grows a second cursor engine:** cluster and affinity facts enter
  through the paragraph adapter; scene code only maps and composes them.
- **One range masquerades as bidi selection:** one selection may retain several
  logically ordered source ranges, while the scene separately owns a set of
  independent selections/insertion points. Geometry preserves both indices.
- **Action content becomes unselectable:** movement beyond the click threshold
  transfers the exact pressed cluster position into visual-selection policy;
  reciprocal bidi caret paths must yield direction-independent ranges.
- **Multi-caret edits become order-dependent:** validate the whole selection
  set, reject overlap and duplicate insertion points, then apply source edits
  in reverse order and publish exactly once.
- **IME commits every preedit:** composition has a separate epoch and generated
  source mapping; only commit publishes a document transaction.
- **Winit defines the core IME model:** a revisioned editable surface answers
  the richer host-driven text/range/geometry/hit-query contract; Winit is one
  reduced-information adapter over the same state machine.
- **Native offsets escape into document APIs:** platform UTF-16, code-point, or
  protocol offsets are converted at the explicit focused-surface boundary;
  semantic snapshot positions remain the internal currency.
- **Composition destroys reuse:** transient and committed paragraph formations
  coexist; cancel must reuse the committed entry.
- **Editor policy invades core:** key chords, focus, pointer gestures, blink,
  and platform IME event translation stay in the example host.
- **Durable-position overclaim:** snapshot positions are never serialized or
  presented as surviving unrelated edits; ADR-0001's anchor gate remains.
- **Shaping record masquerades as grapheme:** consume Parley's stored analysis
  boundaries once, retain every shaped visual slice, and expose only the two
  endpoints of the complete interaction unit.
- **Cross-leaf deletion erases semantics:** apply leaf-local ranges in reverse,
  retain every leaf identity and role, insert once in the first source leaf,
  and continue to reject cross-paragraph structural replacement.

### Completion

The campaign is complete when a user can click and drag through mixed LTR/RTL
and ligature text, create multiple independent carets, see a visual bidi
selection retain its disjoint logical ranges, type and delete through one
atomic multi-selection transaction, exercise real IME preedit/commit/cancel,
serve both event-feed and synchronous host-query IME traces from one state,
activate semantic content through exact hits, and observe honest retained
work. Precomposed and decomposed extended graphemes—including units crossing
semantic or generated-source boundaries—must move and delete atomically while
OpenType ligature components remain independently reachable. The old
interaction approximations must be gone and every local, remote, portability,
API, and proof gate must be green.
