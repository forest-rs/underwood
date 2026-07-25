# Cross-identity preparation cache proof — 2026-07-25

## Result

Underwood now shares exact immutable paragraph preparation across distinct
text consumers without sharing their identity. In the 512-label wind tunnel,
each cleared round performs one analysis, shape, and formation for identical
labels; the other 511 labels reuse the prepared facts and rebuild their own
scene geometry. Distinct text still performs 512 independent preparations.

This is the implemented proof for Design-0016. It adds no dependency and no
`unsafe`; core remains `no_std + alloc` and Rust 1.88 compatible.

## Boundary

`LayoutEngine` owns the cache because it already owns the paragraph backend,
geometry lifetime, and cache diagnostics. A value contains only immutable
paragraph-local facts and an optional identity-free region transcript
template. Every hit creates a fresh `PreparedParagraph` envelope, rebinds
region attempts to the current paragraph, reruns adapter validation against
the current projection, and rebuilds source, semantic, interaction, paint,
fragment, revision, composition, and final geometry identity.

Backends are ineligible by default. `ParagraphFormation` implementations opt
in with `shared_preparation_epoch`; changing or removing the epoch clears
resident shared facts before another lookup. `ParleyParagraphEngine` uses
epoch zero because its `FontSet` is an immutable engine-owned snapshot.

The exact key includes projected text, every analysis/shaping/inline-flow
style and run, paragraph direction and whitespace policy, constraint, region
flow and cursor, empty-line height when relevant, paint-slot partition, and
the backend epoch. It excludes source and semantic topology, document and
paragraph identity, revision, composition identity, alignment, brush values,
and placement.

## Correctness evidence

Blocking tests prove:

- distinct documents receive distinct paragraph, text, semantic, fragment,
  revision, and composition identity after a hit;
- different leaf segmentation and semantic roles can share prepared facts
  while producing their own source and semantic geometry;
- region transcripts are rebound to the consuming paragraph;
- language, word breaking, font weight, spacing, line height, wrap policy,
  direction, constraint, regions, and paint-slot topology miss exactly;
- alignment and brush-only changes hit exactly;
- a backend without explicit eligibility never shares, and epoch changes
  invalidate resident facts;
- cached output is revalidated against the current projection;
- deliberate text-fingerprint collisions still require full key equality;
- zero budgets disable retention, oversized entries are served but not
  retained, and byte-bounded LRU eviction preserves the configured limit;
- stable same-identity geometry reuse remains ahead of shared reuse, while
  `release_document` preserves identity-free facts and `clear_cache` removes
  them.

The primary integration cases are in
`underwood_parley/src/tests/intrinsic_and_cache.rs`; adversarial backend,
poisoned-value, and collision traps are in `underwood/src/scene/tests.rs`.

## CPU result

The release benchmark clears retained state at the start of each round and
prepares 512 distinct identities. Five trials of 50 rounds, or 25,600 total
operations per trial, produced:

| Workload | Before median ns/op | After median ns/op | Stage law |
| --- | ---: | ---: | --- |
| Identical text | 24,077 | 15,057 | 50 fresh + 25,550 shared |
| Distinct text | 45,288 | 47,017 | 25,600 fresh + 0 shared |
| Isolated primed shared hit | — | 12,669 | 25,550 shared hits |

Exact repeated identities are about 37.5% faster than the recorded before
state. The distinct workload is about 3.8% slower because it performs the
exact shared lookup, owns a key after each miss, and retains each distinct
result. This is an explicit cost of enabling the optional cache, not hidden
work. Callers with workloads that do not benefit leave the shared byte budget
at its zero default.

One identical entry has an 8,073-byte deterministic accounting charge in this
fixture. The distinct run retains 512 entries charged at 7,645,184 bytes.
These values cover cache-owned entry storage and nested capacities. They
exclude shared font blobs, external `Arc` backing, allocator metadata, and
container implementation overhead that cannot be measured portably; they are
retention-budget values rather than allocator-exact heap sizes.

## Allocation result

Matched macOS full malloc-stack-logging traces report:

| Workload | Allocation calls | Allocated bytes |
| --- | ---: | ---: |
| Fresh identical miss and insertion | 565 | 138,587 |
| Fresh distinct miss and insertion | 955 | 225,018 |
| Primed shared hit | 432 | 79,181 |
| Stable same-identity geometry hit | 249 | 26,852 |

Compared with a fresh identical miss in the same implementation, a shared hit
avoids 133 allocation calls and 59,406 allocated bytes in this fixture. It
does not approach the stable same-identity path because every new consumer
still requires owned scene materialization. That remaining cost is evidence
for the separate scratch and selective-materialization work, not a reason to
share revision-bound scene output.

Reproduce the timing modes with:

```sh
cargo build --release -p underwood_label_benchmark
./target/release/underwood_label_benchmark cross-identical 50 512
./target/release/underwood_label_benchmark cross-distinct 50 512
./target/release/underwood_label_benchmark shared-hit 50 512
```

On macOS, reproduce allocation deltas with:

```sh
benches/labels/profile-allocations.sh
```

## Public migration

Existing callers retain the old behavior. `CacheBudget::new(max_entries)`
still budgets identity-bound geometry and leaves shared retention disabled.
Benefiting callers opt in:

```rust,ignore
let budget = CacheBudget::new(2_048)
    .with_shared_preparation_bytes(8 * 1024 * 1024);
```

`CacheDiagnostics` adds shared budget, residency, hit, miss, eviction, peak,
and oversized-entry accessors. `WorkReport::shared_preparations` distinguishes
exact shared hits from stable geometry reuse and fresh backend work.

The additive `ParagraphFormation::shared_preparation_epoch` method defaults to
`None`; existing custom backends require no change and remain correct. A
backend may return `Some(epoch)` only when all prepared facts are independent
of paragraph identity and the epoch changes before any hidden resource can
alter output.

`PreparedParagraph` keeps its public constructors and observations. Its
internal clones now share immutable backing, but every cached consumer receives
a fresh paragraph-identity envelope.

## Deliberate limits

- This first cache is exact. Width, flow, or preparation-policy changes miss;
  a later backend-private staged cache may preserve more analysis and shaping
  only if profiles justify its added ownership.
- Geometry and all identity-bound scene records are rebuilt for each consumer.
- The benchmark's 8 MiB budget is a measured fixture configuration, not a
  universal recommendation.
- Runtime font or text-data mutation is not supported by the immutable Parley
  adapter today. A future mutation API must advance the eligibility epoch and
  define cache invalidation before exposing changed output.
