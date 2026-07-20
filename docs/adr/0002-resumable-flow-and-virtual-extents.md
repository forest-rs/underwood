# ADR-0002: Resumable flow and virtual extent contract

- **Status:** Accepted
- **Accepted:** 2026-07-21 by Bruce Mitchener
- **Bead:** `und-oh0.5.1`
- **Authority:** `UNDERWOOD_HANDOVER.md` §§17.3–17.4, 35.1

## Goal

Define retained, resumable document flow and honest virtual extent semantics
before pagination, floats, tables, footnotes, counters, or viewport estimates
acquire hidden state.

## Non-goals

- Choosing a complete line-breaking algorithm.
- Defining table constraint solving.
- Selecting a host scrolling policy.

## Fence

The layout engine owns deterministic resumable flow state and correction
geometry; it explicitly does not own viewport anchoring policy, scheduling, or
platform accessibility realization.

## Constitutional invariants

1. An edit never recomputes an unaffected flow prefix.
2. Flow restarts at a valid predecessor checkpoint.
3. Reflow may stop only when output and successor state converge.
4. Every state required to resume is explicit and versioned.
5. Virtual extents disclose estimated versus measured knowledge.
6. Underwood reports correction geometry; the host chooses the anchor policy.

## Existing seam evidence

Parley `main` at
[`45da4a90248b1600277a4294b70d8bfde5ca8e97`](https://github.com/linebender/parley/commit/45da4a90248b1600277a4294b70d8bfde5ca8e97)
has a useful in-process paragraph breaker:

- `BreakLines` advances a mutable layout one yield at a time;
- `BreakerState` is cloneable and can be restored into the same live breaker;
- callers can set line origin, maximum inline advance, and maximum block
  extent;
- custom out-of-flow boxes yield control to the caller.

This is evidence that controlled, resumable line breaking is practical. It is
not a document-flow checkpoint: iteration and line state remain private,
serialization and compatibility are undefined, and the state owns neither
cross-block floats, footnotes, tables, counters, regions, nor virtual extents.

The architectural fence is therefore firm: Parley may own paragraph breaking
and bounded reshaping; Underwood owns document-flow continuation, checkpoint
validity, convergence, virtualization, and correction reporting.

## Options

### Page and region boundary checkpoints

Simple and naturally serializable, but potentially expensive to resume in long
scrolling regions.

### Fixed block-interval checkpoints

Predictable memory and resume distance, but insensitive to actual flow cost and
document structure.

### Adaptive deterministic checkpoints

Place boundaries using measured resume work and structural opportunities while
preserving deterministic inputs. This offers better cost control but requires a
clear policy identity and more diagnostics.

The final contract may combine region boundaries with adaptive block
checkpoints.

## Candidate checkpoint schema

The experiment uses a private `flow-trace-v0` semantic schema. Encoding remains
test-only until ratification. A serializable checkpoint contains the following
families; no flow feature may smuggle additional continuation state into a
provider closure or stack local.

### Identity and compatibility

- checkpoint format major/minor;
- deterministic checkpoint-policy id;
- engine source revision and numeric-policy id;
- document id and full source revision vector;
- flow-thread id and projection digest;
- region-chain id and revision;
- style, text-data, font-collection, schema, and resource revision set;
- enabled flow-feature set.

### Semantic cursor

- predecessor block id and fingerprint;
- next block id;
- intra-block continuation kind and offset;
- paragraph shaped-text identity and next cluster when a paragraph is split;
- nested flow path and continuation token;
- logical page, column, and region sequence counters.

### Placement state

- current region id and fragmentainer index;
- block and inline coordinates in canonical layout units;
- remaining line intervals for the current block position;
- pending collapsed margins and clearance;
- current baseline/grid state;
- pending keeps, widow/orphan constraints, and break penalties.

### Carried feature state

- active exclusions and floats, including wrap geometry and ownership;
- deferred float queue and placement attempts;
- pending and continued footnotes plus reserved extent;
- table grid identity, row/cell continuation, repeated-header state, and
  unresolved spanning constraints;
- counters, list state, running strings/headers, and generated-content state;
- fragmented block decoration and border state;
- nested-flow continuation digests.

### Convergence evidence

- digest of output immediately preceding the checkpoint;
- digest of all successor-relevant state;
- dependency frontier consumed to reach the checkpoint;
- estimate/measurement frontier at checkpoint creation;
- diagnostic counters for fallbacks and bounded solvers.

Ephemeral task ids, thread handles, allocator addresses, caches, and
cancellation tokens are never serialized. A publication envelope names the
request generation and source revision separately.

## Checkpoint validity and restart laws

A checkpoint is valid only when:

1. its format major and checkpoint-policy id are supported;
2. document, projection, region-chain, style, data, font, schema, and resource
   identities match or have an explicit compatibility rule;
3. its predecessor block still exists with the recorded fingerprint;
4. every carried feature listed in the enabled set has a decoded state;
5. nested continuation paths remain acyclic and valid;
6. the checkpoint was published from a completed, non-cancelled generation.

Given the earliest invalidated semantic block, restart chooses the latest valid
checkpoint whose predecessor is strictly before that block. If none exists,
flow restarts at the thread origin. It never chooses a later checkpoint merely
because its output happens to look similar.

Reflow stops at a later checkpoint only when all are true:

- the semantic boundary is the same;
- the canonical output digest since the previous checkpoint matches;
- the complete successor-state digest matches;
- dependency frontiers and estimate/measurement knowledge match;
- no pending fallback or solver iteration can change successor state.

Geometric coincidence without successor-state equality is not convergence.

## Trace model

Every trace header names corpus digest, source/data/font/resource revisions,
region chain, checkpoint policy, deterministic seed, toolchain, allocator, and
reference machine.

Events include:

| Event | Purpose |
| --- | --- |
| `flow` | request a semantic interval or viewport with work/cancellation budget |
| `edit` | replace text, structure, style, object, footnote, table, or counter input |
| `resource_update` | change font, text data, object metrics, or region geometry |
| `checkpoint_roundtrip` | encode, decode, and resume from a named checkpoint |
| `cancel` | cancel a generation at a deterministic work count |
| `publish` | attempt atomic publication for a request generation |
| `measure` | replace an estimated virtual segment with measured layout |
| `navigate` | request a random semantic position or virtual coordinate |
| `accessibility_query` | request semantic child, range, or geometry knowledge |

Observations contain emitted fragments, source mapping, checkpoint digest and
size, restart point, visited blocks, realized range, successor convergence
point, virtual-extent map, correction report, accessibility knowledge state,
allocation counters, and elapsed distributions.

## Required traces

### Long book

Generate 100,000 blocks across chained pages and columns. Include floats,
footnotes, keeps, counters, running headers, nested flow, and fragmented tables.
Edit near block 100, in the middle, and near the end. Prove predecessor restart,
forward convergence, page-number correctness, and unchanged-prefix retention.

Mutations separately change:

- one glyph without changing metrics;
- one paragraph's height;
- a float's wrap geometry;
- a footnote reference and body;
- a spanning table cell;
- a counter reset;
- a font and a region-chain revision.

### Million-line editor

Generate one million logical lines with folded regions, variable line heights,
inline objects, diagnostics, and sparse measured islands. Exercise first open,
random jumps, page-up/down, edits above and inside the viewport, massive paste,
fold toggles, and deletion of the host-selected scroll anchor.

No trace setup may flow the entire document merely to obtain an extent.

### Adversarial continuation

Force float displacement, footnote deferral, table fragmentation, keep chains,
width-dependent objects, and nested-flow limits. Cancel at every checkpointable
state in a small deterministic case. A cancelled generation publishes neither
fragments, checkpoints, nor measured extents.

## Virtual extent contract

The virtual map is an ordered partition of semantic block intervals. Each entry
contains:

- source revision and semantic interval;
- `Estimated` or `Measured` knowledge;
- lower, current, and upper block-extent estimates;
- the estimator/policy id and evidence sample revision;
- measured fragment range when realized;
- cumulative-prefix digest used for coordinate lookup.

Estimated entries never masquerade as exact geometry. Prefix aggregation
supports coordinate-to-semantic search without realizing every predecessor.

When measurement changes an interval, Underwood reports:

- old and new interval extents;
- old and new cumulative prefix at the interval boundaries;
- the affected semantic interval;
- source and measurement revisions;
- exact correction delta above, through, and below the changed interval;
- whether the host's named semantic anchor survived.

The host chooses whether to preserve a caret, semantic block, viewport fraction,
or raw coordinate. If its anchor was deleted, Underwood reports that fact and
candidate neighboring semantic positions; it does not choose UX policy.

The accessibility tree remains semantically queryable independently of visual
realization. Geometry answers are tagged `Unavailable`, `Estimated`, or
`Measured`. A host may request prioritized realization under a budget; Underwood
must never fabricate precise bounds for unrealized content.

## Measurements and proposed gates

| Pressure | Proposed gate |
| --- | --- |
| Serialized checkpoint round trip | byte-stable canonical re-encode and identical successor digest |
| Checkpoint memory | at most 2% of realized flow memory or eight bytes per source block, whichever is larger |
| Adaptive restart distance | p95 at most 128 blocks and maximum 1,024 blocks in admitted corpora |
| Unchanged prefix after edit | zero re-emitted fragments before selected predecessor checkpoint |
| Technical-editor first viewport | under 50 ms p95; realize at most four viewport-heights |
| Random million-line navigation | under 50 ms p95 with no full-prefix flow |
| Edit-to-corrected viewport | under 16 ms p95 when changed flow converges within 128 blocks |
| Long-book background convergence | bounded by affected frontier; no unchanged suffix traversal after matching checkpoint |
| Cancellation | publication count remains zero after the deterministic cancellation point |
| Extent honesty | measured value lies within the prior reported lower/upper interval or emits an estimator-violation diagnostic |
| Determinism | exact structural/state digests; canonical geometry differs by at most 1/1024 layout unit before digesting |

The first experiment must report memory versus restart-distance curves for page
boundaries, fixed intervals of 16/64/256 blocks, and deterministic adaptive
placement. A candidate is not selected from one favorable density.

## Recommendation

Adopt **region-boundary plus deterministic adaptive block checkpoints** as the
prototype policy. Region boundaries are mandatory. Adaptive checkpoints are
added when the last checkpoint's accumulated measured resume work crosses a
policy threshold, with a hard maximum of 1,024 blocks.

The adaptive decision may depend only on stable structural features and
recorded work units, never wall-clock timing. This makes checkpoint placement
replayable while allowing expensive tables, nested flows, or international
paragraphs to receive denser coverage.

The recommendation authorizes no representation. The checkpoint schema and
gates must first be ratified, then exercised in a separate wind-tunnel crate.

## Open semantic questions

- Exact checkpoint versioning and compatibility policy.
- Whether the complete candidate field inventory is accepted.
- Whether 1/1024 layout unit is the correct canonical geometry tolerance.
- Whether the proposed memory, latency, and restart-distance gates are
  accepted.
- Which reference machine and allocator define the first wall-clock evidence.

## Decision

Accepted on 2026-07-21.

Underwood adopts mandatory region-boundary checkpoints plus deterministic
adaptive block checkpoints, the complete candidate state inventory, strict
predecessor validity and successor convergence laws, honest virtual extents,
host-owned anchoring policy, and the proposed experiment gates.

Private trace and representation experiments are authorized. The serialized
encoding and public flow API remain unchosen; adopting either requires evidence
from the accepted wind tunnel and its normal review gate.

## Migration

The checkpoint representation is private but its deterministic behavior,
serialization version, diagnostics, and correction contract are observable.
Changes require a migration statement.

## Proof impact

This decision gates `layout-scene`, technical-editor virtualization,
multilingual compositor pagination, and the first semantic-to-scene campaign.
