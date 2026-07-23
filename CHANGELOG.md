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
- Replaced advance-sized, character-proportional, and mandatory outline-derived
  glyph clips with explicit source-to-paint ownership. Ordinary whole glyphs
  now render without a clip, so missing outline metrics and synthetic
  emboldening no longer fail preparation. Adapter callers that put a paint
  boundary inside one shaped glyph must still handle
  `PreparationErrorKind::UnsupportedPaintCoverage` until they can provide
  exact component geometry. `SceneFragment::clip` is replaced by optional
  `SceneFragment::paint_clip`; renderer adapters must draw directly for `None`
  and clip only explicit partial-paint segments. Paragraph adapters replace
  `GlyphPaintSegment::new` and `local_clip` with
  `GlyphPaintCoverage::whole` for ordinary paint or
  `GlyphPaintSegment::clipped` and optional `clip` for a validated split.
  Explicit clips are post-synthesis glyph-local geometry; adapters account for
  skew/emboldening and renderers translate the rectangle without applying
  synthesis to it again.
- Added `FontSynthesis::skew_transform` as the canonical `no_std` affine used
  by coverage adapters and renderers. Existing callers may replace local
  degree-to-shear math with this method; `skew_degrees` remains available.
- Added an opt-in `underwood_parley/system-fonts` feature and
  `FontSet::with_system_fonts` for native hosts that need one fixed platform
  catalog snapshot. The default adapter remains caller-font-only and
  deterministic. Linux loads Fontconfig dynamically so enabling the feature
  does not require development headers at build time.
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
- Added `SceneGlyph::sources` and `SceneFragment::sources` for source-complete
  provenance when one shaped glyph crosses semantic text leaves. Existing
  `source` accessors remain as first-slice conveniences; callers performing
  mapping or auditing must migrate to `sources`.
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
- Added `CompositionSession`, checked `CompositionEpoch` updates, explicit
  generated-text provenance, and `LayoutEngine::prepare_composition`. Preedit
  no longer masquerades as committed document edits; cancel retains committed
  paragraph formation and commit publishes one selection replacement.
- Added `EditableSurface` and `EditableSurfaceSnapshot` for a caller-chosen
  semantic focus scope. Native adapters can bind text, the complete
  multi-selection set, marked range, source map, exact scene geometry, and one
  document/composition revision, then perform UTF-8, UTF-16, or Unicode-scalar
  range conversion and synchronous text/geometry/hit queries.
  `EditableSurfaceSnapshot::replacement_selection` maps a host-authored range
  back to a validated logical scene selection without exposing raw snapshot
  position construction.
- Generalized `SceneLine`, `SceneFragment`, `SceneGlyph`, `TextHit`, and
  `SceneCaret` over their source or position type so composition scenes can
  preserve authored and generated provenance. Existing committed-scene type
  annotations can keep using their default snapshot-source parameters.
- Starting composition with several independent selections or one multi-range
  visual bidi selection now performs an explicit, reported collapse to the
  primary extent. Committed scenes and surfaces retain the complete two-level
  selection model; callers must not treat this native marked-region policy as
  a general scene limitation.

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
- Deterministic event-feed and host-driven IME compatibility trace in the
  separate `experiments/ime-compat` workspace crate.
