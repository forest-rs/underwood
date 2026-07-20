# GitHub repository controls

The checked-in workflows enforce objective repository policy. The following
remote controls remain required once `forest-rs/underwood` has an origin:

- protect `main`;
- require pull requests and current-base status checks;
- require conversation resolution;
- enable the merge queue;
- disallow ordinary administrator bypass;
- configure CODEOWNERS for constitutional, ADR, proof, Cargo, and CI paths;
- configure the Beads Dolt remote and backup;
- verify scheduled constitutional audits run.

The bootstrap CI intentionally checks only the tooling workspace. It does not
claim `no_std`, WebAssembly, package, or product conformance before product
crates exist. The crate registry makes creation of a `core` crate fail policy
validation unless CI names genuine `x86_64-unknown-none` and
`wasm32-unknown-unknown` checks.

GitHub Actions are pinned to immutable commit SHAs. Dependabot maintains those
references.
