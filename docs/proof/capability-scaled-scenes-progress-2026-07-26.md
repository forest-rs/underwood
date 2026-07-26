# Capability-scaled scene progress — 2026-07-26

## Status

These checkpoints implement the first four approved Design-0018 boundaries.
This is not the completion proof.

Prepared scenes now distinguish display, source provenance, semantics, hit
testing, selection, navigation, and native text input. A uniform request is
allocation-free to construct, while `SceneFeaturePolicy` supports sparse
paragraph overrides without promoting unrelated siblings. `BlockRequest`
supports the same capability vocabulary.

Display-only scene segments physically omit the source, semantic, hit,
selection, movement, and native-input records that they did not request.
Underwood's Parley adapter also omits cursor-movement lowering below the
selection tier. A warm capability upgrade shares the existing analysis,
shaping, line formation, and immutable layout facts while adding only the
requested sidecars. Reusable adapter facts now have an independent
deterministic byte budget. A zero budget or explicit trim leaves published
scenes valid and makes a later capability upgrade visibly cold.

Paragraph scenes now retain one source-independent glyph table and one
paragraph-local source map. Lines, clusters, movements, semantics, and paint
observations keep compact projected spans or source-map indexes instead of
owning repeated `Vec<LocalRange>` values. Composition epoch rebinding updates
the map once rather than rewriting every source-bearing record. This
consolidates correctness as well as storage: glyph, hit, selection, semantic,
and export paths can no longer rebuild contradictory provenance for the same
multi-leaf grapheme or ligature.

Ordinary paint is run-sized. Adjacent glyphs with the same run and paint state
share one fragment range over a flat paint-glyph table. Explicitly clipped
split-ligature paint remains the exceptional multi-fragment case, and those
paint observations reference one source-independent layout glyph.

Source iterators and hit units are borrowed views over those paragraph tables.
Mapping one hit back to one or many authored leaves does not allocate.
Callers that need to retain a hit unit beyond the scene materialize it
explicitly with `SnapshotTextUnitView::to_owned` or
`ProjectedTextUnitView::to_owned`.

The remaining Design-0018 work is deliberate and tracked by
`und-oh0.13.17.10`:

- requested-versus-resident byte diagnostics;
- upgrade, 2,048-sibling mixed-document, editable-typing, and
  creation/destruction wind tunnels;
- the final complete portability, documentation, and repository gates.

## Public migration

`SceneRequest::new` and `BlockRequest::new` now request
`SceneFeatures::DISPLAY`. Callers opt into more retained facts with
`with_features` or, for documents, `with_feature_policy`.

The named profiles are convenience closures over one capability lattice:

```rust
let request = SceneRequest::new(constraint, &styles, &paint)
    .with_features(SceneFeatures::EDITABLE);
let output = layout.prepare(&snapshot, &request)?;
let editing = output.scene().editing()?;
let hit = editing.hit_test_closest(point);
```

Sparse document requests keep static siblings lean:

```rust
let features = SceneFeaturePolicy::uniform(SceneFeatures::DISPLAY)
    .with_paragraph(editor, SceneFeatures::EDITABLE);
let request =
    SceneRequest::new(constraint, &styles, &paint).with_feature_policy(features);
```

Capability-dependent observations moved from `TextScene` and
`CompositionScene` onto checked borrowed facades:

- source traversal uses `scene.sources()?` and
  `SceneSourceAccess::{for_line, for_fragment, for_glyph}`;
- semantics use `scene.semantics()?.iter()`;
- point queries use `scene.interaction()?`;
- selection construction and geometry use `scene.selection()?`;
- navigation, editing, and composition start use `scene.editing()?`;
- transient composition interaction and editing use the corresponding
  `ProjectedScene*` facades.

Display traversal remains unconditional through `scene.display()`. Existing
`scene.lines()` and `scene.fragments()` traversal remains a display
observation, but line, fragment, and glyph provenance is available only
through the source facade.

There is intentionally no compatibility shim that materializes missing data.
`MissingSceneCapability` reports the required, originally requested, and
resident capability closures, plus the affected paragraph when it is known.

Hit testing now returns `TextHit<SnapshotTextUnitView<'_>>` for committed
scenes and `TextHit<ProjectedTextUnitView<'_>>` for transient composition
scenes. `source().sources()` returns a borrowed exact-size, double-ended
iterator rather than an owned slice. Existing short-lived call sites usually
need only to iterate it. A caller that must retain the complete unit uses
`source().to_owned()`. Fragment, glyph, line, and semantic source accessors
likewise return allocation-free `SnapshotSources` or `ProjectedSources`
iterators.

## Adapter-fact residency

Published scenes and reusable adapter facts have independent lifetimes.
`CacheBudget::with_adapter_facts_bytes` configures the latter; its default is
zero so a wall of stable display labels does not silently retain editor-scale
analysis and shaping state. Editing and showcase hosts opt into an explicit
budget when low-latency reforming and capability upgrades matter.

After a successful output is validated, lowered, and published,
`LayoutEngine` commits its adapter preparation. The Parley adapter then
enforces a deterministic byte-budgeted LRU over its analysis, shaping, formed
line, and portable prepared-output records. Shared font resources are not
charged to each entry. Underwood-owned vector capacities are charged directly;
opaque Parley storage is charged from its observable retained slices because
their private capacities are not available through the adapter API. Allocation
wind tunnels remain the allocator-exact evidence.

`LayoutEngine::trim_adapter_facts` drops this reusable state without
invalidating a caller-held scene. `CacheDiagnostics::adapter_facts` reports
the budget, entries, resident and peak charge, known scratch, hits, misses,
evictions, and explicit releases. Stateless adapters may return `None`.
`PreparationReuse` separately reports adapter-fact hits and misses and warm
and cold capability upgrades.

An exact published-scene hit does not touch adapter recency because it does
not enter formation. Adapter LRU therefore represents reusable-formation
recency rather than display traversal recency.

The public adapter migration is intentionally breaking:

- `ParagraphFormation::retained_entries` became the richer
  `retained_facts` diagnostic;
- retained adapters accept `set_retained_facts_budget`, observe successful
  consumption through `commit_preparation`, and implement
  `trim_retained_facts`;
- `ParagraphFormationOutput::reuse` identifies cold, retained-fact, and exact
  retained-output paths;
- `CacheDiagnostics::backend_entries` became `adapter_facts`.

## Executable evidence

Focused regressions prove:

- a default display scene rejects interaction and has no source, selection, or
  editing facade;
- the error reports requested and resident capabilities rather than returning
  an indistinguishable empty value;
- one sparse editable paragraph does not promote a display-only sibling;
- display preparation omits Parley cursor movements;
- a warm editable upgrade repeats no analysis, shaping, or line formation;
- zero adapter budget immediately evicts reusable facts without invalidating
  the published display scene;
- upgrading after zero-budget eviction is cold, repeats work only for the
  target, and reports the miss and degradation;
- explicit adapter trim preserves a caller-held scene and makes only a later
  upgrade cold;
- an exact one-entry byte budget evicts the least-recently-formed identity
  while leaving both published scenes valid;
- renderer, PDF, showcase, IME, headless, and visual-proof consumers all use
  explicit capabilities and checked facades.
- ordinary mixed-paint text retains fewer paint fragments than glyphs, while
  explicit split-paint ligatures retain their clip topology and one physical
  glyph identity;
- cross-leaf graphemes, collapsed whitespace, bidi hits, and transient
  composition all resolve through the same paragraph-local source map.

## Matched allocation checkpoint

The macOS `malloc_history` wind tunnel was run in release mode for the exact
same one-label scenarios immediately before this representation change and at
this checkpoint. Setup allocations are subtracted in both trees:

| Scenario | Before calls | After calls | Before bytes | After bytes |
|---|---:|---:|---:|---:|
| cold display | 148 | 149 | 62,186 | 61,530 |
| cold selectable | 219 | 184 | 81,970 | 72,778 |
| cold editable | 316 | 186 | 112,090 | 75,634 |
| repeated hit query | 2 | 0 | 96 | 0 |
| paint-slot churn | 35 | 35 | 5,161 | 4,001 |
| alignment churn | 40 | 40 | 8,997 | 5,813 |
| justification churn | 46 | 44 | 9,173 | 5,941 |
| localized edit | 102 | 103 | 18,181 | 16,781 |

The one-call increases on cold display and localized edit come from the new
central source-independent paragraph structures; they still reduce allocated
bytes. The capability-scaled paths are the central claim: selectable drops 35
calls, editable drops 130 calls, and a hot hit with source traversal performs
no allocation. This table does not yet substitute for the required 64,
1,000, and 2,048-item residency and mixed-document matrices.

The checkpoint passes:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The final proof will add the complete Rust 1.88, `no_std`, rustdoc, repository,
and matched allocation/residency matrices after the remaining representation
and wind-tunnel work is complete.
