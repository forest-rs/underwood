# Retained lifecycle progress — 2026-07-25

This records each independently green implementation slice of Design-0017
against the matched public-path wind tunnel established in
`retained-lifecycle-baseline-2026-07-25.md`.

The measurements are release builds on the same Apple arm64 host. Allocation
counts are deterministic event-scoped observations; timings are machine-local
and are included only when enough samples support a useful comparison.

## Slice 2: provenance preflight

`StyleMap` is now a cheap clone over strong immutable backing. Equal assignments
preserve that identity, while a real mutation copy-on-writes the backing state.
Each paragraph cache retains a strong style owner plus its exact paragraph,
constraint, region-flow, and incoming-cursor provenance. An exact provenance
hit occurs before `Projection::new`, so it allocates no projected string,
source map, or projected style/run vectors. Unrelated but value-equal inputs
may be rescued by a complete paragraph-local value comparison; a value
difference continues through the existing checked projection path.

`PreparationReuse::preflight_reuses` makes the fast-path decision directly
observable without allocator instrumentation.

| Event | Paragraphs | Baseline calls | Current calls | Calls removed |
|---|---:|---:|---:|---:|
| retained exact repeat | 64 | 42,607 | 41,966 | 641 |
| retained exact repeat | 1,000 | 666,167 | 656,166 | 10,001 |
| localized prepare | 64 | 43,247 | 42,618 | 629 |
| localized prepare | 1,000 | 666,807 | 656,818 | 9,989 |

The exact-repeat reduction scales at ten calls per paragraph plus one
event-level allocation. Localized preparation retains the same reduction for
every unchanged sibling while the changed paragraph takes the checked path.

### What this does not claim

This slice does not make publication O(1) or localized preparation O(change).
The 1,000-paragraph exact repeat still performs 656,166 allocations and retains
about 86.8 MB of newly copied output. Flat scene materialization remains the
dominant cost and is the next Design-0017 slice. Treating the preflight result
as campaign completion would be a mirage.
