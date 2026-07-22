# ADR-0004: Parley boundary and contingency

- **Status:** Accepted
- **Accepted:** 2026-07-21 by Bruce Mitchener
- **Bead:** `und-oh0.2.1`
- **Authority:** `UNDERWOOD_HANDOVER.md` §§15.1–15.6, 35.1

## Goal

Define the retained preparation contracts Underwood requires from Parley and
the narrow contingency that preserves product sequencing without creating a
second text engine.

## Non-goals

- Owning Unicode analysis, bidi, shaping, or font fallback in Underwood.
- Freezing Underwood's public façade to current Parley types.
- Treating upstream review latency as evidence against upstreaming.

## Fence

`underwood_parley` owns adaptation between Underwood prepared contracts and a
pinned Parley revision; it explicitly does not own semantic documents,
document flow, renderer policy, or a general-purpose fork of text physics.

## Constitutional invariants

1. Analysis, itemization, shaping, used values, and breaking have distinct
   retained identities.
2. Font weight never invalidates Unicode analysis.
3. Paint values never enter shaping identity.
4. Underwood prepared types shield the public façade from Parley churn.
5. Generally applicable changes are proposed upstream.
6. Every temporary divergence has an owner, conformance evidence, removal
   review, and upstreaming plan.
7. Underwood may build a breaker over retained `parley_core` primitives but
   cannot casually duplicate Unicode or shaping machinery.

## Evidence snapshot

This audit was performed on 2026-07-21 against:

- Parley `main` at
  [`45da4a90248b1600277a4294b70d8bfde5ca8e97`](https://github.com/linebender/parley/commit/45da4a90248b1600277a4294b70d8bfde5ca8e97);
- the open retained-shaped-text work in
  [PR #679](https://github.com/linebender/parley/pull/679);
- the broader experimental core seam in draft
  [PR #634](https://github.com/linebender/parley/pull/634).

The audited `parley_core` is version 0.11.0, has MSRV 1.88, is `no_std`, and
offers `std` and `libm` feature paths. These are observed upstream facts, not an
Underwood production pin.

Current `main` has made meaningful progress:

- `Analyzer` writes a reusable, owned `Analysis`;
- `Analysis` exposes per-character facts, bidi levels, and paragraph level;
- `Analysis::itemize` yields source ranges of constant bidi level and script
  while accepting a caller predicate for shaping-relevant topology;
- `Shaper::shape_item` accepts caller-controlled font selection and invokes a
  callback for each `ShapedRun`;
- high-level `parley::Layout` retains shaped arrays and exposes a resumable
  greedy `BreakLines` object with cloneable in-process `BreakerState`;
- high-level inline boxes support in-flow, out-of-flow, and caller-positioned
  custom out-of-flow cases.

The remaining gaps are equally concrete:

- `ShapedRun` is callback-borrowed and explicitly documented as transitional;
  retained `ShapedText` is still PR work;
- current core shaping is horizontal; writing mode and orientation are not
  current-main inputs;
- bounded `apply_break`/`apply_concat` reshaping is not on current `main`;
- inline boxes remain an out-of-band high-level Parley facility rather than a
  first-class core itemization contract;
- `BreakerState` is useful within one live `Layout`, but its iteration and line
  state are private, unversioned, and not a serializable document-flow
  checkpoint;
- data providers and their identities are not injectable through the public
  core pipeline.

### Implementation update — 2026-07-22

Parley `main` at
[`6c81e1dd9b67793cdd959c65cc650c96a1262fb7`](https://github.com/linebender/parley/commit/6c81e1dd9b67793cdd959c65cc650c96a1262fb7)
contains the owned reusable `parley_core::ShapedText` introduced by PR #679.
Underwood adopts that immutable revision under Design-0005 and removes its
callback-shaped `PhysicsRun`/`PhysicsGlyph` copy. The original audit above
remains the evidence that justified the contingency; its retained-output gap
is now resolved. Bounded break reshaping, vertical shaping, core inline
objects, and identified text-data provisioning remain open.

### Implementation update — bounded reshape candidate, 2026-07-22

Underwood temporarily pins
[`waywardmonkeys/parley@181664b`](https://github.com/waywardmonkeys/parley/commit/181664b28144cb59671a7f1b736757c6ebe270f2),
a single commit based on `6c81e1d` that retains per-run reshape inputs and adds
unsafe-region discovery plus bounded `Shaper::apply_break` / `apply_concat`.
The shaped result retains only each bounded pre-break fragment so concat stays
exact even when breaking removes HarfBuzz's concat-safety metadata.
The generally applicable code lives in the Parley fork branch, not in
`underwood_parley`; the adapter owns only legal greedy selection, backtracking,
portable lowering, and the exact committed-reshape count. `und-oh0.2.7` owns
upstream review and replacing the fork URL with `linebender/parley` as soon as
the capability lands there.

## Seam matrix

| Capability | Current upstream seam | Underwood position | Readiness |
| --- | --- | --- | --- |
| Whole-paragraph Unicode analysis | Owned `Analysis`, reusable `Analyzer` | Adapt directly; never reimplement | Usable, still evolving |
| Bidi resolution | Retained levels in `Analysis` | Consume as analysis identity | Usable |
| Itemization | `Analysis::itemize` plus `split_after` predicate | Supply shaping-topology predicate; retain Underwood key | Usable |
| Font selection | `Shaper::shape_item` callback over clusters | Bridge the Underwood font resolver | Usable |
| Shaping | Callback-borrowed `ShapedRun` | Prototype against the seam; do not stabilize storage around it | Transitional |
| Retained shaped text | PR #679 | Prefer upstream; require owned immutable output | Active upstream work |
| Bounded break reshaping | Candidate `181664b` extracted onto current `ShapedText` | Consume exact pin; upstream and retire fork URL | Executable, upstream review pending |
| Greedy line breaking | High-level `BreakLines` and `BreakerState` | Evidence source and possible temporary adapter, not document flow | Usable but wrong ownership |
| Arbitrary line intervals | Mutable line geometry and custom out-of-flow yield | Drive an Underwood breaker over core-shaped data | Partial |
| Vertical shaping | Draft PR #634 only | Upstream first; no horizontal-only public Underwood contract | Gap |
| Inline objects | High-level out-of-band `InlineBox` | Require core marker semantics plus high-level adapter equivalence | Partial |
| Paint-only boundaries | Per-character `u16` user/style data reaches shaping output | Retain slots without changing font/shaping inputs | Needs proof |
| Text-data injection | Compiled/baked constructors inside core | Coordinate with ADR-0003; upstream provider seam required | Gap |

## Required retained contracts

The names below describe Underwood-owned private stage contracts, not approved
public Rust types:

1. `analysis = analyze(projected_text, analysis_options, text_data_id)`.
   Font, size, weight, tracking, paint, and renderer identity are absent.
2. `items = itemize(analysis, shaping_topology)`. The topology changes only for
   script/language/orientation/font-selection and other documented shaping
   inputs.
3. `shaped = shape(items, font_resolution, shaping_values)`. Paint values and
   flow geometry are absent. Paint-slot topology may be carried as uninterpreted
   metadata.
4. `broken = break(shaped, intervals, break_policy)`. A break may request a
   bounded reshaping operation, and removing that break must restore the
   concatenated result.
5. `scene = place_and_lower(broken, flow_state, paint_table)`. Document flow and
   rendering remain outside Parley Core.

Each output must name the identities of every input family on which it actually
depends. Underwood invalidation tests will deliberately perturb one family at a
time.

## Options

### Upstream-only stable releases

Simplest maintenance story, but may block the product indefinitely on missing
retained seams.

### Pinned upstream revision with narrow patch stack

Allows coordinated upstream work and bounded local sequencing. It requires
strict divergence ownership and dual conformance.

### Broad maintained fork

Offers maximum control but creates a second text-engine institution. Rejected
unless a future constitutional decision demonstrates that upstream alignment
is no longer viable.

## Conformance requirements

The future Parley wind tunnel must exercise the same corpus through the accepted
upstream path and any temporary patched path. A patch cannot call itself
conformant merely because its own snapshots are stable.

| Corpus | Required observation |
| --- | --- |
| Latin, Arabic, Devanagari, Thai, CJK, emoji, and mixed bidi | analysis, itemization, clusters, glyphs, and advances |
| Font weight or fallback change | analysis identity unchanged; affected item/shape identity changed |
| Paint value change | analysis, items, glyph ids, advances, and break opportunities unchanged |
| Paint boundary inside `fi` and Arabic cursive text | declared slot coverage with no accidental reshaping |
| Explicit tracking and OpenType feature changes | only documented topology and shaped ranges change |
| Hyphenation or discretionary break inside a ligature | bounded break reshape; concat restores the unbroken result |
| Multiple disjoint intervals and custom out-of-flow object | no duplicated, skipped, or reordered source coverage |
| U+FFFC among LTR, RTL, and neutral text | core marker and high-level adapter agree on bidi/source placement |
| Vertical mixed-script text | orientation, glyph rotation, advances, and source mapping are explicit |
| Repeated build on all supported targets | deterministic prepared digest for identical identities |

Failures report stage, source range, script, bidi level, font identity, data
identity, expected digest, actual digest, and whether upstream or patched code
ran.

## Upstream issue and pull-request map

This is a planning map, not a claim that upstream owes Underwood a schedule.

| Topic | Upstream work | Underwood consequence |
| --- | --- | --- |
| Retained shaped storage | [PR #679](https://github.com/linebender/parley/pull/679) | Track and test; do not invent a public substitute |
| Full core seam, vertical text, break reshaping, inline objects | [draft PR #634](https://github.com/linebender/parley/pull/634) | Split requirements into reviewable upstream slices |
| Shaper-driven itemization | [issue #462](https://github.com/linebender/parley/issues/462) | Ensure font fallback does not blur stage ownership |
| Style incrementality | [issue #432](https://github.com/linebender/parley/issues/432) | Align topology keys and paint separation |
| Custom line positioning | [issue #325](https://github.com/linebender/parley/issues/325) | Inform interval-flow experiments, not checkpoint ownership |
| Ligature behavior under letter spacing | [issue #515](https://github.com/linebender/parley/issues/515) | Include in tracking and break corpus |
| High-level floats and caller-controlled breaking | [PR #421](https://github.com/linebender/parley/pull/421) | Reuse evidence and adapter behavior |

Before implementation begins, this table must be refreshed against the exact
revision proposed for use.

## Proposed pin and patch policy

No production revision is selected by this open ADR.

If implementation is authorized before all required seams are released, the
recommended policy is:

1. Pin one immutable Parley commit, never a branch name.
2. Require green upstream CI, an Underwood conformance run, license review, and
   a recorded source digest before changing the pin.
3. Prefer an upstream commit containing the work. A local patch requires a
   linked upstream issue or PR when generally applicable.
4. Keep patches in `underwood_parley`; no patched Parley type may cross the
   stable façade.
5. Review each divergence every 30 days and at every Parley release.
6. Remove a patch as soon as an upstream revision passes the same conformance
   corpus.

A patch needs a separate human decision before it is carried when any of these
is true:

- it changes Unicode analysis, bidi, shaping, font fallback, or text-data
  semantics rather than exposing or retaining an existing result;
- it adds `unsafe`, a production dependency, or a public Underwood API;
- it exceeds 500 non-test changed lines or spans more than one retained stage;
- it has no named owner, upstream path, dual-run conformance, or removal test;
- it remains carried across two Parley releases or 90 days, whichever occurs
  first.

These thresholds force review; they do not make a smaller patch automatically
acceptable.

## Ownership and removal mechanics

Every divergence record must contain:

- one accountable Underwood owner and one backup reviewer;
- exact base commit, patch digest, affected stages, and public upstream link;
- the conformance cases that fail without it and pass with it;
- the upstream acceptance or replacement condition;
- the next 30-day review date;
- a removal bead created when the divergence is introduced.

The owner may refresh a mechanically rebased patch only when semantics and
evidence are unchanged. Any changed behavior returns to the human gate.

## Recommendation

Choose **pinned upstream revision with a narrow, expiring patch stack** as the
contingency policy, while treating upstream-first development as the normal
path. Underwood should prototype against current retained analysis and
itemization now only after the ADR is ratified, track PR #679 for owned shaped
storage, and pursue bounded reshaping, vertical text, first-class objects, and
data injection as focused upstream seams.

This recommendation gives product sequencing teeth without funding a second
Unicode and shaping institution.

## Open semantic questions

- Exact Underwood prepared input/output types.
- Which breaker responsibilities belong upstream versus document flow.
- Whether PR #679's final ownership and mutation model can support immutable
  prepared snapshots without an Underwood copy.
- Whether first-class inline objects use U+FFFC exclusively or preserve an
  out-of-band construction adapter.
- Who accepts ownership of the first carried divergence, if one becomes
  necessary.

## Decision

Accepted on 2026-07-21.

Underwood will align aggressively with Parley Core's retained stages and use an
immutable pinned upstream revision plus a narrow, expiring patch stack only
when the accepted thresholds and ownership rules are satisfied. Analysis, bidi,
itemization, font selection, and shaping remain Parley responsibilities;
Underwood owns prepared-contract adaptation, document flow, and conformance.

This decision does not select a production revision, add a dependency, approve
a particular patch, or authorize a public adapter API. Each of those remains
subject to its stated gate.

## Migration

Parley types must not leak into the stable Underwood façade. Adapter changes
still require migration notes for lower-level crate consumers.

## Proof impact

This decision gates `parley-alignment`, retained preparation, and the first
semantic-to-scene campaign.
