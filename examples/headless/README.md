<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Underwood headless example

This external workspace crate exercises the first public semantic-to-scene
slice exactly as a downstream caller does. It constructs an immutable semantic
document, shapes mixed Latin and Arabic through `underwood_parley`, observes
scene geometry and source mapping, edits one paragraph, and proves paragraph,
paint-only, and width-only reuse from actual work counters.

The bundled font fixtures retain their upstream licenses in `fonts/`.
