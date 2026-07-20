# Underwood

Underwood is a renderer- and toolkit-independent document composition and
editing platform for Rust.

It owns semantic documents, stable positions, layered annotations, computed
style, projection, incremental flow, transactions, inline objects, semantic
mapping, and renderer-neutral prepared text scenes. Parley owns text shaping
physics. Overstory is the flagship experience layer.

> Parley shapes the text. Underwood makes it a document. Overstory makes it an
> experience.

## Current status

Underwood is in its executable-constitution bootstrap. The complete
architecture is [specified in the handover](UNDERWOOD_HANDOVER.md), but no
foundational product API has been ratified or implemented.

The first product campaign remains gated on:

- Charter-000: spearhead, proof, and stewardship;
- ADR-0001: position and canonical storage;
- ADR-0002: resumable flow and virtual extents;
- ADR-0003: text-data provisioning and identity;
- ADR-0004: the Parley boundary and contingency.

The machine-readable [proof ledger](docs/proof/ledger.tsv) is authoritative for
capability status.

## Repository workflow

Read, in order:

1. [the architectural handover](UNDERWOOD_HANDOVER.md);
2. [the agent constitution](AGENTS.md);
3. [the executable constitution](docs/CONSTITUTION.md);
4. [the governance workflow](docs/governance/README.md).

Find ready work with:

```sh
bd prime
bd ready
```

Validate the bootstrap with:

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

The workspace MSRV is Rust 1.92. The bootstrap CI stable toolchain is Rust
1.96.

## License

Licensed under either Apache-2.0 or MIT at your option.
