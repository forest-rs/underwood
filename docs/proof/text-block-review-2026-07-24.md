# TextBlock and intrinsic-layout review — 2026-07-24

## Summary judgment

Design-0012 is ready to land after review fixes. `TextBlock` is a façade over
the real one-paragraph document path, intrinsic constraints reach Parley's
actual break-sensitive formation, metrics come from prepared line facts, and
cache budgets coordinate geometry with backend physics. The wind tunnel uses
only public APIs and makes identity-local shaping cost explicit.

Good catch: reviewing failure atomicity exposed a backend-retention leak that
the success-path churn proof could not see.

## Must fix

### Failed first preparation could strand backend physics

`ParagraphFormation::form` may allocate a retained backend entry before
returning an error, and validation or geometry lowering can fail after a
successful formation. With no geometry entry yet, that paragraph was absent
from the release and eviction indexes.

Fixed by releasing backend state whenever first formation or lowering fails
without an existing geometry owner. Two regressions cover both sides of the
boundary:

- real Parley formation failure from missing Han coverage;
- successful custom formation followed by invalid UTF-8 source lowering.

Both assert zero geometry and zero backend residency.

### Failed style reshaping must force a retry

The BTree-map cache refactor initially stopped clearing the cached shaping-key
vectors before attempting a new style. A missing-font failure cleared Parley's
partial shaped output but left the old style key, so restoring that old style
looked reusable and failed later as `InvalidOutput`.

Fixed by invalidating the shaping key before the fallible operation, preserving
the existing retry contract. The external headless proof covers
successful style change, expected missing-font failure, and successful
recovery through the public scene path.

## Should fix

### Keep document release indexed

Releasing one discarded label must not scan every other label. A document
index now maps stable document identity to its committed/composition cache
entries, so explicit release scales with the discarded document rather than
global residency.

### Let the first Overstory consumer decide selective materialization

The retained wind-tunnel pass rematerializes 2,048 unchanged full scenes in
about 19 ms on the reference machine, despite performing no text physics.
That is evidence against preparing every unchanged label every frame, but not
yet evidence for a second output shape: a retained UI can keep the already
owned `SceneOutput`.

The integration decision and a before/after frame proof are tracked in
`und-oh0.5.4`. Any future non-editable/selective scene mode must keep the same
paragraph engine, source mapping, and cache rather than becoming a label fast
path.

### State identity uniqueness

`DocumentId` now documents that independent live documents sharing a layout
engine require distinct identities. Cache aliasing is not a supported
deduplication mechanism.

## Could improve

- Add a rich-run `TextBlock` construction API when the first actionable inline
  semantic consumer establishes the right source-editing ergonomics.
- Add diagnostics reset/delta helpers only if hosts need interval telemetry;
  cumulative counters are sufficient for the current proof.
- Consider a reusable cache-policy vocabulary beyond entry count only after a
  consumer supplies byte-cost evidence.

## Suggested tests

- Block/document equivalence for lines, metrics, glyph IDs, positions, and
  advances — present.
- Max-content mandatory breaks, min-content Arabic break reshaping, and
  constrained non-reshaping reflow — present.
- Empty block size and absent baselines — present.
- Cache hit/miss, explicit release, eviction/reload, zero budget, and backend
  coordination — present.
- Failure before geometry ownership releases backend state — present at both
  the core adapter contract and real Parley boundary.
- Thousands of stable, identical, edited, constrained, released, and
  budget-churned public blocks — present in `benches/labels`.
