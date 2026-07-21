# GitHub repository controls

`forest-rs/underwood` is the canonical public origin. The checked-in workflows
and ownership map enforce objective repository policy. The default-branch
ruleset must:

- protect the default branch against deletion and force pushes;
- require pull requests and every named CI job against the current base;
- require conversation resolution;
- enable the merge queue;
- require CODEOWNERS review for constitutional, ADR, proof, Cargo, Beads, and
  CI paths;
- disallow direct administrator bypass; the bootstrap steward may use only an
  audited pull-request bypass while the project has a single maintainer.

Repository settings must allow squash merges only, delete merged branches,
enable automatic merge, pin Actions to immutable commit SHAs, keep the default
workflow token read-only, and retain vulnerability alerts.

`und-oh0.11.2` owns verification of those remote settings, the scheduled
constitutional audit, and a durable Beads Dolt remote plus independent backup.
Git history and the scrubbed JSONL export are review and recovery aids; they do
not replace the authoritative Dolt remote.

The bootstrap CI intentionally checks only the tooling workspace. It does not
claim `no_std`, WebAssembly, package, or product conformance before product
crates exist. The crate registry makes creation of a `core` crate fail policy
validation unless CI names genuine `x86_64-unknown-none` and
`wasm32-unknown-unknown` checks.

GitHub Actions are pinned to immutable commit SHAs. Dependabot maintains those
references.
