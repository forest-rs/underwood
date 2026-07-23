# Native editor proof review — 2026-07-22

- **Scope:** native pointer, keyboard, focus, IME event-feed, editor overlays,
  mixed-script fallback, and retained-work evidence
- **Design:** Design-0009, Slice D editor half
- **Bead:** `und-oh0.10.2.4`
- **Unsafe watch:** no `unsafe` added
- **Dependency watch:** no new direct crate dependency; the showcase enables
  the adapter's optional system-font feature, whose transitive platform
  backends remain confined to that external example path
- **Remote gate:** GitHub Actions run `29978412963` passed all eight jobs on
  editor-only PR #16 at commit `2b13db9`

## Result

The native showcase is now a directly editable projection of the same retained
semantic document it renders. Winit events enter toolkit-neutral showcase
policy, resolve against real Parley-derived scene positions, and publish only
through Underwood's revision-checked selection transactions or composition
sessions. Selection, marked text, the selected preedit range, and every caret
are drawn from public scene geometry rather than a second example-owned cursor
model.

The safe claim is: **Underwood has an executable native mixed-bidi editor
proof.** It is not yet a general editor widget, and semantic activation is the
remaining half of Design-0009 Slice D.

## Deterministic trace

The external showcase test suite proves the product path rather than a
detached model:

1. Two pointer-derived carets receive one atomic insertion. The returned
   positions name the published revision; exactly one paragraph reshapes and
   nine sibling paragraphs are reused.
2. A pointer drag across the Latin/Arabic boundary creates one visual selection
   containing more than one logically disjoint range. The ranges are not
   flattened to a logical union.
3. Backspace follows the exact source range of one Parley shaping cluster,
   including a decomposed combining-mark record, rather than deleting a UTF-8
   byte or guessed scalar.
4. Native preedit creates a generated-text composition epoch while the
   committed document revision remains unchanged. The overlay includes the
   full marked range and a candidate-window caret obtained through
   `EditableSurface`; disable cancels, while commit publishes exactly once.
5. Inserting Latin into the Arabic-authored leaf resolves to the bundled Roboto
   Flex bytes. This regression reproduces the human-discovered `MissingFont`
   failure and prevents a visually plausible system fallback from satisfying
   the proof.

## Host and presentation boundary

The showcase owns focus, modifier interpretation, click/drag policy, caret
blink, keyboard commands, Winit IME translation, and the platform candidate
rectangle. Underwood owns selections, validation, composition projection, and
geometry. Parley owns Unicode analysis, bidi visual order, cluster source
ranges, caret affinity, and cursor transitions. No production crate knows
about Winit or imaging.

The title retains the last meaningful shape/flow/reuse observation across
caret-blink frames while measuring current preparation and rendering
separately. Thus an edit can continue to show `shape 1` and `reused 9` instead
of being immediately overwritten by a zero-work blink frame.

## Bugs made durable

- An Arabic-only fallback policy made Latin inserted into Arabic fail with
  `MissingFont`. The specimen now registers explicit real families for both
  `Arab` and `Latn`, and the regression asserts the selected Latin font bytes.
- Logical deletion is now driven by the scene's exact Parley-derived cluster
  transition instead of byte arithmetic. Full extended-grapheme deletion is
  not claimed until the interaction model can preserve multi-source semantic
  ownership.
- Rapid repeated input could observe the previous scene revision after an
  edit. The showcase refreshes its interaction scene before handling the next
  event and retains only transaction-returned post-edit positions.

## Honest boundary

- The showcase currently exercises Winit's event-feed IME model. Rich
  host-driven protocols are proven separately through `EditableSurface`; no
  AppKit, UIKit, Android, or TSF adapter is claimed here.
- Pointer selection is cluster-exact, not inside-ligature-outline precise.
- Clipboard, undo/redo, word movement, scrolling, accessibility projection,
  durable anchors, and cross-leaf replacement remain outside this slice.
- Native hosts may opt into one fixed system-font catalog snapshot so newly
  inserted scripts can reach Fontique fallback without changing deterministic
  core proofs. The macOS Chinese IME regression selects a real system Han font.
  That product path historically exposed `UnsupportedPaintCoverage`; a
  separate deterministic static-font regression proves that synthetic
  emboldening also prepares without outline-derived clips. Design-0010 and
  `und-oh0.2.9`, landed independently in PR #15, remove the renderer
  prerequisite for both cases. The current
  imaging CPU backend still does not claim synthetic-bold pixel fidelity.
- Full Unicode extended-grapheme deletion across shaping records or authored
  semantic leaves awaits a multi-source interaction-unit design. The current
  adapter deliberately preserves leaf-safe clusters instead of manufacturing
  a single-semantic range for multi-semantic text.
- Semantic hover, pressed-state cancellation, and action dispatch remain in
  `und-oh0.10.2.5`.

## Local validation

The focused native showcase suite passes 21 tests, including the five trace
obligations above plus native Han IME commit and true interior Latin-in-Arabic
fallback regressions. The Parley adapter covers ligature, combining,
cross-semantic-leaf, Arabic-mark, mixed-bidi, hard-break, retained-work,
synthetic-embolden, and macOS native-Han cases. Local formatting, Clippy,
workspace tests, rustdoc, MSRV, repository policy, and no-std portability pass.
GitHub Actions run `29978412963` repeats that proof across Linux, macOS,
Windows, Rust 1.92, denied-warning rustdoc, bare metal, and WebAssembly.
