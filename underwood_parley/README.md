<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `underwood_parley`

`underwood_parley` is the pinned, `no_std + alloc` Parley Core adapter for
Underwood's pre-stable paragraph-preparation contract. It accepts only
caller-supplied font bytes and never enables system font discovery.

The adapter owns analysis and shaping scratch, retains Unicode analysis across
shaping-style changes, copies every shaped result into Underwood-owned records,
and preserves paint boundaries as source and clip metadata without making them
shaping inputs. Complete Underwood shaping runs supply family, weight, width,
style, font size, language, OpenType features, and variable-font coordinates.

`FontSet` is a deterministic Fontique catalog, not an Underwood matcher.
`FontSet::try_from_fonts` registers caller-owned memory fonts with system fonts
disabled; builders configure named generic families and script/language
fallbacks. For every itemized run, Fontique owns attribute matching, coverage,
fallback, and synthesis. The adapter performs only the cluster callback needed
to pass the selected `FontInstance` to Parley Core, then retains exact resource,
synthesis, final normalized-coordinate, and work evidence in Underwood values.

Fontique synthesis variations precede explicit `ShapingStyle` variations at
the Parley Core seam. An explicit coordinate therefore wins for the same axis.
Synthetic embolden and skew are retained for capable renderers but do not alter
Underwood's layout advances.
