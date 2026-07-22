# Live semantic-document showcase review — 2026-07-22

- **Scope:** heading semantics, retained document preparation, native host,
  imaging adapter, CPU rendering, and meeting specimen
- **Design:** Design-0008
- **Bead:** `und-oh0.10.1.9`
- **Review modes:** Lynx correctness, Rook real-versus-mirage, Cedar API and
  presentation
- **Unsafe watch:** no `unsafe` added
- **Dependency watch:** native and rendering dependencies are example-only;
  production dependency edges are unchanged
- **Human gate:** the release-mode live demonstration succeeded on 2026-07-22
- **Remote gate:** GitHub Actions run
  [`29911682384`](https://github.com/forest-rs/underwood/actions/runs/29911682384);
  all eight required jobs passed

## Result

This is a real live retained-document proof. One `DocumentSnapshot` with H1,
H2, body, mixed LTR/RTL text, variable type, OpenType feature overrides, and a
local edit passes through one `LayoutEngine::prepare` call into one
`TextScene`. The public scene is recorded with `imaging`, rasterized by
`imaging_vello_cpu`, converted to softbuffer's channel contract, and presented
in a resizable native window.

The safe claim is: **Underwood's first live retained semantic-document proof.**
It is not yet a complete document-layout product.

## Review findings resolved

1. **Paint-only work initially changed two paint slots.** The leaf slots are
   now stable; only `PaintTable` brushes change. The regression requires zero
   analysis, shaping, flow, and geometry work with all ten paragraphs reused.
2. **The first gradient was assigned to the default body style instead of the
   H1 style.** The H1 now owns a dedicated gradient slot, every non-title
   fragment is asserted not to use it, and the body returns to flat ink.
3. **Process-global animation time made toggles jump and contaminated other
   work reports.** A pauseable clock preserves phase, reaches both extrema
   deterministically, and pauses before edit, paint, or guide actions.
4. **Frame scheduling drifted by render duration.** Deadlines now advance from
   the previous cadence and catch up after overruns.
5. **Physical letter keys assumed a keyboard layout.** Letter controls now use
   logical characters.
6. **The title reported an unlabeled partial duration and a dead zero counter.**
   It now separates `prep` from imaging recording plus CPU `render`, and omits
   break-repair evidence that this corpus does not exercise.
7. **Short viewports could hide content without an explicit state.** Default
   dimensions are tested to fit; the minimum supported viewport reports
   `CLIPPED`.
8. **Arabic correctness was visually plausible but showcase-local evidence was
   thin.** The test now requires the exact bundled fallback bytes, odd bidi
   level, `Arab` script, descending visual source ranges, and visible
   zero-advance mark coverage.

Good catch: the mistaken body gradient looked attractive enough to survive a
casual glance. Binding the visual claim to the title's exact source identity is
what turned that review into a durable proof.

## What is real

- One semantic document and one portable scene, not separately positioned text
  labels.
- Width-only reflow without reshaping.
- Exactly one reshaped paragraph and nine reused siblings for edit and axis
  changes.
- A brush-value-only paint change with all paragraph formation and geometry
  reused.
- Four-versus-six glyph OpenType ligature output.
- Three distinct Roboto Flex width-axis instances.
- Real Fontique fallback and RTL source/ink evidence.
- A gradient brush carried by Underwood's paint table through the CPU renderer.
- Event-driven idle behavior and optional approximately 30 Hz axis animation.

## What remains outside the proof

- Heading roles do not yet drive block styling or margins.
- `TextScene` output vectors are rematerialized during preparation even when
  paragraph formation and geometry are reused; `paint 9` means nine paragraphs
  were lowered, not nine invalidations.
- Both displayed durations exclude RGBA-to-softbuffer conversion and present.
- There is no scrolling, pagination, native accessibility tree, caret UI, or
  general authored-document parser.
- Local release timings are smoke observations, not accepted performance
  budgets.

## Validation

The focused showcase suite covers semantic roles, width reflow, exact sibling
reuse, paint-only negative work, variable coordinates, feature substitution,
Arabic fallback/ink/source evidence, gradient ownership, animation timing,
keyboard mapping, channel conversion, and vertical clipping. Repository-wide
formatting, TOML formatting, all-target/all-feature clippy with warnings denied,
workspace tests, denied-warning rustdoc, repository/text/Beads policy, Rust
1.92, and bare-metal plus WebAssembly portability all pass locally.
GitHub Actions repeated the complete matrix successfully on Linux, macOS, and
Windows in run `29911682384`.

At the default window size on the development Mac, release-mode smoke frames
observed roughly `0.1–0.4 ms` for preparation and `3.9–6.9 ms` for imaging
recording plus CPU rasterization. These values demonstrate why the two phases
are displayed separately; they are not performance thresholds or
cross-platform claims.
