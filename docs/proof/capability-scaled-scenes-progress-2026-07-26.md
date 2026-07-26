# Capability-scaled scene progress — 2026-07-26

## Status

This checkpoint implements the first approved Design-0018 boundary. It is not
the completion proof.

Prepared scenes now distinguish display, source provenance, semantics, hit
testing, selection, navigation, and native text input. A uniform request is
allocation-free to construct, while `SceneFeaturePolicy` supports sparse
paragraph overrides without promoting unrelated siblings. `BlockRequest`
supports the same capability vocabulary.

Display-only scene segments physically omit the source, semantic, hit,
selection, movement, and native-input records that they did not request.
Underwood's Parley adapter also omits cursor-movement lowering below the
selection tier. A warm capability upgrade shares the existing analysis,
shaping, line formation, and immutable layout facts while adding only the
requested sidecars.

The remaining Design-0018 work is deliberate and tracked by
`und-oh0.13.17.10`:

- separate, byte-accounted adapter-fact residency and eviction;
- explicit warm-versus-cold upgrade diagnostics after adapter-fact eviction;
- the paragraph-local compact source map and run-sized paint records;
- requested-versus-resident byte diagnostics;
- cold-label, upgrade, 2,048-sibling mixed-document, editable-typing, and
  creation/destruction wind tunnels.

## Public migration

`SceneRequest::new` and `BlockRequest::new` now request
`SceneFeatures::DISPLAY`. Callers opt into more retained facts with
`with_features` or, for documents, `with_feature_policy`.

The named profiles are convenience closures over one capability lattice:

```rust
let request = SceneRequest::new(constraint, &styles, &paint)
    .with_features(SceneFeatures::EDITABLE);
let output = layout.prepare(&snapshot, &request)?;
let editing = output.scene().editing()?;
let hit = editing.hit_test_closest(point);
```

Sparse document requests keep static siblings lean:

```rust
let features = SceneFeaturePolicy::uniform(SceneFeatures::DISPLAY)
    .with_paragraph(editor, SceneFeatures::EDITABLE);
let request =
    SceneRequest::new(constraint, &styles, &paint).with_feature_policy(features);
```

Capability-dependent observations moved from `TextScene` and
`CompositionScene` onto checked borrowed facades:

- source traversal uses `scene.sources()?` and
  `SceneSourceAccess::{for_line, for_fragment, for_glyph}`;
- semantics use `scene.semantics()?.iter()`;
- point queries use `scene.interaction()?`;
- selection construction and geometry use `scene.selection()?`;
- navigation, editing, and composition start use `scene.editing()?`;
- transient composition interaction and editing use the corresponding
  `ProjectedScene*` facades.

Display traversal remains unconditional through `scene.display()`. Existing
`scene.lines()` and `scene.fragments()` traversal remains a display
observation, but line, fragment, and glyph provenance is available only
through the source facade.

There is intentionally no compatibility shim that materializes missing data.
`MissingSceneCapability` reports the required, originally requested, and
resident capability closures, plus the affected paragraph when it is known.

## Executable evidence

Focused regressions prove:

- a default display scene rejects interaction and has no source, selection, or
  editing facade;
- the error reports requested and resident capabilities rather than returning
  an indistinguishable empty value;
- one sparse editable paragraph does not promote a display-only sibling;
- display preparation omits Parley cursor movements;
- a warm editable upgrade repeats no analysis, shaping, or line formation;
- renderer, PDF, showcase, IME, headless, and visual-proof consumers all use
  explicit capabilities and checked facades.

The checkpoint passes:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The final proof will add the Rust 1.88, `no_std`, rustdoc, repository, and
matched allocation/residency matrices after the remaining representation and
budget work is complete.
