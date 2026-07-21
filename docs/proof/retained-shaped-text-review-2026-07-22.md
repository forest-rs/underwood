# Retained `ShapedText` adversarial review — 2026-07-22

## Summary judgment

The migration is structurally honest and ready for repository-wide validation.
`underwood_parley` now retains Parley Core's owned result instead of presenting
an adapter-private shaped model as equivalent truth. Underwood's public
prepared boundary stays renderer-neutral, dependency direction is unchanged,
and the exact CPU scene remains pixel-identical.

Good catch: the first direct lowering treated Parley's logical RTL cluster
storage as visual order. The mixed-direction poster exposed the reversal before
snapshot acceptance; the adapter and seam experiment now traverse RTL clusters
backward and conformance asserts descending logical source positions in visual
scene order.

## Must fix

All Must findings are resolved.

- **RTL clusters and ligature components:** Logical cluster order cannot be
  emitted directly for RTL. Direction-aware traversal and the corresponding
  preceding-component ligature union are implemented and exercised by the
  headless public path and exact poster.
- **Control-only source:** Rejecting every empty glyph iterator forces callers
  either to lose newline source coverage or manufacture a phantom glyph.
  `PreparedRun` now permits the representation, the real Parley adapter proves
  newline output has zero glyphs, and scene validation rejects glyphless
  ordinary text.
- **Failed partial reshaping:** `ShapedText::clear` occurs before incremental
  appends, so a missing-font failure cannot leave the old shaping identity
  paired with empty or partial storage. Cache identity is invalidated before
  shaping; a paint-driven retry through the public engine proves that a valid
  request reshapes rather than lowering partial state.
- **Relative source-offset width:** Current upstream `ClusterData::text_offset`
  is `u16`. An unbounded same-style item could silently truncate offsets after
  64 KiB. Adapter itemization now splits before another character could become
  unrepresentable, with a boundary test above the limit.

## Should

- Keep `ClusterData` encoding knowledge confined to `underwood_parley`. The
  inline-glyph sentinel, glyph ranges, direction traversal, and ligature
  component rules do not belong in the `underwood` facade.
- Preserve the script sidecar count check until Parley retains script on each
  shaped run. The sidecar is small adaptation metadata, not a competing shaped
  result.
- Continue to describe `und-oh0.2.2`, `und-oh0.2.3`, and `und-oh0.2.4` as open.
  Owned shaping does not supply paragraph breaking, cluster-accurate caret/hit
  behavior, or conformant cross-paint glyph coverage by itself.
- Treat the same-machine timing as diagnostic evidence. It rules out an obvious
  regression but does not measure allocations or establish a release budget.

## Could

- Propose an upstream visual-cluster iterator that encapsulates the inline and
  external glyph encodings plus RTL traversal, reducing pinned-revision glue.
- Remove the local script sidecar if a future `ShapedRun` retains resolved
  script.
- Add allocation counting to the product wind tunnel when the repository has a
  portable allocator-measurement policy.

## Suggested tests

The required tests are present and green:

- exact `ffi` glyph source ownership over bytes `1..4`;
- mixed Latin/Arabic visual traversal with valid logical UTF-8 ranges;
- newline-only shaping with no scene fragment or phantom shape record;
- rejection of an adapter's glyphless Latin run;
- recovery after missing-font partial shaping;
- safe item splitting beyond the upstream relative-offset limit;
- unchanged, paint-only, flow-only, and shaping-change work accounting;
- exact `imaging_vello_cpu` snapshot reproduction.

A future paragraph-breaking campaign should add a full >64 KiB public-path
case alongside legal-break, mandatory-break, and break-sensitive reshaping
corpora. That is not evidence claimed by this intake campaign.
