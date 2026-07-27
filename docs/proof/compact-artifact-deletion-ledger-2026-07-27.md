# Compact paragraph artifact deletion ledger — 2026-07-27

## Purpose

Design-0021 is a conceptual compaction, not a side-by-side representation
migration. This ledger records the pre-migration production symbols and source
size that must disappear or converge.

The final comparison belongs in this file. A renamed equivalent of an obsolete
owner does not count as deletion.

## Canonical path

```text
source snapshot
    → shared projection input
    → ParagraphFormation
    → one validated paragraph artifact
    → scene placement and binding
    → borrowed capability views
```

There must not be another normal path for labels, documents, adapter reuse,
paint changes, or interaction.

## Before-state size

The affected production directories contain 30 Rust files and 19,381 physical
lines when standalone test files are excluded:

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
    xargs wc -l
```

Inline unit tests remain in that coarse count. The number is a coherence
screen, not a claim about semantic code size. The named ownership deletions
below are the stronger gate.

The seven files containing most of the duplicated ownership total 6,161
physical lines:

| File | Before |
|---|---:|
| `underwood/src/adapter/interaction.rs` | 514 |
| `underwood/src/adapter/prepared.rs` | 888 |
| `underwood/src/scene/geometry.rs` | 1,183 |
| `underwood/src/scene/interaction.rs` | 1,790 |
| `underwood/src/block.rs` | 215 |
| `underwood_parley/src/engine.rs` | 927 |
| `underwood_parley/src/interaction.rs` | 644 |

## Required symbol deletions

### Complete portable cursor graph

- `PreparedCursorStep`
- `PreparedCursorMovement`
- `prepared_cursor_movements`
- the `PreparedParagraphFacts::movements` owner
- movement graph membership and dedup validation over repeated source values

Replacement: allocation-free cursor and caret views over the one
interaction-unit table and formed-line facts. The adapter remains responsible
for grapheme grouping, resolved bidi levels, and visual unit order; core
derives navigation without reconstructing Unicode analysis or retaining
another topology.

### Complete copied scene cursor graph

- `CachedCaret`
- `CachedCursorStep`
- `CachedCursorMovement`
- independent `carets` and `movements` sidecar owners
- lowering that copies every portable target and source span into scene values

Replacement: borrowed caret and movement views joining interaction units,
formed lines, hit placement, and the source map.

### Adapter final-output cache

- adapter `RetainedOutput`
- `ParleyParagraphEngine::outputs`
- `PreparationCache::prepared`
- `ParagraphFormationReuse::RetainedOutput` if no distinct owner remains
- exact-output lookup and eviction split between `cache` and `outputs`

Replacement: the scene/shared artifact cache owns exact final-output reuse.
Optional adapter retention owns only reusable preparation stages.

### Clone-based repaint

- `PreparationCache::prepared_paint_runs`
- `PreparedParagraph::try_map_glyph_paint`
- repainting by cloning every line, run, and glyph

Replacement: adapter-supplied source-to-glyph coverage topology plus core-owned
paint binding.

### Nested temporary final form

- `PreparedLineInteraction` as an allocation owner when the artifact has a
  paragraph-level table
- per-line interaction slice/unit owners built only to be flattened
- nested `PreparedLine → PreparedRun → PreparedGlyph` collection built only to
  be copied into scene tables
- `CachedGeometryFacts` if it remains a second portable layout owner

Replacement: one checked flat artifact builder filled directly by the adapter.

### Plain-block miniature document

- `TextBlock { document: Document, ... }`
- `TextBlock::plain` edit/append/commit construction
- `TextBlockSnapshot { document: DocumentSnapshot, ... }`
- conversion back into a temporary document path during `prepare_block`

Replacement: one compact immutable single-paragraph state implementing the
same internal paragraph-source view used by document paragraphs.

## Review questions after every slice

1. Did the change delete an owner, or only make its fields smaller?
2. Can a public result borrow a joined view instead of owning a copy?
3. Is a retained proof value authoritative, or evidence duplicated from
   another authoritative table?
4. Would recalculation at the next explicit preparation boundary be cheaper
   than permanent residency?
5. Does the adapter build a nested form before the scene builds a flat one?
6. Is `TextBlock` still entering through the same projection, formation,
   artifact, and scene path without constructing a document tree?
7. Did affected production source shrink? If not, which new invariant justifies
   the additional code?

## Completion table

| Obligation | Before | After | Status |
|---|---|---|---|
| affected production files | 30 | 31 | in progress; one focused cursor module added |
| affected physical lines | 19,381 | 19,011 | in progress; −370 |
| seven duplicate-owner files | 6,161 | 5,317 | in progress; −844 |
| portable complete cursor graph | present | deleted | complete |
| copied scene cursor graph | present | deleted | complete |
| adapter final-output cache | present | deleted | complete |
| ordinary per-glyph paint coverage | present | deleted | complete |
| clone-based repaint | present | deleted | complete |
| nested-to-flat final lowering | present | deleted | complete |
| plain-block document tree | present | deleted | complete |
| one canonical paragraph artifact | absent | present | complete |
| borrowed indexed capability views | partial | present | complete |

This ledger stays pending until the implementation, numeric gates, and
requirement-by-requirement audit are complete.

## Cursor-derivation checkpoint

The first complete Design-0021 deletion removes:

- the public `PreparedCaret`, `PreparedCursorMovement`, and
  `PreparedCursorStep` types;
- `PreparedCursorTopology`, its position/caret/edge vectors, and its
  membership and source-index validation;
- `prepared_cursor_movements` and the adapter's O(positions × units)
  construction pass;
- cursor-movement arguments from both `PreparedParagraph` constructors;
- the old artificial graph-only tests and fixture helpers.

`scene/cursor.rs` is a borrowed derivation over `PreparedLine` and
`PreparedInteractionUnit`. Source lookup is binary. A visually reordered bidi
line retains only a 32-bit permutation from source rank to its existing visual
unit; source-monotonic lines retain no permutation.

The matched macOS live-heap run, after subtracting Underwood's font baseline,
measured:

| 1,000 labels | Before | After | Change |
|---|---:|---:|---:|
| editable/default | 17,746,272 B | 14,618,272 B | −3,128,000 B |
| editable/warm | 27,445,152 B | 24,317,152 B | −3,128,000 B |
| display | 13,698,272 B | 13,642,272 B | −56,000 B |

Parley's matched retained-layout delta remains 3,378,240 bytes, so editable
Underwood moved from 5.25× to 4.33×. The graph was deleted rather than shifted
to another retained owner.

On the 1,000-unit mixed LTR/RTL query fixture:

| Query | Underwood | Parley |
|---|---:|---:|
| exact point | 139 ns | 157 ns from the prior matched run |
| closest point | 230 ns | 200 ns from the prior matched run |
| byte position | 59 ns | 178 ns |

An intermediate scan-based derivation measured 2,709 ns for byte lookup. It
was rejected and replaced by the conditional 32-bit source-order permutation
before this checkpoint was accepted.

## Indexed scene-paint checkpoint

The second deletion makes the prepared paragraph artifact authoritative for
run and glyph identity. Scene records now retain only placement and paint
binding that cannot be recovered from the artifact:

- `CachedLine` retains bounds and post-formation adjustment, not copied
  advance, break, baseline, ascent, or descent;
- `CachedGlyph` retains its positioned origin and justification delta, not a
  copied glyph ID or complete advance;
- `CachedFragment` indexes one prepared line, run, and glyph range instead of
  cloning font data, synthesis, variation coordinates, bidi level, script, and
  transform;
- ordinary same-paint glyphs coalesce into a run-local fragment;
- split clipped glyphs retain a segment index and share one glyph instance;
- `paint_glyphs`, `paint_sources`, `line_sources`, and `line_fragments` are
  deleted; borrowed views derive source spans and line fragment ranges from
  the authoritative tables.

The scene no longer allocates and copies one variation-coordinate array per
fragment. Public line, fragment, glyph, PDF, and projected-source traversal
continues to borrow the same values and the complete workspace test suite
passes unchanged.

The matched 1,000-label macOS live heap, after subtracting the 1,820,512-byte
Underwood font baseline, measured:

| 1,000 labels | Before | After | Change |
|---|---:|---:|---:|
| display | 13,642,272 B | 12,278,272 B | −1,364,000 B |
| editable/default | 14,618,272 B | 12,902,272 B | −1,716,000 B |
| editable/warm | 24,317,152 B | 22,601,152 B | −1,716,000 B |

Default editable residency is now 3.82× the matched 3,378,240-byte Parley
layout delta, down from 5.25× before Design-0021 implementation began. This
still failed the approved 2× gate at that checkpoint; portable per-glyph paint
coverage and parallel prepared/placement shapes were the next active deletion
targets.

Ordinary per-glyph paint coverage was the next owner removed.
`GlyphPaintCoverage::whole()` is now a zero-payload marker: the prepared glyph
already owns its source, and core binds that source to the authoritative
projected `PaintRun`. Only a glyph genuinely split across paint boundaries
retains two or more explicitly clipped segments.

The 1,000-label editable live-heap delta fell another 1,292,000 bytes, from
12,902,272 to 11,610,272 bytes. That is 3.44× matched Parley. The public
adapter migration is recorded in Design-0021.

## Shared paint-topology checkpoint

Paint-only preparation no longer manufactures a replacement `CachedGeometry`
by cloning its artifact, facts, source map, hit geometry, and semantics.
`ParagraphSceneSegment` owns a small run-sized paint topology separately from
an `Arc<CachedGeometry>`. A paint change rebuilds that topology and shares the
complete immutable geometry allocation; a regression test asserts pointer
identity across the change.

The paint topology remains inline in the already-retained paragraph segment
rather than adding another `Arc` allocation. Recalculation on an explicit
paint or capability boundary is cheaper than another permanently resident
owner. `repaint_geometry` and the clone-based reconstruction path are deleted.

## Canonical paragraph-table checkpoint

The retained prepared artifact is no longer
`Vec<PreparedLine> → Vec<PreparedRun> → Vec<PreparedGlyph>` with another
line-local interaction allocation and per-run coordinate/source allocations.
It owns paragraph-level line, run, glyph, interaction-slice,
interaction-unit, source-order, coordinate, and unrendered-source tables.
Line and run records contain compact 32-bit table ranges; public traversal is
through copyable borrowed views.

The checked `PreparedLine` and `PreparedRun` construction values are drained
when `PreparedParagraph` is created. Their former interaction,
normalized-coordinate, and unrendered-source `Arc`s are deleted rather than
carried into the final artifact.

The matched 1,000-label editable live heap measured 12,991,104 bytes raw.
After subtracting the unchanged 1,820,512-byte Underwood font baseline, the
retained delta is 11,170,592 bytes: another 439,680-byte reduction, and 3.31×
the matched 3,378,240-byte Parley delta.

## Compact text-block checkpoint

`TextBlock` no longer retains a `DocumentState`, persistent
`ParagraphSequence`, paragraph allocation, leaf vector, and publication state
for a one-leaf label. Its immutable state is exactly a document-compatible
identity, revision, and shared string. A cache miss temporarily materializes
the ordinary paragraph contract, then drops it after the same adapter and
scene pipeline has consumed it.

Stable block repeats use a compact published root over the existing paragraph
cache. The layout engine no longer retains a per-block `StyleMap` or a general
published `DocumentSnapshot`; paint-only repeats still rebind paint values
without preparation.

The matched 1,000-label editable live heap measured 12,200,320 bytes raw, or
10,379,808 bytes after the same font baseline. This removes another 790,784
bytes and 4,162 live allocations from the preceding checkpoint. The retained
delta is now 3.07× Parley, which still fails the 2× gate and therefore does not
end the deletion campaign.

Canonical prepared-artifact bytes are now charged to scene layout residency.
The earlier diagnostics counted placement arrays while omitting the artifact
they index; category accounting must expose, not hide, the remaining owner.

## Global-allocation audit

The workspace-only retained comparison now has an optional global counting
allocator with phase-scoped measurements. On the 1,000-label editable fixture
with the normal zero adapter-fact budget:

| Phase | Allocation calls | Allocated bytes | Net bytes |
|---|---:|---:|---:|
| compact block build | 2,009 | 118,352 | 110,192 |
| cold preparation | 97,153 | 24,652,853 | 9,653,136 |
| exact stable repeat | 0 | 0 | 0 |
| edit publication | 2 | 104 | 104 |
| edited-paragraph preparation | 99 | 23,346 | 2,976 |
| paint-only preparation | 0 | 0 | 0 |

The optional warm run separately charges retained adapter facts instead of
mixing them into the ordinary scene figure. At 64 labels those facts retain
408,380 bytes.

`malloc_history -allByCount` then identified the two largest live call-site
classes as `PreparedParagraphFacts::flatten`: 1,456,000 and 1,302,000 bytes
across 1,000 allocations each. The next structural checkpoint therefore
compacts final glyph and interaction records rather than tuning already
allocation-free repeat or repaint paths.

## Compact glyph and interaction records

The canonical artifact no longer retains the wide checked construction values
for glyphs and interaction units:

- an interaction unit stores its source endpoints once, a compact slice range,
  native `f32` advance, bidi/boundary/whitespace bytes, and four flag bits;
  left/right positions are derived from the already-proven endpoints,
  orientation, and affinities;
- interaction-slice advances retain native shaping precision instead of
  widening every record;
- a final glyph stores its ID, source, and native `f32` advance/offset;
- ordinary glyphs retain no paint field at all; exceptional split coverage is
  held in one sorted paragraph-level side table and found only by split-paint
  queries;
- public readers use copyable `PreparedGlyphView` and
  `PreparedInteractionUnitView` values over those canonical tables.

On the same 1,000-label allocation-counter run:

| Cold preparation | Before | After interaction | After glyphs |
|---|---:|---:|---:|
| total allocated bytes | 24,652,853 | 23,722,853 | 22,927,853 |
| net retained bytes | 9,653,136 | 8,816,136 | 8,021,136 |
| scene-cache accounting | 6,274,314 | 5,436,738 | 4,641,162 |

The two record changes remove 1,632,000 retained bytes and 1,725,000 bytes of
cold allocation churn without changing the allocation-free stable-repeat or
paint-only paths. One edited paragraph's net preparation growth falls from
2,976 to 1,824 bytes.

## Single-owner lifecycle bookkeeping

The layout engine no longer retains three parallel trees for paragraph cache
membership, document membership, and recency. The paragraph cache entry is the
authoritative owner:

- document release scans the two bounded cache lanes and removes matching
  entries;
- budget overflow scans the affected bounded lane for the least recently used
  entry, including any newer published-root use;
- no per-paragraph document-membership set or recency node survives between
  lifecycle boundaries.

This deliberately spends bounded work on uncommon release and eviction
boundaries instead of permanent memory on every label. At 1,000 editable
labels it removes 1,330 live allocations and 420,392 net allocator bytes.
Exact stable repeat, paint-only preparation, and edited-paragraph allocation
counts are unchanged.
