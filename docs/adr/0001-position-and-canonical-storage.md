# ADR-0001: Position and canonical storage contract

- **Status:** Open — investigation and human ratification required
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

## Required evidence

- Million-span syntax and diagnostic iteration/mapping trace.
- Dense authored-style mutation and snapshot-sharing trace.
- Sparse anchor creation, resolution, retention, and release budgets.
- Million-line localized replace and paragraph iteration trace.
- Append-heavy publication and prefix-retention trace.
- Loro cursors, rich marks, tree moves, shallow history, and compaction trace.
- Canonical-first and Loro-authoritative snapshot-publication prototypes.
- Selective-undo identity trace.
- Small-label and small-document memory comparison.

## Open semantic questions

- Exact anchor bias and edge laws for every primitive edit.
- Anchor behavior when containers are deleted, split, joined, or moved.
- Range coalescing and overlap laws for tracked authored layers.
- Sound criteria for mapping derived ranges versus recomputation.
- EditSummary granularity and fingerprint stability.
- Authority handoff and failure behavior between adapter and canonical state.

## Decision

Pending evidence and human ratification.

## Migration

Any accepted public position or storage contract requires a migration note even
before 1.0. No foundational public type may precede this decision gate.

## Proof impact

This decision gates `document-transactions` and the first
`semantic-to-scene-spine` campaign.
