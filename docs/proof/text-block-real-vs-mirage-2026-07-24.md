# TextBlock real-vs-mirage audit — 2026-07-24

## Summary judgment

The slice earns one retained text engine, real intrinsic formation, exact
metrics, and bounded cache lifetime. It earns a lightweight application call
site. It does not yet earn the broader claim that rematerializing thousands of
unchanged full scenes is a cheap frame operation.

## Mirage risks

### Mirage: a shorter call site is automatically a cheaper engine

`TextBlock::plain` still constructs one real document paragraph and text leaf,
and `prepare_block` still materializes the complete `TextScene`, including
interaction facts. That is deliberate architectural reuse, not a label-only
fast path. The proof and API docs now say “low ceremony” or “lightweight call
site” rather than implying a separate reduced-cost engine.

The retained pass performs zero analysis, shaping, or formation but takes about 19 ms for 2,048
full outputs on the reference machine. A host that keeps its owned output does
not pay that every frame; whether a real toolkit does so is one integration
review deep and remains tracked in `und-oh0.5.4`.

### Mirage: identical text is globally deduplicated

Distinct paragraph identities currently shape independently. The identical
corpus asserts that fact instead of presenting shared style storage as a
cross-paragraph shaping cache. Adding such a cache would require its own
resource, invalidation, language, fallback, and memory proof.

### Mirage: `TextBlock` already represents arbitrary rich inline content

The first façade owns one plain semantic text leaf. Rich inline semantics are
an intended extension of the same source model, not a capability hidden behind
the present name. No docs or tests claim a rich-run builder today.

## Real strengths

- Block/document equivalence compares actual metrics, line facts, glyph IDs,
  positions, and advances through the same Parley adapter.
- Intrinsic modes are backend inputs, not infinite-width sentinels.
  Max-content mandatory breaks and min-content Arabic break reshaping execute
  in deterministic tests.
- Style sharing is concrete: block requests borrow one computed style, and
  pointer-identity tests prove its family, feature, and variation arrays remain
  shared across clones.
- Cache scaling is structural: geometry and Parley preparation use indexed
  paragraph identity; document release has its own index; LRU budget eviction,
  explicit release, zero budget, reload, and failure cleanup are observable.
- The wind tunnel covers 2,048 public blocks and treats work/cache assertions
  as proof, with wall time only as supporting evidence.

## Most dangerous gap

The first Overstory label integration could accidentally call
`prepare_block` for every unchanged label during every layout pass. That would
turn retained preparation reuse into repeated scene allocation and copying.
The next integration must first prove host output retention. Only if that is
insufficient should Underwood add a selective non-editable scene payload, and
that payload must preserve the existing paragraph engine and source facts.

## Suggested tests

- A real Overstory frame with thousands of stable labels that counts
  `prepare_block` calls, output retention, and allocations.
- Creation/removal churn through Overstory's widget lifecycle and
  `release_document`.
- Before/after evidence if a selective materialization mode is proposed,
  including mixed bidi and link-semantic geometry rather than Latin-only
  labels.
- Memory-cost evidence before considering a byte-budget cache or
  cross-paragraph shaped-result cache.
