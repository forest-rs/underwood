<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Underwood retained-label wind tunnel

This product wind tunnel measures the real public retained single-paragraph
path at UI scale. It owns no shaping, cache, document, or scene implementation.

The retained path uses only `TextBlock`, `BlockRequest`, and public cache
diagnostics. It covers:

- thousands of stable unique labels;
- thousands of distinct labels sharing identical text and style;
- matched cold runs of distinct identities with identical versus distinct
  text, including exact analysis, shaping, and formation counts;
- one localized text change among stable siblings;
- max-content and constrained-width changes;
- explicit destruction and budget-driven cache churn.
- compact projection identity, dense whitespace collapse, and one-to-many
  expansion without per-byte maps.
- real exclusion, float, and multi-column region churn with replayable exact
  slot transcripts.
- retained alignment and Western-justification churn over fixed accepted
  region slots.

Deterministic `WorkReport`, intrinsic-metric, and cache assertions are the
correctness proof. Machine-local elapsed time is supporting evidence only. A
checked-in baseline and current result live in
`docs/proof/text-block-wind-tunnel-2026-07-24.md`.

Run the optimized wind tunnel with:

```sh
cargo run --profile wind-tunnel -p underwood_label_benchmark
```

Isolate the pre-region formation baseline and real region churn with:

```sh
cargo build --release -p underwood_label_benchmark
target/release/underwood_label_benchmark region-ready 200 512
target/release/underwood_label_benchmark region-churn 200 512
target/release/underwood_label_benchmark retained-adjustment 200 512
target/release/underwood_label_benchmark alignment-churn 200 512
target/release/underwood_label_benchmark justification-churn 200 512
target/release/underwood_label_benchmark cross-identical 200 512
target/release/underwood_label_benchmark cross-distinct 200 512
benches/labels/profile-allocations.sh
```

The fonts are included from the repository's audited example fixtures.
