# Design-0014: Reusable text preparation and line layout

- **Status:** Approved — 2026-07-24
- **Date:** 2026-07-24
- **Bead:** `und-oh0.13`
- **Extends:** Design-0013, ADR-0002, ADR-0004
- **CSS profile:** Design-0015

## Overview

### Goal

Replace Underwood's remaining fused preparation path with a sequence of
reusable text-engine tools over explicit representations, then use those tools
to deliver source-complete whitespace processing, a resumable line former,
region filling, paragraph style fidelity, alignment, justification,
preparation tracing, and the region-aware living page.

The campaign must leave behind tools that can be used without an Underwood
document, widget toolkit, renderer, or high-level Parley `Layout`. Underwood
remains the orchestrator that preserves semantic identity, retained
invalidation, portable output, and editing behavior.

### Non-goals

- Parsing CSS, implementing its cascade, recreating a browser formatting
  context, or adopting high-level Parley.
- Moving widget, accessibility, callback, or host policy into Underwood.
- Making high-level `parley` a production dependency of a foundational crate.
- Freezing a new crate for every tool before independent consumers and
  dependency pressure justify the boundary.
- Claiming universal script justification from Western whitespace expansion.
- Completing every locale-sensitive `text-transform` rule before line layout
  can proceed.
- Moving Overstory's output-retention or control-lifecycle policy into
  Underwood.

## Fence

Reusable text-preparation tools own explicit transformations between authored
text, projected text, shaped facts, line candidates, accepted region lines,
and adjusted line geometry; they explicitly do not own document storage,
widget policy, region-placement policy, paint, rendering, or final scene
orchestration.

Underwood owns composition of those tools with immutable snapshots, semantic
identity, source projection, retained invalidation, editing, portable scene
records, and proof accounting; it explicitly does not own Unicode analysis,
font-backed shaping, or a private copy of a general shaping engine.

### Relationship to CSS Text

CSS Text is a valuable semantic and conformance source, not an architectural
antigoal. The distinction is between computed text behavior and a browser
formatting context:

1. **Ignore CSS Text.** This avoids web terminology but invites divergent
   whitespace, wrapping, spacing, and alignment semantics.
2. **Implement the browser stack.** This couples text preparation to CSS
   parsing, cascade, DOM box generation, compatibility behavior, and adjacent
   layout specifications that Underwood does not own.
3. **Define an Underwood CSS Text semantic profile.** Reuse the specification's
   computed-value semantics and conformance cases where they fit, while keeping
   authoring syntax and host policy outside the engine.

Option 3 is the recommended extension point. This campaign implements the
concrete behaviors demanded by current consumers and records how each maps to
CSS Text concepts. It does not claim complete CSS Text conformance. A future
profile may cover whitespace processing, text transformation, spacing,
wrapping, line breaking, alignment, and justification without adding a CSS
parser or DOM vocabulary to Underwood.

The fence is: a CSS Text profile may own computed text behavior and its
conformance map; it may not own CSS syntax, cascade, inheritance resolution,
DOM or box generation, browser compatibility policy, or host editing policy.
Underwood must remain source-complete for selection, IME, accessibility, and
editing even where CSS Text is silent about those native application needs.

## Glossary

- **Authored text:** The immutable text and semantic ranges stored in an
  Underwood snapshot.
- **Projected text:** The presentation string plus complete mappings between
  authored and displayed positions after whitespace processing, text
  transformation, composition, or generated content.
- **Shaped facts:** Parley Engine analysis and shaping records before line
  placement.
- **Line candidate:** A reversible proposed source boundary and its provisional
  measurements, not yet accepted into a region.
- **Line slot:** One exact inline interval and block-axis allowance offered by
  caller-owned region policy.
- **Line adjustment:** Placement or advance changes applied after a candidate
  is accepted, including alignment and justification.

## Required invariants

1. Authored text is never silently rewritten to produce presentation text.
2. Every projected byte and every authored byte participates in an explicit
   identity, replacement, collapse, insertion, or omission relationship.
3. Projection supports identity, many-to-one, one-to-many, many-to-zero, and
   zero-to-many relationships without losing semantic ownership.
4. Projection operates over a paragraph stream and can carry state across
   inline leaf and style boundaries.
5. Editing, selection, hit testing, accessibility, and PDF provenance resolve
   through composed mappings back to authored snapshot positions.
6. Unicode analysis and shaping consume projected text. Document transactions
   continue to consume authored positions.
7. A reusable kernel has no dependency on `DocumentId`, `TextScene`, paint,
   widgets, or high-level Parley layout records.
8. Line formation consumes Parley Engine facts plus explicit line-policy side
   data. It does not resolve fonts, shape a paragraph, or choose regions.
9. A proposed line is not irrevocably committed until line-final shaping and
   the exact region slot both accept its width and height.
10. Restoring a line checkpoint restores traversal state and truncates every
    provisional output owned by the former.
11. Region policy supplies slots and places floats, exclusions, and columns.
    The line former does not contain a float or page-layout algorithm.
12. Alignment is resolved within each accepted slot. Justification produces
    explicit adjustments and does not permanently mutate canonical shaped
    facts.
13. Western space expansion, CJK expansion opportunities, and Arabic
    kashida/tatweel are distinct justification strategies with distinct proof
    claims.
14. Scratch capacity is reusable engine state, never semantic output or hidden
    cache identity.
15. Every stage reports deterministic work and retained-capacity facts suitable
    for the preparation trace and product wind tunnels.
16. Foundational crates remain `no_std + alloc`, Rust 1.88 compatible, and free
    of new `unsafe`.
17. Computed text policy participates only in the earliest stage whose output
    it can change; cache invalidation follows that stage boundary exactly.
18. Paragraph alignment moves every spatial observation of a line together:
    glyphs, fragments, clusters, carets, movements, selections, semantics, hit
    regions, and line bounds cannot disagree.
19. Editable-block convenience resolves and returns revision-bound authored
    positions; it does not introduce a second editing or navigation model.
20. A cloned font catalog preserves Fontique shared backing and never
    re-registers font bytes or repeats system discovery.
21. Cross-identity reuse shares only immutable, identity-free preparation
    facts. Document IDs, revisions, semantic owners, selections, and scene
    geometry are rebound per consumer and never leak through a shared entry.

## Pipeline

```text
authored paragraph + semantic/style ranges
                    |
                    v
projection tools
  whitespace / transform / composition / generated text
                    |
                    v
ProjectedText { display text, provenance, projected ranges }
                    |
                    v
Parley Engine analysis, itemization, font selection, shaping
                    |
                    v
ShapedParagraphFacts
                    |
                    v
resumable line candidate formation
                    |
          +---------+----------+
          |                    |
          v                    v
 line-final shaping      RegionCursor offers LineSlot
          |                    |
          +---------+----------+
                    |
             accept or restore
                    |
                    v
accepted region lines
                    |
                    v
alignment / justification adjustments
                    |
                    v
Underwood portable geometry, interaction, semantics, and paint
```

Each transition is independently testable. No transition receives a mutable
`LayoutEngine` or a general "everything" context.

## Projected text

The exact public spelling remains subject to the API gate, but the
representation must be equivalent to:

```rust
pub struct ProjectedText {
    text: String,
    segments: Vec<ProjectionSegment>,
}

pub enum ProjectionSegment {
    Identity { source: Range<u32>, projected: Range<u32> },
    Replacement { source: Range<u32>, projected: Range<u32> },
    Collapsed { source: Range<u32>, projected: Range<u32> },
    Omitted { source: Range<u32>, at: u32 },
    Inserted { at: u32, projected: Range<u32> },
}
```

This is schematic rather than approved API. The evidence bead must compare
segment runs, boundary maps with affinity, and other compact representations.
The chosen form must:

- compose Underwood leaf-to-paragraph mapping with
  paragraph-to-presentation mapping;
- preserve stable semantic owners across transformed expansion;
- define both directions at ambiguous collapsed boundaries;
- avoid per-byte storage for long identity spans;
- permit deterministic validation of complete coverage;
- support retained reuse when only paint or line geometry changes.

### First real transformation

The first executable projection is whitespace processing, not a synthetic
test-only transform. At minimum, the accepted scope must include the preserved
behavior used today and a real collapsing policy that:

- processes runs across inline leaf boundaries;
- handles spaces, tabs, and segment breaks under an explicit policy;
- emits complete authored provenance for collapsed ranges;
- preserves style and semantic ownership according to documented rules;
- has exact caret, selection, deletion, and hit-test tests.

One-to-many replacement is also proven before downstream APIs stabilize. A
focused transform fixture is sufficient to prove the mapping algebra; complete
locale-sensitive casing remains a separately scoped conformance capability.

### Text transformation extension

Text transformation consumes projected chunks plus explicit language and data
inputs and produces another composable projection. It may not call platform
locale state implicitly. Its future corpus must cover expansion, contraction,
context-sensitive casing, capitalization boundaries, and transformed style
ranges.

## Reusable line formation

The current Parley revision demonstrates resumable per-line geometry through
`BreakLines` and `BreakerState`, but that implementation is coupled to
high-level `LayoutData` and commits canonical paragraph advances directly.

The reusable kernel instead consumes:

- `parley_engine::ShapedText`;
- analysis-derived boundary, whitespace, source, and grapheme facts;
- per-run line height, wrapping, spacing, and emergency-break policy;
- optional concrete inline-item measurements;
- one caller-supplied `LineSlot`.

It produces a reversible candidate carrying source and cluster ranges, break
reason, provisional line-box metrics, trailing-whitespace facts, and the
checkpoint needed for retry.

Underwood performs its existing line-final re-itemization and shaping before
acceptance. A fit-changing result restores the checkpoint and tries the
preceding legal boundary or a revised region slot. Rejected candidates remain
visible work.

The tool may begin as a private, extraction-ready module while the upstream
Parley ownership decision is open. It must not depend on Underwood types in its
algorithmic core. The intended permanent homes are, in preference order:

1. a cohesive module in `parley_engine`;
2. a small sibling Parley text-layout crate over `parley_engine`;
3. an Underwood-private kernel with the same dependency fence until upstream
   ownership is resolved.

## Regions and flow

Region policy is represented by a resumable cursor that offers exact slots:

```rust
pub struct LineSlot {
    pub inline_start: f64,
    pub block_start: f64,
    pub inline_size: PositiveFinite,
    pub block_size: PositiveFinite,
}
```

The exact numeric wrappers remain an API decision. The protocol must support:

- a rectangle as the trivial provider;
- multiple columns;
- block-axis continuation;
- exclusion-generated inline intervals;
- left and right floats;
- a retry when actual line height changes the available interval;
- deterministic replay from a region transcript.

Arbitrary shape decomposition and float placement are caller-owned policies.
The first product proof may use a deliberately small region implementation, but
the line-former protocol may not assume one interval for the entire paragraph.

## Paragraph styles and line adjustment

The forthcoming Overstory gap analysis is an input to the style gate. The known
missing values are:

- letter spacing;
- word spacing;
- `LineHeight::{MetricsRelative, FontSizeRelative, Absolute}`;
- word-break policy;
- overflow-wrap policy;
- wrap/no-wrap policy;
- paragraph alignment.

Their stage ownership is fixed even though exact public spelling is not:

| Value | Owning stage |
| --- | --- |
| Letter spacing | shaping or shaped-advance preparation |
| Word spacing | shaping or shaped-advance preparation |
| Word-break policy | Unicode analysis |
| Overflow-wrap policy | candidate formation |
| Wrap/no-wrap policy | candidate formation |
| Line height | line-box metrics |
| Start/end/left/right/center | accepted-slot placement |
| Justification | post-acceptance line adjustment |

The style evidence bead must settle whether letter and word spacing modify
font shaping inputs, immutable post-shape advance facts, or both. It may not
invalidate canonical font selection merely because applying spacing is
convenient there.

Cache laws are part of the public semantics:

- `WordBreak` invalidates analysis and every dependent stage.
- `OverflowWrap` and wrap mode invalidate formation and downstream geometry,
  but not analysis, font selection, or canonical shaping.
- line height invalidates line metrics, region flow when height affects slots,
  and downstream geometry; it does not select a different font.
- alignment invalidates adjustment and geometry only.
- paint remains independent of text analysis, shaping, formation, and adjustment.

Alignment resolves logical start/end from paragraph direction and the accepted
slot. `Auto` uses the resolved paragraph direction from analysis, not a second
first-strong scan. Alignment does not change the chosen source boundary.

The first alignment proof checks that the same offset reaches line bounds,
fragments, glyphs, clusters, carets, hit testing, selection rectangles,
semantic geometry, and PDF lowering. Moving only painted glyphs is invalid.

Western justification first distributes explicit expansion across eligible
space opportunities while excluding final and mandatory-break lines according
to documented policy. It emits side-table adjustments consumed by geometry,
hit testing, rendering, and PDF lowering. CJK and Arabic strategies retain
separate beads and proof status until their script-specific behavior is real.

## Executable consumer checkpoints

Two parked commits are executable evidence, not proposed APIs:

- Underwood `integration/overstory-text` at
  `faa19ead16054d52d4d921de469a8f28993b6767` prototypes the missing
  `TextBlock`, scene-navigation, and font-catalog operations and passes its
  workspace tests and clippy gate.
- Overstory `integration/underwood-text` at
  `75e22e5d0c4141767d131d237e781bc5ee1ac16f` uses Underwood as its text model.
  With that prototype, 552 of 555 Overstory tests pass.

The three remaining consumer failures are architecture evidence:

1. metrics-relative line height is not represented;
2. finite-width paragraph alignment is absent, including logical start for
   right-to-left paragraphs;
3. identical simple labels with distinct element identities lose the
   cross-element preparation reuse available on the former path.

The first two belong to computed paragraph policy and line adjustment. The
third is not folded into document identity or stable-output retention: it
requires a separately budgeted, identity-free reuse layer. The prototype
demonstrates desired editing behavior around scene positions, selections,
movement, hit testing, replacement, and analysis-backed word navigation, but
the campaign may revise or replace every provisional API.

## CJK line breaking

The exact Parley Engine revision uses ICU4X line segmentation and already
publishes `Normal`, `BreakAll`, and `KeepAll` word-break inputs. Underwood now
projects those authored values through its analysis-style partition and
exposes Parley Engine's `complex-scripts` data as a non-default adapter
feature.

The campaign must distinguish:

- ordinary Unicode line boundaries and punctuation behavior;
- authored word-break policy;
- dictionary-sensitive segmentation and its data cost;
- locale tailoring;
- CJK justification opportunities.

The executable corpus covers Japanese prohibited-start and prohibited-end
punctuation, small kana, iteration marks, ideographic space, Chinese and
Korean text, mixed Latin/CJK, emoji and ZWJ sequences, mandatory breaks, and
all three word-break values. Failures name whether the missing owner is
analysis data, policy projection, candidate formation, or future
justification.

The implemented proof also records an upstream representation limit: Parley
Engine computes word and line boundaries separately but publishes one
precedence-merged boundary value. Underwood can therefore consume the exact
line fact but cannot yet distinguish a dictionary word boundary that overlaps
a legal CJK line boundary. That word-navigation work is tracked separately;
it does not weaken the line-breaking claim.

## Scratch, profiling, and memory

Optimization follows product-path evidence:

1. capture release `sample` profiles for cold, retained, edit, width churn, and
   region churn;
2. measure allocation calls, allocated bytes, peak live bytes, retained
   capacity, and scratch growth in separate benchmark tooling;
3. rank work by measured cost;
4. introduce reusable scratch only where it reduces measured churn without
   leaking semantic state;
5. repeat identical workloads and preserve work-law assertions.

Initial sampling already identifies cursor-movement construction, scene
materialization, and allocator traffic as material costs. That observation is
a hypothesis source, not a substitute for per-workload allocation counts.

`spoor_memory` is an eligible later integration for scratch growth, cache
residency, and preparation counters. It is not an allocation profiler and does
not enter production manifests until its value, dependency direction, and
tooling path pass a human gate.

## Cross-identity preparation reuse

Stable retained outputs remain the first reuse mechanism: a host should not
prepare an unchanged label every frame. That does not solve a screen containing
many distinct elements with identical text and computed text policy.

The reuse proof must separate:

- identity-free projection, analysis, font selection, shaping, and eligible
  formation facts;
- identity-bound source positions, semantic ownership, revisions, interaction
  records, paint, and final scene placement.

The cache key must include every input that can alter the shared fact, including
projected text, language and direction, computed text policy, font-catalog
identity, and relevant constraints. Its lifetime must be explicitly budgeted;
destroying elements cannot retain entries without bound. Reuse is invalid if it
shares revision-bound positions or causes two semantically distinct labels to
share interaction identity.

The wind tunnel compares thousands of identical and distinct labels, repeated
creation and destruction, paint-only changes, width changes, and font-catalog
changes. It reports both avoided work and retained memory. Exact storage and
public ownership remain behind a human cache-semantics gate.

## Editable block operations

Overstory controls need calm primitives over the same scene and transaction
model. The campaign adds, subject to the public API gate:

- scene start and end positions;
- resolution of a represented caret at an exact leaf-local UTF-8 boundary;
- previous and next logical word positions derived from the retained analysis
  facts used by shaping;
- replacement of `TextBlock` selections with selections rebound to the newly
  published revision.

Every operation must define stale revision, non-represented boundary,
collapsed projection, bidi, empty text, and multi-leaf behavior. Word movement
does not call another segmenter in Underwood.

A retained single-line editor façade is optional. It is created only if the
proved primitive call site still exposes document transaction ceremony to
ordinary controls. It must remain a façade over `TextBlock`, snapshot
selection, scene navigation, and the existing transaction machinery.

## Font catalog usability

The Fontique adapter must support:

- an empty embedded-only `FontSet`;
- a system-font-only `FontSet` when the existing feature is enabled;
- stable observation of registered family names;
- cheap clones sharing Fontique collection, source-cache, and font-byte
  backing;
- deterministic embedded-only operation without system discovery.

This work does not introduce a second application font universe. The final
Overstory handoff must show how its calm resource owner constructs and shares
the same catalog. Prototype companion patches are evidence only and are not
copied into Underwood.

## Preparation trace

The trace uses the same stage boundaries as the implementation:

```text
projection -> analysis -> shaping -> candidate formation
           -> region flow -> line adjustment -> lowering
```

It records:

- exact stage work and output units;
- identity-local reuse, shared preparation, adapter calls, and overlapping
  formation, adjustment, and paint invalidation reasons;
- candidate retries and rejected work;
- exact slots, accepted attempts, and height rejection through the replayable
  region transcript;
- stage duration in host tooling;
- scene-output capacity, reusable scratch growth, and retained scene/shared
  cache residency;
- process allocation calls and bytes in external profiling tools.

`WorkReport` remains always available. Detailed `PreparationTrace` is opt-in
through `SceneRequest::with_preparation_trace` or
`BlockRequest::with_preparation_trace` because capacity accounting performs
additional diagnostic work. The trace itself is deterministic; wall time and
allocator behavior are explicitly host observations. A region transcript can
be replayed without fonts or a renderer once shaped candidate facts are
captured.

## Options

### A. Add paragraph features to the current orchestration

This is locally direct but repeats source mapping, line policy, and mutation
assumptions in every feature. Region flow and future transformations would
force a second architectural rewrite.

### B. Move high-level Parley layout into Underwood

This obtains existing alignment and resumable breaker behavior quickly, but
imports the fused `Layout` ownership that Underwood is intended to replace. It
also weakens line-final shaping, retained invalidation, and portable-output
control.

### C. Establish reusable stages, prove projection, then add paragraph policy

This creates the smallest permanent dependency graph. It costs an explicit
projection and line-candidate design before visible region features, but every
subsequent capability lands in its final stage.

Choose C, subject to human approval of ADR-0005 and the foundational public
API gate.

## Usage narrative

A toolkit continues to author calm high-level requests:

```rust
let output = layout.prepare_block(
    &block.snapshot(),
    &BlockRequest::new(TextConstraint::Wrap(width), &style, &paint)
        .with_paragraph_style(paragraph),
)?;
```

The caller does not construct Parley Engine objects, projection segments,
scratch buffers, or region checkpoints for an ordinary rectangle. More
advanced document flow supplies region policy through an Underwood-owned
document preparation API; it does not teach the reusable breaker about
documents.

## Extension points

- Locale- and data-provider-backed text transformation.
- Generated markers, discretionary hyphens, and ellipsis.
- Vertical writing and non-horizontal inline axes.
- Script-specific CJK and Arabic justification.
- Page, footnote, table, and object flow.
- Selective non-editable scene materialization.
- Spoor-backed live preparation and memory visualization.
- Upstream Parley reuse of the line-formation kernel.

## Risks and controls

- **Framework before proof:** whitespace collapse is a real first consumer and
  selection behavior is a blocking corpus.
- **Projection explosion:** compare compact alternatives and measure long
  identity text, dense collapse, and expansion cases before approval.
- **New monolith by another name:** no reusable tool accepts a general engine
  context or mutates unrelated stage output.
- **Crate confetti:** begin with modules; split crates only for dependency or
  independently consumed API evidence.
- **Incorrect line-final shaping:** candidate acceptance retains Underwood's
  Arabic, ligature, bidi, and fit-changing reshape traps.
- **Region callback soup:** slots and checkpoints are concrete records with a
  replayable transcript.
- **Universal-justification overclaim:** proof status and strategies remain
  script-specific.
- **Hidden allocation churn:** allocation count and bytes are first-class wind
  tunnel results before and after scratch changes.
- **Integration drift:** the final Overstory analysis is absorbed before the
  public paragraph-style surface is ratified.
- **Companion-patch anchoring:** Overstory prototypes provide failing call
  sites and invariants, never the default Underwood representation.
- **Identity-poisoned reuse:** cross-element caches contain only identity-free
  facts and rebind all semantic, revision, interaction, and geometry records.
- **Unbounded shared cache:** identical-label reuse has an explicit budget,
  release behavior, and churn proof before it becomes a product claim.
- **Partial alignment:** tests compare every spatial scene record after
  alignment, not only glyph origins.
- **Second editor façade:** editable-block primitives land first; a façade must
  remove measured ceremony without owning new editing semantics.
- **Second font universe:** system-only construction and catalog observation
  preserve shared Fontique backing and application-level single ownership.

## Human gates

Implementation stops for approval before:

1. accepting ADR-0005 or this design;
2. freezing the projected-text representation;
3. creating or changing the foundational public API;
4. choosing a permanent upstream crate/module home for the line former;
5. adding Unicode, profiling, Spoor, or other production dependencies;
6. introducing `unsafe`;
7. claiming CJK or Arabic justification conformance.
8. introducing a retained single-line editor façade rather than only calm
   primitives.
9. freezing cross-identity cache keys, ownership, or lifetime.

## Completion

The campaign is complete when:

- the real public preparation path uses the staged pipeline;
- whitespace preservation and collapse are source-complete and editable;
- the line former is reusable over Parley Engine facts and supports
  candidate/accept/retry semantics;
- rectangle, exclusion, float, and multi-column providers fill regions through
  one slot protocol;
- known Overstory paragraph styles affect the correct stages;
- identical labels can reuse eligible identity-free preparation facts without
  sharing semantic identity or retaining an unbounded cache;
- start/end/left/right/center alignment and honest Western justification are
  represented in portable geometry and interaction;
- editable blocks expose revision-correct start/end, caret resolution, logical
  word movement, and replacement without duplicate segmentation;
- empty, embedded-only, and system-only font catalogs clone shared backing and
  expose stable registered family names;
- CJK breaking has a named executable corpus and honest documented limits;
- preparation traces and allocation wind tunnels expose stage work and memory;
- the living page and guided diagnostics exercise the public path;
- no high-level Parley dependency, new unsafe, hidden second shaper, or
  unbounded cache is introduced;
- all local and protected remote gates are green and proof records name every
  remaining limitation.

## Migration note

This campaign intentionally changes the draft public preparation contract:

- `LineHeight::from_multiplier(multiplier)` and `LineHeight::multiplier()` become
  `LineHeight::{metrics_relative, font_size_relative, absolute}` plus
  `basis()`, `value()`, and `resolve()`. Callers must choose the authored
  basis; the old multiplier maps to `font_size_relative`.
- Paragraph backends receive analysis style/runs separately from shaping and
  publish `ResolvedDirection` through `PreparedParagraph::try_new`. External
  `ParagraphFormation` implementations must thread the direction chosen by
  their Unicode analysis.
- `ParagraphStyle` gains whitespace-collapse and alignment values;
  `ComputedInlineStyle` gains the independently invalidated `AnalysisStyle`;
  existing constructors retain the previous preserve/start/normal defaults.
- `SceneRequest` and `BlockRequest` gain optional region-flow and preparation
  trace configuration. Existing finite-width rectangle callers require no
  change.
- `TextBlock` gains calm text, leaf-identity, and selection-replacement
  operations; `TextScene` gains scene endpoints, exact represented-caret
  resolution, and logical word movement. These additions do not introduce a
  second editing model.

Downstream adapters should update as one source change: split computed style
into analysis/shaping/inline-flow/paint partitions, pass resolved paragraph
direction when constructing prepared facts, and retain prepared outputs rather
than preparing stable labels every frame.

## Implementation outcome — 2026-07-25

The approved design is implemented through one public preparation path. The
path now composes source-complete projected text, Parley Engine analysis and
shaping, reusable candidate formation, exact region slots, accepted-line
adjustment, portable scene records, interaction, and renderer adapters.

The completion evidence is indexed by
`docs/proof/reusable-text-tools-campaign-review-2026-07-25.md`. In particular:

- preserved and collapsed whitespace retain exact authored provenance;
- candidate formation is reversible and independently exercised;
- rectangles, exclusions, floats, columns, and off-page continuation use one
  replayable slot protocol;
- computed line height, spacing, wrapping, direction, alignment, and honest
  Western justification invalidate their owning stages;
- identical elements may share budgeted identity-free preparation while
  revisions, semantics, interaction, paint, and placement are rebound;
- editable blocks, shared font catalogs, CJK line breaking, and preparation
  traces have named executable or measured proof;
- the region-aware living page and PDF proof consume the same portable output;
- foundational crates remain `no_std + alloc`, Rust 1.88 compatible, and free
  of high-level Parley and new `unsafe`.

This outcome does not silently widen the design. Complete CSS Text semantics
remain `und-oh0.13.15`; O(change) document and scene publication remains
`und-oh0.13.17`; Arabic and CJK justification, mixed-bidi PDF viewer
conformance, and pixel snapping retain separate claims and follow-ups.
