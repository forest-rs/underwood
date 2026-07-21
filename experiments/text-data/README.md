# Underwood text-data experiment

This unpublished research crate measures the compiled Parley Core dependency
audited by ADR-0003. It produces separate size artifacts for:

- an empty Rust WebAssembly harness;
- the `minimal` compiled-data path;
- the `complex-segmentation` path enabled by Parley Core's
  `complex-scripts` feature.

The executable exercises grapheme, word, line, normalization, bidi, emoji,
mixed-script, and dictionary-script inputs. Its private identity and replay
model tests the accepted cache/replay and missing-capability laws; it is not a
production provider API.

```sh
cargo run -p underwood_text_data_experiment
bash experiments/text-data/measure.sh
```

This is dependency-distribution evidence, not a product benchmark. Product
performance measurements execute `underwood_parley` through
`benches/semantic-scene`.

`measure.sh` requires the pinned Rust toolchain's
`wasm32-unknown-unknown` target, Brotli, and Node.js. It writes untracked
artifacts, size and memory TSV reports, and native throughput observations
below `target/text-data-wind-tunnel`.
