# ADR-0002: Resumable flow and virtual extent contract

- **Status:** Open — investigation and human ratification required
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

## Required evidence

- Long-book edit near the beginning with downstream page convergence.
- Million-line sparse viewport measurement and random navigation.
- Float, footnote, keep, counter, and fragmented-table carry-state traces.
- Checkpoint serialization round trip and version mismatch behavior.
- Estimate-to-measure correction above, across, and below a viewport.
- Accessibility queries for unrealized, estimated, and realized content.
- Memory versus resume-distance measurements for candidate densities.

## Open semantic questions

- Exact checkpoint versioning and compatibility policy.
- Minimum state for each flow feature without hidden provider state.
- Deterministic adaptive density inputs.
- Successor equivalence and numerical tolerance.
- Cancellation and stale-result publication.
- Correction behavior when the chosen anchor is deleted.

## Decision

Pending evidence and human ratification.

## Migration

The checkpoint representation is private but its deterministic behavior,
serialization version, diagnostics, and correction contract are observable.
Changes require a migration statement.

## Proof impact

This decision gates `layout-scene`, technical-editor virtualization,
multilingual compositor pagination, and the first semantic-to-scene campaign.
