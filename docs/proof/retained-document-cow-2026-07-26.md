# Retained document copy-on-write proof — 2026-07-26

## Result

A one-byte edit no longer clones the document paragraph spine or untouched
paragraph and text storage. The public 64- and 1,000-paragraph edit-staging
workloads now perform the same 20 allocation calls:

| Paragraphs | Before calls | After calls | Before bytes | After bytes |
|---:|---:|---:|---:|---:|
| 64 | 79 | 20 | 7,744 | 1,592 |
| 1,000 | 1,015 | 20 | 105,088 | 1,832 |

Two consecutive release runs produced the same allocation counts, allocated
bytes, and live-allocation observations. The 240-byte retained difference
between 64 and 1,000 paragraphs is one additional bounded 32-way sequence
path, not storage proportional to the paragraph count.

This closes the document-staging term in Design-0017. It does not hide the
remaining all-paragraph `LayoutEngine` traversal: localized preparation still
takes 621 allocation calls at 64 paragraphs and 782 at 1,000 paragraphs in the
same run.

## Representation

Published `DocumentState` owns a persistent paragraph sequence:

- immutable nodes have a branching factor of 32;
- paragraphs are independently shared `Arc` values;
- cloning a revision clones one root handle;
- editing a paragraph copy-on-writes at most one root path and that paragraph;
- appending grows or copy-on-writes the rightmost path;
- traversal uses a fixed-size stack and allocates nothing;
- dropping an old snapshot reclaims exactly the nodes and paragraph values
  unique to that revision.

This is a typed persistent collection, not a general arena. It has no holes,
stable external handles, compaction policy, cross-document lifetime, or
partial-free problem. A `Vec<Arc<Paragraph>>` was not sufficient because its
first mutation would still clone one handle per paragraph.

Each published text leaf stores `Arc<str>`. On the first ranged edit within a
transaction, the touched leaf converts to one mutable `String`. Further
carets or ranges targeting that leaf mutate the same buffer. Commit freezes
each changed paragraph's mutable leaves once; old snapshots continue to own
their original `Arc<str>` values.

## Reproduction

Build and run the checked-in public-path allocator tunnel:

```sh
cargo build --release \
  -p underwood_semantic_scene_benchmark \
  --features allocation-counting
benches/semantic-scene/profile-counted-allocations.sh
```

The two captured runs were:

| Scenario | Paragraphs | Calls | Allocated bytes | Net live calls | Net live bytes |
|---|---:|---:|---:|---:|---:|
| retained exact repeat | 64 | 0 | 0 | 0 | 0 |
| edit staging | 64 | 20 | 1,592 | 11 | 888 |
| localized prepare | 64 | 621 | 162,083 | 352 | 79,463 |
| localized edit + prepare | 64 | 641 | 163,675 | 363 | 80,351 |
| retained exact repeat | 1,000 | 0 | 0 | 0 | 0 |
| edit staging | 1,000 | 20 | 1,832 | 11 | 1,128 |
| localized prepare | 1,000 | 782 | 223,035 | 355 | 79,967 |
| localized edit + prepare | 1,000 | 802 | 224,867 | 366 | 81,095 |

Allocation counts and bytes are observations from `allocation-counter`, not
core diagnostics or a claim about allocator physical residency. The benchmark
builds the document, fonts, selection, and primed scene before measuring the
isolated event.

## Correctness evidence

Focused tests prove:

- indexing and allocation-free traversal across the 32- and 1,024-paragraph
  boundaries;
- a mutation in a cloned 1,000-paragraph sequence leaves the old paragraph
  exact;
- untouched neighboring and distant paragraphs retain pointer identity;
- dropped edits publish nothing and old snapshots remain exact;
- same-leaf multicaret and multi-leaf selection replacement preserve their
  established rebasing and atomicity laws;
- failed ranges do not publish partial state.

The focused gates used for this slice are:

```sh
cargo fmt --all
cargo clippy -p underwood --all-targets --all-features -- -D warnings
cargo test -p underwood --all-features
cargo check -p underwood --no-default-features
```

Full rustdoc, repository, and protected-remote gates remain campaign
completion gates rather than evidence inferred from these focused checks. The
touched core crates additionally pass their Rust 1.88 gate:

```sh
cargo +1.88.0 check \
  -p underwood -p underwood_parley \
  --all-targets --all-features
cargo +1.88.0 test \
  -p underwood -p underwood_parley \
  --all-features --no-run
```

The repository's deliberately Rust-1.92 presentation/PDF members cannot take
part in a workspace-wide 1.88 command: their `imaging`,
`imaging_vello_cpu`, and `krilla` dependencies declare that higher floor.
That existing crate-level split is not attributed to this core change.

## Remaining work exposed by the proof

Document staging is now proportional to touched content plus a bounded tree
path. At the point of this isolated measurement, localized scene preparation
still iterated every paragraph. That follow-on term is now resolved by the
shared-subtree diff recorded in
`retained-localized-preparation-2026-07-26.md`.

Capability-scaled adapter and scene residency remains governed by proposed
Design-0018. This proof neither assumes approval nor treats today's maximal
interaction records as the final memory shape.
