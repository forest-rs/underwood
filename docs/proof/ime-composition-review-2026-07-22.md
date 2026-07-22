# IME composition and editable-surface review — 2026-07-22

## Summary judgment

The composition slice is executable. Underwood now represents IME preedit as a
generated-source projection over one immutable document revision and one
checked, monotonic composition epoch. It does not publish authored text until
commit. A separate committed cache survives preedit churn and cancel, while an
identical final commit can reuse paragraph-engine physics already formed for
the preedit.

The same state serves both protocol families. A feed adapter installs complete
preedit snapshots. `EditableSurfaceSnapshot` binds the richer synchronous host
view—focused text, the complete selection model, marked range, source mapping,
UTF-8/UTF-16/Unicode-scalar conversion, caret and range geometry, and point
hits—to the exact same document revision and composition epoch. Its mutation
side maps an explicit host range back into a scene-validated authored selection
without exposing raw snapshot position construction.

The general scene model still supports multiple selections. One selection may
retain multiple logical ranges for visual bidi selection, and a selection set
may contain several independent insertion points. Native marked text is the
explicit exception: because it has one replacement region, composition begins
by reporting a collapse to the primary extent when the input set cannot be
represented. No disjoint visual ranges are silently unioned.

The slice adds no production dependency and no `unsafe`.

## Executable trace

`cargo run -p underwood_ime_compat_experiment` drives three real paragraphs
through `underwood_parley` and reports:

```text
feed.begin base_revision=DocumentRevision(1) selections=2 normalized=1 changed=true
host.replace surface=0..5 selections=1 source=0..5
feed.preedit epoch=1 shape=1 geometry=1 reused=2 committed_revision=DocumentRevision(1)
host.snapshot epoch=1 text="مرحباalpha" selection=Some(10..10) marked=0..10 marked_utf16=0..5
host.geometry caret=(49.00,0.00,50.00,25.17) first=(49.00,0.00,55.32,25.17) hit=10
feed.selection epoch=2 shape=0 geometry=0 reused=3
feed.cancel publications=0 shape=0 geometry=0 reused=3 selection_count=1
feed.commit revision=DocumentRevision(2) changed=1 shape=0 geometry=1 reused=2
```

The counters are deterministic work evidence, not elapsed-time claims.

## Must fix

All Must findings are resolved.

- **Preedit must not mutate canonical text.** `prepare_composition` receives an
  immutable snapshot, creates explicit composition source spans, and leaves the
  snapshot byte-for-byte observable after Arabic and combining-mark updates.
- **Text and geometry must not cross epochs.** Scene and editable-surface binds
  validate document identity, base revision, composition ID, and exact epoch.
  Stale callbacks and post-commit projections fail.
- **Generated text must retain real shaping context.** Adjacent authored and
  generated spans with equal shaping style remain one shaping run. A generated
  U+0301 after authored `e` produces the same real Parley glyph IDs, positions,
  and advances as authored `e` plus U+0301 while retaining mixed provenance.
- **Composition must not evict committed work.** Transient and committed
  formations have separate caches. Cancel performs zero shaping and geometry;
  unrelated paragraphs remain reused throughout preedit and commit.
- **Invalid updates must be atomic.** Reversed, out-of-bounds, mid-scalar,
  overlapping-clause, and stale-epoch cases fail before state replacement. The
  epoch advances exactly once only for an accepted update, and overflow is
  checked rather than wrapping.
- **Projection targets must be real.** A target with a matching paragraph index
  but no matching semantic text leaf is rejected rather than dropping preedit.
- **Native singularity must not erase the scene model.** Committed bindings
  preserve all independent selections and all visual-bidi ranges. Any
  normalization happens once at composition entry and is reported to the host.
- **Host mutation must not manufacture positions.** Explicit replacement
  ranges pass through the focused surface and exact committed caret map before
  becoming a logical `SnapshotTextSelectionSet`; separators, generated text,
  missing caret stops, and cross-leaf ranges fail explicitly.

## Should

- Platform adapters should retain a bound surface snapshot for the duration of
  each synchronous native callback and must invalidate it after document or
  composition epoch changes.
- Clause categories are validated and retained as host presentation facts;
  core deliberately does not choose underline or candidate styling.
- Screen-coordinate conversion, platform locks, responder lifetime, and
  reentrancy remain host responsibilities.
- An adapter that can express richer replacement topology may map it through
  the focused surface, but must not bypass snapshot/range validation.

## Could

- Add upstream `ui-events` adapters once its feed and host-driven traits settle;
  the production crates intentionally take no dependency on that API today.
- Add protocol-specific invalidation facts when real AppKit/UIKit/Android/TSF
  adapters demonstrate the minimum common contract.
- Generalize beyond one marked region only if a platform supplies real
  multi-composition semantics; do not infer that requirement from ordinary
  multi-selection editing.

## Real-vs-mirage boundary

**Real:** Arabic preedit and an authored/generated combining sequence shape
through the pinned Parley engine. Source provenance survives into fragments,
clusters, hits, carets, movement, and range geometry. UTF-16 conversion rejects
interior surrogate positions. Cancel and commit work counters are asserted.

**Not yet product-proven:** there is no AppKit, UIKit, Android, TSF, Wayland, or
Winit adapter in a production crate; the native showcase does not yet edit the
document; clauses are retained but not painted; platform invalidation and
coordinate conversion are not implemented. The ledger therefore calls this
slice Executable, not Conformant or Product-proven.

## Focused evidence

- `scene::tests::composition_epochs_preserve_generated_provenance_and_committed_cache`
- `scene::tests::composition_projection_rejects_a_missing_semantic_target`
- `editable::tests::explicit_scope_flattens_leaves_and_rejects_ambiguous_identity`
- `editable::tests::encoding_conversion_rejects_interior_utf16_units`
- `tests::event_feed_composition_normalizes_multi_selection_and_retains_committed_work`
- `tests::host_driven_queries_share_the_exact_parley_composition_epoch`
- `tests::generated_combining_mark_shapes_identically_without_authored_provenance`
- `underwood_ime_compat_experiment`

## Validation

The following gates pass on the implementation branch:

- `cargo fmt --all --check`
- `taplo fmt --check --diff`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked`
- `cargo +1.92.0 check --workspace --all-targets --all-features --locked`
- `cargo check` for both production crates with no default features on
  `x86_64-unknown-none` and `wasm32-unknown-unknown`
- `cargo xtask check`, `typos`, `bd lint --status all`, and `bd dep cycles`

Remote CI remains a publication gate rather than local evidence.
