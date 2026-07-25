# Design-0016: Cross-identity preparation cache

- **Status:** Approved — 2026-07-25
- **Date:** 2026-07-25
- **Bead:** `und-oh0.13.14`
- **Extends:** Design-0014 and ADR-0005

## Goal

Let distinct text consumers reuse expensive immutable preparation when their
projected text and preparation inputs are identical, while every consumer
retains its own document, paragraph, revision, source, semantic, interaction,
paint-table, and placement identity.

Stable retained output remains the cheapest path. This cache addresses a
different workload: a UI containing many different elements whose labels share
the same content and computed policy.

## Fence

A shared-formation cache owns immutable paragraph-local prepared facts keyed
only by identity-free preparation inputs; it explicitly does not own document
or paragraph identity, revisions, source-leaf identity, semantic identity,
paint-table values, selections, final geometry, or placement.

## Measured before state

The dedicated release wind tunnel clears all retained state at the start of
each round, then prepares 512 distinct element identities. Five trials of 50
rounds produced:

| Workload | Operations | Median ns/op | Analysis | Shape | Formation |
| --- | ---: | ---: | ---: | ---: | ---: |
| Identical text | 25,600 | 24,077 | 25,600 | 25,600 | 25,600 |
| Distinct text | 25,600 | 45,288 | 25,600 | 25,600 | 25,600 |

One-operation full malloc-stack-logging traces, after subtracting matched setup
processes, report:

| Workload | Allocation calls | Allocated bytes |
| --- | ---: | ---: |
| Identical text | 553 | 136,346 |
| Distinct text | 943 | 222,678 |

The different timings are not evidence of reuse: every operation currently
analyzes, shapes, and forms a paragraph. The exact work counts are the primary
before-state law.

## Required invariants

1. A shared entry contains no `DocumentId`, `ParagraphId`, revision,
   `TextId`, `SemanticId`, composition identity or epoch, selection,
   `SceneFragmentId`, final scene geometry, or placement.
2. Prepared facts use paragraph-local projected UTF-8 ranges only.
3. A hit cannot cross a preparation input that can alter analysis, font
   selection, shaping, spacing, line metrics, breaking, or accepted slots.
4. Alignment and `PaintTable` brush values do not invalidate shared formation.
5. Paragraph-local paint-slot coverage participates while prepared glyphs
   carry that coverage; actual brush values never do.
6. One `LayoutEngine` and its owned paragraph backend define the font-catalog
   universe. Shared entries never cross engines or font snapshots.
7. Region transcripts retained by the shared cache contain paragraph-local
   attempts only. Public transcripts are rebound to the current paragraph.
8. Every hit rebuilds source, semantic, interaction, fragment, and geometry
   identity from the current projection.
9. Stable same-identity retained geometry remains a faster lookup ahead of the
   shared cache.
10. Shared retention is bounded by an explicit byte budget. An entry larger
    than the whole budget is returned to its caller but not retained.
11. `release_document` releases identity-bound state and does not flush useful
    identity-free facts. `clear_cache` clears both.
12. Hits, misses, evictions, resident entries, resident estimated bytes, and
    peak estimated bytes are observable.
13. Core implementation remains `no_std + alloc`, Rust 1.88 compatible, and
    adds neither a dependency nor `unsafe`.
14. A paragraph backend participates only through an explicit
    identity-independence contract. The default for existing and third-party
    backends is no shared reuse.
15. An opted-in backend supplies a cache epoch covering every hidden resource
    that can alter prepared output. An epoch change invalidates all shared
    entries before another lookup.
16. A shared hit is validated against the current projection exactly as a
    fresh backend output is; cache residency never weakens adapter validation.

## Options

### A. Share final `SceneOutput`

This resembles the previous high-level Parley cache and would make a hit very
cheap.

It is rejected. A scene intentionally owns revision-bound positions,
semantics, fragment identity, hit geometry, paint, and final placement.
Sharing it would either leak identity or require a second scene-rewriting
system whose invariants duplicate normal lowering.

### B. Add a shared cache only inside `ParleyParagraphEngine`

This could share Parley analysis and shaping directly and preserve progressive
stage invalidation.

It is not the first slice. It makes cache ownership and budgets
backend-specific, leaves other `ParagraphFormation` implementations with
different semantics, and requires Underwood to coordinate a cache it cannot
observe. It remains a possible later optimization if exact prepared-fact reuse
does not capture enough width or policy churn.

### C. Cache exact portable prepared facts in `LayoutEngine`

After a backend miss produces a validated `PreparedParagraph`, retain its
immutable paragraph-local facts under an exact identity-free formation key.
On a hit, create a cheap current-paragraph envelope, rebuild any public region
transcript with the current paragraph identity, and run the ordinary current
projection through geometry and semantic lowering.

Choose C.

This keeps cache budget, lifetime, diagnostics, and semantic rebinding with the
owner that already coordinates retained geometry. It works for every paragraph
backend that explicitly promises identity-independent prepared facts, does not
expose backend-private Parley records, and removes all analysis, font-selection,
shaping, and formation work for exact repeated labels. It deliberately
continues to pay per-consumer geometry cost.

## Backend eligibility

`ParagraphFormation` currently receives a `ParagraphId`. Existing trait
semantics do not forbid a backend from choosing different metrics, glyphs, or
interaction records for different identities. Reusing such output would be
incorrect even when every visible input in `ParagraphInput` matched.

Add an opt-in method conceptually equivalent to:

```rust
fn shared_preparation_epoch(&self) -> Option<u64> {
    None
}
```

`None` disables cross-identity reuse. `Some(epoch)` promises that prepared
facts are a pure function of the non-identity paragraph input, constraints,
and that epoch. The paragraph identity may still be used for diagnostics,
public output envelopes, and region-transcript rebinding.

`ParleyParagraphEngine` opts in because its `FontSet` is an immutable snapshot
owned by the engine. It initially returns epoch zero. A future runtime font or
data mutation API must advance the epoch before changed output is observable.
`LayoutEngine` clears its shared entries whenever the reported epoch changes.

This additive method is both a capability declaration and a safety fence. A
backend that does not understand the contract remains correct by doing nothing.

## Shared key

The exact key contains:

- projected UTF-8 text;
- analysis style table and source-ordered analysis runs;
- shaping style table and source-ordered shaping runs;
- inline-flow style table and source-ordered inline-flow runs;
- paragraph base direction and whitespace policy;
- exact `TextConstraint`;
- exact region flow and starting cursor, when present;
- empty-paragraph line-height facts;
- paragraph-local paint-slot runs while prepared glyph coverage carries them.
- the opted-in backend preparation epoch.

It excludes:

- document and paragraph identity;
- document revision and paragraph version;
- projection source relations, leaf identity, and semantic identity;
- composition identity and epoch;
- paragraph alignment, because accepted-slot adjustment is later;
- `PaintTable` brushes;
- final block placement.

Font-catalog identity is scoped twice: shared facts never leave the
`LayoutEngine` that owns one backend instance, and the backend epoch changes
before any hidden font or data resource can change prepared output.

The first index uses a deterministic projected-text fingerprint to select a
small collision bucket, followed by complete key equality. This avoids a new
hash-map dependency and keeps collisions correctness-neutral. LRU bookkeeping
is separate from key equality.

## Shared value

`PreparedParagraph` becomes a small paragraph-identity envelope over
reference-counted immutable prepared facts. Its public behavior remains the
same. The cache stores only the facts, never the envelope. A hit constructs a
new envelope and passes it through the ordinary current-projection validation
before geometry lowering.

The facts contain:

- resolved paragraph direction;
- paragraph-local prepared lines, interaction units, runs, glyphs, and cursor
  movement;
- immutable font-resource handles already selected by the backend.

Prepared glyphs currently carry paragraph-local `PaintSlot` coverage. This is
not a brush, paint table, renderer resource, or semantic owner; it is a
validated partition of a glyph's projected source. It may therefore be shared
only when the exact projected paint-slot runs match. Changing `PaintTable`
brush values remains a hit. Changing the source-to-slot partition is a miss.
If glyph paint partitioning later moves out of prepared facts, the slot runs
move out of this key at the same time.

Region transcript facts store cursor transitions, paragraph-local ranges,
slots, measured heights, and outcomes without a paragraph identity. The public
`RegionTranscript` is reconstructed for the consuming paragraph.

On a shared hit, normal geometry lowering consumes the current `Projection`.
That is where current `TextId`, `SemanticId`, composition identity, paint,
fragment identity, and scene placement are created.

## Lifetime and budget

Keep the existing retained-geometry entry budget. Add an explicit shared
preparation byte budget to `CacheBudget`; zero disables shared retention.

The proposed caller shape is:

```rust
let budget = CacheBudget::new(2_048)
    .with_shared_preparation_bytes(8 * 1024 * 1024);
let layout = LayoutEngine::new(paragraphs, budget);
```

The byte weight is a deterministic upper accounting charge for storage owned
by the shared key, entry metadata, and every nested prepared-fact container.
It includes a nonzero fixed charge plus string and vector capacities, uses
saturating arithmetic, and does not charge shared font blobs or other external
`Arc` backing again per entry. The diagnostic is an engine retention
accounting value, not a claim about allocator bookkeeping or an
allocator-exact heap size.

Insertion evicts least-recently-used shared entries until the new entry fits.
An oversized entry is used once without entering the cache. Reads update
recency. `clear_cache` drops all shared entries. `release_document` does not:
identity-free reuse must survive destruction of the first element that
produced it.

This preserves the existing `CacheBudget::new` source shape and memory
semantics for callers that do not opt in, while making the extra retained
memory impossible to enable accidentally.

## Work and diagnostics

`WorkReport` distinguishes:

- stable identity-bound geometry reuse;
- shared prepared-fact reuse;
- fresh backend preparation;
- per-consumer adjustment and geometry work.

`CacheDiagnostics` adds shared preparation budget, resident entries, resident
estimated bytes, peak estimated bytes, hits, misses, evictions, and oversized
non-retentions. Existing geometry diagnostics retain their meanings.

Backend-entry counts are allowed to be lower than retained geometry counts:
consumers served by shared portable facts never create a backend identity
entry. This is a documented diagnostic change, and old equality assertions
become separate upper-bound assertions.

The expected first-hit work law for 512 identities with identical inputs is:

```text
analysis = 1
shape = 1
formation = 1
shared preparation hits = 511
geometry = 512
```

A stable second preparation of each identity should still take the existing
identity-bound geometry hit and report no stage work.

## Correctness and performance proof

Blocking tests cover:

1. Distinct documents with identical text share prepared facts but retain
   distinct document, paragraph, text, semantic, fragment, and revision
   identity.
2. Different leaf segmentation and semantic roles can share prepared facts while
   producing different source and semantic geometry.
3. Distinct compositions rebind composition ID and epoch.
4. Text, language, base direction, word-break policy, shaping style, spacing,
   line height, wrapping policy, width, regions, and paint-slot coverage
   invalidate the exact key.
5. Alignment and `PaintTable` brush changes do not invalidate prepared facts.
6. Separate engines and font snapshots cannot share entries.
7. Backends are ineligible by default; an opted-in identity-dependent test
   backend is caught by differential traps; changing the backend epoch clears
   shared reuse before changed output is observed.
8. Shared entries obey the byte budget, LRU eviction, oversized-entry,
   zero-budget, saturated-weight, explicit-clear, and document-release laws.
9. Identical versus distinct text, stable retention, width churn,
   creation/destruction churn, and paint-only changes retain exact work
   assertions in the label-scale wind tunnel.
10. Allocation calls, allocated bytes, elapsed time, and resident estimated
   bytes are reported before and after on the same workloads.
11. Fingerprint collisions run complete key equality and can produce a miss or
    hit only according to that equality.

## Migration

No existing call site changes for the proposed budget spelling. Callers that
want cross-identity reuse add `with_shared_preparation_bytes`. New diagnostic
accessors and the default-disabled `ParagraphFormation` eligibility method are
additive.

The internal `PreparedParagraph` storage changes, but its constructors and
observational public API remain source-compatible. Backend implementations do
not receive cache policy and do not need to know that exact prepared facts may
be shared after validation.

## Human gate

Approval of this note freezes:

- `LayoutEngine` as the owner of exact portable prepared-fact reuse;
- explicit backend opt-in and epoch invalidation;
- the included and excluded key inputs;
- fresh per-consumer geometry and identity rebinding;
- a separately opt-in, byte-budgeted LRU lifetime;
- the public budget and diagnostic direction.

It does not approve a new dependency, `unsafe`, sharing final scenes, or a
backend-private cache. Those remain out of scope.

## Implementation

Implemented on 2026-07-25. `LayoutEngine` owns an exact-key,
backend-epoch-scoped shared cache in `scene/shared_cache.rs`.
`PreparedParagraph` is now a fresh paragraph envelope over immutable
reference-counted facts, and cached region attempts are rebound into a fresh
public transcript. The public budget, diagnostics, and work-report additions
follow this design without changing the existing default: a zero shared byte
budget disables the cache.

The executable correctness matrix and measured before/after result are
recorded in
`docs/proof/cross-identity-preparation-cache-2026-07-25.md`.
