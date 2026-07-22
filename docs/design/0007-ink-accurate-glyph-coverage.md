# Design-0007: Ink-accurate glyph coverage

- **Status:** Implemented against an upstream candidate
- **Date:** 2026-07-22
- **Beads:** `und-oh0.2.4`, `und-oh0.2.7`, `und-oh0.2.8`
- **Parley candidate:**
  [`d12c801d8fd298ff095f1ec903b6adaa732fcef2`](https://github.com/waywardmonkeys/parley/commit/d12c801d8fd298ff095f1ec903b6adaa732fcef2)

## Decision

A glyph's advance is layout distance, not paint geometry. `underwood_parley`
must derive portable glyph clips from variation-aware font outline bounds. It
must return `UnsupportedPaintCoverage` when exact coverage is unavailable,
rather than manufacture a rectangle from the advance or divide it by source
character count.

The reusable font mechanic belongs in Parley Core. Underwood owns the mapping
from shaped glyph/source results and Underwood paint runs into portable scene
fragments. Renderers consume those fragments and do not reconstruct text
coverage.

## The defect that forced the boundary

Noto Kufi Arabic shapes `ب` as a dotless base plus a visible dot glyph whose
advance is zero. The old adapter used `abs(advance)` as clip width, producing an
empty clip and erasing the dot in the CPU poster. The same model also cropped
Latin overhangs and gave a character-proportional split of an `ffi` ligature an
authority the font never supplied.

This is one general contract failure, not an Arabic exception.

## Ownership

```text
Parley Core
  shaped run + exact font + size + normalized coordinates
  -> callback-scoped glyph ink metrics

underwood_parley
  glyph/source + paint ownership + synthesis
  -> portable glyph paint coverage or a stable explicit error

Underwood / renderer adapter
  retained scene fragment + clip
  -> backend glyph draw under that clip
```

Parley candidate `d12c801` adds `GlyphInkBounds`, `RunGlyphMetrics`,
`ShapedText::with_glyph_metrics`, and the one-off
`ShapedText::glyph_ink_bounds` query. The run-scoped query parses the font and
converts normalized coordinates once for all inspected glyphs. It remains
usable without adopting high-level `parley::Layout` or Underwood's document
model and adds no dependency edge to Parley Core. Underwood promotes the
already-transitive `libm` package to a direct dependency so the canonical
synthetic-skew transform remains available when Cargo feature unification
enables Kurbo's `std` implementation elsewhere in the workspace; this adds no
package to the dependency graph.

This is the intended upstream-sharing pattern: expose a small piece of text
physics at the lowest honest layer, then keep Underwood's stronger document,
region-flow, invalidation, work-accounting, semantic, and portable-scene
contracts above it.

## Coverage contract

For each shaped glyph:

1. Query outline bounds from the run's exact font, face index, font size, and
   normalized variation coordinates.
2. Preserve empty bounds for valid no-outline glyphs such as spaces.
3. Apply the same synthetic-skew bounding transform used by the renderer.
4. Accept the glyph only when one paint run owns its complete shaped source.
5. Emit the real ink rectangle as that glyph's portable coverage.
6. Return `UnsupportedPaintCoverage` for a glyph crossing paint runs, an
   unavailable glyph metric, or synthetic emboldening without a trustworthy
   expansion magnitude.

Line advance remains independent from glyph ink. A zero-advance glyph can have
non-empty paint coverage, and a non-zero-advance glyph can paint outside its
advance interval.

## Multi-paint ligatures

The prior `ffi` implementation divided a glyph's advance by source character
count. That approximation is deleted. An audit found that the bundled Roboto
Flex fixture and Roboto Regular have no GDEF `LigCaretList`; consequently they
cannot justify the old interior boundaries even as font-supplied caret
positions. A caret position would still need a stated conformance rule before
being treated as paint-component ownership.

`und-oh0.2.8` is the removal gate for the explicit error. It requires a
normative or cross-engine rule covering fonts with and without caret data,
overlapping outlines, variable instances, and RTL ligatures before any
multi-paint case is enabled.

## Executable evidence

| Case | Required observation |
| --- | --- |
| Noto Kufi `ب` | a zero-advance dot glyph has non-empty coverage |
| Roboto Flex `j` | ink extends outside the shaped advance |
| Roboto Flex `office`, `liga=1` | four glyphs; the `ffi` glyph owns bytes `1..4` |
| Roboto Flex `office`, `liga=0` | six glyphs |
| paint boundary inside `ffi` | stable `UnsupportedPaintCoverage` |
| mixed Latin/Arabic paragraph | logical source and visual order remain correct |
| CPU poster | exact gold dot pixels occur inside the zero-advance ink clip |
| `parley_core`, no default features + `libm` | glyph metrics compile without `std` |

The poster's diagnostic boxes now show the real Arabic mark bounds and the
Latin `j` overhang. Historical reviews that described proportional split-
ligature clips remain records of the earlier slice; this design supersedes that
claim.

## À-la-carte Parley direction

Underwood should continue extracting mechanisms in this form, not import
private `LayoutData` wholesale:

1. owned reusable shaping (`ShapedText`);
2. bounded break/concat reshaping;
3. run-scoped glyph ink metrics;
4. small line-formation mechanisms such as greedy break state, mandatory-break
   handling, line-local bidi ordering, and metric aggregation;
5. later, proven primitives for tabs, justification, hyphenation, and inline
   objects.

Each extraction must be independently useful to Parley Core callers. Underwood
continues to own document transactions, arbitrary region/page flow, retained
cache identity, invalidation, semantic mapping, diagnostics, and renderer-
neutral scenes.

## Migration

Downstream adapters must stop assuming that every supported glyph clip is the
font-size-high advance rectangle. Callers that place a paint boundary inside a
single shaped glyph must now handle `UnsupportedPaintCoverage`. Callers that
need synthetic emboldening must wait for exact expanded bounds or choose a font
instance whose requested weight is directly available.
