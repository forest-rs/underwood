# Fontique semantic-scene benchmark evidence — 2026-07-22

## Scope

This measurement checks the product benchmark after replacing the ordered-font
shortcut with Fontique matching and after retaining portable synthesis evidence
in every resolved scene fragment. The workload remains the real public path over
64 mixed Latin/Arabic paragraphs. It contains no benchmark-private product
implementation.

The behavioral assertions now run through explicit `Roboto Flex` requests and
a configured `Arab` fallback to `Noto Kufi Arabic`. Cold and edited preparation
therefore include real Fontique selection; unchanged, paint-only, and width-only
operations still assert zero shaping.

## Command

```sh
cargo run --profile wind-tunnel -p underwood_semantic_scene_benchmark --locked
```

## Same-machine comparison

Environment: arm64 macOS 26.5.2, Rust and Cargo 1.96.0. The `main` worktree at
`ff98bc3` and the Fontique branch were run back-to-back after optimized builds.
Each cell is the midpoint of two complete benchmark-process observations. CPU
load was not pinned, so these are diagnostic observations rather than release
thresholds.

| Workload | Iterations | `main` ns/iteration | Fontique ns/iteration | Delta |
| --- | ---: | ---: | ---: | ---: |
| Cold scene | 20 | 1,556,500 | 1,711,740 | +10.0% |
| Retained unchanged | 200 | 82,676 | 89,572 | +8.3% |
| Paint only | 200 | 81,637 | 87,298 | +6.9% |
| Width only | 100 | 345,970 | 367,554 | +6.2% |
| One-paragraph edit | 100 | 99,055 | 107,319 | +8.3% |

## Investigation and correction

The first Fontique implementation measured roughly 20–28% slower on retained
paths. That was not accepted as an unexplained resolver cost because those paths
do not query Fontique.

Two implementation costs were corrected:

- `LayoutEngine` no longer clones a complete preparation key for every
  paragraph merely to discover a cache hit. It compares cached values against
  borrowed projected styles and constructs owned keys only when preparation
  actually changes.
- `FontSynthesis::default()` is now a one-word absent-evidence value with no
  allocation or reference-count traffic. Non-empty variation, embolden, or
  skew evidence is shared only for runs that actually have synthesis.

These corrections reduced hot-path overhead to the range above. The remaining
delta is plausible for the larger, more truthful shaping identity and scene
record plus real selection on cold/edited paths, but this observation is not a
license for unbounded drift. Future style work should rerun the same benchmark
and preserve deterministic work-counter assertions as the primary signal.

## Interpretation

The campaign does not claim that Fontique matching is free or that these local
times generalize across machines. It does establish that the benchmark measures
the actual resolver integration, that unchanged work still avoids the adapter,
and that a discovered hot-path regression was reduced before landing rather
than normalized as inevitable.
