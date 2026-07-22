# Exact text-interaction adversarial review — 2026-07-22

## Summary judgment

The first interaction slice is executable and honest. `underwood_parley`
lowers Parley's shaped clusters into a portable visual stream, while
`underwood` projects their source boundaries into revision-bound semantic
positions. Hit testing no longer treats glyph ink as editable geometry, and
caret lookup no longer copies the pointer coordinate. The public seam covers
ligature components, combining source, whitespace, RTL, soft wraps, explicit
breaks, semantic-leaf boundaries, and empty editable text without adding a
dependency or `unsafe`.

This is exact under Parley's current cluster-advance and cursor-side contract.
It does not claim GDEF ligature-component caret conformance, arbitrary
cross-leaf replacement, selection geometry, or durable anchors.

Good catch: an early closest-hit implementation minimized two-dimensional
distance across every cluster. A far-right drag beside a short first line
could therefore jump to a much wider later line. It now selects the nearest
visual line on the block axis before clamping on the inline axis.

## Must fix

All Must findings are resolved.

- **Ink is not interaction geometry.** A separate cluster stream carries full
  advances even when source has no independently painted glyph. The ligature,
  combining-source, whitespace, and control cases exercise that separation.
- **Caret geometry cannot come from query x.** Hits choose a prepared visual
  side; caret lookup resolves that revision-bound position against stored
  stops. Two points in one cluster return identical caret bounds.
- **Visual line ownership precedes horizontal clamping.** Closest hit first
  minimizes block-axis distance, then inline distance. A regression with a
  short first line and long second line prevents cross-line jumps.
- **Stale positions fail closed.** Caret lookup includes revision, text-leaf
  identity, byte boundary, and affinity. A position from an older snapshot
  returns `None` after publication.

## Should

- Keep Parley cluster encoding and bidi-side derivation inside
  `underwood_parley`; `underwood` should consume only portable prepared facts.
- Preserve explicit affinity at soft wraps and semantic leaf boundaries.
  Collapsing positions to a byte offset would lose both visual caret identity
  and deterministic leaf ownership.
- Keep empty semantic text distinct from empty structure. An empty text leaf
  exposes one downstream caret; a paragraph with no text leaf exposes none.
- Upstream the reusable cluster/cursor mechanics to Parley Core when its
  public shape is agreed, then retire the corresponding adapter knowledge.

## Could

- Index scene clusters and caret stops if a product workload demonstrates that
  linear lookup is material. The present implementation favors a small,
  inspectable contract until selection supplies a representative benchmark.
- Add external platform differential evidence once the host-driven IME surface
  can query these same positions and rectangles.

## Suggested tests

The focused suite now includes:

- independently hittable source components inside an OpenType ligature;
- combining source and whitespace with no dependence on glyph ink;
- descending logical source in visually traversed Arabic RTL clusters;
- upstream and downstream carets for one soft-wrap byte boundary;
- before-control and after-control carets on distinct explicit-break lines;
- affinity-based ownership at adjacent semantic text leaves;
- an exact caret for an empty text leaf and no hit for leafless structure;
- stale-revision rejection and nearest-line-before-inline clamping.

The next selection slice should add mixed-bidi visual movement round trips,
selection rectangles split at line and bidi boundaries, and zero-work
selection-only updates.
