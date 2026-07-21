# Underwood retained-Parley seam experiment

This historical research crate copies current `parley_core` callback results
into private Underwood-owned observation records. It proves the usable
analysis, itemization, font-selection, and horizontal-shaping seams without
turning their current Rust types into a public adapter contract.

```text
corpus + licensed Parley test fonts
                |
                v
       pinned parley_core
                |
                v
private owned observations + explicit gap matrix
```

```sh
cargo test -p underwood_parley_seam_experiment
cargo run -p underwood_parley_seam_experiment
```

The Roboto Flex and Noto Kufi Arabic font files and their licenses come from
the exact pinned `parley_dev` git source. The executable's compact FNV digests
are deterministic fixture checksums, not security or supply-chain identities;
the evidence record carries SHA-256 source identities.

This crate is historical evidence only: it is not a product benchmark,
`underwood_parley`, an owned shaped-text design, or a replacement for upstream
retained output. Current performance measurements belong in
`benches/semantic-scene` and execute the production adapter.
