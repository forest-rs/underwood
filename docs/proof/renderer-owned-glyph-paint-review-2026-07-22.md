# Renderer-owned glyph paint review — 2026-07-22

- **Design:** Design-0010
- **Bead:** `und-oh0.2.9`
- **Parley pin:**
  [`44d155e17a6dbf455c8b9133c2ae40955c9f2af2`](https://github.com/waywardmonkeys/parley/commit/44d155e17a6dbf455c8b9133c2ae40955c9f2af2)
- **Snapshot:** 1600 × 1000 RGBA8, PNG SHA-256
  `56b846d143c4ef6feb1dfd5003a460ddd20b1245ffeebee63f13b7faed476f24`
- **Unsafe watch:** no `unsafe` added
- **Dependency watch:** no new direct dependency; the showcase opts into
  Fontique's existing system-font feature

## Result

Ordinary glyph rendering no longer depends on outline-derived rectangles.
Underwood retains complete source-to-paint ownership and lowers a whole glyph
without a clip. Explicit source-complete clipped segments remain validated for
a future conformant multi-paint operation. Actual painted extent belongs to the
renderer.

This removes a product correctness failure rather than merely a redundant
operation. Committing Chinese text through the native macOS IME selected a real
system Han font, but the former adapter rejected that valid selection as
`UnsupportedPaintCoverage` because ordinary rendering required an
outline-derived clip. The same commit now prepares successfully. A separate
deterministic static-font regression proves that synthetic emboldening also no
longer depends on an exact expanded outline rectangle.

## Executable evidence

| Case | Observation |
| --- | --- |
| ordinary coverage | exactly one unclipped source-complete segment |
| explicit split | one physical glyph identity lowers through two distinct clipped paint fragments |
| invalid split | mixed clipped/unclipped, gapped, incomplete, and lone partial coverage fail |
| Arabic mark | the real zero-advance dot glyph remains present and renders without a glyph clip |
| synthetic embolden | static Noto Kufi requested bold prepares with `embolden == true` |
| native Han | `中文输入` commits once, resolves a non-bundled `Hani` font, and prepares |
| mixed script | Latin inserted at an interior Arabic caret resolves the explicit Latin fallback |
| cross-leaf glyph | a base and combining mark in distinct semantic leaves prepare with both source slices retained |
| cross-paint ligature | a paint boundary inside shaped `ffi` remains `UnsupportedPaintCoverage` |
| interaction | hit, caret, selection, and IME tests use cluster or semantic geometry, never glyph paint extent |
| renderer adapter | `paint_clip == None` draws directly; only explicit segment clips install a clip |

`SceneError` now includes its underlying preparation category in `Display`, so
a future host failure reports, for example,
`scene preparation failed: Preparation (MissingFont)` instead of hiding the
actionable cause.

## Visual review

The release-mode CPU poster was regenerated and inspected. The Arabic dots
remain visible. The old purple outline boxes and `j`-overhang claim are removed;
the diagnostic now marks the zero-advance origin and advance without presenting
outline data as universal ink truth. The exact snapshot test passes against the
new SHA above.

## Parley boundary

Underwood no longer calls the experimental `RunGlyphMetrics`,
`with_glyph_metrics`, or glyph-ink query. The public fork branch
`bounded-break-reshape` was rebuilt on Parley main `38809fb` and force-updated
to clean commit `44d155e`, which contains bounded break/concat reshaping and no
ink-metrics patch. Every Underwood workspace member now uses that one pin.

This leaves a cleaner upstream proposal: Tom can review the reusable bounded
reshaping work independently of a rendering policy Underwood no longer wants.

## Limits kept explicit

- `imaging` records synthetic emboldening, but the current
  `imaging_vello_cpu` backend does not yet execute it. Preparation is correct;
  synthetic-bold pixel fidelity is not claimed.
- A glyph crossing paint runs remains unsupported until `und-oh0.2.8` supplies
  a conformant component rule and geometry.
- Underwood exposes no universal ink bounds. Future damage and culling must be
  conservative unless a renderer supplies backend-complete painted extent.
- System-font pixels, filenames, and glyph IDs are platform-dependent and are
  not deterministic snapshot evidence.

## Local validation

Formatting, all-target/all-feature Clippy with warnings denied, and the full
workspace all-feature test suite pass. The suite includes 35 Underwood tests,
37 Parley-adapter tests, 21 native showcase tests, the headless public path,
and the exact CPU snapshot. The clean Parley commit independently passes its
20 Core tests plus doctest, all-target/all-feature Clippy, and
`no-default-features + libm` checking. Repository policy, rustdoc, MSRV, and
portability also pass locally. Remote CI remains the landing gate.
