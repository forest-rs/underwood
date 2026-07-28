# Conceptual compaction baseline — 2026-07-27

## Purpose

This is an external engineering ledger for `und-0re`. It is not a runtime
model and it does not authorize runtime records whose purpose is to certify
other runtime records.

Underwood is a high-performance, compact text framework. Correctness evidence
belongs here, in tests, and in wind tunnels. Production data exists only when
rendering, querying, editing, reflow, export, cache reuse, or measured
diagnostics need it.

The measured tree is `3fe34114acafa630c58151e29d795359e00154b7`.

## Product laws

1. Untrusted adapter output is checked once when it becomes the canonical
   paragraph artifact.
2. Internal consumers trust that canonical artifact. They do not repeatedly
   prove its topology while traversing it.
3. A cheap derived observation is recalculated unless measurement shows that
   retaining it improves the product enough to pay for its residency.
4. A wrapper or table whose only role is to show that another table is valid
   is a deletion target.
5. Smaller source is useful because it usually means fewer state machines,
   fewer invalid states, and a calmer hot path. Line count alone is not the
   product.

## Reproducible source screen

The affected tree contains 31 non-standalone-test Rust files, 21,366 physical
lines, and 18,407 Rust code lines. `tokei` and `scc` independently report the
same physical and code counts:

```sh
{
    rg --files \
        underwood/src/adapter \
        underwood/src/scene \
        underwood_parley/src |
        rg '\.rs$' |
        rg -v '(^|/)tests(/|\.rs$)'
    printf '%s\n' underwood/src/block.rs
} |
    sort -u |
    tee /tmp/underwood-concept-files.txt |
    xargs tokei

xargs scc < /tmp/underwood-concept-files.txt
```

`tokei` treats 1,102 lines of rustdoc as embedded Markdown while `scc` counts
them as Rust comments; both still report exactly 18,407 Rust code lines. That
is the primary campaign ratchet. The Design-0021 closure ledger separately
records 19,756 production lines before inline test modules, using its earlier
physical-line method. Two small Parley-integration changes landed after that
closure. The final comparison will report 18,407 code lines, 21,366 physical
lines, and named deleted concepts rather than trying to make the old and new
methods appear interchangeable.

The largest current files are:

| File | Physical lines |
|---|---:|
| `underwood/src/scene/engine.rs` | 3,050 |
| `underwood/src/scene/interaction.rs` | 1,802 |
| `underwood/src/adapter/prepared.rs` | 1,692 |
| `underwood/src/scene/views.rs` | 1,623 |
| `underwood/src/scene/geometry.rs` | 1,256 |
| `underwood/src/scene/spine.rs` | 1,065 |
| `underwood/src/scene/projection.rs` | 1,002 |
| `underwood_parley/src/line_break.rs` | 998 |
| `underwood/src/adapter/formation.rs` | 958 |
| `underwood_parley/src/engine.rs` | 722 |
| `underwood/src/scene/facades.rs` | 699 |

## Measured runtime baseline

All timings are seven release samples on the same host. Values are the median
nanoseconds per operation.

| Scale | Operation | Underwood | Parley | Ratio |
|---:|---|---:|---:|---:|
| 64 | exact repeat | 79 | 189 | 0.42× |
| 64 | localized edit | 5,773 | 2,992 | 1.93× |
| 64 | exact hit | 61 | 15 | 4.07× |
| 64 | closest hit | 101 | 15 | 6.73× |
| 64 | byte position | 84 | 13 | 6.46× |
| 64 | churn | 10,059 | 6,406 | 1.57× |
| 1,000 | exact repeat | 114 | 188 | 0.61× |
| 1,000 | localized edit | 5,884 | 3,069 | 1.92× |
| 1,000 | exact hit | 80 | 182 | 0.44× |
| 1,000 | closest hit | 121 | 178 | 0.68× |
| 1,000 | byte position | 124 | 179 | 0.69× |
| 1,000 | churn | 8,511 | 4,734 | 1.80× |

The 64-unit query result matters: Underwood wins the long-query scaling case
but pays too much fixed query ceremony on small text. Compaction should improve
that result, not merely preserve the 1,000-unit headline.

One macOS live-heap sample, after subtracting each engine's own font baseline:

| 1,000 retained labels | Live bytes above font baseline | Parley ratio |
|---|---:|---:|
| Underwood display | 3,300,096 | 0.98× |
| Underwood editable | 3,644,096 | 1.08× |
| Parley | 3,378,240 | 1.00× |

The bounded 64-entry churn state retains 315,344 bytes above Underwood's font
baseline versus Parley's 291,008 bytes, or 1.08×.

`malloc_history` records:

| Scale | Operation | Underwood | Parley |
|---:|---|---:|---:|
| 64 | cold display calls / bytes | 1,061 / 289,622 | — |
| 64 | cold editable calls / bytes | 1,253 / 310,231 | 863 / 284,010 |
| 64 | localized edit calls / bytes | 15 / 2,878 | 3 / 1,147 |
| 1,000 | cold display calls / bytes | 13,075 / 3,244,912 | — |
| 1,000 | cold editable calls / bytes | 16,075 / 3,566,913 | 10,691 / 3,422,654 |
| 1,000 | localized edit calls / bytes | 15 / 2,878 | 3 / 1,147 |

Exact repeat, exact hit, closest hit, and byte-position queries allocate zero;
the profiler's ±1–2-byte differences are subtraction noise. The benchmark's
counting allocator observes 16 calls / 3,200 requested bytes for edited
preparation because the instrumentation itself changes allocation shape.

## Concept inventory

### Irreducible product seams

- `ParagraphFormation` separates Underwood's backend-neutral scene machinery
  from Parley Engine. This is a real extension and ownership boundary.
- One compact `PreparedParagraphFacts` owner is real. It is the final portable
  line/run/glyph/interaction artifact shared by cache and scene.
- Paragraph-local source mapping, capability omission, scene placement,
  region transcripts, paint binding, and the persistent scene spine serve
  distinct product behavior.
- Parley analysis, shaping, line formation, and their reusable cache state are
  expensive facts whose lifetime remains a measured policy decision.

### Construction ceremony

The current public adapter path uses three nested state machines:

- `PreparedParagraphBuilder`;
- `PreparedLineBuilder`;
- `PreparedRunBuilder`.

They carry `failed`, `finished`, `Option<T>`, start offsets, and `Drop`
poisoning. Local constructors validate `PreparedLine`, `PreparedRun`, and
`PreparedGlyph`; builder pushes validate them again against enclosing ranges;
line and run completion validate coverage and table membership; paragraph
completion validates final coverage.

Some ingestion checking is necessary for third-party adapters. The nested
poisoned protocol is not itself necessary. Underwood's only current production
backend is forced through the same protocol even though it already owns the
formed lines and source analysis from which those records are derived.

`PreparedLine` mirrors nearly every field in `PreparedLineRecord`.
`PreparedRun` mirrors nearly every non-index field in `PreparedRunRecord`.
`PreparedGlyph` is expanded and then immediately compacted into
`PreparedGlyphRecord` plus rare placement and paint spill records.

`PreparedParagraphCapacity` causes `underwood_parley::prepared_capacity` to
walk every formed line, run, cluster, and glyph before the lowering walk. The
prepass exists to predict exact allocation, not to produce text behavior.

### Traversal ceremony

The canonical artifact has three iterator-container types and three view
types:

- `PreparedLines` → `PreparedLineView`;
- `PreparedRuns` → `PreparedRunView`;
- `PreparedGlyphs` → `PreparedGlyphView`.

The views perform useful joins between compact records and paragraph tables.
The iterator containers mostly reproduce slice iterator operations and can be
replaced by direct indexed access plus opaque exact-size iterators.

Interaction has the same container/view pattern. It should be considered
together rather than mechanically rewritten one family at a time.

### Scene facade ceremony

`scene/facades.rs` is 699 lines and defines ten one-reference wrappers:

- committed and projected display;
- committed and projected source access;
- committed and projected semantics;
- committed and projected interaction;
- committed selection and editing;
- projected editing.

Most methods forward directly to methods already implemented on `TextScene` or
`CompositionScene`. Display facades duplicate unconditional scene methods.
Editing duplicates interaction and selection methods.

The sharpest artificial state is source access: a caller can pass a view from
one scene into another scene's source facade, so Underwood carries
`ForeignSceneView` plus pointer and revision checks to reject it. A view that
observes its own optional source map cannot express that cross-scene state.

Capability acquisition currently scans paragraph segments. The scene spine can
summarize capability union/intersection once; successful hot checks can then be
O(1), with a paragraph scan reserved for constructing a cold error diagnostic.

### Committed/projected duplication

`scene/views.rs` and `scene/interaction.rs` maintain parallel committed and
composition wrapper families. The distinction between authored snapshot
positions and generated composition positions is real. The repeated line,
fragment, glyph, iterator, placement, and forwarding implementations are not
automatically real. A private scene mode with public aliases, or a unified
source observation where appropriate, can preserve the semantic distinction
without duplicating traversal.

### Diagnostics versus product state

`FormationWork`, `PreparationTrace`, cache accounting, and residency accounting
are small or computed diagnostic observations. They do not justify retained
copies of layout. The compaction design must keep diagnostics optional and
must not let their vocabulary dictate the canonical runtime representation.

`ParagraphFormationChange` is performance policy, not a correctness proof. Its
nine booleans should survive only if each independently avoids measured work;
a compact mask or generation tuple is preferable to a getter-heavy public
record when the distinction remains useful.

## Ranked whole-concept deletion candidates

### 1. Delete nested poisoned adapter builders

Replace line/run/glyph input mirrors and three nested builder state machines
with flat canonical parts assembled directly by a backend and checked once at
ingestion. Keep one checked public constructor. The in-tree Parley adapter
fills the same flat tables directly; it does not receive a special less-safe
runtime representation.

This should also delete the exact-capacity prepass. Reused engine-owned table
storage or ordinary amortized growth must be compared before choosing the
replacement.

### 2. Delete the scene facade taxonomy

Put unconditional display traversal directly on scenes, source observations on
the views that own them, and capability-checked query operations directly on
scenes. Summarize available capabilities so successful checks are O(1).
Delete `ForeignSceneView` by making the foreign-view combination
unrepresentable rather than validating it at runtime.

### 3. Delete iterator-container forwarding families

Keep compact record views but expose direct `line/run/glyph` lookup and opaque
exact-size iterators. Apply the same rule to interaction units. Do not replace
the current named wrappers with a generic abstraction larger than the code it
removes.

### 4. Converge committed and projected traversal

Share the traversal implementation behind the two genuine position/source
models. Public aliases may preserve useful type distinctions. This slice
follows the facade deletion so it is measured against the smaller surface.

## Rook audit

### Real

- The one-artifact runtime representation is genuinely compact relative to the
  pre-Design-0021 tree.
- Ordinary display and editable residency remain close to or better than
  matched Parley.
- Exact-repeat and query allocation results are real.
- Source completeness, bidi interaction, regions, and sparse capabilities are
  exercised by product paths rather than placeholder APIs.

### Mirage

- “Checked streaming construction” sounds like one compact boundary, but it is
  three nested state machines plus mirrored metadata values.
- Capability facade names imply distinct operational objects; almost all are
  one-reference forwarding wrappers.
- `ForeignSceneView` protects an invalid combination created by the facade API
  itself.
- Long-text query wins can hide substantial fixed overhead on label-sized
  text.

### Most dangerous gap

Deleting public wrappers while recreating the same phases behind private
helpers would improve line count without simplifying the system. Design-0022
must name the states that cease to exist, show the new hot call paths, and
measure small-label queries as well as long-text scaling.

## Next gate

Design-0022 must present the exact replacement call sites and migration, then
receive explicit human approval before the public adapter and scene APIs
change.
