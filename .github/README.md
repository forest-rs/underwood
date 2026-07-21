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

The scheduled `governance.yml` workflow runs `audit-remote.sh` against the live
repository every Monday and on demand. It rejects drift in merge policy,
workflow permissions, immutable Action pinning, vulnerability alerts, the
protected-main ruleset, or its exact required checks. `cargo xtask check`
protects both files from accidental removal.

The built-in workflow token cannot read repository Actions policy or
vulnerability-alert settings. The workflow therefore requires the
`UNDERWOOD_GOVERNANCE_TOKEN` Actions secret. It must be a fine-grained token
selected only for `forest-rs/underwood`, with repository `Administration: read`
and an explicit expiration. Do not substitute a broad classic personal access
token. After creating or rotating the secret, dispatch `Governance Audit`
manually and record the successful run in `und-oh0.11.2`.

`und-oh0.11.2` owns that audit and a durable Beads Dolt remote plus independent
backup. Git history and the scrubbed JSONL export are review and recovery aids;
they do not replace the authoritative Dolt remote.

The bootstrap CI intentionally checks only the tooling workspace. It does not
claim `no_std`, WebAssembly, package, or product conformance before product
crates exist. The crate registry makes creation of a `core` crate fail policy
validation unless CI names genuine `x86_64-unknown-none` and
`wasm32-unknown-unknown` checks.

GitHub Actions are pinned to immutable commit SHAs. Dependabot maintains those
references.
