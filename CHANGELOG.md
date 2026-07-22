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
