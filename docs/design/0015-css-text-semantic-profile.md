# Design-0015: CSS Text as a semantic profile

- **Status:** Exploratory — non-blocking for Design-0014
- **Date:** 2026-07-24
- **Related:** Design-0003, Design-0014, ADR-0005
- **Tracking:** `und-oh0.13.15`

## First read

### Goal

Define how Underwood can use CSS Text as a source of text semantics,
terminology, operation ordering, and conformance cases without becoming a CSS
implementation or browser formatting engine.

The proposed direction is an **Underwood CSS Text semantic profile**:

- a named set of computed text behaviors;
- an explicit mapping from each behavior to an Underwood preparation stage;
- source-complete editing and interaction rules beyond CSS's presentation
  requirements;
- a conformance manifest that states supported values, deviations, required
  data, and executable evidence;
- no CSS parser, cascade, DOM, box tree, or browser compatibility mode.

This note is intentionally non-blocking for the reusable-text-tools campaign.
Design-0014 should establish the right stage boundaries first. Those boundaries
make a future CSS Text profile possible without making every current feature
wait for a broad conformance claim.

### Non-goals

- Parsing CSS syntax or accepting CSS declarations as Underwood's core API.
- Implementing cascade, inheritance, custom properties, computed-value
  resolution, or CSS-wide keywords.
- Recreating HTML segment-break normalization or a DOM inline formatting
  context.
- Claiming complete conformance with CSS Text, CSS Inline, CSS Writing Modes,
  CSS Fonts, or a web browser.
- Treating web-platform behavior as more important than source-complete native
  editing, IME, accessibility, or portable scene output.
- Adopting property names whose CSS meaning does not match the engine concept.
- Making experimental Level 4 behavior a stable contract before its semantics,
  data needs, and consumer value are proved.

### The idea in one example

A host may author style through CSS, a toolkit theme, a document format, or
plain Rust. It resolves that authoring language into computed behavior before
calling Underwood:

```text
host authoring policy
    "white-space: normal; overflow-wrap: anywhere; text-align: start"
                         |
                         v
host-owned computed-style adapter
                         |
                         v
Underwood computed text policy
    collapse spaces / allow emergency breaks / align to logical start
                         |
                         v
projection -> analysis -> shaping -> formation -> adjustment -> TextScene
```

Underwood never needs to know that the values originated in CSS. It does need
precise behavior for them. CSS Text is useful because it has already specified
many of those behaviors and their interactions.

### Why this is appealing

CSS Text represents decades of accumulated edge-case work:

- It names distinctions that text engines otherwise rediscover, including
  collapse versus preservation, ordinary versus emergency wrapping, word-break
  policy, language-sensitive transformation, hanging spaces, alignment, and
  justification.
- It specifies combinations rather than isolated toggles. Text behavior is
  mostly in the interactions.
- It supplies a processing order and a large body of conformance cases.
- Its logical `start` and `end` vocabulary works across paragraph directions.
- It is familiar to toolkit and document-format authors, reducing adapter
  invention.
- It gives Underwood a defensible answer when several plausible behaviors
  exist.

Most importantly, CSS whitespace processing explicitly changes rendering
without changing underlying document data. That principle agrees with
Underwood's authored-versus-projected text boundary.

### Why this is appalling

“Implement CSS Text” sounds bounded but is not:

- Text behavior crosses into CSS Inline Layout, Writing Modes, Fonts, Sizing,
  Overflow, Ruby, and Unicode algorithms.
- Some rules begin after document-language parsing and therefore assume a DOM
  and normalized segment breaks that Underwood does not have.
- CSS presentation conformance does not define Underwood's required caret,
  selection, deletion, IME, accessibility, semantic, and PDF provenance.
- Language-sensitive transform, hyphenation, dictionary breaking, and
  justification require data and policy that can dwarf the core algorithms.
- Property interactions create a large conformance matrix across language,
  script, direction, writing mode, whitespace, wrapping, breaking, and
  adjustment.
- Web compatibility sometimes preserves historical behavior rather than
  presenting an ideal engine abstraction.
- A broad “CSS Text compliant” claim would imply far more than a useful native
  text engine should promise casually.

The answer is not to ignore CSS Text. It is to profile it deliberately.

## Fence

The CSS Text semantic profile owns:

- computed text behavior selected for the profile;
- mapping that behavior onto Underwood's reusable preparation stages;
- operation ordering and invalidation laws;
- source-complete interaction behavior for each transformation;
- conformance fixtures and an honest capability manifest.

It explicitly does not own:

- CSS tokens, selectors, declarations, cascade, or inheritance resolution;
- HTML or XML parsing and segment-break normalization;
- DOM, anonymous boxes, inline box generation, or browser quirks;
- widget callbacks, accessibility policy, or editor command policy;
- font discovery, region placement, paint, or renderer implementation.

## Glossary

- **CSS concept:** A behavior defined by CSS Text or an adjacent CSS module,
  independent of CSS source syntax.
- **Computed text policy:** Host-resolved values consumed by Underwood after
  authoring-language concerns are gone.
- **Semantic profile:** The exact CSS concepts and values Underwood commits to
  implement, including named deviations and extensions.
- **Presentation unit:** Text or generated content passed to analysis and
  shaping after projection.
- **Source-complete:** Every presentation effect can be mapped back to authored
  positions for editing, interaction, semantics, and export.
- **Formatting context:** The browser-owned box construction and layout state
  that this profile deliberately excludes.

## Second read

## Architectural layers

### 1. Host authoring and computation

The host owns its authoring language and policy. A CSS-speaking host may parse,
cascade, inherit, and resolve relative values. Overstory may lower its own
computed styles. A document format may expose a smaller fixed vocabulary.

All produce the same Underwood-facing policy. Core APIs accept enums, flags,
finite scalar values, languages, and directions—not CSS strings.

### 2. Reusable text preparation

Design-0014's stages own the behavior:

```text
authored text
  -> projection
  -> Unicode analysis
  -> font selection and canonical shaping
  -> resumable candidate formation and line-final shaping
  -> accepted-slot adjustment
  -> portable scene lowering
```

No stage receives a bag of arbitrary CSS properties. Each value is carried only
to the earliest stage whose output it can change.

### 3. Underwood orchestration

Underwood composes the stages with immutable snapshots, source mappings,
semantic identity, retained invalidation, interaction geometry, and
revision-bound editing positions.

### 4. Host presentation

The host retains prepared output, places scenes, supplies regions, paints,
routes input, and decides which semantic ranges become actions or
accessibility nodes.

## Profile tiers

The profile should grow in named tiers so “supported” remains meaningful.

### Tier A: Core horizontal text

This is the natural overlap with Design-0014:

- whitespace preservation and collapse;
- wrap versus no-wrap;
- ordinary and emergency breaking;
- `word-break`;
- letter and word spacing;
- text transformation with explicitly supported locale behavior;
- logical and physical alignment;
- Western justification with named limits;
- companion `line-height` behavior from CSS Inline Layout.

Tier A is not automatically part of the current epic. The epic should record
the CSS mapping for behavior it implements; the profile earns Tier A only when
the complete tier corpus passes.

### Tier B: Language- and data-sensitive text

- `line-break` strictness and language tailoring;
- automatic hyphenation and hyphenation limits;
- locale-sensitive case and width transformation;
- CJK autospace and spacing trim;
- script-specific justification strategies.

Tier B requires explicit data-provider, footprint, and fallback decisions.

### Tier C: Advanced inline-axis policy

- first-line and hanging indentation;
- `text-align-last`;
- hanging punctuation;
- tab sizing and alignment;
- balanced, pretty, or stable wrapping strategies;
- generated discretionary marks and overflow indicators.

### Separate profiles or campaigns

These affect broader geometry or ownership and should not be smuggled into a
CSS Text claim:

- vertical writing, text orientation, and orthogonal flow;
- ruby and emphasis marks;
- inline boxes with margin, border, padding, replaced content, and baseline
  alignment;
- text decoration painting;
- browser intrinsic sizing and fragmentation;
- DOM bidi embeddings and overrides.

They may reuse the same kernels later.

## Concept-to-stage map

The table records architectural ownership, not a claim of current support.

| CSS concept | Underwood owner | Source relation | Earliest invalidation |
| --- | --- | --- | --- |
| `white-space-collapse` | projection | identity, collapse, omission | projection |
| `white-space-trim` | projection plus line context | omission at scoped edges | projection or formation |
| `text-wrap-mode` | candidate formation | identity | formation |
| `text-wrap-style` | formation strategy | identity | formation |
| `text-transform` | projection | replacement, expansion, omission | projection |
| `tab-size` | projected advance policy | identity or generated advance | shaping/formation |
| `word-break` | analysis boundary policy | identity | analysis |
| `line-break` | analysis boundary policy | identity | analysis |
| `overflow-wrap` | emergency candidate policy | identity | formation |
| `hyphens` | analysis/formation plus generated mark | insertion or authored soft hyphen | analysis/formation |
| `letter-spacing` | shaped-advance preparation | identity | shaped advances |
| `word-spacing` | shaped-advance preparation | identity | shaped advances |
| `text-indent` | first accepted slot | identity | region flow |
| `hanging-punctuation` | candidate fit and adjustment | identity | formation/adjustment |
| `text-align` | accepted-slot adjustment | identity | adjustment |
| `text-align-last` | accepted-slot adjustment | identity | adjustment |
| `text-justify` | line-final shaping/adjustment strategy | identity or generated glyph | adjustment or formation |
| `text-spacing-trim` | projection/adjustment | omission or advance change | projection/adjustment |
| `text-autospace` | projection/adjustment | generated spacing | projection/adjustment |
| `line-height` (CSS Inline) | line-box metrics | identity | metrics and region flow |
| base `direction` (Writing Modes) | analysis and logical resolution | identity | analysis |

Several rows deliberately name two possible owners. Design-0014's focused
evidence must settle those seams. For example, trimming that depends on an
actual line end cannot be finalized by a context-free projection pass.

## Operation ordering

CSS Text defines a useful conceptual order:

1. pre-wrapping whitespace processing;
2. text transformation;
3. text combination and orientation;
4. wrapping with per-line bidi, line-end whitespace, glyph selection, spacing,
   and hanging punctuation;
5. justification;
6. alignment.

Underwood need not execute identical loops if the result is equivalent, but it
must preserve the dependencies. Its line-final shaping requirement makes the
cycle explicit:

```text
project -> analyze -> shape canonical facts
                    |
                    v
propose boundary -> shape line-final text -> measure
        ^                                  |
        |----------- retry if needed ------|
                    |
                    v
accept slot -> justify -> align -> lower
```

This is one place where treating CSS as a semantic reference rather than an
implementation architecture is especially valuable.

## Source-complete transformation laws

CSS requires whitespace and transforms to leave underlying document data
unchanged. Underwood adds stronger observable laws:

1. Every authored byte has a recorded identity, replacement, collapse,
   omission, or insertion relationship.
2. Every presentation byte has an authored semantic owner or an explicit
   generated-content owner.
3. A projected boundary resolves to stable upstream and downstream authored
   positions where those differ.
4. Selection painting may span projected units, but edit transactions operate
   on authored ranges.
5. Copying authored content does not accidentally copy generated hyphens or
   transformed casing unless the caller explicitly requests presentation text.
6. IME composition modifies authored text and composes with projection; it does
   not edit a lossy display string.
7. Accessibility and PDF extraction retain authored provenance even when
   visible glyphs came from replacements or insertions.

### Collapse example

```text
authored:   "A \n  B"
             012345
display:    "A B"
             012
```

The displayed space owns a many-to-one source range. A caret on either side
needs a documented affinity rule, and deleting the displayed space needs an
authored-range policy rather than pretending one source byte survived.

### Expansion example

```text
authored:   "straße"
display:    "STRASSE"
```

One authored unit can own multiple presentation units. Hit testing and
selection must not manufacture invalid UTF-8 boundaries in the authored text.
Locale-sensitive results must be identified by the policy and language used to
produce them.

### Generated-mark example

An automatic hyphen at a chosen line break has no authored byte. It belongs to
the break decision, remains non-editable, and disappears when reflow chooses a
different boundary. An authored soft hyphen has different provenance even if
the visible result is similar.

## Computed API direction

The exact public API remains a later gate. Its shape should resemble composed
computed behavior, not a CSS declaration block:

```rust,ignore
pub struct ComputedTextPolicy {
    pub whitespace: WhitespacePolicy,
    pub transform: TextTransform,
    pub wrapping: WrapPolicy,
    pub breaking: BreakPolicy,
    pub spacing: TextSpacing,
    pub alignment: ParagraphAlignment,
}
```

This sketch is intentionally incomplete and not approved API. Important
properties:

- structural equality and hashing for cache identity;
- values already resolved into finite engine units;
- language and base direction supplied explicitly;
- independent partitions for exact invalidation;
- no stringly typed property/value storage;
- no dependence on a CSS crate.

A CSS adapter may live in Overstory, a separate integration crate, or another
consumer. It should be testable against the profile mapping without moving CSS
types into foundational crates.

## Naming policy

Use CSS names when all of these are true:

1. the semantics match closely;
2. the name is understandable outside CSS;
3. adopting it reduces translation at call sites;
4. its value set does not import authoring-language machinery.

Use an Underwood-specific name when:

- the engine concept is lower-level than the CSS property;
- several CSS properties combine into one stage policy;
- source-complete editing requires a materially stronger contract;
- the CSS term carries browser-box assumptions.

Aliases that only make the API look CSS-like should be avoided.

## Conformance manifest

A machine-readable or simply structured manifest should eventually record one
entry per adopted concept:

```text
concept
source specification and anchor
supported values
required language/script data
known deviations
source-mapping behavior
invalidation stage
executable corpus
proof-ledger capability
```

Claims use narrow language:

- **mapped:** ownership and intended semantics are documented;
- **executable:** the public path has deterministic tests;
- **conformant for values X:** selected normative cases pass;
- **partial:** supported values and missing behavior are named;
- **unsupported:** the adapter must reject or lower deliberately.

“CSS Text compliant” is not a permitted umbrella claim without a separately
reviewed conformance statement.

The CSS Working Group test suites are valuable evidence sources. Directly
imported cases must retain provenance and compatible licensing; otherwise
Underwood should write compact fixtures from normative requirements and link
the relevant specification section.

## Relationship to native application behavior

CSS compatibility never weakens these Underwood requirements:

- multiple and bidi-visual selections;
- affinity at wraps and bidi discontinuities;
- exact hit testing over retained geometry;
- grapheme-safe editing and analysis-backed word movement;
- simple and host-mediated IME models;
- semantic identities across inline leaves;
- accessible authored text and actionable ranges;
- renderer-neutral output and PDF provenance;
- deterministic behavior in embedded-only and `no_std + alloc` builds.

Where CSS is silent, Underwood specifies the behavior. Where CSS presentation
conflicts with safe source editing, the profile documents both the visual rule
and the authored operation.

## Extension points

- A CSS-facing adapter crate can lower computed values into the profile.
- A conformance runner can translate selected web-platform fixtures into
  headless Underwood scenes and source-mapping assertions.
- Vertical writing can generalize line slots from horizontal coordinates to
  logical inline/block axes without changing authored projection.
- Hyphenation and dictionary breaking can plug into explicit analysis and
  candidate interfaces behind data-provider gates.
- Region flow can reuse CSS concepts such as logical start/end without
  adopting CSS float or box algorithms.
- Other document standards can map to the same computed policy and compare
  semantics against the CSS profile.

## Gotchas and controls

- **Specification drift:** every profile entry pins a source URL and review
  date; unstable behavior is marked experimental.
- **Profile by name only:** each supported value needs public-path executable
  evidence and source-interaction traps.
- **DOM leakage:** segment-break normalization is an explicit host input, not
  an implicit HTML assumption.
- **Order mismatch:** tests combine whitespace, transform, bidi, wrapping,
  spacing, and alignment rather than testing each property alone.
- **Locale optimism:** unsupported language-sensitive behavior is rejected or
  documented, never silently approximated as universal.
- **Cache poisoning:** computed policies participate only in their owning stage
  keys, including language and external-data identity where relevant.
- **Display-string editing:** all edits resolve through projection mappings to
  authored snapshot positions.
- **Browser-sized scope:** advanced writing modes, inline boxes, and decorations
  remain separately approved campaigns.
- **Stability mismatch:** experimental CSS values do not become stable
  Underwood APIs merely because a draft specification names them.

## Open decisions

1. Is the first named profile based on stable CSS Text Level 3 behavior plus a
   small allowlist from Level 4, or on consumer-selected concepts independent
   of spec level?
2. Should the conformance manifest be data checked by `xtask`, a prose table,
   or both?
3. Which whitespace rules require caller-supplied segment boundaries rather
   than interpreting Unicode newline sequences directly?
4. How should presentation-text copy be exposed without confusing it with
   authored-text copy?
5. Which language-sensitive transforms can be offered with current text data,
   and how is data-provider identity reflected in caches?
6. Does `text-wrap-style: balance` belong in reusable candidate formation or a
   higher-level multi-line optimization policy?
7. Which CSS test artifacts can be reused directly with clean attribution and
   maintenance cost?

## Recommended next step

Do not pause Design-0014 to implement this profile wholesale. During that
campaign:

1. add a CSS concept mapping to every new computed text policy;
2. preserve the source-complete and invalidation laws in this note;
3. collect candidate conformance fixtures alongside Underwood-specific traps;
4. keep unsupported CSS values explicit;
5. revisit the first named profile after whitespace, breaking, spacing,
   alignment, and line-height are executable.

That sequence earns a useful profile from real engine behavior instead of
building a property-shaped façade over incomplete semantics.

## Primary references

- [CSS Text Module Level 3](https://www.w3.org/TR/css-text-3/)
- [CSS Text Module Level 4](https://www.w3.org/TR/css-text-4/)
- [CSS Inline Layout Module Level 3](https://www.w3.org/TR/css-inline-3/)
- [CSS Writing Modes Level 4](https://www.w3.org/TR/css-writing-modes-4/)

