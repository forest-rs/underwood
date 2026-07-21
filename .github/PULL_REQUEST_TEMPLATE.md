## Intent

<!-- What permanent outcome does this change produce? -->

Bead: `und-`

## Fence

<!-- This component owns X; it explicitly does not own Y. -->

## Design and tradeoffs

<!-- Name the chosen design, credible alternatives, and why this belongs now. -->

## Evidence and proof

- Previous proof status:
- Proposed proof status:
- Checked-in evidence:

<!-- Code volume, effort, and screenshots are not evidence. -->

## Validation

```text
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

## Human gates

- [ ] No new production dependency or core `std` expansion.
- [ ] No `unsafe` or weakened safety invariant.
- [ ] Foundational public API has an approved design record and migration note.
- [ ] No stable identity, storage, hashing, wire, or on-disk format decision.
- [ ] No crate/module ownership change.
- [ ] Or: the required human decision and ADR are linked here.

## Completion

- [ ] The change belongs to the active campaign or repository maintenance.
- [ ] Product/example/wind-tunnel use only public contracts.
- [ ] Behavior changes have deterministic tests.
- [ ] Public APIs and relevant fields/variants are documented.
- [ ] ADRs and migration notes are updated where required.
- [ ] The proof ledger remains honest and references real evidence.
- [ ] Beads notes contain decisions, tradeoffs, and validation results.
- [ ] `.beads/issues.jsonl` reflects the final issue state.
