<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Editable TextBlock operations review

- **Date:** 2026-07-25
- **Bead:** `und-oh0.13.9`
- **Design:** Design-0014
- **Scope:** complete-scene endpoints, represented caret resolution, logical
  word movement, atomic block replacement, revision rebinding, and the parked
  Overstory editor call site

## Result

Underwood now provides the small revision-correct operation set needed by a
toolkit-owned single-line text control without adding a retained editor
abstraction:

- `TextScene::start_position` and `end_position` return the logical endpoints
  of the complete prepared scene, including scenes with several paragraphs;
- `TextScene::position_at` resolves only a caret actually represented at one
  leaf-local UTF-8 boundary;
- `previous_word_position` and `next_word_position` consume boundary and
  whitespace facts retained from the existing Parley Engine analysis;
- `TextBlockSnapshot::text_id` exposes the stable semantic leaf identity;
- `TextBlock::text` observes the current value; and
- `TextBlock::replace_selections` delegates to the existing atomic document
  transaction and returns collapsed selections bound to the published
  revision.

No new dependency, `unsafe`, Unicode segmenter, selection representation,
transaction path, or editor object was added.

## Ownership fence

```text
Overstory text control
  keyboard / pointer / focus / IME / widget state
                    |
                    v
Underwood TextScene
  represented positions / movement facts / selections
                    |
                    v
Underwood TextBlock
  one atomic replacement / one published revision
```

Underwood owns dense revision-bound positions, source-complete selections,
adapter-derived movement facts, and publication. The toolkit owns input
policy, modifier meanings, focus, scrolling, accessibility actions, and
whether the control is single-line. This deliberately leaves the existing
host-driven and event-feed IME models unchanged.

## Exact behavior

`position_at` is a resolver, not a byte-position constructor. It rejects a
foreign leaf, an interior UTF-8 byte, and a semantic leaf seam inside one
extended grapheme. When a bidi discontinuity or soft wrap represents two
affinities at one byte, leaf byte zero prefers downstream affinity and other
bytes prefer upstream affinity, then the method accepts either exact
represented stop.

Complete-scene endpoints select the earliest paragraph-local logical start
and latest paragraph-local logical end in document order. They do not confuse
the end of the first paragraph with the end of the document, as the parked
prototype did.

Word movement is logical rather than visual. It filters prepared interaction
units to non-whitespace Unicode boundary candidates, orders them by semantic
paragraph, leaf, and byte identity, and resolves the selected candidate
through the represented-caret map. At either extreme it returns the scene
endpoint. A stale, foreign, or unrepresented input position returns `None`.

Replacement retains the existing multi-selection contract: one insertion per
independent selection, even when one visual bidi selection contains several
logical ranges. The complete set validates before mutation, publishes once,
and returns one collapsed post-edit selection per input selection.

## Correctness evidence

The real Parley-backed regression traps prove:

- empty text has one start, end, word-navigation, and caret position;
- complete endpoints cross semantic paragraph boundaries;
- leaf starts and ends retain their distinct affinities;
- interior UTF-8 bytes and foreign leaves do not fabricate carets;
- a grapheme split across semantic leaves has no editable interior seam;
- collapsed whitespace across leaves preserves Arabic and Latin logical word
  starts;
- mixed bidi word movement follows logical source order;
- selection replacement updates the block once, preserves the stable
  `TextId`, leaves the old snapshot immutable, and returns a caret accepted by
  the newly prepared scene; and
- the old scene rejects the new-revision caret.

The scene queries perform no allocation. Endpoint resolution and exact
position resolution scan existing private movement records. Word movement
scans retained cluster facts and resolves only the chosen candidate, rather
than allocating a second word map.

## Overstory consumer proof

The parked Overstory checkpoint
`75e22e5d0c4141767d131d237e781bc5ee1ac16f` was extracted into a disposable
directory and checked against this Underwood worktree. The proof changed no
live Overstory worktree.

As required by the existing dependency gate, crates.io Parlance was patched to
the same Parley revision consumed by Underwood. Two unrelated stale
Understory presentation patterns in `ui/paint.rs` were also updated in the
disposable copy. With those already-known integration fixes:

```text
cargo check -p overstory --lib --offline
    Finished successfully
```

The real `TextInputEditState` directly uses `TextBlock::text`,
`TextBlockSnapshot::text_id`, `TextScene::position_at`, scene endpoints, word
movement, selections, and `TextBlock::replace_selections`. It contains no
duplicate Unicode navigation, byte mutation, or transaction implementation.
That is calm enough that a new Underwood single-line editor façade would move
toolkit policy into the wrong crate.

The full parked library test run remains at 552/555, the checkpoint's same
three intentional integration failures: Overstory has not yet lowered
metrics-relative line height, its runtime paragraph override does not yet
lower finite-width logical alignment, and its prepared-output/cache policy
does not yet enable identical-label sharing. The focused TextInput run passes
25/26; its sole failure is that separately tracked paragraph-alignment
lowering, not an editing operation.

## Public migration

This approved pre-stable change is additive. Existing document mutation,
selection, movement, hit-testing, and `TextBlock::set_text` call sites keep
their behavior. Toolkit controls can remove private byte-position
construction, word segmentation, and selection-rebinding helpers and call the
new scene and block methods instead.

The returned positions and selections remain dense snapshot values. Callers
must prepare the new block revision after replacement and must not persist or
apply the returned carets to an older scene.
