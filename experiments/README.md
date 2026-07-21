<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Research experiments

These crates preserve deterministic hypothesis tests and evidence gathered
before or outside a permanent production path. They may contain deliberately
simplified or duplicated models. Consequently, they are not product
benchmarks, cannot establish current Underwood performance, and must not be
imported by production crates.

When a product implementation exists, all ongoing performance measurement
moves to a public-path crate under `benches/`.
