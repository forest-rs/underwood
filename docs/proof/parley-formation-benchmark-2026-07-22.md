# Parley paragraph-formation benchmark — 2026-07-22

## Scope

This measurement compares the real public 64-paragraph semantic-to-scene path
before and after moving finite-width line formation behind the Parley adapter.
The baseline is exact `main` commit `8882147`; the candidate is production
commit `023c777`. Both use the same pinned Parley Core revision `6c81e1d`, font
bytes, document, styles, and work-report assertions.

The baseline performs provisional glyph-edge wrapping in scene construction.
The candidate performs legal boundary selection, line-local bidi ordering,
font-metric resolution, complete portable-line lowering, and scene construction.
The width-only comparison is therefore a cost-of-new-semantics observation, not
an apples-to-apples regression against an equivalent implementation.

## Command

```sh
cargo run --profile wind-tunnel -p underwood_semantic_scene_benchmark --locked
```

Environment: arm64 macOS 26.5.2, Rust and Cargo 1.96.0. Each cell is the midpoint
of two complete benchmark-process observations after compilation.

| Workload | Iterations | `main` ns/iteration | formed ns/iteration | Delta |
| --- | ---: | ---: | ---: | ---: |
| Cold scene | 20 | 1,780,341 | 1,749,699 | -1.7% |
| Retained unchanged | 200 | 86,216 | 89,087 | +3.3% |
| Paint only | 200 | 84,354 | 86,846 | +3.0% |
| Width only | 100 | 360,162 | 563,905 | +56.6% |
| One-paragraph edit | 100 | 105,025 | 109,061 | +3.8% |

## Interpretation

Cold formation is slightly faster in this observation; unchanged, paint-only,
and one-paragraph-edit paths are within 3.8% of `main`. The width-only path adds
about 204 microseconds for all 64 paragraphs, or 3.2 microseconds per paragraph,
while doing the paragraph work absent from the baseline.

An initial candidate observation was about 583 microseconds per width change.
Retaining Parley's logical-cluster facts with `ShapedText` and reusing the
line-plan allocation reduced that to 564 microseconds without changing the
public contract or adding dependencies.

The remaining delta is dominated by constructing source-complete portable runs
and glyph coverage for newly formed lines before scene lowering. Caching a
second adapter-private shaped-glyph model would improve this benchmark at the
cost of undoing Design-0005's ownership simplification, so it was rejected. A
future optimization needs allocation/profile evidence and must preserve the
single retained `ShapedText` truth.

These are diagnostic same-machine observations, not release thresholds or a
universal speed claim.

## Bounded-reshape candidate addendum

The final candidate was measured against safe-break checkpoint `023c777` in
the same session. Each cell is again the midpoint of two complete benchmark
processes. The corpus uses safe line opportunities, so this measures the
always-ready retained reshape state and disposable formed copy, not the
additional cost of executing an unsafe break.

| Workload | Safe-break ns/iteration | Final candidate ns/iteration | Delta |
| --- | ---: | ---: | ---: |
| Cold scene | 1,803,747 | 1,780,811 | -1.3% |
| Retained unchanged | 90,980 | 98,171 | +7.9% |
| Paint only | 89,321 | 97,115 | +8.7% |
| Width only | 567,117 | 592,346 | +4.4% |
| One-paragraph edit | 109,709 | 117,767 | +7.3% |

The width-only path adds about 25 microseconds for all 64 paragraphs, or 0.39
microseconds per paragraph, while preserving zero analysis and initial-shaping
work. Retained and paint-only requests do not clone the formed shape; their
small observed increase is recorded without attributing it to the reshape path
from timing alone. No second shaped-glyph cache or special safe-only fast path
was introduced to conceal the cost.
