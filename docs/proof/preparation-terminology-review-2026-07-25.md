# Preparation terminology review — 2026-07-25

## Result

Underwood no longer uses one speculative-mechanics metaphor as a catch-all for
text work. Live Rust identifiers, public rustdoc, examples, benchmarks,
diagnostics, README files, current architecture fences, and current proof
claims use the stage or artifact they actually mean:

- Unicode analysis and itemization;
- font selection and shaping;
- prepared paragraph facts;
- line formation and accepted-line adjustment;
- scene geometry and materialization;
- identity-bound or shared preparation caches.

This is not a new umbrella vocabulary. It makes ownership, invalidation, work
counters, and cache boundaries more legible.

## Code migration

The Parley adapter's private retained record is now `PreparationCache`, and
local bindings name preparation rather than implying a different algorithmic
domain. The seam experiment reports a `shaping_digest`. Showcase work capture
uses `has_text_preparation_work`, and benchmark/test assertions name the exact
preparation work they exclude.

No public type or function changed. This is a private identifier, test,
diagnostic-label, and documentation migration.

## Historical exceptions

A case-insensitive repository search has only three remaining matches outside
Beads history. They are exact former private symbol spellings quoted in:

- `docs/design/0005-retained-parley-shaped-text.md`, which records the old
  representation that the design replaced;
- `docs/proof/retained-shaped-text-benchmark-2026-07-22.md`, which identifies
  the exact before-state types measured by that historical benchmark.

Rewriting those symbols as though they had different source names would
falsify the old design and measurement. They are neither live identifiers nor
current architectural vocabulary.

## Validation

The blocking search is:

```sh
test -z "$(rg -il 'phys''ics' --glob '*.rs' --glob '!target/**')"
```

Repository-wide matches must remain limited to the two historical evidence
files above. Formatting, all-target/all-feature Clippy with warnings denied,
workspace tests, rustdoc, and repository policy protect the renamed live code.
