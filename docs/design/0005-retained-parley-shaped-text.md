# Design-0005: Retained Parley shaped text

- **Status:** Approved
- **Approved:** 2026-07-22 by Bruce Mitchener
- **Beads:** `und-oh0.2.5`, `und-oh0.10.1.4`
- **Authority:** ADR-0004; Design-0002; Design-0004
- **Upstream:** Parley `6c81e1dd9b67793cdd959c65cc650c96a1262fb7`

## Goal

Retire Underwood's callback-era shaped-run copy after Parley Core gained the
owned, reusable `ShapedText` result in `91388dc`. Retained analysis, shaping,
font instances, clusters, glyphs, metrics, and normalized coordinates must
come from Parley Core without leaking a Parley type through Underwood's public
adapter contract.

## Fence

`underwood_parley` owns adaptation from Underwood's paragraph inputs into
Parley Core and one-way lowering from retained Parley results into portable
`PreparedParagraph` output; it explicitly does not own a second shaped-text
storage model, cluster model, glyph model, or line-breaking implementation.

Parley Core owns the reusable shaped paragraph, including source-aware
clusters, glyph storage, resolved font instances, normalized coordinates,
metrics, and bidi levels. Underwood owns semantic source identity, paint slots,
portable output, document/region flow, and stage-specific invalidation.

```text
Underwood ParagraphInput
  -> Analysis + itemization
  -> Fontique selection callback
  -> parley_core::ShapedText (retained in adapter cache)
  -> script sidecar + paint lowering
  -> Underwood PreparedParagraph
  -> TextScene
```

## Invariants

1. `PhysicsCache` contains one `ShapedText`; the local `PhysicsRun`,
   `PhysicsGlyph`, `copy_run`, manual HarfRust scaling, and character-offset
   table are removed.
2. `ShapedText` is cleared and reused only when shaping identity changes.
   Paint-only and cache-hit preparation never rerun itemization, font
   selection, or shaping.
3. Every `ShapedText::shape_item` appended run receives exactly one ISO 15924
   script sidecar entry. Sidecar and run counts are validated before lowering.
4. The selected Parley `FontInstance` is the exact font resource and Fontique
   synthesis evidence lowered into Underwood. Final normalized coordinates are
   read from the run's range in `ShapedText`.
5. Cluster source offsets are interpreted relative to the containing shaped
   run and checked as UTF-8 byte ranges before entering portable output.
   Itemization creates a safe boundary before another character could exceed
   the current `u16`-relative `ClusterData` offset representation.
6. A glyph-bearing ligature-start cluster owns the union of its source range
   and its adjacent ligature-component clusters: following in LTR logical order
   and preceding in RTL logical order. Continuation clusters never create
   phantom glyphs.
7. Regular clusters lower their inline or external glyph storage without
   changing Parley's scaled advances or offsets. Parley's logical cluster
   storage is traversed forward for LTR and backward for RTL so portable glyphs
   retain visual order.
8. Control-only shaped runs may contain no renderable glyphs. Underwood retains
   their source coverage without inventing a `.notdef` or zero-advance phantom
   glyph.
9. No Parley or Fontique engine type crosses the `underwood` facade. Core
   crates remain `no_std + alloc`, no new dependency edge is added, and no
   `unsafe` is introduced.

## Options considered

### Keep the callback copy until paragraph breaking lands

This leaves two shaped-text representations alive precisely when breaking will
begin depending on cluster boundaries and metrics. It also forfeits upstream
storage reuse. Rejected.

### Expose `parley_core::ShapedText` from Underwood

This would make a backend implementation type part of the renderer-neutral
facade and couple semantic scene consumers to Parley Core's storage evolution.
Rejected.

### Retain `ShapedText` privately and lower once per preparation output

Chosen. The cache owns Parley's native result. The existing Underwood adapter
contract remains the portable boundary, while the next paragraph-breaking
campaign can build against the same retained cluster and metric truth.

## Dependency uptake

The workspace's existing `fontique`, `parlance`, and `parley_core` git pins move
together from `45da4a90248b1600277a4294b70d8bfde5ca8e97` to Parley main
`6c81e1dd9b67793cdd959c65cc650c96a1262fb7`. This is an immutable revision
update of approved production dependencies, not a new dependency. The lockfile
must resolve all Parley workspace crates from the same commit.

## Public migration note

No public signature changes. `PreparedRun::try_new` is relaxed to accept an
empty glyph iterator for a non-empty source range so adapters can represent
newline/control-only shaped runs honestly. Consumers must treat `glyphs()` as
possibly empty; creating a phantom glyph for source coverage is incorrect.
Existing non-empty runs retain their validation and behavior.

## Executable proof

- Latin, Arabic, fallback, OpenType feature, and variable-font proofs produce
  the same portable resources, glyph identities, bidi levels, synthesis, and
  normalized coordinates after the migration.
- An `ffi` ligature maps its emitted glyph to the full ligature source range,
  while Parley's continuation clusters emit no duplicate glyphs.
- Mixed Latin/Arabic run source ranges remain valid UTF-8 and source ordered.
- A control-only run retains its source without a fabricated glyph.
- Font-request, shaping-style, paint-only, and full cache-hit work counters
  preserve their existing invalidation behavior.
- The same cache-owned `ShapedText` value survives across reshapes; wall-time
  benchmarks are compared with the previous pin on the same machine. This is
  not an allocation-count claim.

## Deferred work

- Parley Core's current `ShapedText` does not itself expose paragraph line
  formation. `und-oh0.2.2` owns the breaking seam and must not be smuggled into
  this uptake.
- Fontique cluster-selector convergence remains `und-oh0.2.6`.
- Cluster-accurate hit/caret output and paint coverage remain `und-oh0.2.3` and
  `und-oh0.2.4`.
- The script sidecar can disappear if Parley later retains script per shaped
  run or exposes an equivalent stable query.

## Validation

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo +1.92.0 check --workspace --all-targets --locked
cargo check -p underwood -p underwood_parley --target x86_64-unknown-none --locked
cargo check -p underwood -p underwood_parley --target wasm32-unknown-unknown --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
cargo xtask check
bd lint --status all
bd dep cycles
git diff --check
```
