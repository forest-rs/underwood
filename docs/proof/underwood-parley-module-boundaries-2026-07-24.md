<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `underwood_parley` module-boundary review — 2026-07-24

## Judgment

The adapter is behaviorally unchanged and its implementation structure now
matches its architectural ownership. The crate root fell from 4,569 lines to
32 lines containing crate policy, private module declarations, public
re-exports, and the test-module declaration. No public path, dependency,
feature, work-report field, cache rule, or text algorithm changed.

The largest production module is now `interaction.rs` at 611 lines. It owns one
cohesive invariant—source-complete grapheme interaction and cursor movement—
rather than unrelated adapter work. Splitting it further without a second
owner would optimize for line count rather than architecture.

## Ownership

| Module | Owns | Explicitly does not own |
| --- | --- | --- |
| `engine` | paragraph identities, invalidation, retained preparation | shaping, line-breaking, lowering, interaction algorithms |
| `font` | immutable Fontique catalog construction and validation | shaping-time selection policy |
| `shaping` | analysis, itemization, font selection, initial shaping | line formation and portable records |
| `line_break` | constraints, legal breaks, line metrics, line-local bidi | font selection, shaping internals, scene construction |
| `lowering` | portable glyph, source, synthesis, and paint records | shaping and renderer policy |
| `interaction` | grapheme units, visual slices, cursor transitions | editing and line selection |
| `validation` | fail-closed adapter input coverage | text preparation and error presentation |

The test corpus is likewise divided into font/analysis, line breaking, editing,
interaction, paint, and intrinsic/cache ownership, with shared fixtures in the
parent test module.

## Adversarial review

### Must

None.

### Should

Two findings were resolved before publication:

1. Test-only aliases initially leaked into the crate root. Focused tests now
   import their owning private modules directly.
2. Shaping initially accessed `FontSet` fields with crate-wide visibility.
   `FontSet` now keeps its fields private and exposes one internal paired
   resource borrow.

### Could

Further split `interaction` only when a distinct owner—such as host protocol
mapping versus portable cursor facts—actually emerges. The current single
invariant is preferable to arbitrary fragmentation.

## Executable evidence

- all 43 `underwood_parley` unit tests pass in their new module locations;
- the full workspace test suite and doctests pass;
- the committed CPU visual snapshot and PDF proof remain exact;
- workspace Clippy passes for every target and feature with warnings denied;
- workspace rustdoc passes with warnings denied;
- Rust 1.88 checks `underwood` and `underwood_parley`;
- bare-metal and WebAssembly `no_std` targets pass;
- repository policy, formatting, spelling, Beads lint, and dependency-cycle
  checks pass;
- semantic-scene and 2,048-label release wind tunnels execute successfully.

## Unsafe and dependency watch

No `unsafe`, production dependency, feature, or version change was introduced.
The temporary Parley fork remains intentionally unchanged in this structural
checkpoint; replacing it is the separately gated `und-oh0.5.5.2` slice.
