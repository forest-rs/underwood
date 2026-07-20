# Architecture decision records

ADRs record durable Underwood decisions that would otherwise become folklore.
They are not retrospective decorations.

## When an ADR is required

Create or update an ADR before changing:

- crate or module ownership;
- public semantics or migration obligations;
- stable identity, hashing, storage, wire, or on-disk behavior;
- safety invariants or `unsafe` policy;
- dependency direction or a production dependency;
- cache identity or invalidation law;
- cross-system interoperability contracts;
- conformance claims whose limits need durable explanation.

## Lifecycle

1. **Open:** fence, question, alternatives, evidence, and decision authority are
   recorded. No choice is implied.
2. **Experimental:** authorized prototypes or traces are collecting evidence.
3. **Proposed:** evidence supports a recommendation ready for human review.
4. **Accepted:** the decision and migration consequences are ratified.
5. **Superseded:** a newer ADR replaces the decision and links both directions.

Open and Experimental ADRs must not be cited as settled public contracts.

## Mandatory foundation decisions

| Record | Status | Accepted |
| --- | --- | --- |
| [ADR-0001](0001-position-and-canonical-storage.md) — position and canonical storage | Accepted | 2026-07-21 |
| [ADR-0002](0002-resumable-flow-and-virtual-extents.md) — resumable flow and virtual extents | Accepted | 2026-07-21 |
| [ADR-0003](0003-text-data-provisioning.md) — text-data provisioning and identity | Accepted | 2026-07-21 |
| [ADR-0004](0004-parley-boundary-and-contingency.md) — Parley boundary and contingency | Accepted | 2026-07-21 |

Acceptance ratifies the decisions and experiment gates stated in each record.
It does not upgrade a capability's proof status beyond the evidence in the
proof ledger.

## Ownership

A cross-crate decision belongs to the crate that owns the invariant. Before
product crates exist, mandatory records live here. After a crate exists, new
records normally live in that crate's `docs/adr/` directory.

Use `0000-template.md` as the minimum structure.
