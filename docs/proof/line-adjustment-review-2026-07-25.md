<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Line-adjustment review

- **Date:** 2026-07-25
- **Bead:** `und-oh0.13.8`
- **Design:** Design-0014 and ADR-0005
- **Scope:** resolved paragraph direction, logical and physical alignment,
  Western inter-word justification, trailing whitespace, retained
  invalidation, portable geometry, composition, PDF, visual proof, and
  measured adjustment churn

## Result

Underwood now adjusts an accepted line inside its exact finite-width or region
slot without changing canonical shaping or its chosen source boundary.
`ParagraphStyle::with_alignment` accepts `Start`, `End`, `Left`, `Center`,
`Right`, and `Justify`. `Start` and `End` consume the direction already
resolved by the paragraph backend's Unicode analysis, including
`BaseDirection::Auto`.

Western justification expands explicit eligible U+0020 inter-word spaces on
soft-wrapped lines. Final and mandatory-break lines remain logical-start
aligned. Arabic and CJK adjustment remain separate capabilities rather than
receiving a misleading generic space-stretch implementation.

The adjusted coordinates are the scene coordinates. Glyphs, clusters, hit
slices, carets, selection rectangles, semantic bounds, PDF output, the CPU
visual proof, and the native showcase do not apply independent renderer-side
translations.

## Usage

```rust,ignore
use underwood::{BaseDirection, ParagraphStyle, TextAlignment};

styles.set_paragraph_style(
    paragraph,
    ParagraphStyle::new(BaseDirection::Auto)
        .with_alignment(TextAlignment::Justify),
);
```

The immutable result remains observable without exposing mutable shaping
state:

```rust,ignore
for line in scene.lines() {
    let adjustment = line.adjustment();
    println!(
        "{:?}: offset={}, spaces={}, expansion={}",
        adjustment.direction(),
        adjustment.inline_offset(),
        adjustment.expanded_opportunities(),
        adjustment.opportunity_expansion(),
    );
}
```

## Glossary

- **Accepted slot:** the exact contiguous inline interval in which formation
  accepted one source-complete line.
- **Canonical shaping:** reusable font-backed glyph facts before placement or
  justification changes.
- **Logical edge:** start or end resolved from the analyzed paragraph
  direction.
- **Physical edge:** left or right regardless of paragraph direction.
- **Hanging whitespace:** source-complete logical trailing whitespace excluded
  from the aligned visible-content width.
- **Adjustment opportunity:** an explicitly marked Western inter-word space
  eligible for expansion on a soft-wrapped line.

## Ownership and invariants

The boundary is:

> Formation chooses a source boundary and exact slot. Adjustment translates
> that accepted line or distributes explicit eligible-space deltas. It does
> not analyze, select fonts, shape, rebreak, or choose another slot.

The Parley adapter owns the facts needed by that operation:

- `PreparedParagraph::resolved_direction` retains the Unicode-analysis result;
- `PreparedLine::trailing_whitespace_advance` identifies source-complete
  logical trailing whitespace;
- prepared interaction units explicitly mark Western opportunities;
- the mark is emitted only for exact U+0020 units whose resolved shaping
  script is Latin, Greek, or Cyrillic.

Underwood validates those facts against projected text and authored paragraph
direction. An explicit LTR or RTL request that conflicts with backend analysis
fails closed. A claimed Western opportunity that does not project to exactly
one ordinary space also fails closed.

`LineAdjustment` is immutable side data. It records authored alignment,
resolved direction, inline translation, hanging trailing-whitespace advance,
per-opportunity expansion, and expanded-opportunity count. It never mutates
the retained `PreparedParagraph`.

## Placement semantics

Alignment uses visible content width: line advance minus logical trailing
whitespace. The whitespace remains represented, interactive, and included in
the source-complete line advance, but hangs outside the aligned content edge.

For non-overflowing lines:

- logical `Start` maps to left for LTR and right for RTL;
- logical `End` maps to right for LTR and left for RTL;
- physical `Left` and `Right` do not change with direction;
- `Center` divides remaining visible-content space evenly;
- eligible `Justify` divides all remaining space evenly over marked Western
  opportunities.

When visible content overflows the slot, every alignment falls back to logical
start rather than shifting an already-overfull line away from its reading
origin. Unconstrained min-content and max-content requests retain a zero
translation because they have no external slot to align within.

## Geometry and interaction

Adjustment is resolved once while cached scene geometry is built. The same
translation and expansion update:

- `SceneLine` bounds and advance;
- fragment glyph positions and adjusted space advances;
- cluster and per-semantic hit slices;
- the reciprocal caret-coordinate map;
- visual and logical selection geometry;
- paragraph and inline semantic bounds;
- committed document and projected composition scenes.

Western expansion uses source identity to match each eligible interaction unit
to exactly one glyph. Missing or duplicate glyph ownership fails with
source-coverage error rather than publishing inconsistent hit and paint
geometry.

PDF lowering already consumes public scene glyph positions, so no
`underwood_pdf` alignment branch was added. The proof document centers its
heading and logically end-aligns its footer and asserts both nonzero
adjustments before Krilla export.

## Correctness evidence

The focused real-Parley tests prove:

- `auto_rtl_start_and_end_consume_the_analyzed_paragraph_direction` resolves
  `Auto` RTL once and changes only adjustment and geometry;
- `physical_left_and_right_ignore_rtl_logical_edges` keeps physical edges
  independent of RTL logical start/end;
- `empty_explicit_rtl_paragraph_keeps_its_caret_on_logical_start` places the
  represented empty caret at the right edge;
- `center_moves_mixed_bidi_paint_hits_carets_selections_and_semantics_together`
  compares glyphs, hits, carets, disjoint visual selections, and semantic
  bounds before and after one shared offset;
- `composition_projection_consumes_the_same_alignment_geometry` moves active
  mixed-script preedit glyphs and marked-text geometry through the same
  adjustment-only cache path;
- `western_justification_expands_only_eligible_soft_wrapped_lines` fills an
  exact region slot, leaves final and mandatory lines unexpanded, and proves
  Arabic does not borrow the Western strategy.

Core adjustment tests cover logical edges, trailing whitespace, centering, and
overflow fallback. The malicious-adapter test rejects an explicit direction
that conflicts with reported analysis.

## Product proof

Three product artifacts now teach the same public path:

- the deterministic CPU poster visibly fills the first mixed LTR/RTL line
  through real Western adjustment while the final line stays natural width;
- the native release showcase centers its variable-font heading and deck,
  justifies its mixed-script flowing body, and centers its width-axis specimen;
- the deterministic PDF proof centers its title, justifies its mixed-script
  body, and logically end-aligns its footer using the exact scene geometry
  exported to Krilla.

The visual snapshot is committed and pixel-tested. The showcase test asserts a
nonzero centered title offset and at least one genuinely expanded soft line.
The PDF test asserts real centered, justified, and end-aligned line
adjustments before byte-deterministic export. A separate margin trap requires
the visible right edge of a justified soft line to exactly equal the
end-aligned footer edge.

## Retained invalidation

Alignment is deliberately excluded from the formation cache key but included
in the final geometry-reuse decision. Changing only alignment:

- reuses projection and Unicode analysis;
- reuses itemization and selected fonts;
- reuses canonical and line-final shaping;
- reuses accepted source boundaries, slots, and the region transcript;
- recomputes adjustment and scene geometry.

`WorkReport::adjustment` makes that stage visible. A fully repeated request
reports no adjustment or geometry work.

## Performance and allocation evidence

The `underwood_label_benchmark` crate exercises a retained, multi-line public
`TextBlock` inside an exact rectangular region. Five release trials used 200
rounds of 512 labels per scenario, or 102,400 operations per trial:

| Workload | Median ns/label | Five-trial range |
| --- | ---: | ---: |
| Fully retained adjusted output | 22,484 | 22,354–23,247 |
| Center/end adjustment churn | 100,086 | 99,706–100,313 |
| Start/Western-justify churn | 100,202 | 100,050–100,535 |

The adjustment-only paths assert zero analysis, itemization, font selection,
canonical shaping, line-final shaping, and formation work on every operation.
The measured cost is scene reprojection. Western expansion adds only 0.1% over
translation at the median on this fixture.

The macOS full-allocation trace, after subtracting matched primed process
states, reported:

| Workload | Allocation calls | Allocated bytes |
| --- | ---: | ---: |
| Fully retained adjusted output | 965 | 104,344 |
| Center/end adjustment churn | 1,752 | 353,421 |
| Start/Western-justify churn | 1,756 | 353,533 |

These one-label traces include cloning the owned public output. Relative to an
unchanged retained output, rebuilding adjustment and geometry adds 787 calls
and 249,077 bytes for alignment and 791 calls and 249,189 bytes for
justification. This is explicit evidence for the preparation-trace and scratch
work in `und-oh0.13.11`; it is not hidden behind a claim that retained shaping
makes scene reprojection free.

These are local macOS observations, not portable performance guarantees.

## Public migration

This is an approved pre-stable public API migration:

- `ParagraphStyle` gains `with_alignment` and `alignment`; existing
  construction remains source-compatible and defaults to logical `Start`;
- `TextAlignment`, `ResolvedDirection`, and `LineAdjustment` are new public
  types;
- `SceneLine::adjustment` and `WorkReport::adjustment` are additive
  observations;
- `PreparedParagraph::try_new` now requires one `ResolvedDirection`;
- custom paragraph backends must report the result of their own Unicode
  analysis rather than asking Underwood to infer direction from glyph order;
- the existing `PreparedInteractionUnit::try_new` remains source-compatible
  and marks no justification opportunity;
- backends implementing Western expansion opt in with
  `PreparedInteractionUnit::try_new_with_justification`;
- prepared lines expose trailing-whitespace and eligible-opportunity
  observations without changing their constructor.

Custom adapters that do not emit opportunities still support all translation
alignments and honestly leave `Justify` at logical start. There is no fallback
that guesses eligible spaces from glyph IDs or re-analyzes source in scene
geometry.

## Deliberate limits and extension points

- Western justification currently means even expansion of eligible ordinary
  spaces in Latin, Greek, and Cyrillic shaping contexts.
- CJK inter-character adjustment, Japanese punctuation compression, Arabic
  kashida, and generated tatweel are not claimed.
- Authored U+0640 remains ordinary source-complete text; this work does not
  manufacture source characters.
- Last-line justification and author-controlled justification limits are not
  represented.
- Vertical writing modes are not represented.
- A future script strategy may publish richer immutable opportunity data, but
  it must preserve the same source, interaction, and invalidation laws.

## Adversarial review

**Summary judgment:** the slice satisfies Design-0014's accepted-slot fence and
Bead acceptance criteria. It is suitable to land as measured Western
adjustment, not as universal justification.

**Must fix:** none remain.

**Should fix:** scene reprojection allocation is substantial and must remain
visible in `und-oh0.13.11`; no claim of cheap alignment mutation is made.

**Could improve:** the living page can add direct slot-edge overlays and an
interactive alignment control once preparation tracing exists.

**Good catch:** retaining resolved direction from the backend avoids a second
first-strong scan that could disagree with bidi shaping for empty, neutral, or
explicit-direction paragraphs.

No `unsafe` or production dependency was introduced.

## Gate results

The completed local matrix is green:

```sh
cargo fmt --all --check
taplo fmt --check --diff
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo xtask check
typos
bd lint --status all
bd dep cycles
cargo +1.88.0 check --workspace --all-targets --all-features --locked \
    --exclude underwood_showcase \
    --exclude underwood_visual_proof \
    --exclude underwood_pdf \
    --exclude underwood_pdf_proof
cargo check -p underwood -p underwood_parley \
    --target x86_64-unknown-none --locked
cargo check -p underwood -p underwood_parley \
    --target wasm32-unknown-unknown --locked
```
