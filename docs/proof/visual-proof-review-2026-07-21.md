# Visual-proof Lynx and Rook review — 2026-07-21

- **Scope:** computed inline styles, the Parley adapter, public examples, and
  the deterministic CPU poster
- **Review modes:** Lynx adversarial correctness; Rook real-versus-mirage audit
- **Snapshot:** 1600 × 1000 RGBA8, PNG SHA-256
  `8f09637ec658c345dbe4a511082b4c947057d7f4eb4443ec4bf63a70304bdc6a`
- **Unsafe watch:** no `unsafe` in Underwood-owned Rust
- **Remote gate:** pending the current pull request's Linux, macOS, and Windows
  snapshot matrix
- **Local result:** exact snapshot reproduction and the complete semantic
  evidence suite pass

## Lynx review

### Summary judgment

The poster is now a real heterogeneous-style consumer, not a collage standing
in for an absent API. One semantic document projects complete computed styles
into paragraph-local shaping and flow tables, the Parley adapter executes those
values, `TextScene` retains the resulting font instances and source identity,
and `imaging_vello_cpu` renders the checked-in pixels.

### Must — resolved

1. **The first projection merged adjacent values but did not actually intern
   style partitions.** Repeated non-adjacent shaping and flow values now reuse
   deterministic paragraph-local dense IDs, with an executable A/B/A test.
2. **An empty paragraph's default style was absent from its flow cache key.**
   The computed empty-line height now has an explicit geometry identity; a
   default line-height change reflows only the empty paragraph without shaping.
3. **The first variable specimen changed `wght` and `opsz` together.** The
   checked-in row now moves from `100/8` to `900/8` and then to `900/144`, while
   assertions require distinct normalized coordinates at each single-axis step.
4. **A zero-work paint assertion did not prove that a new slot reached retained
   fragments.** The headless public path now also locates the affected source
   fragments and requires their reassigned paint slot.
5. **The design signature claimed `const` constructors that the owned style
   representation cannot provide.** The approved API sketch now matches the
   implemented ordinary functions.
6. **The old review described four default ligature labels and said the style
   path could not select axes or features.** It has been replaced by this audit
   of the computed-style implementation and current snapshot.

Good catch: isolating one variable axis at a time turns a visually plausible
font specimen into attributable shaping evidence.

### Should

- Add automatic optical sizing only with a specified used-value contract; an
  absent `opsz` currently and deliberately means the font default.
- Keep unsupported feature and axis tags deterministic no-ops, then add a
  named conformance case if higher-level diagnostics become a product need.
- Keep the dense style IDs paragraph-local and backend-facing; do not let them
  become durable document or serialization identities.

### Could

- Extract the complete-run projection only after a second real consumer proves
  the storage contract.
- Add another script only when it exercises a named fallback, cluster, or
  shaping boundary beyond the existing Latin/Arabic proof.

### Suggested tests

- Canonical feature and variation ordering, duplicate-last-wins, and finite
  number validation.
- A/B/A partition interning for shaping and inline-flow values.
- `liga` on/off exact glyph counts from the public path.
- `wght`-only and `opsz`-only normalized-coordinate changes.
- Shaping-style changes that reuse Unicode analysis.
- Line-height-only flow work, including an empty paragraph.
- Paint-slot assignment and brush-value changes with no shaping or flow work.
- Exact RGBA comparison with the committed CPU snapshot on every host matrix.

## Rook audit

### Mirage risks

- **Mirage:** this is not an authored-style system or cascade. Callers provide
  complete computed values directly.
- **Mirage:** explicit `wght` coordinates do not implement font-family,
  weight, width, or style matching.
- **Mirage:** explicit `opsz` coordinates do not imply automatic optical sizing.
- **Mirage:** line height is currently a validated multiplier over provisional
  first-slice line metrics, not CSS inline formatting or mature paragraph flow.
- **Mirage:** letter spacing, word spacing, baseline shift, decorations,
  paragraph breaking, and block layout remain deferred.
- **Mirage:** the example-local imaging adapter is not a production renderer
  package or a general text-rendering conformance suite.

### Real strengths

- **Real:** the public style is partitioned into shaping, inline flow, and paint
  identities with negative-work assertions at each boundary.
- **Real:** paragraph projection emits interned shaping and flow tables plus
  complete contiguous runs; it is no longer only described as doing so.
- **Real:** `underwood_parley` retains Unicode analysis across style changes and
  passes per-run size, language, features, and variations into Parley shaping.
- **Real:** one poster specimen is one three-paragraph semantic document with
  mixed sizes, line heights, paints, feature settings, and variable instances.
- **Real:** `office` produces four glyphs with `liga=1` and six with `liga=0`.
- **Real:** the displayed axis instances retain different normalized
  coordinates after changing exactly one axis at each step.
- **Real:** the mixed LTR/RTL line, split-ligature clips, local edit, retained
  sibling, paint-only work, and final pixels all execute through the same public
  Underwood-to-Parley-to-scene path.

### Most dangerous gap

Underwood still wraps provisional prepared glyphs itself. Consequently, the
current line-height and heterogeneous-size proof is genuine for this
first-slice geometry, but it is not evidence for Parley's future retained
paragraph breaking, hit testing, or caret topology. Beads `und-oh0.2.2`,
`und-oh0.2.3`, and `und-oh0.2.5` keep that migration explicit.

### Follow-on obligations

- `und-oh0.2.2`: replace provisional glyph wrapping with Parley-backed
  paragraph breaking;
- `und-oh0.2.3`: move cluster hit and caret geometry behind the paragraph seam;
- `und-oh0.2.4`: replace proportional ligature paint coverage with
  conformance-backed coverage;
- `und-oh0.2.5`: retire callback-shaped copy-out when retained Parley results
  land upstream; and
- future style beads: specify font matching, automatic optical sizing, spacing,
  and baseline behavior before exposing those properties.
