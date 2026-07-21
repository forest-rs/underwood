# Design-0006: Parley-backed paragraph formation

- **Status:** Approved for implementation, upstream reshape gate open
- **Approved:** 2026-07-22 by Bruce Mitchener
- **Bead:** `und-oh0.2.2`
- **Supersedes:** the private glyph-width breaker in Design-0002
- **Authority:** ADR-0002, ADR-0004, `UNDERWOOD_HANDOVER.md` §§15, 17

## Overview

Underwood will stop pretending that a glyph which crosses a rectangle edge is
a line-breaking algorithm. A paragraph engine receives the projected text,
shaping values, inline-flow values, paint topology, and current inline
constraint. It returns portable formed lines: legal source ranges, visual runs,
glyphs, and font-derived line metrics. `TextScene` lowers those records and
stacks paragraphs; it no longer decides where text may break or invents a
baseline from an 80/20 split.

The implementation uses retained `parley_core::ShapedText`. Width-only and
line-height-only changes rerun formation without rerunning Unicode analysis,
font selection, or shaping. Break opportunities and mandatory breaks come from
Parley Core. Committing a break which affects shaping must use Parley Core's
bounded break/concat operation; that last primitive is still absent from pinned
main and remains an explicit upstream gate.

This design does not adopt high-level `parley::Layout` as Underwood's document
architecture, expose a Parley type, implement regions or pagination, or copy
high-level Parley's private layout state.

## Vocabulary

- **Preparation:** Unicode analysis, itemization, font selection, and shaping;
  independent of available width.
- **Formation:** choosing paragraph line boundaries, applying break-sensitive
  reshaping, computing line boxes, and ordering each line visually.
- **Document flow:** placing formed paragraph lines into regions/pages and
  retaining resumable cross-paragraph state; owned by Underwood.
- **Safe break:** a legal boundary whose glyphs are already valid on both sides.
- **Unsafe break:** a committed boundary that must sever a cursive join or
  ligature through bounded reshaping.
- **Oracle:** experiment-only high-level Parley output used to check policy and
  metrics, never a production dependency or representation.

## The first read: one paragraph changes width

```text
semantic paragraph + computed runs
              |
              v
      ParagraphFormation::form
       | retained preparation |  <- same ShapedText after width change
       | legal line formation |
              |
              v
 PreparedParagraph { lines: [PreparedLine, ...] }
              |
              v
 document flow / scene lowering / paint
```

At width 520, the adapter may produce two lines. At width 360, it reuses the
same `Analysis` and `ShapedText`, chooses three legal source boundaries, and
returns three newly formed lines. The work report says zero analysis,
itemization, selection, and shaping work, and non-zero formation work. A paint
value change does not even rerun formation.

## Ownership

| Concern | Owner | Contract |
| --- | --- | --- |
| Unicode boundaries and bidi levels | Parley Core | retained `Analysis`/cluster facts |
| Font selection and shaping | Parley Core + Fontique | retained `ShapedText` |
| Greedy policy over available intervals | `underwood_parley` | small explicit adapter policy |
| Break-sensitive reshape | Parley Core | bounded `apply_break` / `apply_concat` |
| Portable lines and metrics | Underwood adapter contract | no backend types |
| Region/page continuation and checkpoints | Underwood | ADR-0002 flow state |
| Scene geometry, paint, hit testing | Underwood | consumes formed lines |

The adapter's greedy policy is not a Unicode algorithm. It consumes Parley's
legal/mandatory boundary classifications, retains the last legal opportunity,
hangs trailing whitespace consistently, allows an overlong unbreakable unit,
and commits the best boundary when the interval is exceeded. Future Knuth-
Plass, hyphenation, or exclusion-aware policies can replace it without moving
Unicode or shaping ownership.

## Exact upstream snapshot

The production fence remains Parley commit
`6c81e1dd9b67793cdd959c65cc650c96a1262fb7`.

At that commit:

- `Analysis` classifies `Boundary::{None, Word, Line, Mandatory}`;
- `ShapedText` owns logical clusters, visual glyph storage, bidi levels, exact
  font instances, normalized coordinates, and scaled `FontMetrics`;
- high-level `parley::Layout` has a capable resumable greedy breaker, real line
  boxes, CRLF handling, overflow policy, and per-line bidi reordering;
- high-level breaker inputs and mutable state are coupled to private
  `LayoutData`, so an external adapter cannot feed it retained Core output;
- Core exposes neither a line breaker nor bounded break/concat reshaping.

Draft [Parley PR #634](https://github.com/linebender/parley/pull/634)
specifies `ShapeContext::apply_break` and `apply_concat`, unsafe-region discovery,
and a caller-owned greedy-break example. Those operations are not on main. The
Underwood production path must not manufacture equivalent-looking output and
call the gap closed.

## Public contract

The pre-stable trait changes from preparation-only output:

```rust,ignore
impl ParagraphPreparation for MyEngine {
    fn prepare(
        &mut self,
        input: ParagraphInput<'_>,
    ) -> Result<ParagraphPreparationOutput, PreparationError>;
}
```

to paragraph formation:

```rust,ignore
impl ParagraphFormation for MyEngine {
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, FormationError>;
}
```

`ParagraphInput` adds an interned `InlineFlowStyle` table and covering
`InlineFlowRun`s. `ParagraphConstraints` initially carries one validated finite
maximum inline advance. It is a record, not a bare `f32`, so disjoint intervals
and continuation state can be added deliberately without changing the meaning
of the first field.

`PreparedParagraph` changes from whole-paragraph runs to source-ordered
`PreparedLine`s. Each line owns:

- a paragraph-local source range and explicit break reason;
- advance, baseline-from-top, and line-box height;
- content ascent/descent evidence;
- visual-order `PreparedRun`s clipped to the line;
- backend-independent glyph, paint coverage, font, synthesis, and source data.

A glyphless line created by a mandatory break is valid. Runs within a line need
not tile the line because control characters can carry source without glyphs;
line source ranges must tile the paragraph exactly, including CRLF as one hard
break event. Visual run order is not source order and constructors validate
coverage without sorting it away.

`PreparationWork` becomes `FormationWork`. Existing analyzer/itemizer/selection/
shaper counters remain, and `formed_lines` plus `break_reshapes` are added. This
is a breaking draft API change, not a compatibility shim.

### Call-site result

The normal public product call does not become more complicated:

```rust,ignore
let mut layout = LayoutEngine::new(ParleyParagraphEngine::new(data, fonts)?);
let output = layout.prepare(&snapshot, &SceneRequest::new(width, &styles, &paint))?;
```

Only adapter implementors migrate from `prepare` to `form`; product callers
continue to supply width once through `SceneRequest`.

## Formation laws

1. Boundary selection walks logical clusters; visual ordering happens only
   after a line range is fixed.
2. A regular line begins at a Parley `Boundary::Line`; an explicit line ends at
   a mandatory boundary. A regular break never splits a grapheme or ligature.
3. CRLF emits exactly one explicit break and both code points remain covered.
4. An unbreakable unit may overflow; it is never split merely to fit.
5. Line metrics use each contributing run's scaled ascent/descent and the
   requested line height. Half-leading is distributed around the font box; no
   percentage constant substitutes for font metrics.
6. UAX #9 L2 run reordering is applied per formed line. Glyphs inside an RTL run
   remain in Parley's visual order.
7. Committing an unsafe break reshapes only Parley's reported bounded region.
   Removing it restores the unbroken glyphs and advances.
8. Width is absent from analysis and shape identities. Inline line height is
   absent from those identities but present in formation identity. Paint values
   are absent from all text-physics identities.
9. A failed formation publishes no partial paragraph or scene.

## Executable corpus

| Case | Required observation |
| --- | --- |
| `alpha beta gamma` at a narrow width | lines start only at legal boundaries; no arbitrary glyph split |
| LF, CR, CRLF, U+2028, U+2029 | exact explicit-line count and complete source coverage |
| non-breaking space and a long word | no illegal split; honest overflow |
| mixed `office مرحبا world` | logical line ranges and visual run order both correct |
| mixed sizes and line-height multipliers | baseline/height derived from real metrics and max contributing boxes |
| width A, width B | line output changes; analyzer/itemizer/selector/shaper counters stay zero |
| line height A, line height B | metrics change; shaping stays byte-for-byte identical |
| Arabic join or discretionary `fi` seam | glyphs change after break; concat restores original output |
| paint value only | no formation; same line/glyph geometry |

The experiment crate runs applicable cases through high-level Parley as an
oracle. The product tests run them through `LayoutEngine` and assert work, source
coverage, and geometry rather than merely comparing pixels.

The pinned high-level oracle currently breaks after an overflowing NBSP. That
observation is locked as divergence evidence, not copied into the product's
legal-break policy.

## Migration note

`CHANGELOG.md` / `Unreleased` will say:

```text
### Draft API

- Replaced `adapter::ParagraphPreparation::prepare` with
  `adapter::ParagraphFormation::form`. Formation now receives validated inline
  constraints and inline-flow runs and returns source-complete `PreparedLine`
  records with font-derived metrics. Adapter implementations must move their
  whole-paragraph `PreparedRun`s into formed lines and report formation work;
  `LayoutEngine` callers are unchanged.
```

No deprecated bridge is retained. The API is pre-stable and the previous shape
would preserve the wrong ownership.

## Rejected options

### Keep breaking in `TextScene`

Adding boundary flags and metrics to `PreparedGlyph` would improve symptoms but
leave text physics hidden in scene lowering. It also makes break-sensitive
reshaping impossible without a callback into the adapter. Rejected.

### Use high-level `parley::Layout` in production

It would repeat style resolution and shaping, couple paint to Parley's layout
representation, and discard the retained Core seam just established in
Design-0005. Its private state is useful oracle evidence, not Underwood's
architecture. Rejected.

### Copy Parley's high-level breaker

The private breaker is roughly 1,500 lines and depends on private layout items,
alignment, boxes, and style resolution. Copying it creates a fork and obscures
the small policy Underwood actually needs. Rejected.

### Land safe breaks and declare victory

Legal wrapping and real metrics without break-sensitive shaping would leave an
acceptance criterion false. The branch may make that progress, but the bead and
PR do not become complete until the upstream seam is executable. Rejected as a
landing claim.

## Upstream packet for Tom

Underwood needs a reviewable extraction from PR #634, not the entire umbrella:

1. On current `parley_core::ShapedText`, expose whether a cluster boundary is
   safe to break/concat and the minimal affected ranges.
2. Add bounded `Shaper::apply_break` and `apply_concat` using the original
   analysis, shape options/font-selection context, and retained output.
3. Guarantee no-op behavior at safe boundaries, contiguous source coverage,
   bounded work, and break+concat equivalence.
4. Include Arabic cursive and Latin ligature tests plus a caller-owned greedy
   break example.

Underwood can supply its wind-tunnel corpus and consume an immutable main
revision immediately. Until that lands, the production implementation and
oracle can prove every other formation law, while the unsafe-break test remains
a named upstream gate rather than hidden technical debt.
