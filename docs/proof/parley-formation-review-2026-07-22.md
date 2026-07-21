# Parley paragraph-formation review — 2026-07-22

## Disposition

The safe-break checkpoint in commit `023c777` is real, product-path work and is
fit for a draft pull request. It is not the completion of `und-oh0.2.2` and must
not land as though break-sensitive shaping were solved. The pinned Parley Core
revision `6c81e1dd9b67793cdd959c65cc650c96a1262fb7` still lacks bounded
break/concat reshaping, so the bead and upstream gate remain open.

No `unsafe` code or production dependency was added. The public API changes are
covered by the approved Design-0006 migration note.

## Summary judgment

**Real:** `LayoutEngine` now passes finite width and inline-flow runs through
`ParagraphFormation`; `underwood_parley` chooses legal and mandatory boundaries
from retained Parley facts, computes font-derived line boxes, reorders each line
visually, and returns portable formed lines. Scene construction consumes those
lines directly. The old glyph-edge wrapper and 80/20 baseline split are gone.

**Mirage if overclaimed:** committed boundaries still reuse unbroken shaping.
That is correct only for safe boundaries. `FormationWork::break_reshapes` and
`WorkReport::break_reshapes` therefore remain zero on the production adapter;
no test, document, or visual label calls that missing work complete.

## Must findings resolved

1. **Incomplete line source:** the initial `SceneLine` lowering kept only the
   first semantic leaf intersecting a line. `SceneLine::sources` now retains all
   ordered snapshot-local slices, and a mixed-size two-leaf product test proves
   complete coverage.
2. **Phantom format glyphs:** bidi isolate controls initially survived Parley
   lowering as zero-advance glyph fragments. `PreparedRun` now distinguishes
   explicit unrendered source, the adapter derives it from Parley's
   `contributes_to_shaping` analysis, and scene validation requires every scalar
   to be covered by a real glyph or an intentional omission.
3. **Weak portable coverage:** visual runs could sit within a line without
   proving complete source coverage. `PreparedLine::try_new` now validates a
   temporary source-sorted view while preserving retained visual order.
4. **Discarded work evidence:** bounded break-reshape observations would have
   disappeared when adapter work became a scene report. `WorkReport` now
   exposes the exact count and a boundary test proves propagation.
5. **Visual-only wrapping claim:** the previous mixed-direction specimen was a
   single line. The poster now asserts and draws two finite-width legal lines,
   their break reasons, real baselines, mixed LTR/RTL ordering, and fallback
   fonts through the public scene API.

## Open Must gate

Parley PR #634's bounded `apply_break` / `apply_concat` capability, or an
equivalent reviewed Core seam on main, must execute an Arabic joining or Latin
ligature boundary and restore the unbroken result after concat. Until then:

- `und-oh0.2.2` stays open;
- Design-0006 retains status “upstream reshape gate open”;
- the pull request stays draft and is not landed;
- the safe-break implementation may be reviewed and measured, but not promoted
  as complete paragraph formation.

## Product evidence

The focused corpus executes through `Document`, `LayoutEngine`,
`ParleyParagraphEngine`, and `TextScene`:

- legal soft wrapping of `alpha beta gamma`, including explicit break reasons;
- CR, LF, CRLF, U+2028, and U+2029 hard breaks with complete source;
- NBSP and long-word overflow without an invented split;
- font-derived baseline, ascent, descent, and mixed-size line height;
- complete line source across two semantic leaves;
- mixed LTR/RTL visual glyph order;
- bidi isolate controls without phantom glyph fragments;
- width-only and line-height-only formation with zero analysis, selection, or
  shaping work;
- exact visual snapshot SHA-256
  `547f0f6eb8d6ad43454818c0917ee09638ef70b3ac4323b07b2945345940dd45`.

The experiment-only high-level Parley oracle covers the applicable policy and
metric cases. Its current overflowing-NBSP divergence remains recorded rather
than copied into production.

## Should / tracked boundaries

- Ligature paint clips still use the temporary coverage policy owned by
  `und-oh0.2.4`; this checkpoint does not claim caret-accurate component paint
  geometry.
- Fontique selector convergence remains owned by `und-oh0.2.6`.
- Tabs, hyphenation, justification, inline objects, regions, and pagination are
  outside Design-0006's first constraint record and are not implied by the
  formed-line vocabulary.

## Unsafe watch

No `unsafe` was introduced. No backend pointer, borrow, or Parley-specific type
crosses the portable adapter boundary.

## Local validation

The complete local Definition of Done passed after the production commit and
evidence updates:

- `cargo fmt --all --check` and `taplo fmt --check --diff`;
- copyright headers and `typos`;
- workspace Clippy with all targets/features and `-D warnings`;
- workspace tests with all features, including the exact CPU snapshot;
- workspace rustdoc with `RUSTDOCFLAGS="-D warnings"`;
- Rust 1.92 workspace MSRV check;
- `underwood` and `underwood_parley` on `x86_64-unknown-none` and
  `wasm32-unknown-unknown` with exact Rust 1.96.0;
- `cargo xtask check`, `bd lint --status all`, and `bd dep cycles`.

Remote three-OS and exact-pixel validation remains pending for the draft pull
request. Passing it will validate this checkpoint but will not satisfy the
upstream break-reshape gate.
