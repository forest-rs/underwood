<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `underwood_parley`

`underwood_parley` is the pinned, `no_std + alloc` Parley Core adapter for
Underwood's pre-stable paragraph-formation contract. Its default feature set
accepts only caller-supplied font bytes and never enables system discovery. A
native host can explicitly enable `system-fonts` and call
`FontSet::with_system_fonts` to add one fixed Fontique platform-catalog snapshot
before constructing the paragraph engine. Linux uses Fontique's dynamic
Fontconfig loading so compiling the optional feature does not require
Fontconfig development headers; if the runtime library is absent, no system
fallback is added.

The adapter owns analysis and shaping scratch, retains Parley Core's native
`ShapedText` across reusable formations, and lowers it into Underwood's
portable formed-line records without maintaining a second shaped-run model.
Parley Core boundary classes select legal and mandatory breaks, line boxes use
the selected fonts' scaled metrics, and each line's runs are reordered visually
only after its logical source range is fixed. Paint
boundaries remain source and clip metadata rather than shaping inputs. Complete
Underwood shaping runs supply family, weight, width, style, font size,
language, OpenType features, and variable-font coordinates.

`FontSet` is a deterministic Fontique catalog, not an Underwood matcher.
`FontSet::try_from_fonts` registers caller-owned memory fonts with system fonts
disabled; builders configure named generic families and script/language
fallbacks. The optional native-host builder adds platform fonts without making
them part of deterministic proof. For every itemized run, Fontique owns
attribute matching, coverage, fallback, and synthesis. The adapter performs
only the cluster callback needed to pass the selected `FontInstance` to Parley
Core, then retains exact resource, synthesis, final normalized-coordinate, and
work evidence in Underwood values.

Parley stores shaped clusters in logical order. The adapter lowers LTR clusters
forward and RTL clusters backward so scene glyphs remain in visual order, and
applies line-local UAX #9 L2 run reordering for mixed-direction text. A
ligature glyph owns the complete source range represented by its start and
continuation clusters. Parley's `contributes_to_shaping` analysis identifies
controls and format characters which intentionally produce no glyphs; their
source remains explicit while shaping-only sentinel glyphs are discarded.

Formed lines also retain a separate visual cluster stream. Every Parley cluster
keeps its own source slice and advance, including ligature components,
whitespace, combining source, and controls, while its left and right sides map
to explicit paragraph-local boundaries and upstream/downstream affinities.
Underwood can therefore project exact semantic hits and caret stops without
reconstructing bidi direction from glyphs or using ink clips as interaction
geometry. Soft wraps retain both affinities for their shared logical boundary,
and mandatory breaks keep the visible caret before the control.

Paint coverage records source-to-paint ownership, not universal glyph ink. A
glyph wholly owned by one paint run lowers without a per-glyph clip, leaving
outline, bitmap, color-graph, and synthesis extent to the renderer. A glyph
crossing paint runs returns `UnsupportedPaintCoverage` until a
conformance-backed component rule can provide explicit source-complete clips.
Advances and character counts are never substituted for component geometry.
Hit testing, carets, and selections use the separate shaped-cluster stream.

Fontique synthesis variations precede explicit `ShapingStyle` variations at
the Parley Core seam. An explicit coordinate therefore wins for the same axis.
Synthetic skew is retained for capable renderers and does not alter
Underwood's layout advances. Synthetic emboldening is likewise retained and no
longer fails preparation merely because outline-derived extent is unavailable.
Renderer support for either synthesis operation is an independent fidelity
capability and must not be inferred from successful preparation.
