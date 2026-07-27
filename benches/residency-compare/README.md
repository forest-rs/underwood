# Underwood / Parley retained comparison

This workspace-only wind tunnel compares Underwood's retained public path with
the high-level Parley `Layout` from the exact Parley revision pinned by the
workspace.

The fixture holds these inputs equal:

- embedded Roboto Flex and Noto Kufi Arabic font files;
- deterministic embedded-only Fontique catalogs and Arabic fallback;
- source corpus, font family, 15-unit font size, and 180-unit wrap width;
- retained source text required to rebuild edited content;
- 64- and 1,000-item scales.

The compared retained shapes are intentionally reported separately:

- `underwood-label-display` retains display-only scene facts and drops adapter
  facts;
- `underwood-label-editable` retains editable scene sidecars with the default
  zero adapter-fact budget;
- `underwood-label-editable-warm` additionally retains reusable adapter facts;
- `parley-label` retains one ordinary high-level `Layout` per source;
- `underwood-document-mixed` retains one editable paragraph among display-only
  siblings with the default zero adapter-fact budget;
- `underwood-document-mixed-warm` additionally retains reusable adapter facts
  for every paragraph admitted by the byte budget;
- `parley-document-flat` retains one newline-separated `Layout`, while
  `parley-document-paragraphs` retains one `Layout` per paragraph.

These are not declared byte-for-byte equivalent. Parley `Layout` always keeps
its shaped clusters and interaction-capable data, while Underwood chooses scene
capabilities and adapter-fact lifetime independently.

## Ledgers

Run the release timing matrix:

```sh
cargo build --release -p underwood_residency_compare
benches/residency-compare/profile-timing.sh
```

On macOS, record live heap and process observations:

```sh
benches/residency-compare/profile-live-memory.sh
```

Record allocation histories separately:

```sh
benches/residency-compare/profile-allocations.sh
```

Build the wind tunnel with its optional global counting allocator to partition
Underwood's label path by lifecycle phase:

```sh
cargo run --release -p underwood_residency_compare \
  --features allocation-counting -- underwood-allocation-phases 1000
```

Append `-warm` to the scenario name to retain adapter formation facts. The
unadorned scenario uses the normal zero adapter-fact budget so scene residency
and optional re-formation residency remain distinct.

The phase report distinguishes total allocation churn, peak live growth, and
net retained bytes for block creation, font and engine setup, cold preparation,
stable reuse, edit publication, edited preparation, paint-only preparation,
and teardown. The allocator is confined to this workspace-only benchmark.

The Rust output labels Underwood's deterministic capacity charges as
`scene_*_bytes` and `adapter_*_bytes`. They exclude allocator metadata, shared
font blobs, and process runtime state. Parley's equivalent private vector
capacities are not public, so the harness prints public topology counts and
`accounted_bytes=unavailable` instead of estimating them.

`heap` reports currently live allocator bytes for the whole process.
`vmmap` reports process physical footprint. `malloc_history` reports allocation
events, not retained memory. The scripts preserve those three meanings and
include runtime/font baselines rather than presenting any one as the other.

## Interaction comparison

The query cases use one unwrapped line containing 64 or 1,000 word units,
including periodic Arabic runs. They measure:

- exact point hit testing near the visual end;
- closest point hit testing near the visual end;
- lookup of the final authored scalar by byte position.

Inputs are passed through `black_box` inside every iteration so the compiler
cannot hoist an immutable query out of the measured loop.
