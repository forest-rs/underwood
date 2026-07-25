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

## Historical exception

Current Rust, normative architecture, proof prose, examples, benchmarks, and
diagnostics contain no use of the retired metaphor. The older benchmark now
describes its before-state structurally rather than repeating former private
symbol spellings.

Append-only Beads history retains older discussion written before this
migration. Those records are historical audit evidence, not live API,
architecture, or project vocabulary.

## Validation

The blocking search for live Rust is:

```sh
test -z "$(rg -il 'phys''ics' --glob '*.rs' --glob '!target/**')"
```

Normative docs and proof prose are also reviewed with a repository-wide search
excluding append-only Beads history. Formatting, all-target/all-feature Clippy
with warnings denied, workspace tests, rustdoc, and repository policy protect
the renamed live code.
