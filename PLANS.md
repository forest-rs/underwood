# Underwood execution plans

## Foundation decision-support campaign

**Status:** Complete — all five records ratified 2026-07-21

**Beads:** `und-oh0.11.1.1`, `und-oh0.2.1.1`, `und-oh0.7.1.1`,
`und-oh0.5.1.1`, `und-oh0.3.1.1`

### Goal

Make Charter-000 and ADR-0001 through ADR-0004 decision-ready using explicit
local evidence, executable trace designs, alternatives, thresholds, and
unresolved human choices.

### Fence

This campaign owns evidence and recommendations for the mandatory foundation
decisions; it explicitly does not ratify those decisions, create product
crates, establish foundational public APIs, or select permanent representations
on the human's behalf.

### Non-goals

- No production dependencies, dependency pins, or feature changes.
- No `unsafe`.
- No public product APIs or prototype implementations.
- No Parley fork, patch, or upstream communication without explicit authority.
- No GitHub repository creation, remote mutation, commit, or push.

### Steps

1. Audit the checked-out Parley revision and map retained-preparation seams,
   data entry points, conformance needs, and evidence-backed gaps.
2. Specify canonical-storage and position traces for the three credible
   authority models.
3. Specify resumable-flow traces, checkpoint laws, virtual-extent corrections,
   and prototype decision thresholds.
4. Turn Charter-000 into a ratification packet with explicit proposed answers,
   alternatives, owners, and unresolved commitments.
5. Run repository, text, Beads, and dependency-cycle validation; export the
   scrubbed Beads graph; leave a durable handoff.

### Risks and controls

- **Local source drift:** record exact source revisions and distinguish observed
  facts from proposed contracts.
- **Paper architecture:** require each recommendation to name a trace,
  measurement, conformance law, or upstream seam.
- **Premature permanence:** keep representations private and decisions open
  until explicit human ratification.
- **Scope expansion:** stop before adding dependencies, crates, public APIs,
  `unsafe`, remotes, or external messages.

### Completion

The campaign is complete when all five records are ready for a human decision,
their support beads contain validation and unresolved-choice notes, and the
repository is green. The records remain Open until explicitly ratified.

All five records are Accepted. Their private proof obligations continue as
`und-oh0.10.1.1` through `und-oh0.10.1.4`; the first permanent-slice design is
`und-oh0.10.1.5`.

## First semantic-to-scene campaign

**Status:** Active

**Beads:** `und-oh0.10.1`, `und-oh0.10.1.1`, `und-oh0.10.1.2`,
`und-oh0.10.1.3`, `und-oh0.10.1.4`, `und-oh0.10.1.5`

### Goal

Carry one permanent, headless path from an immutable semantic document through
retained Parley preparation into renderer-neutral scene geometry, paint slots,
and semantic mapping. The path must be useful to the living agent document and
must prove local-edit and paint-only reuse rather than merely rendering a
string.

### Fence

This campaign owns the first public vertical slice and the four private proof
obligations selected by the accepted ADRs. It does not stabilize the complete
document model, create a second shaping engine, promise general flow from a
single rectangle, or treat a wind-tunnel representation as a public contract.

### Non-goals

- No `unsafe`.
- No production dependency before its explicit gate.
- No Loro dependency before the collaboration-authority experiment gate.
- No speculative split into every crate named by the five-year topology.
- No public stable position type before the identity traces earn it.
- No claim above the evidence recorded in the proof ledger.

### Steps

1. Ratify the first-slice packet in
   `docs/design/0001-first-semantic-to-scene-slice.md`.
2. Execute the dependency-free canonical baseline of `identity-trace-v0` in a
   separate position wind-tunnel crate; record failures as well as passes.
3. Add the initial production crate boundary and draft public path only after
   the packet's crate and API gate.
4. Add the exact Parley pin and adapter only after its production-dependency
   gate and refreshed upstream audit.
5. Prove immutable snapshot publication, paragraph-local edit invalidation,
   paint-only reuse, semantic-to-geometry mapping, and deterministic headless
   scene output.
6. Measure position/storage, resumable flow, text-data footprint, and retained
   Parley seams against their accepted gates before broadening the façade.

### Risks and controls

- **Surface outruns hands:** begin with one production façade and one adapter
  fence; split only when dependency or ownership pressure is real.
- **Convincing but fake scene:** the first proof includes shaped glyph identity,
  source mapping, hit/caret evidence, and reuse counters; placeholder glyphs do
  not qualify.
- **Benchmark theater:** deterministic work counters are primary; wall time
  names machine, allocator, samples, and confidence.
- **Upstream drift:** pin an immutable Parley revision and rerun the seam audit
  before adding it.
- **Premature API permanence:** draft APIs carry a migration note and remain
  pre-stable until the first product trace exercises them.

### Completion

The campaign reaches Executable when the public vertical path runs headlessly
with no private product shortcut and its repository checks pass. It reaches
Measured only when all four accepted experiment beads contain checked-in
evidence. Higher proof stages remain unavailable until their named corpora and
owners exist.

## CPU visual-proof slice

**Status:** Active

**Bead:** `und-oh0.10.1.8`

### Goal

Turn the executable semantic-to-scene spine into a compelling, inspectable
image. A downstream example must lower the real `TextScene` through `imaging`,
render it with `imaging_vello_cpu`, and retain a deterministic poster snapshot
that makes the hard text and invalidation evidence visible.

### Fence

This slice owns an external renderer adapter, visual composition, PNG output,
and snapshot verification. It does not move rendering into `underwood` or
`underwood_parley`, add production dependencies, broaden the draft public API,
or substitute decoration for real shaped output.

### Steps

1. Add one unpublished top-level example crate with released `imaging` and
   `imaging_vello_cpu` dependencies and an explicit governance fence.
2. Lower public scene fragments into clipped imaging glyph runs, preserving
   font instance data, glyph positions, transforms, and paint brushes.
3. Compose a poster from real Latin ligature, Arabic RTL, source, hit/caret,
   line, semantic, edit, reuse, and paint-only evidence.
4. Render with Vello CPU, inspect the output, and iterate until the composition
   is legible and compelling rather than merely technically non-empty.
5. Commit the accepted PNG, exact pixel snapshot test, and evidence notes; run
   the full stable, MSRV, policy, text, and Beads gates.

### Risks and controls

- **Pretty mirage:** all text, proof values, and diagnostics derive from real
  Underwood output; imaging-only primitives are limited to presentation.
- **Core contamination:** renderer and PNG dependencies stay in the external
  example crate, and the no-std production targets remain unchanged.
- **Snapshot gremlins:** use the CPU backend's stable render mode and require an
  exact RGBA match across the repository's Linux, macOS, and Windows CI jobs.
- **Font drift:** reuse the checked-in licensed font bytes rather than host
  font discovery.

### Completion

The slice is complete when a human-inspectable PNG is checked in, its pixels
are regenerated exclusively through the public Underwood path, the visual
evidence assertions and exact snapshot test pass, and the repository is green.
