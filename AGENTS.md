# Underwood Agent Constitution

This repository is maintained by people and coding agents working through the
same architectural, evidence, and review rules.

## Onslaught

Underwood is a five-year document, editing, composition, and prepared-scene
platform. We do not reduce that destination to make an early demonstration
easier.

Incrementalism is a property of the machine, not a reduction of the ambition.
Work lands in complete, permanent slices of the final architecture. Incomplete
capabilities are named honestly and are never presented as done.

- Concentrate force: one capability campaign is active at a time.
- Use public paths: products, examples, and wind tunnels receive no private
  shortcut.
- Prefer evidence to volume: code, crate count, screenshots, and effort are not
  proof.
- Keep the destination whole: inactive workstreams remain explicit and dormant
  rather than being silently deleted from the architecture.
- Refactor without attachment when evidence overturns a design.

## Repository fence

This repository owns Underwood's semantic document, transactional editing,
preparation, flow, and renderer-neutral scene contracts; it explicitly does not
own text shaping physics, toolkit interaction policy, task execution, or pixel
production.

The tooling-only `xtask` crate owns repository policy validation; it explicitly
does not own product behavior or architectural decisions.

## Forest engineering tenets

1. **Build to endure.** Optimize for structural strength, not short-term
   applause.
2. **Modularity is power.** Give every subsystem a narrow responsibility,
   minimal dependency surface, stable seam, and replaceable internals.
3. **Incremental computation everywhere.** Deltas over rebuilds, retained work
   over recomputation, explicit budgets over unbounded spikes.
4. **Introspection is non-optional.** Time, memory, work units, invalidation,
   and retained resources are architectural outputs.
5. **Explicit over implicit.** No hidden state, scheduling, lifetime,
   capability, or performance behavior.
6. **Long-term over short-term.** Clean structure over clever shortcuts.
7. **Replaceability is a constraint.** A subsystem that cannot be replaced must
   be small and contained.
8. **Calm interfaces.** Internal sophistication must produce a small,
   intentional public surface.
9. **No sacred subsystems.** Evidence can overturn any implementation.

## North star

- Keep foundational crates small, predictable, `no_std + alloc` where
  practical, and long-lived.
- Prefer simple, explicit, concrete designs over generic machinery.
- Avoid dependency creep and control compile time and feature surface.
- Optimize for the correct long-term architecture over temporary
  compatibility.
- Preserve the three constitutional foundations: immutable snapshots,
  deliberate position forms, and revisioned prepared resources.

## Proof law

Every major capability has exactly one honest status:

```text
Specified -> Executable -> Measured -> Conformant -> Product-proven
```

- **Specified:** the contract, fence, invariants, and evidence plan exist.
- **Executable:** a real public path runs without a private shortcut.
- **Measured:** checked-in workloads and retained artifacts establish budgets.
- **Conformant:** the relevant corpora, failure cases, differential tests, and
  platforms pass.
- **Product-proven:** the living agent document depends on the capability under
  sustained real use.

Only an explicit proof review may promote a capability. See
`docs/proof/README.md`.

## Definition of Done

A change is not done unless all applicable items hold:

- The owning Beads issue has testable acceptance criteria and is updated with
  decisions, evidence, and validation results.
- `cargo fmt --all --check` passes.
- `taplo fmt --check --diff` passes.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  passes.
- `cargo test --workspace --all-features --locked` passes.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
  --locked` passes.
- `cargo xtask check` passes.
- `typos` passes.
- `bd lint --status all` and `bd dep cycles` pass.
- Public APIs are documented.
- Behavior changes include deterministic tests.
- Architecture, invariant, ownership, stable identity, format, or public
  semantic decisions update an ADR.
- Proof claims update the ledger and reference checked-in evidence.
- Examples, benchmarks, corpora, conformance harnesses, and wind tunnels live
  in separate top-level workspace crates. Core crates gain no dev-dependencies.

## `no_std` and dependency policy

- Foundational crates default to `#![no_std]`, using `alloc` when needed.
- `std` is an explicit feature when a core capability genuinely needs it.
- Avoid `std` collections in core crates; use `alloc` types and approved
  `hashbrown` configurations where evidence requires hashing.
- Workspace dependencies are centralized and use `default-features = false`
  where practical.
- New production dependencies require human approval before editing manifests.
- Tooling dependencies remain exceptional and must be justified in the owning
  Beads issue.

## Beads workflow

Beads is the only task graph. Do not use `tk`, markdown checklists, or source
TODOs as substitute tracking.

At the beginning of a session:

```sh
bd prime
bd ready
bd show <id>
bd update <id> --claim
```

During work:

- Record newly discovered work with `bd create`.
- Use parent/child relationships for decomposition and blocking dependencies
  for real ordering.
- Record durable architectural reasoning in ADRs; keep execution observations
  in Beads notes.
- If a bead's assumptions fail, stop that path and create or escalate a
  decision bead. Do not compensate with local cleverness.

Before completion:

```sh
bd lint --status all
bd dep cycles
bd close <id> --reason "summary, evidence, and validation"
bd export --scrub --output .beads/issues.jsonl
```

Close a bead only after its acceptance criteria are satisfied and its durable
artifacts are present. Keep the JSONL export synchronized after the final state
change.

## Thinking and human gates

Use xhigh reasoning at architectural seams and proof gates. Medium reasoning is
appropriate only for execution-ready beads with a fixed fence, invariants,
inputs, outputs, acceptance criteria, proof target, and validation.

Ask the human before:

- adding a production dependency or widening a core `std` feature;
- introducing `unsafe` or weakening a safety invariant;
- creating or changing a foundational public API;
- choosing stable identity, storage, hashing, wire, or on-disk semantics;
- splitting, merging, or changing ownership of crates or modules;
- choosing between credible permanent representations;
- changing review or PR sequencing when it affects architectural review.

When blocked by a gate, present the fence, exact decision, two or three options,
recommendation, and evidence needed.

## Documentation and ADRs

- `UNDERWOOD_HANDOVER.md` is the architectural north star.
- `docs/CONSTITUTION.md` defines the execution system.
- `docs/charter/` owns product, proof, and stewardship ratification.
- `docs/adr/` owns durable architectural decisions.
- `docs/proof/` owns proof semantics and the machine-readable ledger.
- Plans belong in Beads. Add a plan document only when substantial durable
  reasoning would otherwise be lost.

Cross-crate decisions live in the ADR of the owning crate or, before that crate
exists, in the repository ADR directory. Other locations link to the owning
record rather than duplicating it.

## Agent personas

Invoke the matching skill when the role is clear:

- `$alder` — architecture, boundaries, invariants, naming
- `$cedar` — API ergonomics, rustdoc, examples
- `$marten` — reproduction-first bugs and regression tests
- `$stoat` — benchmarks, profiling, measured optimization
- `$wren` — design documents and handoffs
- `$badger` — Cargo, CI, features, and repository health
- `$otter` — adapters, migrations, and integration seams
- `$lynx` — adversarial Must/Should/Could review
- `$shepherd` — human decision gates

If a request changes crate ownership, `$otter` defers to `$alder`. Correctness
Must-fixes precede polish or optimization. Product benchmarks under `benches/`
must call the real public product crates and exercise the same path as external
callers. Hypothesis implementations and duplicated algorithms belong under
`experiments/`; their measurements are research evidence, never product
benchmarks. Performance work without a product benchmark creates one against
the actual implementation before making optimization claims.
