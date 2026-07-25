# ADR-0005: Reusable text-preparation pipeline

- **Status:** Accepted
- **Date:** 2026-07-24
- **Bead:** `und-oh0.13.1`
- **Owners:** Underwood project and human architecture authority
- **Supersedes:** None

## Goal

Choose the permanent ownership and representation boundaries for projected
text, reusable line formation, region flow, and line adjustment before
paragraph style, alignment, justification, and region capabilities expand the
current preparation path.

## Non-goals

- Selecting the final crate name for every future reusable tool.
- Parsing CSS, implementing its cascade, or ratifying complete browser
  formatting behavior.
- Moving Overstory control policy into Underwood.
- Replacing Parley Engine analysis or shaping.
- Choosing a production dependency before evidence and human approval.

## Fence

Reusable text-preparation kernels own explicit transformations of projected and
shaped text facts; they explicitly do not own Underwood documents, UI policy,
region placement policy, paint, rendering, or a general mutable layout
context.

CSS Text remains an eligible semantic and conformance source. A later
Underwood profile may adopt its computed whitespace, transform, spacing,
wrapping, breaking, alignment, and justification behavior without owning CSS
syntax, DOM construction, or browser compatibility policy.

## Context and invariants

Underwood currently preserves semantic source identity and delegates Unicode
analysis and shaping to Parley Engine, but paragraph projection, candidate
formation, geometry, and interaction are orchestrated through paths that would
become increasingly fused as whitespace processing, text transformation,
spacing, line height, regions, alignment, and justification arrive.

High-level Parley contains useful resumable-breaker behavior, but its
implementation borrows and mutates private `LayoutData`, resolved styles,
inline items, line records, and whole-layout metrics. Importing that fused type
would surrender Underwood's retained, source-complete, and renderer-neutral
boundaries.

The durable choice must preserve:

1. authored source distinct from presentation text;
2. complete bidirectional source mapping through collapse and replacement;
3. public Parley Engine analysis and shaping as the only Unicode and shaping engine;
4. reversible line candidates and line-final shaping before acceptance;
5. caller-owned region and float policy;
6. immutable canonical shaping plus explicit line adjustments;
7. `no_std + alloc`, Rust 1.88, and dependency discipline;
8. reusable scratch and observable work without semantic hidden state;
9. cross-element reuse only through identity-free, explicitly budgeted facts.

## Options

### Option A: Extend the current Underwood orchestration directly

Add each capability to `Projection`, `ParleyParagraphEngine`, and
`LayoutEngine` as needed.

Benefits:

- minimal immediate API design;
- no upstream coordination;
- existing retained and scene paths remain recognizable.

Costs and failure modes:

- transformation, formation, flow, and adjustment become inseparable;
- independent consumers cannot reuse the algorithms;
- scratch and invalidation ownership become implicit;
- later extraction requires another correctness migration.

### Option B: Adopt high-level Parley layout as the reusable layer

Construct or adapt a Parley `Layout` and use its breaker and alignment.

Benefits:

- existing alignment and per-line breaker behavior;
- existing Parley tests for exclusions and floats;
- less initial line-layout implementation.

Costs and failure modes:

- introduces a high-level production dependency and fused layout state;
- line-final shaping and source-complete Underwood interaction do not naturally
  fit the commit model;
- Underwood's portable records become an adapter over an all-owning object;
- capabilities absent from Parley still accumulate around the monolith.

### Option C: Build reusable kernels over explicit stage records

Create compact projected-text, line-candidate, slot, checkpoint, and adjustment
representations. Keep Underwood orchestration as an adapter. Seek an upstream
Parley home for algorithms that depend only on Parley Engine facts.

Benefits:

- each capability has one owner and an independent corpus;
- high-level callers remain calm while low-level tools remain reusable;
- source mapping, line-final shaping, region policy, and portable output remain
  compatible;
- scratch, invalidation, and preparation tracing align with real stages.

Costs and failure modes:

- representation choices require evidence and a public API gate;
- temporary private modules may exist before upstream ownership is resolved;
- over-generalization is possible without a real first transformation.

## Required evidence

- Compact projection prototypes for identity, dense collapse, omission,
  insertion, and one-to-many replacement.
- Exact selection, deletion, hit, accessibility, and PDF source traps over
  transformed ranges.
- A real whitespace-collapse path across styled semantic leaves.
- Differential line-break and metric fixtures against current Underwood and
  the exact Parley breaker where their policy overlaps.
- Arabic joining, ligature, bidi, fit-changing retry, CRLF, intrinsic, and
  grapheme-interaction corpora.
- Rectangle, exclusion, float, and multi-column region transcripts.
- CJK breaking corpus covering punctuation, mixed Latin/CJK, Korean, emoji,
  and `Normal`/`BreakAll`/`KeepAll`.
- Release CPU samples plus allocation calls, bytes, peak live memory, scratch
  growth, and retained-capacity measurements.
- The parked Underwood checkpoint
  `faa19ead16054d52d4d921de469a8f28993b6767` and Overstory checkpoint
  `75e22e5d0c4141767d131d237e781bc5ee1ac16f`, including the 552/555
  public-path label result and its three remaining failures.
- A cross-identity reuse wind tunnel proving identical-label work avoidance
  without sharing revision-bound identity or retaining entries without bound.

## Decision

Adopt Option C: build reusable kernels over explicit stage records, with
Underwood retaining document, identity, editing, scene, and orchestration
ownership.

## Rationale

The approved boundary preserves source-complete native editing and retained
scene behavior while making projection, line formation, region slots, and line
adjustment independently understandable and testable. It continues to delegate
Unicode analysis and shaping to Parley Engine without importing high-level
Parley's fused layout state.

The two executable integration checkpoints show that the calm `TextBlock`
consumer shape is credible: 552 of 555 Overstory tests already pass. Their
remaining failures map cleanly to stage-owned work—line-height policy,
direction-aware finite-width alignment, and separately budgeted
cross-identity reuse—rather than requiring another all-owning layout object.

CSS Text computed semantics are a positive conformance source under this
decision. CSS parsing, cascade, DOM box construction, and browser compatibility
remain outside the boundary.

## Consequences

If Option C is accepted:

- paragraph preparation becomes an explicit staged pipeline;
- current public APIs may break with a migration note;
- reusable algorithms cannot import Underwood document or scene types;
- high-level Parley remains outside foundational production dependencies;
- adjustment output becomes immutable side data rather than mutation of
  canonical shaping;
- cross-identity preparation reuse, if adopted, contains only immutable
  identity-free stage results and has an explicit budget;
- heavier text data or instrumentation dependencies remain separate human
  gates.

## Migration

The migration is complete. Existing `TextBlock` and document callers retain a
calm façade while the public request surface now carries computed paragraph
policy and optional `RegionFlow`. Rectangle preparation remains the ordinary
path; advanced callers may supply exact slots without constructing projection,
shaping, or line-formation internals.

The source-level changes and their replacements are recorded in
Design-0014's migration note. No high-level Parley type entered the public or
production dependency surface.

## Proof impact

The migration updates `style-projection`, `paragraph-formation`,
`layout-scene`, `exact-text-interaction`, and
`retained-text-block-intrinsics`. The proof ledger now names measured
`projected-text`, `reusable-line-formation`, `region-flow`,
`line-adjustment`, `cross-identity-text-reuse`, and `preparation-trace`
capabilities, plus executable computed-policy, CJK, font-catalog, and editable
block evidence.

The aggregate campaign review is
`docs/proof/reusable-text-tools-campaign-review-2026-07-25.md`. It deliberately
does not claim complete CSS Text semantics, script-independent justification,
mixed-bidi PDF extraction conformance, pixel snapping, or O(change) scene
publication.
