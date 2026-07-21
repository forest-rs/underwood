<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Underwood semantic-scene benchmark

This is a product benchmark. It depends on `underwood` and
`underwood_parley`, uses only their public APIs, and executes the same
caller-supplied-font path as `examples/headless`.

It measures five distinct workloads over a 64-paragraph mixed-script document:

- cold scene preparation with a fresh retained engine;
- unchanged retained preparation;
- paint-value-only lowering;
- width-only reflow;
- one-paragraph editing with 63 unchanged siblings.

Every measured retained workload asserts the corresponding real
`WorkReport`. There is no benchmark-private document, shaper, layout algorithm,
or cache implementation.

Run the optimized benchmark with:

```sh
cargo run --profile wind-tunnel -p underwood_semantic_scene_benchmark
```

The font binaries are included from the licensed external-example fixtures, so
there is one audited copy in the repository.
