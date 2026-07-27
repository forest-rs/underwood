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

Replacement: compact indexes over one unique position/caret table and one
interaction-unit table. The adapter remains responsible for resolved bidi
policy; core borrows its topology rather than reconstructing Unicode behavior.

### Complete copied scene cursor graph

- `CachedCaret`
- `CachedCursorStep`
- `CachedCursorMovement`
- independent `carets` and `movements` sidecar owners
- lowering that copies every portable target and source span into scene values

Replacement: borrowed caret and movement views joining the artifact's position,
edge, unit, line, and source-map tables.

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
| affected production files | 30 | — | pending |
| affected physical lines | 19,381 | — | pending |
| seven duplicate-owner files | 6,161 | — | pending |
| portable complete cursor graph | present | — | pending |
| copied scene cursor graph | present | — | pending |
| adapter final-output cache | present | — | pending |
| clone-based repaint | present | — | pending |
| nested-to-flat final lowering | present | — | pending |
| plain-block document tree | present | — | pending |
| one canonical paragraph artifact | absent | — | pending |
| borrowed indexed capability views | partial | — | pending |

This ledger stays pending until the implementation, numeric gates, and
requirement-by-requirement audit are complete.
