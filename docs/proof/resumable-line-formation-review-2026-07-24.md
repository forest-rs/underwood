# Resumable line-formation review — 2026-07-24

## Status

The rectangle product path uses one allocation-free, resumable line-former
kernel over retained Parley Engine cluster facts. This review records the
measured extraction checkpoint. Regions, exclusions, floats, and columns were
separate at that checkpoint and now consume the same state and retry protocol;
see `region-flow-review-2026-07-25.md`.

## Ownership fence

`underwood_parley::line_former` imports only:

- `alloc::vec::Vec` for atomic truncation of caller-owned provisional output;
- `core::ops::Range`;
- Parley Engine `Boundary` and `Whitespace` facts.

It imports no Underwood document, scene, style, adapter, renderer, font-catalog,
or high-level Parley type. `line_break` is the adapter edge: it translates
Underwood constraints and errors, invokes line-final shaping, computes
Underwood line metrics, and lowers accepted candidates.

The split is:

```text
retained Parley Engine cluster facts
        ↓
LineFormer: propose → checkpoint → final-fit decision → accept/retry/restore
        ↓
Underwood adapter: line-final shape + metrics + portable line records
```

This keeps line traversal reusable without moving document or scene ownership
downward.

## State contract

A candidate carries:

- logical cluster and UTF-8 source ranges;
- regular, mandatory, or end break reason;
- provisional canonical advance;
- trailing-whitespace cluster range and advance.

Candidate proposal does not move the cursor. `commit` receives the line-final
advance and height plus optional inline and block limits. It has three results:

1. accept and advance the cursor;
2. reject a fit-changing regular candidate and propose the preceding legal
   boundary;
3. reject the current slot without advancing when the line-final height does
   not fit.

An accepted line has therefore passed the line-final checks applicable to its
slot. Unbreakable, mandatory, and terminal lines preserve Underwood's existing
honest-overflow rule; acceptance explicitly distinguishes inline overflow
instead of reporting a false fit.

A checkpoint stores traversal, terminal-break state, and provisional output
length. Restoring it resets traversal and truncates the caller's `Vec` in one
operation. Diagnostic counters are monotonic work observations and do not
rewind.

## Product-path behavior

The old fused loop selected a canonical boundary, shaped it, mutated the end
index on overflow, and built Underwood lines in one scope. The rectangle path
now:

1. asks `LineFormer` for a candidate;
2. shapes exactly that source range in bounded line context;
3. derives actual advance and font-backed height;
4. commits or retries through the state machine;
5. publishes only accepted shaped output.

The unwrapped single-line fast path also proposes and commits through the same
kernel, while continuing to reuse the canonical retained `ShapedText`.
Transient formation telemetry was removed from the paragraph preparation
cache; cache entries contain reusable prepared state, not observations from
the most recent call.

## Observable rejected work

`LineShapingWork` retains its existing four-argument constructor. The additive
`with_formation` builder records proposed candidates, rejected candidates, and
checkpoint restores. `FormationWork` and `WorkReport` expose those values,
including accepted candidates derived from proposed minus rejected work.

The real Arabic fit-changing fixture reports exactly one rejected candidate and
one additional proposal while retaining every line-final shaping attempt. A
scene-level adapter fixture proves the counters survive portable formation and
scene aggregation.

## Correctness evidence

Focused state-machine tests prove:

- checkpoint restoration rewinds traversal and provisional output together;
- line-final expansion retries the preceding legal boundary;
- a line-final height exceeding the slot is rejected without cursor movement;
- CRLF forms one mandatory candidate and requests the terminal empty line;
- source ranges and trailing-whitespace facts are exact.

The real adapter corpus remains green for:

- Arabic joining changes at a zero-width break;
- Arabic line-final expansion that rejects a formerly fitting seam;
- ligature components and exact source coverage;
- mixed LTR/RTL visual ordering and bidi affinities;
- CRLF, LF, line separator, and paragraph separator mandatory breaks;
- min-content, max-content, and constrained formation;
- non-breaking spaces and unbreakable overflow;
- extended graphemes split across semantic leaves;
- soft-wrap carets, mandatory-break carets, and cursor movement.

No high-level Parley dependency was introduced. Current high-level
`BreakLines` remains a behavioral oracle for resumable state, not an
implementation dependency.

## Performance and allocation evidence

The comparison used release builds on the same host and the same pinned
dependencies. The baseline was commit `af50391`; the candidate used this
slice. Each timing operation reforms a retained public `TextBlock` across
alternating finite widths.

| Region-ready width reformation | Baseline | Candidate |
| --- | ---: | ---: |
| Median ns/label | 25,754 | 25,576 |
| Allocation calls | 181 | 181 |
| Allocated bytes | 25,592 | 25,592 |

The timing difference is within machine noise and slightly favors the
candidate. Five candidate trials ranged from 25,477 to 25,941 ns/label; three
completed baseline trials ranged from 25,369 to 25,924 ns/label.

Removing transient work from retained cache entries also improved the matched
one-operation allocation traces:

| Workload | Baseline bytes | Candidate bytes | Calls |
| --- | ---: | ---: | ---: |
| Cold identical | 130,464 | 130,288 | 542 |
| Identity churn | 225,921 | 225,745 | 1,009 |

Retained, paint-only, localized-edit, width-churn, projection, and region-ready
counts are unchanged. The kernel itself allocates nothing during proposal,
fit checking, acceptance, or retry. Restore can truncate caller-owned
provisional capacity but does not allocate.

A five-second release `sample` of region-ready work showed the state machine
inlined into `form_lines`; bounded `shape_line` work remains the visible
formation cost. No separate line-former allocation or hot symbol appeared.

## Public migration

This is an additive public diagnostics change:

- existing `LineShapingWork::new(attempts, clusters, runs, glyphs)` call sites
  require no change;
- adapters may append `.with_formation(candidates, rejected, restores)`;
- consumers may read the new formation counters from `FormationWork` or
  `WorkReport`.

The reusable line former itself remains private while its region and computed
style consumers are built. Moving it to Parley Engine or a sibling crate later
must preserve this corpus and receive its own API migration.

## Deliberate limits and follow-ups

- Inline-item measurement, wrap policy, emergency breaking, spacing, and
  complete line-height semantics belonged to the computed-policy slice and are
  reviewed in `computed-text-policy-review-2026-07-25.md`.
- Slot transcripts, exclusions, floats, columns, and product height rejection
  are reviewed in `region-flow-review-2026-07-25.md`.
- Alignment and justification consume accepted slots and trailing-whitespace
  facts; they do not mutate canonical shaping.
