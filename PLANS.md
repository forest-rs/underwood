# Underwood execution plans

## Foundation decision-support campaign

**Status:** Active

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
