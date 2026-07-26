# Retained structural-law proof — 2026-07-26

## Overview

This checkpoint closes the structural gaps found after the first
Design-0017 allocation win. Exact scene reuse was already O(1), and a
same-structure normal-flow edit was already O(change), but four related paths
still rebuilt or searched document-scale values:

- document region transcripts were flattened on every publication;
- region edits continued through every following paragraph;
- local style overrides lived in flat copy-on-write vectors;
- interaction lookup and paragraph append did not use the persistent spine.

The corrected implementation retains paragraph attempt blocks in the scene
spine, stops region reflow when its input cursor converges, stores style
overrides in persistent paragraph buckets, routes normal-flow points and exact
text positions to one paragraph, and appends through persistent document and
scene paths.

This proof does not claim a two-dimensional spatial index for arbitrary
regions, constant-time paragraph-local interaction search, or capability-scaled
scene residency. Design-0018 owns the remaining display/editable sidecar and
local-index work.

## Concepts and glossary

- **Scene spine:** immutable balanced summary tree whose leaves retain
  paragraph-local scene segments.
- **Region cursor:** exact continuation state before or after a paragraph's
  region attempts.
- **Cursor convergence:** the first unchanged successor whose retained input
  cursor matches the newly produced cursor; its complete suffix remains valid.
- **Style bucket:** immutable overrides for one paragraph, reached through a
  sparse persistent 32-way tree.
- **Scene transcript:** cheap scene-core handle that traverses retained
  paragraph attempt blocks without flattening them.

## Public migration

`SceneOutput::region_transcript` and
`CompositionSceneOutput::region_transcript` now return a cheap owned
`SceneRegionTranscript`. Its attempt sequence is an exact-size iterator:

```rust
let transcript = output
    .region_transcript()
    .expect("the request used region flow");

for attempt in transcript.attempts() {
    inspect(attempt);
}

assert_eq!(transcript.replay(&flow)?, transcript.end());
```

Callers that previously indexed the borrowed attempt slice iterate or collect
explicitly. Cloning the transcript clones one scene-core `Arc`; it does not
copy attempts. `replay` remains an explicit O(attempts) verification operation
and is no longer implicit publication work.

`StyleMap::set`, `StyleMap::set_paragraph_style`, `LayoutEngine::prepare`, and
the scene interaction method call shapes are unchanged.

## Structural laws

### Region publication

Each `ParagraphSceneSegment` retains its validated paragraph transcript.
`SceneSummary` combines start cursor, end cursor, attempt count, rejection
count, and seam validity. Scene publication checks that summary in O(1) and
constructs a `SceneRegionTranscript` view over the spine.

No document attempt vector is collected, cloned, or replayed during
publication. The existing first-region test now proves zero attempt-scratch
growth.

### Localized region preparation

The engine structurally diffs document and style roots, starts at each changed
paragraph, and reforms only until an unchanged paragraph's full preflight key
accepts the newly produced region cursor. Consecutive affected segments are
installed with one persistent range replacement, so the spine cost is
O(log P + A), not O(A log P).

The public Parley-backed regression uses 64 paragraphs. A same-height edit in
paragraph 31 performs one paragraph of shaping, flow, and paint; accounts 63
paragraphs as retained; preserves the transcript attempt count; and allocates
less scene output than cold publication.

### Paragraph-local styles

`StyleMap` is an immutable state handle whose per-document overrides live in a
sparse seven-level 32-way tree. One mutation copies a bounded root path and
one normally small paragraph bucket. Structural comparison skips shared
subtrees by `Arc` identity and reports only changed paragraph indexes.

Local style changes feed the same localized normal- and region-flow paths as
text changes. Default-style changes intentionally remain global.

### Interaction routing

Normal-flow point queries descend scene block-extent summaries to one
paragraph before searching its clusters. Snapshot caret validation,
`position_at`, cursor steps, cross-paragraph movement, and text rank descend
by the paragraph encoded in `TextId`.

Arbitrary region flow remains candidate-dependent because several columns can
share one block coordinate. Design-0017 explicitly forbids inferring a 2D
index from the normal-flow prefix tree.

### Append

The persistent document sequence proves append by comparing shared node
prefixes, including leaf and root-growth boundaries. The scene spine performs
a balanced persistent right append. Existing paragraph segments and old scene
roots remain shared.

The append wind tunnel gives the cache a `P + 1` entry budget because the
result legitimately retains one additional paragraph. A `P` budget remains
correct but must evict an entry and therefore cannot prove the retained append
law.

## Matched evidence

Commands:

```sh
cargo build --release -p underwood_semantic_scene_benchmark \
  --features allocation-counting
benches/semantic-scene/profile-counted-allocations.sh
benches/semantic-scene/profile-localized-timing.sh \
  target/release/underwood_semantic_scene_benchmark 21
```

Allocation-counting results:

| Event | Paragraphs | Calls | Requested bytes |
|---|---:|---:|---:|
| exact repeat | 64 | 0 | 0 |
| exact repeat | 1,000 | 0 | 0 |
| localized text | 64 | 613 | 159,547 |
| localized text | 1,000 | 616 | 160,339 |
| localized region | 64 | 616 | 159,995 |
| localized region | 1,000 | 619 | 160,787 |
| localized style | 64 | 588 | 156,165 |
| localized style | 1,000 | 591 | 156,957 |
| append | 64 | 558 | 152,314 |
| append | 1,000 | 562 | 153,370 |

Twenty-one-sample release timing:

| Event | Paragraphs | Min | Median | Max |
|---|---:|---:|---:|---:|
| localized text | 64 | 64.208 µs | 70.625 µs | 93.416 µs |
| localized text | 1,000 | 74.542 µs | 81.583 µs | 110.250 µs |
| localized region | 64 | 64.166 µs | 70.000 µs | 79.875 µs |
| localized region | 1,000 | 73.833 µs | 80.292 µs | 91.125 µs |
| localized style | 64 | 60.500 µs | 64.500 µs | 72.250 µs |
| localized style | 1,000 | 72.375 µs | 77.458 µs | 100.250 µs |
| append | 64 | 42.667 µs | 47.291 µs | 59.333 µs |
| append | 1,000 | 48.166 µs | 50.500 µs | 57.833 µs |

The 64-to-1,000 allocation deltas are three calls for localized text, region,
and style, and four calls for append. Requested-byte deltas are bounded by
persistent-tree depth and do not scale with paragraph records.

## Extension points

- Design-0018 moves source, semantic, hit, selection, navigation, and native
  editing facts into requested sidecars.
- Paragraph-local sorted indexes can replace the remaining local scans without
  changing spine routing.
- A region/slot spatial summary can make arbitrary-region point lookup
  sublinear when a product workload justifies it.
- Persistent style buckets can gain removal without changing their provenance
  model.

## Gates and risks

The checkpoint passes:

- `cargo fmt --all`;
- workspace clippy with `-D warnings`;
- workspace tests and rustdoc tests;
- `cargo xtask check`;
- Rust 1.88 workspace checking with the documented renderer exclusions;
- `x86_64-unknown-none` and `wasm32-unknown-unknown` checks for `underwood`
  and `underwood_parley`.

No production dependency or `unsafe` was added. Region-attempt iteration is a
public migration and is recorded above and in Design-0017. Capability-scaled
residency is deliberately not inferred from these structural wins.
