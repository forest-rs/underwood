<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `underwood_parley`

`underwood_parley` is the pinned, `no_std + alloc` Parley Core adapter for
Underwood's pre-stable paragraph-formation contract. It accepts only
caller-supplied font bytes and never enables system font discovery.

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
fallbacks. For every itemized run, Fontique owns attribute matching, coverage,
fallback, and synthesis. The adapter performs only the cluster callback needed
to pass the selected `FontInstance` to Parley Core, then retains exact resource,
synthesis, final normalized-coordinate, and work evidence in Underwood values.

Parley stores shaped clusters in logical order. The adapter lowers LTR clusters
forward and RTL clusters backward so scene glyphs remain in visual order, and
applies line-local UAX #9 L2 run reordering for mixed-direction text. A
ligature glyph owns the complete source range represented by its start and
continuation clusters. Parley's `contributes_to_shaping` analysis identifies
controls and format characters which intentionally produce no glyphs; their
source remains explicit while shaping-only sentinel glyphs are discarded.

Fontique synthesis variations precede explicit `ShapingStyle` variations at
the Parley Core seam. An explicit coordinate therefore wins for the same axis.
Synthetic embolden and skew are retained for capable renderers but do not alter
Underwood's layout advances.
