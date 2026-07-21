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

### Added

- Executable repository constitution and governance workflow.
- Beads capability, decision, and proof planning graph.
- Machine-readable proof ledger.
- Tooling-only Cargo workspace and policy validator.
- Initial CI and review scaffolding.
- Dependency-free `no_std` `underwood` production crate boundary.
- Bare-metal and WebAssembly portability checks for foundational crates.
