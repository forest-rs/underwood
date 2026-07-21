# Underwood position wind tunnel

This unpublished crate begins executing the private `identity-trace-v0`
contract from ADR-0001. Its candidates, event model, digests, and counters are
experiment machinery, not production storage or public position APIs.

Run the current dependency-free canonical baseline with:

```sh
cargo run -p underwood_position_wind_tunnel
```

The report distinguishes semantic or complete-gate `PASS` results from
preliminary `SCREEN` observations and is deliberately honest about
unimplemented corpora and failed budgets. A failing baseline establishes
measurement and semantic controls; it does not select the production
representation.
