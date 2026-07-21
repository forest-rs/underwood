# Underwood position experiment

This unpublished research crate executes the private `identity-trace-v0`
contract from ADR-0001. Its candidates, event model, digests, and counters are
experiment machinery, not production storage or public position APIs.

Run the current dependency-free canonical baseline with:

```sh
cargo run -p underwood_position_experiment
```

This is not a product benchmark. The report distinguishes semantic or
complete-gate `PASS` results from
preliminary `SCREEN` observations and is deliberately honest about
unimplemented corpora and failed budgets. A failing baseline establishes
measurement and semantic controls; it does not select the production
representation.

The one-GiB append pressure run reuses one immutable 64-KiB payload across
16,384 logical batches. This isolates persistent publication-metadata work; it
does not measure one GiB of source ingestion, distinct retained payload memory,
or allocator overhead. Distinct-payload tests separately cover source order and
snapshot preservation.

The tree-anchor model executes local text-container split, adjacent join,
sibling move, and delete laws. Its node identifiers and flat representation are
private semantic-test machinery, not a production tree choice or evidence for
recursive/collaborative tree operations.

The `form-10k` corpus is exactly 10 KiB with 256 authored spans and 64 sparse
anchors. Four successive replacements model an active IME composition ending
in `日本語`; every published frontier is checked against both the canonical
model and private chunked/blocked candidates.
