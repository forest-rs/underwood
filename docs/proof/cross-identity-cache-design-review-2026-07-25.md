# Cross-identity cache design review — 2026-07-25

## Summary judgment

Design-0016 chooses the correct layer—portable prepared facts above a paragraph
backend and below identity-bound geometry—but its first draft was not safe to
implement against the existing generic `ParagraphFormation` contract. Accept
the revised design only with explicit backend eligibility, epoch invalidation,
identity-free stored values, exact current-projection revalidation, and a
defined byte-accounting contract.

Good catch: the generic paragraph trait receives `ParagraphId`, so
identity-independence cannot be inferred merely because the current Parley
adapter happens to produce paragraph-local facts.

## Must fix

### Backend identity dependence must be explicit

The original proposal would have bypassed `ParagraphFormation::form` for a new
identity without any trait promise that the backend returns the same facts.
That is an observable correctness change for a legal backend.

Resolution in the revised design:

- reuse is disabled by default;
- a backend explicitly opts in by returning a preparation epoch;
- opt-in promises that non-envelope output depends only on non-identity inputs,
  constraints, and the epoch;
- an epoch change invalidates the shared cache.

Required tests:

- a default backend never receives shared hits;
- an opted-in backend receives them;
- an epoch-changing backend misses after the change;
- a deliberately identity-varying fixture proves why opt-in is required.

### Stored values must be structurally identity-free

`PreparedParagraph` currently contains `ParagraphId`, and every
`RegionAttempt` contains it as well. Caching either public object directly
would violate the fence even if consumers later overwrote the IDs.

Resolution in the revised design:

- cache only reference-counted prepared facts below a fresh paragraph envelope;
- cache paragraph-local region-attempt facts below a fresh public transcript;
- validate the rebound envelope against the current projection before geometry
  lowering;
- generate fragment, semantic, source, composition, and revision identity only
  through ordinary current-consumer geometry.

Required tests:

- two documents with identical projected text retain distinct document,
  paragraph, text, semantic, fragment, and revision identity;
- distinct composition IDs and epochs survive a shared hit;
- different leaf segmentation and semantic roles share preparation but not source
  or semantic geometry;
- a poisoned cached fact cannot bypass current-projection validation.

### The memory budget needs an enforceable accounting contract

Calling an informal estimate a byte budget would overclaim bounded memory.
Nested vectors, key storage, and entry metadata must participate, while shared
font blobs must not be charged as though copied per entry.

Resolution in the revised design:

- every entry has a nonzero fixed charge;
- owned string and nested vector capacities are included;
- arithmetic saturates;
- external shared backing is excluded and named;
- an entry exceeding the entire budget is never retained.

Required tests:

- zero budget retains nothing;
- an oversized entry is served but not retained;
- insertion evicts to the byte budget;
- repeated tiny entries cannot bypass the budget;
- weight overflow saturates and cannot wrap into eligibility.

### Paint semantics must be named precisely

Prepared glyphs contain `PaintSlot` coverage today. Claiming that shared facts
contain no paint at all would be false, while excluding every paint-related
fact would prevent correct prepared-glyph reuse.

Resolution in the revised design:

- actual brushes and `PaintTable` values are excluded;
- the exact projected source-to-`PaintSlot` partition remains in the key and
  shared prepared facts;
- brush-only changes hit;
- changing the slot partition misses.

Required tests:

- changing only a brush reuses prepared facts;
- changing a computed paint slot does not reuse incompatible glyph coverage;
- source-complete clipped coverage remains validated.

## Should fix

- Keep the identity-bound `FormationKey` separate from the new shared key.
  Weakening its version or source-map law would damage stable geometry reuse.
- Document that backend retained-entry counts can be lower than geometry entry
  counts after shared hits; existing equality checks should become explicit
  bounds.
- Use a deterministic text fingerprint only as an index. Complete key equality
  must remain authoritative so collisions are correctness-neutral.
- Put shared-cache mechanics in a focused scene module rather than growing
  `scene/engine.rs`.

## Could improve

- A later backend-owned staged cache could preserve analysis and shaping across
  related but nonidentical width keys. It should be justified by post-slice
  profiles rather than folded into the exact first cache.
- The preparation trace can name the precise key partition that missed once
  deterministic invalidation reasons land in `und-oh0.13.11`.

## Suggested tests

In addition to the Must-fix traps:

- alignment-only and region-identical consumers share prepared facts while
  producing their own adjusted geometry;
- width and region changes miss exact formation but do not corrupt existing
  retained geometry;
- `release_document` keeps shared facts and removes identity-bound state;
- `clear_cache` clears both layers;
- LRU recency changes on a hit;
- a stable second preparation continues to use the faster identity-bound
  geometry cache rather than the shared cache;
- the 512-label wind tunnel reports one analysis, one shape, one formation,
  511 shared hits, and 512 current-consumer geometry projections.

No `unsafe` exists or is proposed in this design.
