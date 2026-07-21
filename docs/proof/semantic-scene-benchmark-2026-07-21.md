# Semantic-scene product benchmark evidence — 2026-07-21

## Scope

`benches/semantic-scene` measures the real `underwood` and
`underwood_parley` public path over a 64-paragraph mixed Latin/Arabic document.
It contains no benchmark-private product implementation. The two font fixtures
are the same licensed bytes used by `examples/headless`.

The benchmark asserts `WorkReport` invariants inside every measured operation:

- cold preparation shapes all 64 paragraphs;
- unchanged and paint-only preparation perform no analysis, shaping, or flow;
- alternating width performs no analysis or shaping and reflows 64 paragraphs;
- one edited paragraph reshapes once and reuses 63 siblings.

## Command

```sh
cargo run --profile wind-tunnel -p underwood_semantic_scene_benchmark --locked
```

## Local observation

Environment: arm64 macOS 26.5.2, Rust and Cargo 1.96.0. These single-process
observations verify the benchmark and establish an initial local reference;
they are not a cross-machine regression threshold.

| Workload | Iterations | Nanoseconds per iteration |
| --- | ---: | ---: |
| Cold scene | 20 | 1,477,708 |
| Retained unchanged | 200 | 68,903 |
| Paint only | 200 | 67,770 |
| Width only | 100 | 295,340 |
| One-paragraph edit | 100 | 86,500 |

## Interpretation

The useful result is not a universal latency claim. It is that every number now
measures actual document projection, retained preparation, Parley shaping when
required, flow, scene materialization, paint, and public work accounting. The
former synthetic wind tunnels have been reclassified under `experiments/` and
cannot be cited as product performance.
