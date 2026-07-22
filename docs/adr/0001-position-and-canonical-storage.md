# ADR-0001: Position and canonical storage contract

- **Status:** Accepted
- **Accepted:** 2026-07-21 by Bruce Mitchener
- **Bead:** `und-oh0.3.1`
- **Authority:** `UNDERWOOD_HANDOVER.md` §§8.3–8.4, 11.5–11.6, 35.1

## Goal

Define the semantic and cost contracts for durable positions, persistent dense
ranges, derived dense ranges, canonical text storage, and collaboration
authority before these identities enter public APIs.

## Non-goals

- Selecting the complete document tree representation.
- Standardizing a generalized collaboration trait.
- Optimizing one benchmark before the competing traces exist.

## Fence

The document foundation owns position meaning, edit mapping, and preparation
snapshots; it explicitly does not expose an open storage backend through the
document façade.

## Constitutional invariants

1. Sparse human-meaningful positions survive edits with explicit bias.
2. Dense authored ranges transform in bulk without per-span anchor resolution.
3. Dense derived ranges are revision-bound and reject stale reuse.
4. A concrete `Document` façade protects preparation hot paths.
5. Collaboration identity is tested against a maintained Loro implementation.
6. Representation is chosen by product traces, not ideology.

## Evidence protocol

The experiment uses one semantic trace model against three private candidate
implementations. Its encoding is test-only and versioned `identity-trace-v0`;
the record semantics below are the contract under review, not a stable public
file format.

Every trace begins with:

- trace schema version and deterministic seed;
- corpus id and content digest;
- authority mode: `canonical`, `loro`, or `sealed-hybrid`;
- implementation source revision;
- text-data, schema, and font identities when preparation is observed;
- allocator, target, toolchain, and reference-machine id for measurements.

The event vocabulary is:

| Event | Required fields |
| --- | --- |
| `create` | document id, initial structure, UTF-8 content, authored layers |
| `transaction` | transaction id, base revision, ordered primitive edits |
| `replace_text` | container, stable start/end or initial offsets, replacement bytes |
| `replace_spans` | layer id, affected range, ordered authored spans and edge laws |
| `anchor_create` | logical name, container, position, before/after bias |
| `anchor_resolve` | logical name, expected state and resolved position |
| `tree_splice` | parent, child interval, inserted/moved identities |
| `remote_apply` | peer, causal/version identity, operation or delta |
| `publish` | expected semantic frontier and changed preparation frontier |
| `selective_undo` | peer, target transaction, expected outcome class |
| `compact` | history frontier, retention set, expected surviving identities |
| `observe` | snapshot, layers, edit summary, preparation, and cost counters |

Each `observe` event emits:

- document and component revisions;
- canonical semantic digest over structure, text, authored layers, and resource
  references;
- ordered resolutions of all live anchors;
- ordered dense authored ranges and their edge behavior;
- stale/success result for every derived snapshot range;
- normalized `EditSummary`;
- preparation snapshot digest where the candidate supports publication;
- allocated, retained, copied, visited, and resolved record counts;
- elapsed time as secondary machine-specific evidence.

The semantic digest deliberately excludes internal node shape, allocator
addresses, CRDT tombstones, chunking, and cache contents. Candidate-specific
diagnostics are retained beside it.

## Proposed anchor and range cases

The traces must make the following proposed laws explicit. Ratification may
change a law, but no prototype may choose silently.

| Operation at a boundary | `Before` anchor | `After` anchor |
| --- | --- | --- |
| Insert at anchor | remains before inserted content | remains after inserted content |
| Delete containing anchor | collapses to deletion start, before future inserts | collapses to deletion start, after future inserts |
| Split text container | follows the referenced logical side and preserves bias | follows the referenced logical side and preserves bias |
| Join adjacent containers | maps through the join with no ordering inversion | maps through the join with no ordering inversion |
| Move containing node | moves with the node identity | moves with the node identity |
| Delete containing node | becomes explicitly unresolved/tombstoned | becomes explicitly unresolved/tombstoned |

An unresolved durable anchor is not silently coerced into an unrelated nearby
container. A separate policy operation may choose a recovery position.

Dense authored ranges use declared start/end edge behavior, transform once per
`EditSummary`, remain ordered and coalesced according to layer policy, and
never resolve millions of sparse anchors. Dense derived ranges either map
through a sound summary or reject the revision mismatch.

## Options

### Canonical-first

Underwood owns text, tree, and range state. Loro mirrors transactions and
returns merged operations.

This protects a compact no_std core and simple label economics, but risks two
sources of truth and lossy collaboration identity mapping.

### Loro-authoritative

Loro owns collaborative text, tree, marks, and cursor identity. Underwood
publishes immutable optimized preparation snapshots.

This gives collaboration native identity, but raises no_std, memory,
small-document, snapshot-publication, and adapter-boundary concerns.

### Sealed hybrid

Small and non-collaborative documents use the compact canonical store while
collaborative documents use a sealed CRDT-aware backend behind the concrete
façade.

This may fit the workloads best, but creates two implementations whose laws and
preparation output must remain equivalent.

## Corpus matrix

| Corpus | Scale and pressure | Required candidates |
| --- | --- | --- |
| `label-64` | 64 UTF-8 bytes, eight paint/style spans, four anchors, repeated snapshots | all |
| `form-10k` | 10 KiB, 256 authored spans, 64 anchors, IME-shaped replacements | all |
| `dense-million` | one million authored or derived spans with localized edits and ordered scans | canonical, hybrid canonical path |
| `editor-million` | one million lines, sparse viewport reads, localized replaces, syntax and diagnostics | all |
| `append-gib` | one GiB logical stream in 64 KiB transactions, bounded retained tail | canonical, hybrid canonical path, Loro if supported |
| `collab-rich` | three peers, concurrent inserts/deletes/marks, boundary expansion, comments, presence | all |
| `collab-tree` | movable trees, concurrent move/delete, schema payload changes | all |
| `history` | shallow load, compaction, retained cursors, selective undo and redo | canonical+Loro mirror, Loro, hybrid |

The million-scale corpora may generate content deterministically rather than
check it into Git. Generator revision and seed are part of the trace identity.

## Equivalence and correctness laws

1. Replaying a trace twice under one candidate produces identical semantic
   observations.
2. All correct candidates produce the same semantic and preparation digests at
   declared convergence points.
3. Immutable snapshots never change after publication.
4. A localized edit preserves unchanged prefix/suffix identities and does not
   publish an unrelated full-document frontier.
5. Sparse anchors preserve the ratified bias and lifecycle laws.
6. Authored layers transform in bulk and preserve declared overlap/coalescing
   policy.
7. Derived ranges cannot be read against an unnamed revision.
8. All peers converge after receiving the same operation set regardless of
   delivery order permitted by the adapter.
9. Selective undo removes or compensates only local intent and returns the
   declared applied, partial, no-effect, or conflict outcome.
10. Compaction preserves every retained identity and makes every discarded
    identity fail explicitly.
11. Mirrored and authoritative modes publish equivalent Underwood preparation
    snapshots for the same semantic state.

Semantic failure eliminates a candidate regardless of speed or memory.

## Measurements and proposed experiment gates

The wind tunnel records both wall-clock distributions and representation-neutral
work counters. Wall-clock gates use a checked-in reference-machine description;
work gates remain comparable across machines.

| Pressure | Proposed gate |
| --- | --- |
| `label-64` fixed document heap | at most 4 KiB, excluding shared registries and allocator arena slack reported separately |
| Snapshot clone | no source-byte copy and at most 256 newly allocated bytes |
| Sparse durable anchor | at most 128 retained bytes per live anchor, amortized over 10,000 anchors |
| Dense authored span | at most 48 retained bytes per span at one million spans |
| Dense derived range | at most 32 retained bytes per range at one million ranges |
| One-byte localized edit | p95 transaction-to-snapshot publication under 16 ms on the reference machine |
| Million-line localized edit work | at most 4,096 index/range records plus records intersecting the changed frontier |
| Dense layer transform | no per-span anchor resolution and work proportional to changed/overlapping runs plus logarithmic index work |
| 64 KiB append transaction | p95 publication under 16 ms; unpublished retained tail at most two configured batches |
| Collaborative merge batch | p95 merge-to-preparation publication under 50 ms for 1,000 received operations |
| Snapshot publication | visits changed frontier plus tree-height metadata; no whole-document scan |

The first experiment report must include allocator sensitivity and confidence
intervals. A gate may be revised by ratification when measurement exposes a bad
assumption; it may not be weakened inside implementation code.

## Prototype boundaries

After this ADR is ratified, experiments belong in a separate top-level
wind-tunnel crate. Candidate representations remain private. They expose only
the trace driver, observations, and counters required above.

The prototype may implement:

- a compact persistent UTF-8 canonical store and range index;
- a Loro-backed authoritative projector;
- a canonical-to-Loro mirrored transaction adapter;
- the smallest sealed dispatch necessary to measure hybrid overhead.

It may not create `Document<S>`, stabilize position types, or place Loro types
in Underwood's façade. Adding Loro or any other production dependency remains a
separate human gate; a wind-tunnel-only dependency must still be explicit.

## Decision rule

The recommended decision sequence is:

1. Eliminate any candidate that fails semantic, anchor, replay, or preparation
   equivalence.
2. Choose **canonical-first** if it preserves collaboration identities without
   lossy mappings and meets every scale gate.
3. Choose **Loro-authoritative** if canonical mirroring cannot preserve
   identity/undo semantics and Loro publication meets the small-document and
   large-document gates without a full-state copy.
4. Choose **sealed hybrid** only if the canonical path is necessary for
   small/non-collaborative economics and the Loro path is necessary for
   collaboration semantics. Both paths must produce identical public snapshots,
   and sealed dispatch may add no more than 15% to publication latency.
5. If no candidate passes, revise the representation experiment; do not expose
   a backend trait as an escape from the evidence.

This rule is a recommendation for ratification, not the result of an experiment
that has not run.

## Open semantic questions

- Range coalescing and overlap laws for tracked authored layers.
- Sound criteria for mapping derived ranges versus recomputation.
- EditSummary granularity and fingerprint stability.
- Authority handoff and failure behavior between adapter and canonical state.
- Whether the proposed anchor table and quantitative gates are accepted.
- Whether hybrid dispatch's proposed 15% ceiling is the right maintenance and
  runtime tax.
- Which reference machine and allocator define the first wall-clock report.

## Decision

Accepted on 2026-07-21.

Underwood ratifies the three distinct position forms, the proposed anchor laws,
the concrete sealed `Document` façade, the trace protocol, and the experiment
gates. **Canonical-first** is the default storage authority. Loro-authoritative
or sealed-hybrid storage may supersede that default before public stabilization
only when the accepted traces demonstrate that canonical mirroring loses
collaboration semantics or cannot meet the ratified economics.

Private representation experiments are authorized. This decision does not
authorize `Document<S>`, stabilize a public position type, add Loro as a
production dependency, or create a new workspace crate without its normal
review gate.

### Accepted interaction addendum — 2026-07-22

The interactive snapshot slice ratifies the dense, revision-bound selection
shape specified by Design-0009:

- one snapshot position names a document revision, semantic text leaf, UTF-8
  boundary, and affinity;
- one snapshot selection is one insertion point and owns one or more logically
  ordered ranges; visual bidi selection may therefore be logically disjoint;
- one snapshot selection set owns zero or more independent selections, with
  the first member designated as primary; and
- a replacement transaction consumes a current selection set and returns
  collapsed selections for the newly published revision.

The two levels are not interchangeable: several ranges in one visual selection
receive one insertion, while several independent selections each receive an
insertion. These values remain derived snapshot observations, not durable
anchors. Cross-leaf and structural replacement remain future transaction
operations rather than implied behavior of this addendum.

## Migration

Any accepted public position or storage contract requires a migration note even
before 1.0. No foundational public type may precede this decision gate.

## Proof impact

This decision gates `document-transactions` and the first
`semantic-to-scene-spine` campaign.
