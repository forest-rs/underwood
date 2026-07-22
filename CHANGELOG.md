# Changelog

All notable changes to Underwood will be documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Underwood does not yet make compatibility promises.

## Unreleased

### Draft API

- Added the first review-gated semantic-to-scene path through `Document`,
  `LayoutEngine`, `TextScene`, and `underwood_parley`.
- IDs are document-scoped and not serialized; scene source ranges are valid
  only for their named immutable snapshot.
- This pre-stable API intentionally replaces no prior public product API.
- Replaced `adapter::ParagraphPreparation::prepare` with
  `adapter::ParagraphFormation::form`. Formation now receives validated inline
  constraints and inline-flow runs and returns source-complete `PreparedLine`
  records with font-derived metrics. Adapter implementations must move their
  whole-paragraph `PreparedRun`s into formed lines and report formation work;
  `LayoutEngine` callers are unchanged. `SceneLine::source` is replaced by
  `SceneLine::sources` so lines crossing semantic leaves retain complete source
  instead of exposing only their first slice. `PreparedRun::try_new` now also
  receives explicit unrendered source ranges for controls and format characters;
  adapters must account for every scalar without manufacturing phantom glyphs.
- Replaced advance-sized and character-proportional glyph paint clips with
  variation-aware font outline bounds from Parley Core. Zero-advance marks and
  glyph overhangs now retain their real ink coverage. Adapter callers that put
  a paint boundary inside one shaped glyph, or request synthetic emboldening
  without exact expanded bounds, must handle
  `PreparationErrorKind::UnsupportedPaintCoverage`; Underwood no longer emits
  approximate component clips for those cases.
- Added `FontSynthesis::skew_transform` as the canonical `no_std` affine used
  by coverage adapters and renderers. Existing callers may replace local
  degree-to-shear math with this method; `skew_degrees` remains available.
- Added `ParagraphRole::HEADING_1` and `ParagraphRole::HEADING_2`. These roles
  preserve authored heading semantics in `TextScene`; callers still resolve
  their computed visual styles explicitly through `StyleMap`.
- Replaced fragment-ink hit testing and pointer-derived carets with exact
  Parley-backed cluster geometry. `PreparedLine::try_new` now receives visual
  `PreparedCluster` records whose sides carry explicit UTF-8 boundaries and
  `TextAffinity`; paragraph-adapter implementations must provide source-complete
  clusters in addition to visual runs. `TextHit::source` now names the exact
  cluster, while `TextHit::position` returns a revision-bound
  `SnapshotTextPosition` and `TextHit::semantic_id` returns its semantic leaf.
  Migrate `scene.caret(&hit)` to
  `scene.caret(hit.position()).expect("position belongs to scene")` and handle
  `None` for stale or foreign positions. Use `TextScene::hit_test_closest` for
  pointer selection that clamps through whitespace or empty editable text;
  `TextScene::hit_test` remains exact and returns `None` outside cluster
  geometry.
- Added revision-bound `SnapshotTextSelectionSet` support. Each
  `SnapshotTextSelection` is one insertion point and can expose several
  logically ordered ranges for visual bidi selection; independent carets are
  separate members of the set. `TextScene::selection`, `selection_set`,
  `move_selections`, and `selection_geometry` preserve this distinction.
  `Document::replace_selections` validates and publishes the complete set,
  inserts once per selection rather than once per range, and returns collapsed
  post-edit selections in input order.
- Replaced `PreparedParagraph::try_from_lines` with
  `PreparedParagraph::try_new`, which also receives complete
  `PreparedCursorMovement` records. Adapter implementations now provide exact
  caret placement plus visual/logical cursor steps and the source cluster
  crossed by each step. This keeps bidi, affinity, soft-wrap, and deletion
  mechanics behind the paragraph seam instead of reconstructing them in
  `TextScene`.
- `ChangeSet::paragraphs` is now always in document order. Transactions that
  apply source operations in reverse order no longer leak staging order into
  their public change summary.

### Added

- Deterministic CPU-rendered visual proof through `imaging` and
  `imaging_vello_cpu`, kept in a separate top-level example crate.
- Native resizable document showcase through `winit`, `softbuffer`, `imaging`,
  and `imaging_vello_cpu`, kept in a separate top-level host crate. Resize,
  local-edit, paint-only, variable-axis animation, and diagnostic controls all
  execute the public retained Underwood path.
- Executable repository constitution and governance workflow.
- Beads capability, decision, and proof planning graph.
- Machine-readable proof ledger.
- Tooling-only Cargo workspace and policy validator.
- Initial CI and review scaffolding.
- Dependency-free `no_std` `underwood` production crate boundary.
- Bare-metal and WebAssembly portability checks for foundational crates.
