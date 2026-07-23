# Design-0010: Renderer-owned glyph paint extent

- **Status:** Implemented locally; remote review pending
- **Date:** 2026-07-22
- **Beads:** `und-oh0.2.9`, `und-oh0.10.2.4`
- **Supersedes:** Design-0007's mandatory outline-clip contract

## Decision

Underwood carries the ownership of shaped source by paint. It does not require
or publish outline-derived rectangles as complete glyph ink, and it does not
clip an ordinary source-complete glyph to such a rectangle.

One paint slot owning a glyph's complete source lowers to one unclipped paint
operation. A future conformant source split may lower one physical shaped glyph
to several explicitly clipped paint operations. The split must cover the
glyph's source exactly; Underwood never invents component geometry from
advances, character counts, or outline bounds.

The renderer owns the actual painted extent. Damage and culling remain
conservative unless a renderer has backend-complete knowledge. Hit testing,
carets, selections, and IME geometry remain based on shaped clusters and line
geometry rather than painted pixels.

## Evidence that changed the design

The outline-clip implementation repaired a real Arabic defect: a visible
zero-advance dot had previously received an empty advance-sized clip. It also
removed a false proportional split of an `ffi` ligature. Those results remain
valid evidence against advance-derived paint geometry.

It did not establish a universal ink contract. Outline lookup cannot describe
bitmap glyphs or every color-font paint graph. It also rejected synthetic
emboldening because the adapter could not calculate the expanded rectangle.
The native editor made the broader limitation a product failure: committing
Chinese text through the macOS IME selected a real system Han font, and scene
preparation returned `UnsupportedPaintCoverage` before the renderer saw the
glyph. A separate deterministic static-font path preserves the synthesis half
of the regression without claiming that the current Han selection always
requires emboldening.

The correction is to remove the unnecessary prerequisite, not to create a
larger outline API. Ordinary glyph rendering already delegates the glyph ID,
font instance, synthesis, and paint to the backend.

## Ownership

```text
Parley Core / Fontique
  shaping, glyph IDs, source clusters, font selection, synthesis
                    |
                    v
underwood_parley
  complete source-to-paint ownership
                    |
                    v
Underwood scene
  whole glyph: no clip
  conformant split: explicit source-complete segment clips
                    |
                    v
renderer
  actual outline, bitmap, color graph, synthesis, and painted extent
```

## Invariants

1. One `PreparedGlyph` remains one physical shaped glyph even when a future
   paint split produces several draw fragments.
2. Ordinary coverage names one paint owner and requires no clip.
3. Explicit clipped segments are contiguous, non-overlapping, and cover the
   glyph's complete source.
4. A coverage value cannot mix clipped and unclipped segments.
5. Explicit clips are post-synthesis glyph-local geometry. Adapters account
   for skew or emboldening before publication; scene lowering translates the
   clip once and renderers do not synthesize it again.
6. A glyph crossing paint runs remains `UnsupportedPaintCoverage` until
   `und-oh0.2.8` establishes conformant component geometry.
7. Missing outline data and synthetic emboldening are not preparation errors.
8. Layout advances and cluster rectangles are never relabeled as ink bounds.
9. Page, viewport, selection, and genuinely partial-paint clips remain valid;
   this decision removes only the mandatory per-glyph outline clip.

## Migration

`SceneFragment::clip` is replaced by `SceneFragment::paint_clip`, which returns
`None` for ordinary glyphs. Renderer adapters draw directly in that case and
install a clip only for `Some`.

`SceneGlyph::sources` and `SceneFragment::sources` expose every leaf-local
source slice when shaping crosses an authored semantic boundary. The singular
`source` accessors remain first-slice conveniences, not source-complete audit
APIs.

Tests and diagnostics that used glyph clips as pointer targets move to exact
scene hit, caret, cluster, or selection geometry. Tests that need to prove a
zero-advance mark survives assert the shaped glyph and final pixels rather than
requiring outline metadata from the scene.

The Parley ink-bounds candidate is no longer required by Underwood. The clean
upstream branch retains only the generally useful bounded-reshape work.

## Proof obligations

- a native Chinese IME commit resolves system Han fallback and prepares;
- a synthetic-emboldened glyph prepares and retains its synthesis request;
- ordinary Arabic, Latin, Han, and whitespace glyph fragments have no paint
  clip;
- explicit split coverage lowers one glyph identity into distinct clipped
  paint operations with exact source and paint ownership;
- mixed, overlapping, gapped, or incomplete coverage is rejected;
- the CPU proof retains the Arabic dots after ordinary clips are removed;
- full workspace formatting, lint, tests, portability, and remote CI pass.
