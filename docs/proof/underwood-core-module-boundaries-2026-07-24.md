# Underwood core module-boundary proof

Date: 2026-07-24
Bead: `und-oh0.5.5.3`

## Claim

The foundational `underwood` crate now expresses its existing ownership
boundaries in the filesystem. The public crate root and four subsystem roots
are calm façades; implementation modules each own one preparation or state
phase.

This is a behavior-neutral structural checkpoint. It changes no public name,
public call path, dependency, feature, MSRV, or prepared-scene behavior.

## Fences

- `adapter/formation.rs` owns formation inputs, outputs, work, and the backend
  contract; prepared-record validation is elsewhere.
- `adapter/interaction.rs` owns backend-produced caret, cursor, cluster, and
  line-boundary facts; document selections are elsewhere.
- `adapter/prepared.rs` owns validated fonts, glyphs, runs, lines, and
  paragraphs; formation policy and scene lowering are elsewhere.
- `adapter/paint.rs` owns source-complete glyph paint partitions; brush values
  and renderer execution are elsewhere.
- `document/model.rs` owns persistent semantic state, snapshots, and
  publications; edit validation is elsewhere.
- `document/transaction.rs` owns staged mutation, replacement validation, and
  atomic publication; immutable state representation is elsewhere.
- `editable/surface.rs` owns the semantic leaves exposed to a host; host
  encoding and bound geometry queries are elsewhere.
- `editable/snapshot.rs` owns atomic scene/selection binding, offset encoding,
  provenance, and geometry queries; semantic scope selection is elsewhere.
- `scene/engine.rs` owns preparation sequencing and coordinated cache lifetime;
  projection and geometry construction are elsewhere.
- `scene/projection.rs` owns complete semantic, source, style, and composition
  projection into adapter inputs; cache policy is elsewhere.
- `scene/geometry.rs` owns snapshot-independent cached geometry and
  source-aware lowering; shaping and public interaction policy are elsewhere.
- `scene/output.rs` owns metrics, provenance records, stage work, and output
  wrappers; scene interaction is elsewhere.
- `scene/interaction.rs` owns immutable hit testing, movement, and selection
  geometry over committed and projected scenes; visual records are elsewhere.
- `scene/records.rs` owns renderer-neutral lines, glyph fragments, semantics,
  carets, hits, and highlight records; preparation is elsewhere.

## Structural evidence

| Previous catch-all | Before | Façade after | Largest owned implementation |
| --- | ---: | ---: | ---: |
| `adapter.rs` | 1,868 | 39 | 589 |
| `document.rs` | 1,180 | 29 | 396 |
| `editable.rs` | 1,013 | 36 | 601 |
| `scene.rs` | 5,057 | 56 | 990 |

`underwood/src/lib.rs` remains the 59-line public façade. Unit tests moved
intact beside their subsystem roots. Sibling implementation modules share
construction-only details through `pub(super)` visibility; no new crate-visible
or externally visible surface was introduced.

## Behavioral evidence

- All 42 `underwood` unit tests pass.
- The adapter's 14 validation tests pass unchanged.
- The document's 6 transaction and snapshot tests pass unchanged.
- The editable surface's 2 encoding and scope tests pass unchanged.
- The scene's 13 cache, projection, geometry, interaction, and composition
  tests pass unchanged.
- The crate's executable doctest passes.
- Full workspace tests and warning-denied Clippy pass.
- Warning-denied workspace rustdoc passes.
- Rust 1.88 workspace checks pass for every crate within the workspace MSRV
  policy.
- `x86_64-unknown-none` and `wasm32-unknown-unknown` checks pass for
  `underwood` and `underwood_parley`.
- The exact CPU visual proof regenerates without changing the committed PNG.
- The deterministic mixed-script PDF proof generates successfully.
- The semantic-scene and 2,048-label release wind tunnels execute through the
  unchanged public path and retain their cache/work invariants.
- Repository, proof-ledger, copyright, formatting, spelling, Cargo metadata,
  and Beads policy checks pass.
- No `unsafe` appears in `underwood/src`.

## Adversarial review

**Summary judgment:** accept after the final scene-model split.

- **Must fix:** none.
- **Should fix:** the first decomposition left a 1,616-line scene model owning
  output accounting, interaction policy, and visual records. It was split into
  `output.rs`, `interaction.rs`, and `records.rs`.
- **Could improve:** split the 990-line interaction module only when another
  stable owner emerges. Dividing the two parallel committed/projected scene
  implementations today would create duplication-oriented modules instead of
  invariant-oriented ones.
- **Suggested tests:** retain the existing public-path workspace suite,
  portability checks, exact visual snapshot, PDF proof, and wind tunnels as
  blocking gates for future moves.

Good catch: the final model split made construction-only access explicit.
Those fields are now visible only to sibling scene implementation modules
through `pub(super)`, not to the crate or downstream callers.
