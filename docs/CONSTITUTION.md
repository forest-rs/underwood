# The Underwood Constitution

- **Status:** executable bootstrap constitution
- **Authority:** `UNDERWOOD_HANDOVER.md`, ratification decisions, and checked-in
  evidence
- **Bead:** `und-oh0.1.1`

## Overview

Underwood exists to build one coherent semantic document, editing,
composition, and prepared-scene platform from labels through living,
collaborative application documents.

The constitution's goal is to keep implementation pressure aligned with that
destination. It makes admission, sequencing, evidence, and completion rules
explicit and testable.

Its non-goals are:

- choosing document representations before the mandatory ADRs;
- replacing engineering judgment with automation;
- expanding the active work surface to mirror the whole handover;
- treating repository machinery as product progress;
- promising compatibility before conformance earns it.

## Fence and invariants

The constitution owns how work is admitted, executed, proven, and declared
complete; it explicitly does not choose subsystem implementations that belong
to ratified ADRs and capability owners.

The following invariants are constitutional:

1. The handover remains the complete destination until explicitly amended.
2. Only one cross-cutting capability campaign receives active product pressure.
3. Every landed implementation slice belongs to the final architecture.
4. Product and wind-tunnel clients use public crate contracts.
5. No proof status advances through assertion.
6. Objective rules are automated; judgment-heavy promotions remain explicit
   reviews.
7. Human gates stop permanent choices without stopping independent safe work.
8. Inactive capability fronts are named `dormant` or `gated`, not implied to be
   underway.

## Alternatives considered

### Prose-only discipline

This is lightweight, but memory and enthusiasm eventually reinterpret it.
Rejected because Underwood's surface is too broad for unwritten enforcement.

### Central approval for every change

This can protect coherence but turns ordinary work into ceremony and creates a
single bottleneck. Rejected because objective rules should execute
automatically and bounded work should proceed autonomously.

### Executable constitution

Chosen. Prose states intent, Beads orders work, Cargo encodes package policy,
`xtask` validates repository claims, CI blocks objective drift, and the proof
ledger constrains status language.

The tradeoff is maintenance: governance artifacts can themselves drift. A
scheduled audit and small dependency-free validator keep that cost visible.

## Onslaught execution model

Onslaught means concentrated, durable pressure rather than simultaneous
activity.

```text
xhigh campaign design
        |
        v
human decision gates
        |
        v
medium-ready execution beads
        |
        v
xhigh integration review
        |
        v
proof gate and next evidence horizon
```

Long-term capability epics remain coarse. Only the current campaign is
decomposed to implementation resolution. Planning stops at the next evidence
boundary because prototypes and traces must inform later decisions.

## Admission and completion

Work is admitted when:

- an owning bead exists;
- the ownership fence is clear;
- acceptance is observable;
- blockers and human gates are represented;
- proof impact is stated;
- the change does not secretly expand the active campaign.

Work completes when:

- acceptance is satisfied;
- code, tests, docs, and diagnostics agree;
- all applicable local checks pass;
- durable decisions and migrations are recorded;
- evidence is checked in;
- the ledger remains honest;
- the Beads export records the final state.

## Minimal repository skeleton

The bootstrap contains only:

- constitutional and governance documents;
- the architectural handover;
- Beads planning data;
- the root Cargo workspace;
- a dependency-free, tooling-only `xtask`;
- CI and review scaffolding.

No foundational product crate is created before the mandatory decision gate.
When that gate closes, crates enter only as real dependency fences with a
public consumer, tests, documentation, and registry entry.

## Extension points

The governance system can grow through:

- new proof validators tied to real evidence types;
- crate-specific fence checks;
- conformance corpus manifests;
- benchmark budget manifests;
- cross-repository compatibility fixtures;
- platform evidence records;
- release-readiness validation.

Each addition needs a demonstrated failure mode. The constitution is not a
general policy plugin system.

## Example

A proposed stable-anchor implementation does not begin as an untracked code
experiment. `ADR-0001` owns the semantic choice. Its bead names the competing
representations and required million-span and Loro traces. Prototype beads may
execute behind private experiment boundaries. The human gate ratifies the
contract. Only then is the public implementation campaign decomposed.

## Gotchas and risks

- A large Beads graph can create planning theatre. Keep future fronts coarse.
- A green CI run proves only encoded rules. It cannot prove architectural fit.
- A public demo can hide private shortcuts. Wind tunnels must use public APIs.
- Proof labels can become marketing. Promotions require checked-in evidence.
- Exceptions can accumulate. Every exception needs an owner and removal
  condition.
- Governance can consume the project. Prefer the smallest mechanism that
  blocks a demonstrated form of drift.

## Glossary

- **Campaign:** the one active cross-cutting capability frontier.
- **Fence:** one sentence naming what a module owns and explicitly excludes.
- **Human gate:** a permanent decision that agents may prepare but not silently
  ratify.
- **Onslaught:** concentrated execution from deep design through honest proof.
- **Proof status:** one literal stage from Specified to Product-proven.
- **Wind tunnel:** a demanding public-API workload that attacks architectural
  assumptions.
