# Underwood flow experiment

This unpublished research crate executes the private `flow-trace-v0` contract
from ADR-0002. It compares deterministic checkpoint policies, restart and
convergence laws, cancellation publication, and honest virtual extents.

```sh
cargo run -p underwood_flow_experiment
```

The synthetic block/feature model tests hypotheses about Underwood-owned
continuation semantics. It is not a product benchmark, paragraph breaker,
paginator, public checkpoint encoding, or substitute for real public-path
conformance.
