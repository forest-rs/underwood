# Retained TextBlock wind tunnel — 2026-07-24

## Scope

`benches/labels` measures the public single-paragraph path before and after
Design-0012. The corpus contains 2,048 retained labels and uses the repository's
audited Roboto Flex and Noto Kufi Arabic font resources.

Deterministic work and cache assertions are the proof. These wall times are
same-machine screens from an Apple Silicon macOS host using:

```sh
cargo run --profile wind-tunnel -p underwood_label_benchmark
```

They are not portable latency guarantees.

## Pre-TextBlock baseline

The baseline uses the complete old ceremony for every label:

`DocumentId -> Document -> Edit -> Paragraph -> TextLeaf -> Publication ->
StyleMap -> FiniteWidth -> SceneRequest`.

One immutable computed style is constructed once. Each `StyleMap` clones that
style; its family, feature, and variation storage remains Arc-shared.

| Workload | Operations | Total ns | ns/operation |
| --- | ---: | ---: | ---: |
| cold unique labels | 2,048 | 80,419,916 | 39,267 |
| retained unchanged labels | 2,048 | 18,794,250 | 9,176 |
| width-only resize | 2,048 | 65,711,541 | 32,085 |
| one localized edit | 1 | 111,666 | 111,666 |

The retained pass asserts zero analysis, shaping, flow, and geometry work.
Width-only preparation asserts zero analysis and shaping plus one paragraph of
flow per label. The localized edit asserts one shaped paragraph.

## Baseline finding

The old public API exposes no release operation, cache budget, resident-entry
diagnostics, or eviction evidence. Creation/destruction churn is therefore
`NOT_OBSERVABLE`, and the two retained cache layers grow through linear
`Vec::position` lookup.

That established the acceptance bar for the post-implementation run: explicit
stable, identical, localized-edit, intrinsic/constrained, release, and
budget-eviction evidence rather than convenience-call timing alone.

## Public `TextBlock` result

The stable-identity corpus uses the same alternating `Save` / `Open retained
document` values as the baseline. Every block borrows one shared
`ComputedInlineStyle` and `PaintTable`; no per-block `StyleMap` is authored by
the benchmark.

| Workload | Operations | Total ns | ns/operation |
| --- | ---: | ---: | ---: |
| cold stable blocks | 2,048 | 79,259,958 | 38,701 |
| retained unchanged blocks | 2,048 | 18,443,542 | 9,005 |
| constrained-width change | 2,048 | 63,491,125 | 31,001 |
| one localized edit | 1 | 106,042 | 106,042 |
| explicit release | 2,048 | 10,573,667 | 5,162 |
| cold identical text / distinct identities | 2,048 | 66,321,375 | 32,383 |
| retained identical text / distinct identities | 2,048 | 16,279,125 | 7,948 |
| create/destroy budget churn | 2,048 | 103,335,292 | 50,456 |

The timings show the façade did not buy convenience by introducing a slower
second path: the comparable cold, retained, and constraint-change screens stay
in the baseline range. The localized-edit number is one machine-local sample,
so the deterministic one-paragraph work assertion carries more weight than
its elapsed time.

## Deterministic proof

The wind tunnel asserts:

- cold stable and identical blocks each shape exactly one identity-local
  paragraph;
- an unchanged second pass performs zero analysis, itemization, font
  selection, shaping, flow, and geometry;
- changing only max-content to a finite constraint performs flow without
  repeating analysis, selection, or shaping;
- changing one block analyzes and shapes exactly that block;
- min-content width is no greater than max-content width, and its height is no
  less; non-empty baseline metrics are present;
- explicitly releasing all 2,048 blocks leaves zero geometry entries and zero
  Parley physics entries;
- a 64-entry churn budget causes exactly 1,984 evictions, observes a transient
  peak of 65 while the newest owned output materializes, retains 64 geometry
  and 64 backend entries, and reaches zero/zero after `clear_cache`.

The separate deterministic unit suite additionally proves mandatory breaks in
max-content, min-content Arabic break reshaping, exact advance-derived metrics,
empty-block metrics, block/document glyph equivalence, zero-budget owned
outputs, LRU reload, and explicit release.

## Result

The lightweight call site is real, but it remains intentionally honest:
distinct text identities do not gain an undocumented cross-paragraph shaping
cache. The material gain is a calm borrowed-style API, true intrinsic modes,
exact host metrics, logarithmic identity lookup, and an observable bounded
lifetime across both retained layers.
