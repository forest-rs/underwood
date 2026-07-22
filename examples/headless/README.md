<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Underwood headless example

This external workspace crate exercises the first public semantic-to-scene
slice exactly as a downstream caller does. It constructs an immutable semantic
document, resolves named and generic family requests through Fontique, shapes
mixed Latin and Arabic through a configured `Arab`/`ar` fallback, observes
scene geometry and source mapping, edits one paragraph, and proves paragraph,
font-request, paint-only, and width-only reuse from actual work counters. It
also proves variable weight/width synthesis, explicit-axis precedence,
synthetic-oblique evidence, deterministic missing-family diagnostics, visible
coverage for a zero-advance Arabic mark, and Latin ink outside shaped advance.
Real `ffi` substitution and source ownership remain executable, while a
separate adapter regression requires an explicit error instead of an invented
paint partition when one ligature crosses paint runs.

The bundled font fixtures retain their upstream licenses in `fonts/`.
