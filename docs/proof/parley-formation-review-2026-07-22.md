# Parley paragraph-formation review — 2026-07-22

## Disposition

The safe-break checkpoint in commit `023c777` was real product-path work but
was not sufficient to close `und-oh0.2.2`. The final branch now pins the narrow
Parley candidate `181664b28144cb59671a7f1b736757c6ebe270f2`, commits unsafe line boundaries
through bounded Core reshaping, and preserves the original unbroken
`ShapedText` as reusable width-independent physics.

No `unsafe` code or new production dependency was added. The existing Parley
dependency moves temporarily to an exact public fork commit under ADR-0004's
upstream-and-removal lifecycle. The public API changes are covered by the
approved Design-0006 migration note.

## Summary judgment

**Real:** `LayoutEngine` now passes finite width and inline-flow runs through
`ParagraphFormation`; `underwood_parley` chooses legal and mandatory boundaries
from retained Parley facts, computes font-derived line boxes, reorders each line
visually, and returns portable formed lines. Scene construction consumes those
lines directly. The old glyph-edge wrapper and 80/20 baseline split are gone.

**Break-sensitive path:** each width formation starts from the retained
unbroken shape. Safe boundaries remain zero-work. At an unsafe boundary the
adapter calls Parley Core `apply_break`, recollects actual advances and glyphs,
and backs up to an earlier legal opportunity if the reshaped line no longer
fits. Only the committed broken result is lowered into portable lines.

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

## Resolved Must gate

The narrow Core candidate executes both required semantic traps: an Arabic
cursive break changes glyph output and a Latin `fi` break decomposes the
ligature; `apply_concat` restores exact shaped structure in both cases and
across a legal U+200B default-ignorable seam. Safe boundaries are proven
no-ops. The Underwood product corpus additionally commits a legal U+200B break
inside Arabic cursive context and proves that:

- the width-only request performs zero analysis and initial shaping;
- real glyph IDs and sources change after the committed break;
- no glyph source crosses the new line seam;
- `WorkReport::break_reshapes()` is exactly one.

The adapter policy corpus also chooses an unsafe boundary whose reshaped
advance no longer fits. It proves `apply_concat` restores the exact canonical
`ShapedText` before selection backs up to the earlier legal safe boundary.

Upstream adoption remains the explicit `und-oh0.2.7` dependency-lifecycle
follow-up, not a false product claim or a reason to leave PR #9 artificially
incomplete.

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
- a legal Arabic zero-width opportunity requiring one bounded reshape with
  changed glyph output and seam-local source coverage;
- reshape-induced overflow with exact concat rollback and legal backtracking;
- exact visual snapshot SHA-256
  `547f0f6eb8d6ad43454818c0917ee09638ef70b3ac4323b07b2945345940dd45`.

The hash proves deterministic CPU reproduction, not complete visual
conformance. An Arabic-reader audit found the dots below `ب` are absent because
the temporary paint-coverage policy gives Noto Kufi's zero-advance dot glyph a
zero-width clip. Source text, bidi order, and shaping are correct; the
renderer-neutral ink-coverage defect is explicit follow-up `und-oh0.2.4`.

The experiment-only high-level Parley oracle covers the applicable policy and
metric cases. Its current overflowing-NBSP divergence remains recorded rather
than copied into production.

## Should / tracked boundaries

- Ligature paint clips still use the temporary coverage policy owned by
  `und-oh0.2.4`; it is now known to clip zero-advance Arabic dots and this
  checkpoint does not claim ink-accurate component paint geometry.
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

GitHub Actions run `29894614476` passed all eight jobs on final implementation
head `d878dbf`. Linux, macOS, and Windows passed workspace Clippy and tests;
formatting/text policy, repository policy, MSRV, denied-warning rustdoc,
bare-metal, and WebAssembly jobs are green. This is the remote evidence for the
bounded reshape implementation. Earlier run `29870025982` validates only the
safe-break checkpoint and is not substituted for the final run.
