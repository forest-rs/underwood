# Underwood PDF

`underwood_pdf` is the replaceable Krilla adapter for prepared Underwood text
scenes. It consumes only the public renderer-neutral scene and immutable
snapshot contracts; shaping, fallback, bidi resolution, line formation, and
semantic ownership remain in Underwood.

The first executable slice exports one prepared scene as one PDF page. It
shares the scene's embedded font backing without copying it, and preserves face
indices, glyph identifiers, prepared positions, fragment transforms, 8-bit
solid sRGB paint, and explicit partial-paint clips.

This is a visual PDF lowering, not yet tagged PDF or PDF/UA. It deliberately
rejects non-default variable-font instances, synthetic font transforms, and
gradient or image brushes until those mappings can be made exact.

The adapter supplies real Unicode for every glyph mapping, including glyphs
whose source crosses semantic leaves. Compatible glyphs are emitted as runs so
Krilla can encode RTL reversal and multi-glyph clusters. A partial-painted
glyph contributes Unicode once; its other paint observations are outlines.
It does not claim viewer-independent logical-order extraction or visual
selection across mixed-direction runs.

Krilla performs the final glyph-program lowering. With Underwood's deliberately
small Krilla configuration, outline and COLR glyph programs are available.
SVG glyphs require a renderer callback that this slice does not configure, and
embedded bitmap glyph programs require Krilla's disabled `raster-images`
feature.

Krilla 0.8.2 requires Rust 1.92, so this renderer-host adapter has an explicit
Rust 1.92 floor even when Underwood's foundational crates target an older
compiler.
