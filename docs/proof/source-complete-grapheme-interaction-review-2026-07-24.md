<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Source-complete grapheme interaction proof review — 2026-07-24

- **Scope:** analysis-derived grapheme units, complete semantic provenance,
  caret movement, selection geometry, atomic range transactions, composition,
  and the native showcase
- **Design:** Design-0011
- **Bead:** `und-oh0.10.2.6`
- **Dependency watch:** no dependency added
- **Local gate:** complete
- **Remote gate:** head run `30030655809` and merge-group run `30060216305`
  passed all eight jobs
- **Landed:** PR #20, squash commit `23b94c6`

## Overview

The goal is to make one Unicode extended grapheme the unit of interaction even
when shaping emits several records, source crosses semantic leaves, or an IME
mixes committed and generated provenance. Parley remains the only owner of
grapheme boundaries and paragraph cursor physics. Underwood projects those
facts into revision-bound semantic sources, selection geometry, and atomic
document transactions.

The non-goals are word or sentence movement, cross-paragraph structural edits,
durable anchors, a second Unicode segmenter, platform key policy, and a general
editor widget. The work does not change shaped-glyph paint ownership.

## First read: concept and example

The release showcase still displays `café`, but its final extended grapheme is
authored as two semantic leaves:

```text
EMPHASIS("...cafe") + TEXT(U+0301) + EMPHASIS(". Drag...")
```

Parley analysis says that `e + U+0301` is one unit. The paragraph adapter
retains the base and mark shaping slices inside that unit. The scene returns
both leaf-local sources from a hit, exposes only the unit's two endpoints, and
produces one selection rectangle. Backspace or Delete then publishes one
same-paragraph transaction that removes source from both leaves without
removing, merging, or restyling either leaf.

```text
Parley grapheme boundary
          |
          v
interaction unit -> [base slice, mark slice]
          |
          +--> exact hit semantic + complete source
          +--> two carets + one selection rectangle
          +--> two leaf-local deletions + one publication
```

The native application executes that path for pointer placement, left and
right arrows, Backspace, Delete, and a two-caret Backspace transaction.

## Glossary

- **Interaction unit:** one analysis-derived extended grapheme with all visual
  shaping contributions retained.
- **Visual slice:** one shaping-record contribution inside an interaction
  unit.
- **Endpoint owner:** the authored or generated source that owns one selectable
  side of a unit.
- **Source-complete hit:** a hit that returns every source segment in the unit
  while retaining the exact pointed-at semantic identity.
- **Range transaction:** one publication that edits canonical leaf-local
  ranges without changing document structure.
- **Synthetic paint carrier:** a test-only glyph record used to drive
  unbundled-script interaction units through scene APIs; it is not font-shaping
  or pixel evidence.

## Usage example

Application policy asks the scene for movement and gives the returned ranges to
the document. It does not segment text or derive byte offsets:

```rust
let hit = scene.hit_test_closest(pointer).expect("text is selectable");
let caret = scene.collapsed_selection(hit.position())?;
let carets = scene.selection_set([caret])?;
let deletion =
    scene.move_selections(&carets, TextMovement::PreviousLogical, true)?;
let applied = document.replace_selections(&deletion, "")?;
```

For a grapheme split across leaves, `deletion.primary().ranges()` contains one
canonical range per leaf. The replacement is inserted once at the first
logical range, the edit publishes once, and `applied.selections()` names the new
revision.

## Second read: ownership and invariants

`underwood_parley` reads `Analysis::is_grapheme_start` once. It groups all
formed shaping records into `PreparedInteractionUnit` values, preserves
line-local visual order as `PreparedInteractionSlice` values, rejects partial
coverage or a unit scattered across lines, and derives cursor transitions whose
crossed source is the complete unit. OpenType ligature components remain
separate character units.

`underwood` projects each committed unit into `SnapshotTextUnit`. Ordered
`SnapshotTextRange` values preserve every `TextId`, byte range, and revision.
The composition path retains the more general `ProjectedTextRange`, allowing
one unit to contain an authored base and a generated mark. `TextHit` returns the
complete unit plus the exact visual slice's `SemanticId`; a zero-advance mark
does not gain a fabricated pointer interior.

`Document::replace_selections` accepts canonical ranges across leaves only when
they belong to one paragraph. It validates the complete original-revision plan,
conflicts, UTF-8 boundaries, and checked resulting sizes before staging. It
applies leaf-local operations in reverse document order, inserts once per
independent selection, publishes once, preserves leaf identity and role, and
rebases returned carets through every earlier operation.

Production crates remain `no_std + alloc`. The implementation adds no
dependency and no `unsafe`.

## Proof matrix

| Case | Executable evidence | Observation |
|---|---|---|
| precomposed `é` | `logical_delete_and_backspace_remove_one_extended_grapheme` | one unit, one logical move, one atomic deletion |
| decomposed `e + U+0301` in one leaf | `analysis_units_lock_extended_grapheme_trap_corpus`; `logical_delete_and_backspace_remove_one_extended_grapheme` | analysis and the public product path agree on one complete source |
| decomposed sequence across leaves | `split_leaf_grapheme_is_one_hit_movement_and_atomic_replacement_unit`; showcase arrow, Backspace, and Delete regressions | two sources and endpoint owners survive; one rectangle and one publication result |
| OpenType `ffi` | `exact_interaction_uses_ligature_components_not_glyph_ink`; showcase ligature specimen | three character units remain independently reachable while shaping produces four glyphs for `office` |
| CRLF | `product_path_coalesces_crlf_and_honors_mandatory_breaks`; `logical_delete_and_backspace_remove_one_extended_grapheme`; `mandatory_break_keeps_before_and_after_carets_on_distinct_lines` | one deletion source with carets on opposite sides of the mandatory break |
| emoji ZWJ, regional indicator, spacing mark | `analysis_units_lock_extended_grapheme_trap_corpus`; `unbundled_grapheme_corpus_drives_complete_movements_and_transactions` | real Parley boundaries drive reciprocal public scene movement and one complete transaction |
| Arabic base plus mark | `logical_delete_and_backspace_remove_one_extended_grapheme`; `zero_advance_arabic_mark_uses_unclipped_whole_glyph_paint`; showcase Arabic fallback proof | the mark remains painted and deletes with its base |
| mixed bidi | `visual_bidi_selection_retains_disjoint_ranges_and_set_ownership`; `mixed_bidi_drag_retains_disjoint_logical_ranges`; actionable-text drag regression | visual movement and reciprocal drags retain canonical, possibly disjoint logical source |
| composition-generated mark | `generated_combining_mark_shapes_identically_without_authored_provenance`; showcase preedit regression | one unit contains snapshot and generated sources; cancel publishes nothing and reuses all ten paragraphs |
| committed IME text | showcase preedit commit and native Han fallback regressions | commit publishes once and at least nine sibling paragraphs retain formation |
| multiple carets | `two_pointer_carets_publish_one_atomic_insertion`; `two_pointer_carets_delete_complete_units_atomically` | independent pointer carets insert or delete complete units in one publication |
| shared-leaf multicaret | `multi_leaf_and_shared_leaf_multicaret_rebases_from_original_revision` | original-coordinate conflict checks and returned carets are deterministic |
| stale, foreign, overlap, malformed, cross-paragraph | document transaction and adapter validation regressions | the whole operation fails before staging or publication |

The emoji, regional-indicator, and spacing-mark row deliberately separates
interaction proof from font proof. Parley performs the real Unicode analysis.
A test-only synthetic paint carrier then exercises the production scene,
selection, and document APIs because the repository's deterministic bundled
fonts do not cover those scripts. The row does not claim real shaping,
font-fallback conformance, glyph appearance, or pixels for those specimens.

## Public migration

- `PreparedCluster` and `PreparedLine::clusters` become
  `PreparedInteractionUnit`, `PreparedInteractionSlice`, and
  `PreparedLine::units`.
- `PreparedCursorStep::source` now names the complete extended grapheme crossed
  by a movement.
- Committed `TextHit::source` now returns `SnapshotTextUnit`; callers enumerate
  leaf-local ranges with `sources()`.
- Transient `TextHit` retains `ProjectedTextRange`, now covering the complete
  grapheme and all snapshot/generated provenance.
- `Document::replace_selections` accepts same-paragraph multi-leaf ranges and
  reports `CrossParagraphSelection` for the remaining structural prohibition.
- Exact action routing continues through `TextHit::semantic_id`, not through a
  broad unit-wide semantic rectangle.

The migration is intentionally breaking. No compatibility shim preserves
shaping-record deletion under a cluster-shaped name.

## Adversarial review

### Must

All Must findings are resolved.

- The adapter and scene no longer equate shaping records with grapheme
  interaction units.
- Unit, slice, run, cursor, UTF-8, and source coverage are validated before a
  scene is published.
- Internal shaping-record, semantic-leaf, and generated-source boundaries do
  not become carets or duplicate selection geometry.
- Multi-leaf transactions validate the whole plan before staging and preserve
  semantic structure.
- Native movement and deletion execute through the real release showcase
  document rather than a detached benchmark model.

Good catch: the first final matrix had native multi-caret insertion and native
grapheme deletion as separate facts, and left emoji/Indic movement at the
analysis boundary. Review added the two-caret full-unit Backspace regression
and the explicit unbundled-script scene/transaction proof before this record
could call the matrix complete.

### Should

- Keep the `Analysis::is_grapheme_start` seam aligned with upstream Parley and
  delete any local glue when Parley exposes a smaller reusable lowering
  primitive.
- Add platform-independent font-backed emoji and complex-script corpora when
  the repository adopts an explicit international font-data policy; do not
  smuggle large fixtures into this interaction slice.
- Keep host deletion, word movement, and modifier policy above Underwood's
  logical and visual movement primitives.

### Could

- Add differential cursor traces against platform engines once the platform
  conformance harness exists.
- Add fuzz/property coverage for unit/slice partitions and multi-selection
  transaction plans after deterministic seed and artifact policy is defined.
- Index interaction-unit lookup only after a benchmark shows the current
  line-local scans are material.

## Real-versus-mirage boundary

**Real:** Parley-owned grapheme boundaries, source-complete visual slices,
committed and generated provenance, exact semantic hits, bidi-aware endpoint
movement, nonduplicated geometry, atomic same-paragraph range transactions,
native multi-caret editing, IME publication rules, and retained sibling work
are executable.

**Not claimed:** the test-only carrier is not font shaping or rendering
evidence for emoji or Devanagari. This is not a Unicode word-breaking policy, a
cross-paragraph editor, durable collaborative anchoring, platform text-service
integration, accessibility, clipboard, undo, or a reusable widget. Calling the
showcase a complete editor would be a mirage.

The most dangerous remaining gap is deterministic, font-backed product
coverage for scripts absent from the bundled fixtures. That belongs to the
international text-data and font-conformance work, not to a second segmenter
or a synthetic claim inside this slice.

## Extension points

The representation can add vertical-flow endpoint placement, indexed lookup,
and richer platform conformance without changing source ownership. A future
Parley primitive can replace adapter-local grouping so long as it returns the
same complete unit ranges and shaping slices. Cross-paragraph replacement must
remain a separately designed structural transaction.

## Gotchas and risks

- A zero-advance slice still owns source; omitting it corrupts deletion even
  when pixels look correct.
- A unit-wide semantic identifier would make adjacent inline roles behave like
  one broad action. Use the exact hit slice's identity.
- Sorting ranges after publication is too late. Callers provide canonical
  original-revision order, and validation rejects malformed plans atomically.
- Composition preparation may precompute work later reused by commit, so the
  changed paragraph can report zero or one new shape operation. The durable
  invariant is one publication and at least nine retained siblings.

## Local gate record

The final branch passes Rust and TOML formatting, copyright headers, spelling,
locked Cargo metadata, and `cargo xtask check`. Workspace Clippy denies all
warnings across every target and feature; workspace tests and doctests pass;
rustdoc denies warnings; Rust 1.92 checks every workspace target and feature;
and `underwood` plus `underwood_parley` check for
`x86_64-unknown-none` and `wasm32-unknown-unknown`.

The native application also builds and runs with
`cargo run --release -p underwood_showcase`. Visual inspection confirms that
the semantic-leaf split does not change the rendered specimen, Arabic marks
remain present, and the document retains its prior layout.

GitHub Actions repeated the eight-job matrix on the final pull-request head
across Linux, macOS, Windows, Rust 1.92, denied-warning rustdoc, repository
policy, bare metal, and WebAssembly in run `30030655809`. Merge-group run
`30060216305` repeated the same matrix against the synthesized queue head.
PR #20 then landed through the protected merge queue as squash commit
`23b94c6`.
