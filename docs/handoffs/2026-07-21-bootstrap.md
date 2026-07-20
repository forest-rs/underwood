# Underwood bootstrap handoff

- **Date:** 2026-07-21
- **Campaign:** `und-oh0.1`
- **Local repository health:** Green
- **Remote repository health:** Yellow — no GitHub origin or Beads remote exists

## First read

The repository now has an executable constitution without pretending that
foundational document architecture has been decided.

The bootstrap created:

- the concise operational law in `AGENTS.md`;
- the durable execution model in `docs/CONSTITUTION.md`;
- an open product/proof/stewardship charter;
- four mandatory, evidence-driven open ADRs;
- a two-resolution Beads capability graph;
- a machine-readable proof ledger and crate registry;
- a dependency-free tooling-only Cargo workspace;
- `xtask` validators for repository, proof, crate, and Beads policy;
- pinned GitHub Actions, review template, and scheduled audit;
- licenses and ordinary repository metadata.

No production dependency, `unsafe`, product crate, foundational public API,
commit, push, remote mutation, or semantic decision was introduced.

### Example: what happens next

The first semantic-to-scene campaign is already represented by
`und-oh0.10.1`, but it is blocked by:

1. the completed bootstrap; and
2. `und-oh0.12`, the mandatory-decision gate.

The gate cannot close until Charter-000 and ADR-0001 through ADR-0004 have
checked-in decisions, required evidence or authorized experiments, and human
ratification. This makes it structurally difficult to create foundational
public types merely because implementation pressure arrives.

## Second read

### Repository boundaries

The bootstrap workspace contains only `xtask`. The crate registry classifies
it as unpublished tooling and gives it an Alder fence.

When the first core crate is registered, `cargo xtask repo` requires it to:

- declare `no_std=yes`;
- contain `#![no_std]`;
- have no dev-dependencies;
- inherit workspace lints;
- exist behind CI that names genuine `x86_64-unknown-none` and
  `wasm32-unknown-unknown` checks.

This deliberately prevents empty product scaffolding while making the future
policy executable.

### Proof posture

Governance is `executable`: the checked-in validator runs and CI invokes it.
All product capabilities remain honestly `specified`, `gated`, or `dormant`.
Nothing is Measured, Conformant, or Product-proven.

The proof validator requires:

- valid states and literal status vocabulary;
- stable unique capability identifiers;
- an owner for active work;
- existing specification references;
- checked-in evidence for Executable and higher;
- measurement/budget evidence for Measured and higher;
- corpus/conformance/platform evidence for Conformant and higher;
- a product scenario for Product-proven.

### Beads graph

The durable root is `und-oh0`. Its children cover the handover workstreams.
Only bootstrap and mandatory pre-foundation decisions are decomposed to the
next evidence horizon.

Immediate decision-support beads:

- `und-oh0.11.1.1` — prepare Charter-000 for ratification;
- `und-oh0.3.1.1` — design identity and storage evidence traces;
- `und-oh0.5.1.1` — design checkpoint and virtual-extent traces;
- `und-oh0.7.1.1` — audit text-data entry points and budgets;
- `und-oh0.2.1.1` — audit retained Parley seams and upstream path.

Remote-governance bead:

- `und-oh0.11.2` — establish GitHub controls and durable Beads sync.

### Validation completed

The final bootstrap state passed:

```text
cargo fmt --all --check
taplo fmt --check --diff
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo +1.92 check --workspace --all-targets --locked
cargo metadata --locked --no-deps --format-version 1
cargo xtask check
typos
bash .github/copyright.sh
bd lint --status all
bd dep cycles
```

`xtask` has thirteen deterministic unit tests, including adversarial checks for
repository-relative evidence, extensible workspace membership, target-specific
dev-dependencies, semantic Beads gate identity, and header-policy agreement.

### Known limitations and human gates

- The repository has no Git remote. CI, branch protection, merge queue,
  CODEOWNERS, and scheduled runs are prepared but cannot be proven remotely.
- Beads has no Dolt remote. The scrubbed JSONL export is checked in for review,
  but it is not a full database backup.
- `bd doctor` does not currently support embedded mode.
- The sandbox could not set `beads.role` in `.git/config`; ordinary Beads
  operations succeeded. Configure `git config beads.role maintainer` when
  remote repository setup is authorized.
- Charter-000 contains a recommendation, not an institutional commitment.
- Every mandatory ADR remains Open. None may be cited as a settled contract.

## Ordered next steps

### 1. Review and establish remote controls

Review the bootstrap as one constitutional change. After authorization, execute
`und-oh0.11.2`: create the origin, configure Beads Dolt sync and backup, add
real CODEOWNERS handles, apply branch protection, enable merge queue, and prove
a clean-clone bootstrap.

### 2. Ratify the project charter

Execute `und-oh0.11.1.1` at xhigh. Name the initial ownership and stewardship
commitments that cannot be inferred by an agent. Present options and record the
human decision in Charter-000 and `und-oh0.11.1`.

### 3. Run the four decision investigations

The investigations may collect evidence independently, but the decisions are
reviewed as one coherent foundation:

1. Audit Parley seams and upstream path (`und-oh0.2.1.1`).
2. Audit text-data entry points and budgets (`und-oh0.7.1.1`).
3. Design position/storage traces and prototype thresholds
   (`und-oh0.3.1.1`).
4. Design checkpoint/virtual-extent traces and prototype thresholds
   (`und-oh0.5.1.1`).

Use xhigh reasoning for seams, representations, identity, formats, and decision
criteria. Medium is appropriate for bounded inventories, fixtures, harness
implementation, and measurements once their beads are medium-ready.

### 4. Ratify the mandatory gate

Write the decision, rationale, alternatives, consequences, migration, and proof
impact into each ADR. Record explicit human ratification. Close decision beads
only when acceptance is satisfied; then close `und-oh0.12`.

### 5. Decompose the first product campaign

At xhigh, turn `und-oh0.10.1` into the first high-resolution execution graph.
Create only the final crates required by the ratified semantic-to-scene path,
with real consumers, tests, instrumentation, and wind-tunnel pressure.

### 6. Execute in permanent slices

Run medium-ready beads rapidly. Stop and escalate when evidence invalidates a
fence or reaches a human gate. End every evidence horizon with an xhigh
integration and proof review before decomposing the next horizon.

## Glossary

- **Bootstrap:** executable governance only; not product capability.
- **Decision support:** investigation that prepares but does not ratify a
  permanent choice.
- **Evidence horizon:** the furthest point that can be planned honestly before
  new prototypes or measurements must inform the graph.
- **Remote green:** repository controls and checks proven on the authoritative
  GitHub and Beads remotes.
- **Two-resolution graph:** coarse complete destination plus detailed current
  campaign.
