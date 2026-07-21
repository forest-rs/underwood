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

### Added

- Deterministic CPU-rendered visual proof through `imaging` and
  `imaging_vello_cpu`, kept in a separate top-level example crate.
- Executable repository constitution and governance workflow.
- Beads capability, decision, and proof planning graph.
- Machine-readable proof ledger.
- Tooling-only Cargo workspace and policy validator.
- Initial CI and review scaffolding.
- Dependency-free `no_std` `underwood` production crate boundary.
- Bare-metal and WebAssembly portability checks for foundational crates.
