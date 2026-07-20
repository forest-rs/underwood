# ADR-0004: Parley boundary and contingency

- **Status:** Open — investigation and human ratification required
- **Bead:** `und-oh0.2.1`
- **Authority:** `UNDERWOOD_HANDOVER.md` §§15.1–15.6, 35.1

## Goal

Define the retained preparation contracts Underwood requires from Parley and
the narrow contingency that preserves product sequencing without creating a
second text engine.

## Non-goals

- Owning Unicode analysis, bidi, shaping, or font fallback in Underwood.
- Freezing Underwood's public façade to current Parley types.
- Treating upstream review latency as evidence against upstreaming.

## Fence

`underwood_parley` owns adaptation between Underwood prepared contracts and a
pinned Parley revision; it explicitly does not own semantic documents,
document flow, renderer policy, or a general-purpose fork of text physics.

## Constitutional invariants

1. Analysis, itemization, shaping, used values, and breaking have distinct
   retained identities.
2. Font weight never invalidates Unicode analysis.
3. Paint values never enter shaping identity.
4. Underwood prepared types shield the public façade from Parley churn.
5. Generally applicable changes are proposed upstream.
6. Every temporary divergence has an owner, conformance evidence, removal
   review, and upstreaming plan.
7. Underwood may build a breaker over retained `parley_core` primitives but
   cannot casually duplicate Unicode or shaping machinery.

## Options

### Upstream-only stable releases

Simplest maintenance story, but may block the product indefinitely on missing
retained seams.

### Pinned upstream revision with narrow patch stack

Allows coordinated upstream work and bounded local sequencing. It requires
strict divergence ownership and dual conformance.

### Broad maintained fork

Offers maximum control but creates a second text-engine institution. Rejected
unless a future constitutional decision demonstrates that upstream alignment
is no longer viable.

## Required evidence

- Inventory of required analysis, itemization, shaping, break, vertical, and
  inline-object seams at the pinned revision.
- Conformance cases that execute through public Underwood prepared types.
- Paint-boundary and ligature behavior traces.
- Tracking-topology and font-scale experiments.
- Bounded break reshaping and arbitrary-interval flow prototype.
- Upstream issue/PR map and review-threshold policy.
- Patch ownership, review cadence, and removal mechanics.

## Open semantic questions

- Exact Underwood prepared input/output types.
- Pinned revision and compatibility window.
- Evidence threshold and time/review conditions for carrying a patch.
- Dual-backend conformance requirements.
- Which breaker responsibilities belong upstream versus document flow.

## Decision

Pending evidence, upstream discussion, and human ratification.

## Migration

Parley types must not leak into the stable Underwood façade. Adapter changes
still require migration notes for lower-level crate consumers.

## Proof impact

This decision gates `parley-alignment`, retained preparation, and the first
semantic-to-scene campaign.
