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
| affected physical lines | 19,381 | 19,013 | in progress; −368 |
| seven duplicate-owner files | 6,161 | 5,380 | in progress; −781 |
| portable complete cursor graph | present | deleted | complete |
| copied scene cursor graph | present | deleted | complete |
| adapter final-output cache | present | — | pending |
| clone-based repaint | present | — | pending |
| nested-to-flat final lowering | present | — | pending |
| plain-block document tree | present | — | pending |
| one canonical paragraph artifact | absent | — | pending |
| borrowed indexed capability views | partial | — | pending |

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
