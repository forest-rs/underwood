<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Underwood visual proof

This external example turns the real public semantic-to-scene path into an
inspectable image. It shapes bundled Latin and Arabic fonts through
`underwood_parley`, lowers public `TextScene` fragments into `imaging` glyph
runs, and renders deterministic RGBA pixels with `imaging_vello_cpu`.

The poster deliberately exposes difficult evidence. Its large Arabic and Latin
specimen draws font-derived ink boxes around a visible zero-advance Arabic glyph
and a `j` whose ink overhangs its advance. A second paragraph mixes Latin LTR
and Arabic RTL runs with real fallback and finite-width line formation. One
heterogeneous document carries three Fontique-selected Roboto Flex
weight/width instances plus explicit `opsz`, mixed font sizes and line heights,
paint slots, and `liga` on/off shaping with asserted glyph counts. An absent
primary family reaches Noto Kufi through an `Arab`/`ar` fallback and executes a
retained 14° synthetic-oblique transform. A local edit, retained sibling, and
paint-only update expose the corresponding invalidation paths.

The example does not invent paint partitions inside a ligature. Underwood now
returns `UnsupportedPaintCoverage` when one shaped glyph crosses paint runs and
no conformant component geometry is available.

Generate the committed snapshot from the repository root:

```sh
cargo run -p underwood_visual_proof
```

The output is `examples/visual-proof/snapshots/underwood-visual-proof.png`.
The crate's test renders the same scene and requires an exact RGBA match with
that PNG. Font bytes are reused from `examples/headless/fonts/`, where their
upstream licenses are retained.
