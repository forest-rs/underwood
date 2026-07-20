# Capability proof

The proof ledger prevents architectural specification, code volume, or product
presentation from being mistaken for demonstrated capability.

## Status law

Statuses are ordered and literal:

1. `specified`
2. `executable`
3. `measured`
4. `conformant`
5. `product-proven`

Promotion requirements:

| Status | Minimum evidence |
| --- | --- |
| Specified | Contract, fence, invariants, evidence plan |
| Executable | Real public path and integration test |
| Measured | Reproducible workload, budget, retained artifact |
| Conformant | Named corpus, failure cases, differential/platform evidence |
| Product-proven | Sustained dependency from the living agent document |

## Capability state

- `active`: belongs to the current campaign and has an owner.
- `gated`: blocked by an explicit decision or prerequisite.
- `dormant`: remains part of the destination but receives no active execution.

Dormant does not mean rejected. It is how the project avoids pretending to run
company-sized conformance fronts simultaneously.

## Ledger format

`ledger.tsv` is a versioned, tab-separated file so the dependency-free `xtask`
can validate it. Fields are:

- `capability`: stable kebab-case identifier;
- `state`: `active`, `gated`, or `dormant`;
- `proof`: one literal status;
- `owner`: owner identifier or `unassigned`;
- `spec`: repository-relative specification reference, optionally with a
  heading fragment;
- `evidence`: semicolon-separated repository paths or `-`;
- `product`: product scenario identifier or `-`.

For executable and higher statuses, every evidence path must exist.
Conformant evidence must include a corpus, conformance, or platform artifact.
Product-proven entries must name a product scenario.

## Promotion review

A promotion bead contains:

- previous and proposed status;
- exact evidence paths;
- workloads and platforms;
- known exclusions and degraded modes;
- an adversarial review;
- the human decision when the claim affects external expectations.

The ledger is updated in the same logical change as the evidence.
