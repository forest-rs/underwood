# Design-0001: First permanent semantic-to-scene slice

- **Status:** Accepted — dependency and public-API gates remain
- **Accepted:** 2026-07-21 by Bruce Mitchener
- **Bead:** `und-oh0.10.1.5`
- **Campaign:** `und-oh0.10.1`
- **Authority:** Charter-000 and ADR-0001 through ADR-0004

## Outcome

The first permanent slice accepts an immutable semantic document containing a
root, stable paragraph identities, inline text leaves, and semantic roles. It
projects one changed paragraph, retains Parley-owned analysis, itemization, and
shaping, performs finite-width single-region line breaking, and emits a
renderer-neutral scene with:

- stable fragment identity;
- glyph and line geometry;
- paint slots separate from shaped identity;
- source ranges from projected text back to semantic content;
- hit and caret observations sufficient for headless tests;
- semantic fragments that join document meaning to geometry.

The slice is complete only when a paragraph edit invalidates that paragraph
without re-analyzing its siblings and a paint-only change reuses analysis,
shaping, and flow. A string-to-placeholder-glyph demo is not this slice.

## Invariants

1. Semantic identity precedes presentation identity.
2. Published document and scene snapshots are immutable.
3. Stable document identity is not a byte offset.
4. Public position types remain absent until `identity-trace-v0` earns them.
5. Analysis, itemization, shaping, breaking, geometry, paint, and semantics
   expose distinct private identities and reuse counters.
6. Parley owns Unicode analysis, bidi, itemization, font selection, and
   shaping; Underwood does not implement a fallback shaper.
7. Paint never enters analysis or shaping identity.
8. Scene data contains no renderer, GPU atlas, AccessKit, or Overstory type.
9. The first finite-width breaker is a private capability subset, not a claim
   that general region flow is solved.
10. Every externally visible draft item has rustdoc and an explicit migration
    posture.

## Chosen crate shape

Begin with two permanent production crates, but create the second only when its
dependency is approved:

```text
underwood/             calm façade plus private document, projection, retained
                       preparation, finite-flow, and scene modules
underwood_parley/      the only production adaptation seam to pinned Parley
experiments/position/  private canonical-storage and identity experiment
```

`underwood` is a foundational `#![no_std]` crate using `alloc`; it is
unpublished while the first product trace is changing the draft API.
`underwood_parley` depends inward on Underwood-owned prepared contracts and
outward on Parley. No Parley type crosses the façade. The wind tunnel is a
`std`, unpublished experiment crate and may not be imported by production code.

Document, style, layout, and scene begin as explicit private modules inside
`underwood`. They become crates only when one of these facts exists:

- an independently useful public contract has a maintainer;
- a dependency must be prevented from entering another layer;
- target or feature policy differs;
- compile-time or release evidence justifies the split.

Moving a private module into a crate before 1.0 is an internal migration.
Splitting public packages speculatively is not.

## Credible alternatives

### Full topology immediately

Create `underwood_core`, `underwood_document`, `underwood_layout`,
`underwood_scene`, `underwood_parley`, and the façade before the first path.
This maximizes static fences but creates six review, version, documentation,
and dependency surfaces with only one current owner. It is rejected for the
first slice under the surface-to-hands constraint.

### One crate including Parley adaptation

Put all code and the Parley dependency in `underwood`. This minimizes package
count but lets upstream types, features, and churn pressure the foundational
crate. It is rejected because the Parley seam is a genuine dependency and
contingency fence.

### Core identities first, vertical path later

Publish IDs, revisions, anchors, and ranges before preparation exists. This
looks foundational but lets attractive types become permanent before the
product and identity traces test them. It is rejected as API-first rather than
capability-first.

## Private experiment boundary

`experiments/position` owns candidates, trace generation, observations, work
counters, and machine-specific timing for ADR-0001. Candidate types are
crate-private. Its first dependency-free baseline must:

- encode the ratified insert/delete anchor-bias laws;
- reject a derived range at the wrong revision;
- transform authored ranges without resolving sparse anchors;
- demonstrate immutable snapshot cloning without source-byte copying;
- emit an honest gate report, including failures.

Loro, allocator instrumentation, or another wind-tunnel-only dependency needs
its own explicit gate. No candidate becomes production code by winning; its
representation is re-reviewed against the permanent crate fence.

## Dependency plan

The dependency-free wind tunnel may start immediately. The proposed first
production dependency is Parley at audited commit
`45da4a90248b1600277a4294b70d8bfde5ca8e97`, or a newer immutable commit chosen
after refreshing ADR-0004's seam table. Approval must name:

- the exact immutable revision;
- selected `default-features = false` feature set;
- license/source review result;
- the owned shaped-output strategy if PR #679 is not merged;
- the conformance cases that can run on the unpatched revision.

No local patch, Loro, geometry, hashing, serialization, or benchmark dependency
is silently bundled into the Parley approval.
[Design-0002](0002-first-public-api-gate.md) separately names Kurbo and Peniko
because a real scene needs the forest's shared geometry and paint vocabulary.
Those rows remain independent production-dependency approvals even when the
human reviews the coherent end-to-end patch as one packet.

## Draft public path

The implementation review should expose the smallest path that an independent
headless client can use:

1. construct or parse the first supported semantic fragment;
2. publish an immutable document snapshot;
3. prepare it with explicit text data, fonts, and finite-width flow inputs;
4. receive an immutable scene;
5. inspect geometry, paint slots, source mapping, and semantic fragments.

The packet deliberately does not freeze Rust names. The first API patch must
show complete call sites, rustdoc, ownership/lifetime behavior, error types,
and a migration note before approval. It may not expose a generic storage
backend, Parley context, renderer, async runtime, or universal property bag.

## Proof targets

| Claim | Required first evidence |
| --- | --- |
| Semantic input is real | root/paragraph/text structure and role survive into the semantic map |
| Snapshot publication is real | old snapshot remains byte-for-byte observable after a new publication |
| Retention is real | stage keys and counters show sibling paragraph and paint-only reuse |
| Shaping is real | Parley glyph ids, advances, clusters, bidi/source coverage, and deterministic digest |
| Scene is real | geometry, paint, hit/caret, and semantic observations agree on source coverage |
| Headless adoption is real | an external test crate uses only public Underwood paths |
| `no_std` is real | `x86_64-unknown-none` and `wasm32-unknown-unknown` checks run in CI |

The proof ledger may promote the spine to Executable after the external
headless path passes. Measurements remain owned by the four experiment beads.

## Migration posture

Both production crates begin unpublished and pre-stable. Every intentional
draft-API change receives a short migration note in the pull request and
`CHANGELOG.md`; callers are updated atomically. Public names are not declared
stable merely because rustdoc exists. Package publication, 0.x compatibility,
and any durable scene or storage format require later decisions.

## Exact human gates

Implementation of the permanent slice is governed by these explicit approvals:

| Gate | Status | Effect |
| --- | --- | --- |
| One `underwood` façade plus the `underwood_parley` dependency fence | **Approved 2026-07-21** | The initial ownership shape is selected |
| Create the dependency-free `underwood` workspace crate | **Approved 2026-07-21** | The real `no_std` package boundary and portability proof may land |
| Exact production Parley revision and features | **Approved 2026-07-21** | Design-0002 pins the adapter dependency fence |
| Exact Kurbo and Peniko versions/features | **Approved 2026-07-21** | Design-0002 pins the core geometry and paint vocabulary |
| First draft public API with complete call sites and migration note | **Approved 2026-07-21** | Design-0002 authorizes the coherent implementation slice |

Bruce separately approved [Design-0002](0002-first-public-api-gate.md) on
2026-07-21, crossing the remaining dependency and draft-API gates without
promoting the API to stable or approving any wire identity.

## Decision

Accepted on 2026-07-21.

Underwood begins with one small, unpublished, `no_std + alloc` production crate
named `underwood`. The Parley adapter is a separate `no_std + alloc` production
crate built against the exact revision and features approved by Design-0002.
The first public API is pre-stable and is exercised atomically by the external
headless example.
