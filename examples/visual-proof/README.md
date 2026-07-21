<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Underwood visual proof

This external example turns the real public semantic-to-scene path into an
inspectable image. It shapes bundled Latin and Arabic fonts through
`underwood_parley`, lowers public `TextScene` fragments into `imaging` glyph
runs, and renders deterministic RGBA pixels with `imaging_vello_cpu`.

The poster deliberately exposes difficult evidence: one `ffi` ligature painted
through two focused source clips, one paragraph mixing Latin LTR and Arabic RTL
runs with real font fallback, four default OpenType ligature substitutions, a
bounded hit-derived caret, a local text edit, retained sibling reuse, and a
paint-only update that performs no shaping.

Generate the committed snapshot from the repository root:

```sh
cargo run -p underwood_visual_proof
```

The output is `examples/visual-proof/snapshots/underwood-visual-proof.png`.
The crate's test renders the same scene and requires an exact RGBA match with
that PNG. Font bytes are reused from `examples/headless/fonts/`, where their
upstream licenses are retained.
