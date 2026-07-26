# Retained lifecycle progress — 2026-07-25

This records each independently green implementation slice of Design-0017
against the matched public-path wind tunnel established in
`retained-lifecycle-baseline-2026-07-25.md`.

The measurements are release builds on the same Apple arm64 host. Allocation
counts are deterministic event-scoped observations; timings are machine-local
and are included only when enough samples support a useful comparison.

## Slice 2: provenance preflight

`StyleMap` is now a cheap clone over strong immutable backing. Equal assignments
preserve that identity, while a real mutation copy-on-writes the backing state.
Each paragraph cache retains a strong style owner plus its exact paragraph,
constraint, region-flow, and incoming-cursor provenance. An exact provenance
hit occurs before `Projection::new`, so it allocates no projected string,
source map, or projected style/run vectors. Unrelated but value-equal inputs
may be rescued by a complete paragraph-local value comparison; a value
difference continues through the existing checked projection path.

`PreparationReuse::preflight_reuses` makes the fast-path decision directly
observable without allocator instrumentation.

| Event | Paragraphs | Baseline calls | Current calls | Calls removed |
|---|---:|---:|---:|---:|
| retained exact repeat | 64 | 42,607 | 41,966 | 641 |
| retained exact repeat | 1,000 | 666,167 | 656,166 | 10,001 |
| localized prepare | 64 | 43,247 | 42,618 | 629 |
| localized prepare | 1,000 | 666,807 | 656,818 | 9,989 |

The exact-repeat reduction scales at ten calls per paragraph plus one
event-level allocation. Localized preparation retains the same reduction for
every unchanged sibling while the changed paragraph takes the checked path.

### What this does not claim

This slice does not make publication O(1) or localized preparation O(change).
The 1,000-paragraph exact repeat still performs 656,166 allocations and retains
about 86.8 MB of newly copied output. Flat scene materialization remains the
dominant cost and is the next Design-0017 slice. Treating the preflight result
as campaign completion would be a mirage.

## Slice 3: persistent scene publication

Committed and transient scenes now retain paragraph-local immutable geometry
behind a balanced persistent scene spine. Each spine node summarizes its
subtree's paragraph count, block extent, record counts, bounds, and baselines.
Changing one paragraph replaces only that leaf and its logarithmic ancestor
path. Unchanged paragraph segments remain shared, including when an earlier
paragraph changes height.

Scene revision and paragraph origin are no longer copied into every retained
record. `SceneLineView`, `SceneFragmentView`, `SceneGlyphView`, and semantic
views bind the current scene revision and prefix origin while traversing.
`TextScene` and `CompositionScene` therefore remain immutable cheap handles,
and caller-retained old scenes continue to mint their own exact source
revision.

The approved public migration is complete across the repository:

- flat `SceneLine`, `SceneFragment`, `SceneGlyph`, and `SemanticFragment`
  records are gone;
- `lines`, `fragments`, `glyphs`, `sources`, and `semantics` expose
  allocation-free iterator/view types;
- indexed consumers use `scene.line(index)`, `scene.fragment(index)`, or the
  corresponding view `get`;
- the showcase, headless and visual proofs, IME experiment, label wind tunnel,
  Parley adapter tests, and PDF exporter all consume the same retained views;
- there is no flat-record compatibility path on the renderer hot path.

Glyph-instance identity follows the retained geometry allocation plus the
paragraph-local shaped-glyph ordinal. Split-paint observations still compare
equal; replacing a paragraph produces distinct identities; structurally
shared downstream geometry retains its identities even when changed prefix
height or fragment counts alter its scene-space position.

### Matched public-path evidence

The allocation observations use the release allocation-counting feature around
the event itself. Timings are the median of three fresh release processes on
the same host.

| Event | Paragraphs | Calls | Requested bytes | Median |
|---|---:|---:|---:|---:|
| retained exact repeat | 64 | 0 | 0 | 458 ns |
| retained exact repeat | 1,000 | 0 | 0 | 541 ns |
| one-byte localized prepare | 64 | 651 | 168,883 | 82,583 ns |
| one-byte localized prepare | 1,000 | 812 | 229,835 | 292,542 ns |

Against the original baseline, persistent publication removes 42,596
allocation calls from the 64-paragraph localized prepare and 665,995 calls
from the 1,000-paragraph prepare. Exact-repeat publication falls from 42,607
and 666,167 calls respectively to zero. The remaining 161-call localized
scale delta is not record publication: the current preparation loop still
visits every paragraph to validate provenance, update recency, and flatten
region transcript evidence.

The existing label allocation matrix independently observes zero calls and
zero requested bytes for:

- a retained identical label;
- retained width adjustment served by the exact root;
- paint-table value rebinding over the same geometry root.

### Structural correctness proof

Focused tests establish that:

- paint-table rebinding retains the exact `SceneCore`;
- a localized committed edit shares unchanged sibling segments and replaces
  only the changed segment;
- changed prefix height updates downstream scene-space positions through spine
  summaries without rewriting downstream geometry;
- old and new scenes lazily mint their own source revisions over shared
  records;
- glyph identity follows shared or replaced geometry rather than global
  fragment ordinals;
- exact composition epochs retain one root, updated composition replaces only
  its paragraph path, and committed siblings remain shared;
- mixed bidi movement, hit testing, selections, semantics, renderer traversal,
  and PDF traversal continue through the public view API.

### What remains

This slice earned O(1) exact repeats and eliminated document-scale scene
publication. Its then-remaining document clone and all-paragraph localized
scan were subsequently resolved in
`retained-document-cow-2026-07-26.md` and
`retained-localized-preparation-2026-07-26.md`. The historical numbers above
remain the before-observation for those independent Design-0017 slices.

## Completion-audit corrections — 2026-07-26

An adversarial pass found two post-migration diagnostics and lifecycle
contracts that the initial persistent-scene proof had not earned:

- `PreparationMemory::scene_output_capacity_bytes` still described flat
  newly owned vectors, reported the complete binary spine for a localized
  publication, and reported zero for a newly retained composition path. It
  now charges only scene-spine node payload newly retained relative to the
  reusable prior spine. Exact roots charge zero, a localized or composition
  update charges its unshared root path, and cold publication charges the
  complete spine. Paragraph geometry remains in scene-cache accounting.
- `prepare_composition` promised not to evict committed work while one
  combined LRU allowed exactly that at a full budget. `CacheBudget` now
  enforces committed and transient-composition entry limits independently.
  The transient limit defaults to the committed limit and can be set to zero;
  zero retains the committed exact root while making repeated composition
  preparation observably cold.

Focused traps prove exact and localized spine charges, first and repeated
composition charges, simultaneous one-entry committed and composition
residency, committed-root identity after composition, and the zero-transient
degradation rule. The public budget-semantic migration is recorded in
Design-0017 rather than hidden behind the old total-entry interpretation.

The post-correction allocator rerun is identical to the localized proof:
exact repeat remains 0 calls / 0 bytes at both 64 and 1,000 paragraphs, while
localized preparation remains 612 calls / 158,771 bytes and 615 calls /
159,275 bytes respectively. Splitting the recency structures therefore fixes
the transient lifetime law without adding normal-flow event allocation.
