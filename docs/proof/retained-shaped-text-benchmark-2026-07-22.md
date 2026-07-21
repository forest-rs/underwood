# Retained `ShapedText` benchmark evidence — 2026-07-22

## Scope

This measurement checks the real public semantic-to-scene path after replacing
`underwood_parley`'s callback-time `PhysicsRun`/`PhysicsGlyph` copy with one
reusable upstream `parley_core::ShapedText` per paragraph. The benchmark owns no
alternate shaping or layout implementation. Its five workloads execute the
same 64-paragraph Latin/Arabic document through `Document`, `LayoutEngine`,
`ParleyParagraphEngine`, and `TextScene` while asserting exact `WorkReport`
invalidation behavior.

The comparison is against synchronized `main` commit `867acb7`, which uses
Parley `45da4a9` and the callback-copy adapter. The candidate begins with
implementation commit `0dc5590`, includes the review correction that bounds
items to Parley's relative cluster-offset representation, and pins Parley
`6c81e1d` with owned reusable `ShapedText`.

## Command

```sh
cargo run --profile wind-tunnel -p underwood_semantic_scene_benchmark --locked
```

## Same-machine comparison

Environment: arm64 macOS 26.5.2, Rust and Cargo 1.96.0. Each cell is the
midpoint of two complete benchmark-process observations from detached `main`
and candidate worktrees. CPU load was not pinned, so these are diagnostic
observations rather than release thresholds.

| Workload | Iterations | `main` ns/iteration | `ShapedText` ns/iteration | Delta |
| --- | ---: | ---: | ---: | ---: |
| Cold scene | 20 | 1,863,891 | 1,848,318 | -0.8% |
| Retained unchanged | 200 | 89,281 | 87,356 | -2.2% |
| Paint only | 200 | 90,091 | 88,356 | -1.9% |
| Width only | 100 | 372,875 | 370,106 | -0.7% |
| One-paragraph edit | 100 | 108,170 | 109,091 | +0.9% |

## Interpretation

All five observations remain within 2.2% of `main`, with improvements and
regressions mixed across the workloads. This is consistent with
ordinary run-to-run noise rather than a systematic retained-path cost. Cold and
edited paths now append directly into reusable upstream storage and no longer
build a second adapter-private shaped model; unchanged, paint-only, and
width-only paths still report zero shaping.

The result does not claim a universal speedup or allocation proof. It does show
that taking the upstream owned result did not purchase architectural simplicity
with an obvious product-path latency regression.
