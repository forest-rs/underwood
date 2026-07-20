# Governance workflow

This directory explains how the executable constitution operates. `AGENTS.md`
is the concise operational authority; this document gives the second-read
details.

## Sources of truth

| Concern | Owner |
| --- | --- |
| Complete architectural destination | `UNDERWOOD_HANDOVER.md` |
| Admission, execution, and proof law | `docs/CONSTITUTION.md` |
| Work and dependency state | Beads |
| Permanent architectural decisions | `docs/adr/` |
| Product/proof/stewardship ratification | `docs/charter/` |
| Capability claims | `docs/proof/ledger.tsv` |
| Crate classifications and fences | `docs/governance/crates.tsv` |
| Objective enforcement | `xtask` and CI |

## Starting work

```sh
bd prime
bd ready
bd show <id>
bd update <id> --claim
```

Before implementation, verify:

1. The bead belongs to the active campaign or repository maintenance.
2. Its fence, acceptance criteria, and proof target are explicit.
3. All blocking decisions are closed.
4. It does not trigger an unapproved dependency, safety, public API, identity,
   format, ownership, or sequencing choice.

## Recording decisions

Execution notes remain on the bead. Create or update an ADR when a decision
changes architecture, ownership, invariants, stable identity, public semantics,
formats, dependency direction, or interoperability.

An open ADR records alternatives and required evidence without pretending a
choice has been made. A decision bead closes only after human ratification is
present in both the bead and checked-in record.

## Completing work

Run the locally applicable green path:

```sh
cargo fmt --all --check
taplo fmt --check --diff
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo xtask check
typos
bd lint --status all
bd dep cycles
```

Then update the bead with:

- implementation summary;
- decisions and tradeoffs;
- validation commands and results;
- evidence paths;
- proof impact;
- follow-up beads.

Export and close:

```sh
bd close <id> --reason "summary, evidence, and validation"
bd export --scrub --output .beads/issues.jsonl
```

## Proof promotion example

Suppose paragraph projection is already Specified.

1. A public `DocumentSnapshot -> ParagraphProjection` path and integration test
   can support promotion to Executable.
2. Benchmarked source-map overhead and edit-frontier work can support Measured.
3. Password, generated-content, IME, folding, and mapping corpora can support
   Conformant.
4. Sustained use by the living agent document can support Product-proven.

Each promotion is a separate review. Later evidence does not retroactively make
an earlier claim honest.

## Embedded Beads limitation

The bootstrap uses embedded Dolt without a remote because the Git repository
has no origin. `bd doctor` currently reports that embedded mode is unsupported.
Use `bd lint`, `bd dep cycles`, and the checked-in scrubbed JSONL export for
local integrity. Configure a durable Dolt remote and stronger health checks
when the GitHub repository exists.
