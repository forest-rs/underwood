# Capability-scaled scene progress — 2026-07-26

## Status

These checkpoints implement the first five approved Design-0018 boundaries.
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
`und-oh0.13.17.10`: creation/destruction churn, the source-heavy/bidi/native
corpus matrix, the comparison with high-level Parley `Layout`, remaining
table/query compaction justified by those measurements, and the final complete
portability, documentation, and repository gates.

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

The prepared-adapter interaction representation is also intentionally
breaking. `PreparedLine::{try_new, try_new_in_slot}` now accept one flat visual
slice table followed by range-indexed interaction-unit records.
`PreparedInteractionUnit::{try_new, try_new_with_justification}` accept the
unit's slice-table range and exact advance rather than collecting an owned
slice vector. Final line construction validates that the ranges partition the
table, each unit remains source-complete, and recorded advances agree with the
table. A unit is therefore never observable with a placeholder metric.

`PreparedLine::units` now returns the allocation-free
`PreparedInteractionUnits` traversal. Its borrowed
`PreparedInteractionUnitView` dereferences to the unit record and exposes the
resolved visual slices through `slices()`. Existing adapters should build one
slice vector per line, append each unit's slices contiguously, and store the
resulting index range in the unit. There is no compatibility shim that
recreates one allocation per grapheme.

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

## Scene residency diagnostics

`TextScene` and `CompositionScene` expose `residency()` and an allocation-free
`paragraph_residencies()` traversal. Each paragraph observation names the
normalized requested features, the physically resident feature closure, and
deterministic charges for structure, layout, paint, sources, semantics, hit
testing, selection, navigation, and native text input.

These are representation charges, not allocator-exact process memory. They
cover owned table capacities and immutable packed records and exclude allocator
metadata, fonts, paint values, caller storage, and renderer resources.
`CacheDiagnostics::scene_cache_residency()` reports the same category model
for engine-owned paragraph cache entries. Published-scene structure includes
its persistent spine; cache residency does not.

The diagnostic found and retired one real mirage before this checkpoint:
accessible text initially denied the hit-testing facade but still retained the
complete cluster/hit table because semantic bounds were derived through it.
Semantic bounds are now accumulated directly from interaction units into
per-leaf bounds. An accessible scene retains semantics and sources with zero
hit-testing bytes and never constructs the cluster table.

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

## Capability scaling checkpoint

The checked release runner
`benches/labels/profile-capability-scaling.sh` measures one editable paragraph
among display-only siblings. Each repeat asserts zero preparation work; each
typing event asserts exactly one analyzed, shaped, and lowered paragraph; and
the per-paragraph residency traversal proves that no display sibling retains
sources or interaction sidecars.

| Paragraphs | Upgrade ns | Exact repeat ns | Typing ns/keystroke |
|---:|---:|---:|---:|
| 64 | 53,458 | 68 | 24,957 |
| 1,000 | 345,250 | 72 | 35,532 |
| 2,048 | 759,958 | 63 | 47,373 |

The upgrade intentionally publishes a new sparse-capability scene and scales
with the persistent document shape. Exact published-root reuse is constant
time. Typing grows only with logarithmic persistent document/scene path depth;
unchanged siblings perform no projection, formation, or geometry work.

The same scenes report:

| Paragraphs | Total bytes | Structure | Layout | Paint | Editor sources | Editor hit | Editor selection | Editor navigation |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 64 | 197,080 | 31,496 | 87,824 | 64,512 | 352 | 4,704 | 2,560 | 5,632 |
| 1,000 | 2,892,760 | 495,752 | 1,375,760 | 1,008,000 | 352 | 4,704 | 2,560 | 5,632 |
| 2,048 | 5,911,000 | 1,015,560 | 2,817,808 | 2,064,384 | 352 | 4,704 | 2,560 | 5,632 |

Only the one editor owns the four interaction columns; their charges remain
constant as display siblings are added.

Matched `malloc_history` traces reinforce the work law:

| Paragraphs | Exact-repeat calls/bytes | One typing event calls/bytes |
|---:|---:|---:|
| 64 | 0 / 0 | 176 / 52,752 |
| 1,000 | 0 / 0 | 179 / 53,784 |
| 2,048 | within 1 call / 128 bytes of baseline | 184 / 55,080 |

The 2,048 exact-repeat subtraction was `-1` call / `-128` bytes because the
separate baseline process contained one more runtime allocation; it is
reported as profiler noise, not as negative allocation. The changed editable
paragraph is not allocation-free: stack attribution shows most calls in
scene geometry/sidecar construction and adapter interaction/cursor lowering,
with much smaller contributions from shaping and document publication. That
is the measured target for the remaining typed-table and scratch work.

The focused allocator subset is:

```sh
benches/labels/profile-allocations.sh \
    target/release/underwood_label_benchmark 1 2048 capabilities
```

For the large mixed-document typing proof, the `typing` subset runs only the
matched primed, exact-repeat, and one-editor typing processes. This avoids
retaining irrelevant full allocation histories:

```sh
benches/labels/profile-allocations.sh \
    target/release/underwood_label_benchmark 1 1000 typing
```

The profiler script fails closed if `malloc_history` cannot attach or extract a
trace. An attachment failure must never be rendered as a zero-allocation
result.

## Packed adapter-interaction checkpoint

Replacing the changed paragraph's independently allocated visual-slice vectors
with one line-local table removes exactly 24 allocation calls and 2,888
requested bytes at every measured document scale:

| Paragraphs | Before calls/bytes | Packed calls/bytes | Change |
|---:|---:|---:|---:|
| 1 | 168 / 50,376 | 144 / 47,488 | -24 / -2,888 |
| 64 | 176 / 52,752 | 152 / 49,864 | -24 / -2,888 |
| 1,000 | 179 / 53,784 | 155 / 50,896 | -24 / -2,888 |
| 2,048 | 184 / 55,080 | 160 / 52,192 | -24 / -2,888 |

Exact repeat remains zero allocations at 1, 64, and 1,000 paragraphs. At 2,048
paragraphs its subtraction remains within the same one-call/128-byte
cross-process profiler noise described above. The constant 24-call reduction
is the physical proof: the removed calls belong to the changed paragraph's old
per-unit representation, not unchanged siblings, and those adapter allocations
no longer exist.

The 64/1,000/2,048 release timing rerun reports 66/73/66 ns per exact repeat and
25.350/37.033/50.412 microseconds per typed edit. These single-run observations
are within ordinary machine noise of the prior checkpoint; the representation
change is an allocation and residency improvement, not a claimed CPU
improvement.

## Packed scene-hit checkpoint

Scene lowering previously repeated the same shape one layer later: every
`CachedCluster` owned a `Vec<CachedHitSlice>`. The packed scene representation
stores a range in each cluster and one exact-capacity hit-slice table for the
paragraph. Clusters and slices share one `Arc<CachedHitGeometry>` because they
are one independently reusable capability sidecar: paint-only publication and
capability supersets share the pair without copying, and dropping the last
paragraph segment reclaims both tables together. Two separate shared arrays or
a global arena would add ownership machinery without improving this lifetime.

For the mixed editable paragraph, deterministic hit-testing residency falls
from 4,704 to 2,880 bytes, a reduction of 1,824 bytes. The complete mixed scene
falls by the same amount at 64, 1,000, and 2,048 paragraphs because only the
editor retains hit facts.

Matched typing traces against the immediately preceding packed-adapter
checkpoint are:

| Paragraphs | Adapter-packed calls/bytes | Adapter + scene packed calls/bytes |
|---:|---:|---:|
| 1 | 144 / 47,488 | 116 / 43,224 |
| 64 | 152 / 49,864 | 124 / 45,600 |
| 1,000 | 155 / 50,896 | 127–128 / 46,632–48,296 |
| 2,048 | 160 / 52,192 | 133 / 48,056 |

The repeated 1,000-paragraph process differed by one runtime allocation and
1,664 bytes; both observations are retained rather than selecting one silently.
The second trace reproduces the 28-call/4,264-byte representation delta seen at
1 and 64 paragraphs. At 2,048 paragraphs the matched exact-repeat process is
within one call/128 bytes of its baseline, the same documented cross-process
noise floor.

Against the pre-compaction checkpoint, the 64-paragraph event has fallen from
176 calls/52,752 bytes to 124 calls/45,600 bytes while preserving exactly one
changed paragraph. Exact-repeat work remains O(1) and allocation-free.

The 64/1,000/2,048 timing rerun reports 64/78/63 ns per exact repeat and
24.155/34.098/45.342 microseconds per typed edit. Those single-run values do
not establish a CPU speedup; they show no measured regression from replacing
nested vectors with flat indexed traversal.

The checkpoint passes:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo +1.88.0 check --workspace --all-targets --all-features \
    --exclude underwood_showcase \
    --exclude underwood_visual_proof \
    --exclude underwood_pdf \
    --exclude underwood_pdf_proof
cargo check -p underwood -p underwood_parley \
    --target x86_64-unknown-none
cargo check -p underwood -p underwood_parley \
    --target wasm32-unknown-unknown
cargo xtask check
```

The final proof will rerun these gates and the complete matched
allocation/residency matrices after the remaining representation and
wind-tunnel work is complete.
