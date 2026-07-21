# Underwood retained-Parley seam experiment

This research crate consumes current `parley_core::ShapedText` and lowers it
into deterministic observation records. It proves the retained analysis,
itemization, font-selection, source-cluster, and horizontal-shaping seams
without turning Parley types into Underwood's public adapter contract.

```text
corpus + licensed Parley test fonts
                |
                v
       pinned parley_core
                |
                v
retained ShapedText + observations + explicit gap matrix
```

```sh
cargo test -p underwood_parley_seam_experiment
cargo run -p underwood_parley_seam_experiment
```

The Roboto Flex and Noto Kufi Arabic font files and their licenses come from
the exact pinned `parley_dev` git source. The executable's compact FNV digests
are deterministic fixture checksums, not security or supply-chain identities;
the evidence record carries SHA-256 source identities.

This crate is decision-support evidence only: it is not a product benchmark or
`underwood_parley`. Current performance measurements belong in
`benches/semantic-scene` and execute the production adapter.
