# Design-0008: Live semantic-document showcase

- **Status:** Implemented
- **Date:** 2026-07-22
- **Bead:** `und-oh0.10.1.9`
- **Authority:** Design-0001, Design-0003, Design-0006, Design-0007

## Decision

Underwood's first native showcase presents one immutable semantic document
through the existing public retained path. The document carries heading and
body roles, while the showcase resolves their visual treatment explicitly
through `StyleMap`. `ParagraphRole::HEADING_1` and `HEADING_2` are semantic
facts; they do not introduce a block-style cascade or make typography a
property of the role enum.

The host is an unpublished external workspace crate. `winit`, `softbuffer`,
`imaging`, and `imaging_vello_cpu` remain outside the production crates:

```text
winit events
     |
     v
one DocumentSnapshot -> LayoutEngine -> TextScene
                                      |
                                      v
                         imaging -> imaging_vello_cpu
                                      |
                                      v
                              RGBA -> softbuffer
```

Every visible glyph is lowered from that single `TextScene`. Page rails, the
page surface, and optional line guides are presentation geometry and contain no
text.

## Interaction laws

The default specimen contains ten paragraphs and twenty semantic text leaves.
Tests require the following work, rather than relying on window pixels alone:

| Interaction | Required retained observation |
| --- | --- |
| narrower width | more visual lines, zero shaping, non-zero flow |
| local edit | exactly one paragraph shaped, nine siblings reused |
| paint toggle | zero analysis, shaping, flow, and geometry; all ten reused |
| `wght` motion | changed normalized coordinates, exactly one paragraph shaped, nine reused |
| `wdth` specimen | three distinct normalized-coordinate instances |
| `liga` specimen | `office` has four glyphs on and six glyphs off |

The paint toggle changes brush values without changing any leaf's paint slot.
The H1 gradient is itself an Underwood `PaintTable` brush, not an
imaging-specific overlay. Animation pauses before edit, paint, or guide actions
so their work reports remain attributable to the interaction being shown.

## International text evidence

The English and Arabic specimen is one flowing paragraph. The Arabic leaf
requests an absent primary family, selects the bundled Noto Kufi fallback,
retains an odd bidi level and `Arab` script, and exposes non-empty ink coverage
for a zero-advance mark. This is the same renderer-neutral repair proven by
Design-0007, exercised again in the live corpus.

## Host behavior

- Rendering is event-driven while animation is paused.
- Animation advances a persistent deadline rather than sleeping after each
  render, and its phase survives pause/resume without jumping.
- Character shortcuts use logical keys; Space and Escape use named keys.
- The title reports `prep` (`LayoutEngine::prepare`) separately from `render`
  (imaging recording plus CPU rasterization). Buffer conversion and present are
  deliberately outside both numbers.
- A viewport too short for the complete document says `CLIPPED`. The showcase
  does not imply that scrolling already exists.

## Non-claims

This slice is not a general document application. It does not provide
role-driven block styling, authored style cascade, scrolling, pagination,
lists, region flow, native accessibility projection, or editable caret UI.
Authored U+0640 tatweel is ordinary source text and can be shaped; automatic
Arabic kashida justification is separate formation work tracked by
`und-oh0.5.2`.
The title-bar counters are live diagnostic evidence, not a stable telemetry
API. The showcase-local composition types are not candidates for promotion
into production crates.

## Migration

`ParagraphRole` gains the additive `HEADING_1` and `HEADING_2` constants.
Existing callers require no change. Callers that use the new roles must still
assign complete computed styles to their text leaves; Underwood does not infer
visual styling from paragraph semantics.
