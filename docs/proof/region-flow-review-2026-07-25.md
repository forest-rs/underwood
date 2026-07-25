<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Region-flow review

- **Date:** 2026-07-25
- **Bead:** `und-oh0.13.7`
- **Design:** Design-0014 and ADR-0005
- **Scope:** exact region slots, exclusions, physical floats, columns,
  height-sensitive retry, retained invalidation, source and interaction
  geometry, deterministic replay, and measured churn

## Result

The public document, composition, and `TextBlock` paths now form text through
one immutable `RegionFlow`. Ordered `FlowRegion` values compile rectangles,
rectangular exclusions, physical left/right floats, and columns into exact
line slots before preparation begins. The same resumable cursor then crosses
paragraph boundaries.

This is product formation, not a geometry-only demo. The Parley adapter drives
the existing reversible `LineFormer` with each offered slot width, performs
line-final shaping, checks the measured line height, and either accepts the
line or restores text traversal before advancing only the region cursor.
Accepted lines retain their exact slot, and scene lowering places every
spatial record in that slot.

Every preparation publishes a replayable `RegionTranscript`. It records each
source range, offered slot, measured height, and accepted or height-rejected
outcome. Underwood replays and cross-checks backend output before caching or
publishing geometry. An adapter that ignores region constraints is rejected
with `SceneErrorKind::Flow`; Underwood never quietly falls back to one width.

## Ownership fence

The boundary is:

> Region flow owns deterministic slot decomposition, cursor progression,
> physical float placement, and ordered-region continuation. It explicitly
> does not own line breaking, shaping, paragraph policy, callbacks, paint, or
> rendering.

The line former continues to own candidate traversal, checkpoints, final-fit
decisions, and restoration. Scene preparation owns document and composition
projection, retained invalidation, transcript validation, semantic identity,
and materialization. No callback-shaped general layout context entered the
kernel.

The rejected alternatives were:

1. putting exclusions and columns inside the line former, which would merge
   text traversal with document geometry;
2. a broad region-provider trait with callbacks during formation, which would
   make replay, cache identity, and allocation behavior implicit.

The chosen concrete transcript is deliberately replayable data. More elaborate
shape or float policies can decompose their decisions into the same slots
without entering the shaping engine.

## Public protocol

The additive public vocabulary is:

- `FlowRegion`, `RegionFloat`, and `FloatSide` for immutable authored geometry;
- `RegionFlow` for compiled ordered regions;
- `RegionCursor` and `LineSlot` for resumable traversal;
- `RegionAttempt`, `RegionAttemptOutcome`, and `RegionTranscript` for exact
  execution evidence;
- `SceneRequest::with_region_flow` and `BlockRequest::with_region_flow`;
- `SceneOutput::region_transcript` and
  `CompositionSceneOutput::region_transcript`;
- adapter access through `ParagraphConstraints::region_flow`,
  `ParagraphConstraints::region_cursor`, `PreparedLine::slot`, and
  `ParagraphFormationOutput::in_regions`.

`RegionFlow::new` validates and precompiles all vertical bands and inline
intervals. Clones share the compiled backing. Slot lookup, acceptance,
rejection, and replay allocate no scratch storage. Region requests normalize
their otherwise irrelevant fallback constraint to the widest region; exact
slot widths are the only formation limits.

Floats use physical `Left` and `Right` sides. Logical start/end placement
belongs to paragraph policy and forthcoming alignment, not to shape
decomposition.

## Retry and progression law

For every nonempty line:

1. the current cursor offers one exact `LineSlot`;
2. the line former proposes a source candidate using the slot width;
3. Underwood shapes that complete candidate and measures its final line box;
4. an inline fit-changing result retries an earlier legal boundary in the same
   slot;
5. a block-size failure records `HeightRejected`, restores the line checkpoint
   and provisional output, advances the region cursor, and reproposes from the
   same text traversal state;
6. acceptance records the source and slot together and advances by the maximum
   accepted height across same-row intervals.

An empty paragraph performs the same height-sensitive slot transaction using
its deterministic empty-line height. It consumes flow and retains a
represented caret without fabricating source text, glyphs, or a `SceneLine`.

The full document and transient composition loops carry one cursor across
paragraphs. Cache hits retain the paragraph transcript and recover its exact
end cursor, so a stable leading paragraph cannot move later paragraphs by
skipping flow progression.

## Correctness evidence

The core traps prove:

- a rectangle advances by accepted line height;
- a central exclusion offers both same-row intervals;
- left and right floats compile into exact vertical bands;
- ordered rectangles continue as columns;
- accepted and height-rejected attempts replay to the recorded cursor.

The real Parley product tests prove:

- `product_path_restores_text_after_height_rejection_and_continues_in_a_column`
  retries the same source after a too-short first column and reports the
  rejected candidate and checkpoint restore;
- `exclusion_intervals_share_a_row_without_overlapping_text_geometry` places
  source-complete lines in both intervals without crossing the exclusion;
- `floats_decompose_into_distinct_zero_allocation_slot_bands` consumes the
  actual float-shaped bands;
- `paragraphs_resume_one_cursor_across_region_boundaries` carries one cursor
  across semantic paragraphs and columns;
- `empty_paragraph_consumes_height_without_fabricating_text` preserves an
  empty source and exact caret;
- `line_height_change_retries_regions_without_reshaping` changes slot
  acceptance with zero analysis, canonical shaping, or line reshaping;
- `region_offsets_move_mixed_bidi_hits_carets_and_selections_together` keeps
  bidi levels and logical source order while line bounds, hit slices, carets,
  and visual-selection rectangles move by the same offset;
- `composition_projection_flows_through_the_same_exact_region_transcript`
  proves generated IME text uses the same public region path and provenance;
- `region_request_rejects_an_adapter_that_ignores_exact_slots` proves the
  portable adapter boundary fails closed;
- `empty_region_output_rejects_a_cursor_height_that_disagrees_with_geometry`
  prevents an adapter from advancing downstream flow by a height different
  from the empty caret and semantic geometry;
- `hit_area_padding_does_not_inflate_zero_advance_intrinsic_width` keeps the
  region-aware metrics calculation from leaking one-pixel interaction padding
  into ordinary intrinsic measurement.

Prepared lines are cross-checked against accepted transcript attempts for
source, exact slot, and line height. Every attempt must name the prepared
paragraph and a valid projected UTF-8 range. The complete transcript must
replay from the requested start cursor to the backend's recorded end cursor.
For an empty paragraph, the one accepted attempt must also consume exactly the
computed empty-line height used by scene geometry.

## Retained invalidation

Region geometry and start cursor are explicit formation keys in both the
Underwood scene cache and the Parley adapter cache. Changing only region
geometry:

- reuses projection and Unicode analysis;
- reuses itemization and selected fonts;
- reuses canonical paragraph shaping;
- reforms lines for the new slots;
- rebuilds all geometry from accepted slots.

Changing only line height retains analysis and glyph shaping but reforms
height-sensitive region placement. Ordinary non-region line-height changes
retain the existing cheaper line-metric update path.

## Performance and allocation evidence

The separate `underwood_label_benchmark` crate exercises the real public
`TextBlock` request and output path. Five paired release trials used 200 rounds
of 512 retained labels per scenario, or 102,400 operations per trial:

| Workload | Median ns/label | Five-trial range |
| --- | ---: | ---: |
| Pre-region width reformation | 25,050 | 24,923–25,180 |
| Exclusion/float/column region churn | 26,309 | 26,247–26,404 |

Advanced region churn was 5.0% slower on this host. The comparison is
deliberately conservative rather than perfectly identical: `region-ready` is
the checked-in pre-region finite-width formation baseline, while
`region-churn` alternates real exclusion and float/column flows. Both retain
analysis, font selection, and canonical shaping and reform one paragraph per
operation.

The macOS full-allocation trace, after subtracting matched process setup,
reported:

| Workload | Allocation calls | Allocated bytes |
| --- | ---: | ---: |
| Pre-region width reformation | 183 | 25,737 |
| Exclusion/float/column region churn | 188 | 26,793 |

Region churn therefore adds five allocation calls and 1,056 allocated bytes
per one-label trace. Slot lookup itself allocates nothing; the added owned data
is the exact transcript and region-bearing result/cache state.

A five-second release `sample` captured 4,179 main-thread samples during active
region churn. No region symbol exceeded 17 top-of-stack samples. The visible
cost remains paragraph formation, line-final shaping, and scene geometry.
`prepared_cursor_movements` alone accounted for 277 top-of-stack samples, and
allocator/free traffic was prominent. That is actionable evidence for
`und-oh0.13.11`; it is not attributed to the region protocol.

These are local macOS observations, not portable performance guarantees.

## Public migration

This is an approved additive pre-stable API change:

- existing `SceneRequest::new`, `BlockRequest::new`, and non-region adapters
  remain source-compatible;
- callers opt in with `.with_region_flow(&flow)`;
- region construction reports `SceneErrorKind::InvalidRegion` for invalid
  rectangles, exclusions, floats, or transcript geometry;
- adapters supporting a region request must use the supplied flow and cursor,
  retain the exact slot on every prepared line, and return the replayable
  transcript with `ParagraphFormationOutput::in_regions`;
- adapters that do not implement the protocol continue to work for ordinary
  constraints but fail a region request explicitly;
- `ParagraphConstraints` is now `Clone` rather than `Copy` because it may own a
  cheap clone of immutable compiled region policy;
- a region request supersedes the request's single finite width. Hosts should
  express all desired widths through `FlowRegion` bounds and exclusions.

No compatibility layer converts callbacks into regions or accepts geometry
without replay evidence.

## Deliberate limits and follow-ups

- One `LineSlot` is one contiguous inline interval. Multiple intervals in a
  band become consecutive independently represented scene lines at the same
  block coordinate, not one logical line with multiple fragments.
- Built-in decomposition is rectangular. Curved or polygonal shape policy must
  currently supply a rectangular approximation or await a separately proven
  decomposer.
- Float placement is caller-authored and physical. Underwood does not implement
  CSS float avoidance, clearance, logical float sides, or re-placement.
- Region exhaustion returns an error without publishing a partial scene.
  Pagination or continuation tokens beyond the existing end cursor need a
  separate product contract.
- Alignment and Western justification do not belong to region flow. They are
  the next `und-oh0.13.8` adjustment stage over accepted slots.
- CJK and Arabic justification remain separate capabilities; no claim is
  inferred from rectangle, bidi, or Arabic-composition coverage here.

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

The workspace tests include the deterministic CPU visual snapshot and PDF
proof. `cargo tree -d --locked` reports one coherent dependency universe.
