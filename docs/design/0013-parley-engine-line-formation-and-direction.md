# Design 0013: Parley Engine line formation and paragraph direction

**Status:** Accepted and implemented on 2026-07-24

**Bead:** `und-oh0.5.5.2`

## Decision

Underwood consumes Parley Engine, Fontique, and Parlance from exact revision
`9c41a4d0b9aa1aae7b8fdad8cf31728c9c3476bb`. The temporary bounded-break
fork is removed.

Underwood owns a private line former over public Parley Engine facts:

1. retain one canonical paragraph `Analysis` and `ShapedText`;
2. choose legal and mandatory boundaries from canonical clusters;
3. re-itemize and shape each committed line source range through
   `Analysis::itemize` and `Shaper::shape_item`;
4. retry the preceding legal boundary when the line-final advance does not fit;
5. lower only accepted line results into portable Underwood records.

A single unwrapped line reuses the canonical shaped result. Changing only line
height recomputes metrics over retained accepted line results. Wrapped lines
are conservatively shaped because current public Parley Engine does not expose
unsafe-to-break regions.

## Fence

Parley Engine owns Unicode analysis, bidi resolution, itemization, font-backed
shaping, and shaped records. Underwood owns paragraph style, intrinsic and
constrained line policy, fit-changing backtracking, retained invalidation,
portable lowering, and exact work reporting.

The private former may call public Parley Engine APIs. It may not copy HarfRust
algorithms, depend directly on HarfRust, or expose a Parley Engine type through
Underwood's public scene contract.

## Paragraph direction

Base direction affects paragraph analysis, not inline font shaping.
`ParagraphStyle` therefore owns `BaseDirection` separately from
`ShapingStyle`. `StyleMap` provides an automatic-direction default and
per-paragraph overrides. `BlockRequest` accepts the same value for retained
single-paragraph text.

Changing paragraph direction invalidates analysis, itemization, shaping, line
formation, and geometry for that paragraph. Font and paint invalidation laws
remain unchanged.

The executable corpus covers:

- explicit RTL numeric and neutral text;
- explicit RTL empty text;
- explicit LTR overriding Arabic first-strong inference;
- product-path cache invalidation and bidi levels;
- mixed-direction line lowering and interaction.

## Work accounting

Canonical paragraph shaping and line-final shaping are separate observable
stages:

- `WorkReport::shape` and `font_selection` describe canonical shaping;
- `WorkReport::line_shape` and `line_font_resolution` describe committed and
  rejected line candidates. Line formation reuses the canonical font choice
  rather than querying Fontique again;
- `WorkReport::line_reshapes` counts every line-final shaping attempt.

Rejected fit-changing candidates remain visible work. A line-height-only
change reports flow and geometry work without line shaping.

## Public migration

- `BaseDirection` is re-exported by `underwood`.
- Construct paragraph values with
  `ParagraphStyle::new(BaseDirection::{Auto,Ltr,Rtl})`.
- Assign document overrides with
  `StyleMap::set_paragraph_style(paragraph, style)` or change the default with
  `StyleMap::with_default_paragraph_style(style)`.
- Add paragraph values to a retained block with
  `BlockRequest::with_paragraph_style(style)`.
- Replace `WorkReport::break_reshapes()` with
  `WorkReport::line_reshapes()`. The new count is shaping attempts, not only
  boundaries that upstream marked unsafe.
- Backends constructing `FormationWork` must additionally supply a
  `LineShapingWork` record containing attempts, clusters resolved to retained
  fonts, shaped runs, and shaped glyphs. Use `LineShapingWork::default()` for a
  backend which performs no separate line shaping.
- Internal and experiment-only dependency names change from `parley_core` to
  `parley_engine`.

No compatibility shim preserves the old work-counter meaning: treating the new
public-only line former as if it still performed bounded unsafe-region mutation
would hide real line shaping and font-resolution cost.

## Follow-up boundary

If Parley Engine later exposes stable break-safety evidence or a reusable
line-formation primitive, Underwood may selectively avoid safe line shaping.
That change requires the same Arabic join, ligature, fit-changing, bidi,
interaction, and wind-tunnel evidence. It does not justify retaining the old
fork in the meantime.
