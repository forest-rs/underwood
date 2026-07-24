# Design-0011: Source-complete grapheme interaction

- **Status:** Accepted
- **Date:** 2026-07-23
- **Approved:** Bruce Mitchener, 2026-07-23
- **Beads:** `und-oh0.10.2.6`
- **Extends:** Design-0009

## Fence

Parley owns Unicode extended-grapheme boundaries, shaping-record membership,
bidi visual order, and paragraph-local caret transitions. Underwood owns
projection of one paragraph interaction unit into revision-bound semantic
sources, selection geometry, and atomic document transactions. The document
layer explicitly does not segment Unicode, and the paragraph adapter explicitly
does not invent or collapse semantic leaf ownership.

The showcase continues to own key and pointer gesture policy. Backspace asks
for previous-logical movement; it does not decide whether a base and combining
mark form one deletion unit.

## Symptom and executable trap

The current adapter publishes one interaction cluster per Parley shaping
record. That keeps ligature components and combining-mark records
source-complete, but a shaping record is not necessarily a Unicode extended
grapheme cluster.

A focused trap constructed one paragraph from two semantic leaves:

```text
TEXT("e") + EMPHASIS(U+0301 COMBINING ACUTE ACCENT)
```

Preparation succeeded and glyph provenance retained both `TextId` values.
Starting at the final caret, extending one `PreviousLogical` step, and replacing
the resulting selection with `""` left the base leaf as `"e"` and emptied only
the mark leaf. The desired assertion that both leaves became empty failed.

This proves three separate facts:

1. source-complete paint preparation is already real;
2. the cursor map still treats one grapheme as two deletion units;
3. `Document::replace_selections` cannot yet publish the required multi-leaf
   selection even after cursor movement is corrected.

The trap was removed after reproduction so `main` remains green. It becomes the
first permanent regression test once this gate is approved.

## Options

### A. Keep shaping records as deletion units

This preserves the current API and deletes the combining mark separately.
Reject: it is observably wrong for Unicode editing and contradicts the Bead's
acceptance criteria.

### B. Segment graphemes in Underwood

Underwood could add a Unicode segmenter or infer units from scalar categories.
Reject: this duplicates Parley's analysis, risks Unicode-version disagreement,
adds dependency or table pressure to the foundational crate, and makes the
document layer a second cursor engine.

### C. Lower Parley analysis boundaries through the paragraph seam

The Parley adapter reads the already-computed `CharInfo::is_grapheme_start`
flags once, groups shaped records into paragraph-local interaction units, and
preserves each record as a visual/semantic slice of that unit. Underwood
projects the contiguous paragraph range into all leaf-local sources.

Choose C. It keeps Unicode physics in Parley, semantic projection in Underwood,
and platform gesture policy above both.

## Plumbing

```text
Parley Analysis::is_grapheme_start + ShapedText records
                         |
                         v
underwood_parley
  PreparedInteractionUnit
    paragraph source range
    visual slices + advances
    bidi level + visual sides
    logical/visual cursor transitions
                         |
                         v
Underwood Projection
  one paragraph range -> ordered leaf-local source ranges
  visual slice         -> exact SemanticId for hit routing
  unit endpoints       -> distinct SnapshotTextPosition owners
                         |
             +-----------+------------+
             |                        |
             v                        v
       hit/caret/selection       document replacement
       source-complete unit      reverse leaf-local edits
                                 one insertion, one publication
```

Paint remains shaped-glyph-owned and does not move through this new type.

## Chosen representation

### Paragraph adapter

Replace the ambiguous interaction meaning of `PreparedCluster` with two
explicit records:

```rust
pub struct PreparedInteractionSlice {
    source: Range<u32>,
    advance: f64,
}

pub struct PreparedInteractionUnit {
    source: Range<u32>,
    slices: Vec<PreparedInteractionSlice>,
    advance: f64,
    bidi_level: u8,
    boundary: ClusterBoundary,
    whitespace: ClusterWhitespace,
    left: PreparedClusterSide,
    right: PreparedClusterSide,
}
```

The names are schematic; approval covers the ownership and data shape rather
than freezing these exact spellings.

- `source` is one contiguous paragraph-local extended grapheme.
- `slices` preserve the visual contribution and paragraph source of every
  Parley shaping record in visual order.
- the canonical source union of `slices` exactly covers `source`, even when
  visual order is reversed or a slice has zero advance.
- `advance` is the checked sum of slice advances.
- `left` and `right` are the only selectable visual sides of the unit.
- `PreparedCursorStep::source` crosses the complete unit range, never one
  shaping slice.
- one-character graphemes used as OpenType ligature components remain separate
  interaction units and therefore remain independently reachable.
- regular line formation may not split a unit. A backend result that scatters
  one unit across nonadjacent visual positions or lines is invalid output.
- `boundary` is the legal break fact at the unit's logical start.
  `whitespace` describes the complete unit; CR, LF, and CRLF units that end a
  mandatory line are `Newline` rather than inheriting an arbitrary slice's
  classification.

`underwood_parley` derives unit ranges from the existing `Analysis`; it does not
call another segmenter. Break reshaping may replace shaping records, but it
must not replace the immutable analysis-derived unit boundaries.

The unit's `left` and `right` values are source endpoints selected from its
line-local visual rectangle; they are not line-placement coordinates.
`PreparedCursorMovement` remains the sole authority for mapping an endpoint
and affinity to a line and inline coordinate. In particular, a CRLF unit is
one deletion source while its before-break and after-break endpoint carets may
belong to different lines. The adapter must not encode hard-break placement by
collapsing the unit's two logical endpoints into one source position.

### Scene projection

Add a committed source-complete unit analogous to the current projected source
container:

```rust
pub struct SnapshotTextUnit {
    sources: Arc<[SnapshotTextRange]>,
}
```

Each source retains its `TextId`, revision, and leaf-local UTF-8 bytes. Unit
endpoints remain ordinary `SnapshotTextPosition` values, so a unit starting in
one leaf and ending in another preserves both endpoint owners without creating
a synthetic cross-leaf position.

The transient path already has the more general `ProjectedTextRange`, whose
ordered `ProjectedTextSource` values can represent an authored base plus a
generated composition mark. Keep that name: the type also carries
source-complete line, glyph, and fragment provenance, so renaming it to an
interaction unit would make those callers less honest.

`TextHit` returns the source-complete unit, the selected endpoint, the unit bidi
level, and the exact visual slice's `SemanticId`. Exact semantic activation
therefore continues to route through the source actually under the pointer; a
broad multi-semantic unit does not become a broad action rectangle.
Zero-advance slices have no independent pointer interior, but their source and
leaf identity remain present in the returned unit; they are never discarded or
assigned a fabricated rectangle.

Selection and caret geometry are computed once per interaction unit, not once
per shaping slice. This prevents a base and zero-advance mark from producing
duplicate highlights or an internal caret.

### Document transaction

`Document::replace_selections` accepts ordered ranges from more than one text
leaf when every range belongs to the same paragraph. This is a range
transaction, not a structural edit:

- each selection's ranges must already be in canonical document order by leaf
  index and byte range, with no duplicate or overlapping source;
- semantic leaves retain their identities, roles, and order;
- deleted source makes a leaf empty rather than removing or merging it;
- the replacement is inserted once at the first logical range;
- all leaf-local operations are validated before staging and applied in reverse
  document order;
- overlapping independent selections still fail atomically;
- the resulting caret belongs to the insertion leaf in the new revision;
- cross-paragraph replacement remains rejected because paragraph structure and
  block joining are outside this campaign.

All operation coordinates refer to the original revision. Validation computes
conflicts and checked resulting lengths per leaf before creating an edit.
Resulting carets are then rebased per leaf across every earlier original-source
operation, with input selection order preserved. Two empty insertions at one
boundary, an insertion on a deleted boundary, or any shared nonempty source
conflict exactly as they do for today's same-leaf transaction; distributing one
selection across leaves does not weaken atomic conflict detection.

This rule handles the split-grapheme case without smuggling semantic structure
into the adapter. It also makes already-representable cross-leaf scene
selections honestly editable.

## Invariants

1. Every nonempty paragraph byte belongs to exactly one analysis-derived
   interaction unit.
2. Unit ranges are ordered, contiguous, valid UTF-8 ranges, and never overlap.
3. Every shaping slice is fully contained in exactly one unit; all slices of a
   unit are retained, including unrendered controls and zero-advance marks.
4. The scene's ordered unit sources exactly cover the adapter's paragraph
   range, including all authored or generated provenance segments.
5. A unit exposes only its two endpoint carets. No caret is synthesized at an
   internal shaping-record or semantic-leaf boundary.
6. Hit testing retains the exact visual slice's semantic identity while
   returning the complete unit source.
7. Logical movement and deletion cross one complete extended grapheme.
8. Visual movement uses the same unit endpoints and remains bidi- and
   soft-wrap-correct.
9. Selection direction cannot change the canonical logical source ranges.
10. Cross-leaf replacement preserves leaf structure, inserts once, publishes
    once, and invalidates only the owning paragraph.
11. Composition units may mix snapshot and generated sources; preedit still
    publishes no document revision and commit publishes exactly once.
12. Hard-break units retain both source endpoints even when the endpoint carets
    are placed on different lines.
13. Multi-leaf plans are canonical original-revision coordinates; conflict,
    size, and resulting-caret rebasing are validated across the whole selection
    set before staging.
14. Production crates remain `no_std + alloc`, add no dependency, and add no
    `unsafe`.

## Public migration

Approval of this design authorizes a foundational public API change with the
following migration:

- paragraph adapters replace `PreparedCluster` values and
  `PreparedLine::clusters` with source-complete interaction units and their
  visual slices;
- `PreparedCursorStep::source` changes from one shaping-record range to the
  complete extended-grapheme range crossed by the step;
- committed `TextHit::source` changes from one `SnapshotTextRange` to a
  source-complete unit, with a `sources()` accessor for leaf-local ranges;
- transient `TextHit` continues to use `ProjectedTextRange`, but its hit-source
  value covers the complete extended grapheme rather than one shaping record;
- callers that need application semantics continue to use
  `TextHit::semantic_id`, which remains the exact hit slice's identity;
- `Document::replace_selections` accepts same-paragraph, multi-leaf range
  selections and rejects cross-paragraph structural replacement;
- the `CrossLeafSelection` error is replaced or narrowed to a
  cross-paragraph/structural error whose name describes the remaining
  prohibition.

No compatibility shim should preserve shaping-record deletion semantics under
a cluster-shaped name.

## Minimal implementation sequence

1. Restore the failing split-leaf backspace trap and add same-leaf decomposed,
   precomposed, CRLF, emoji ZWJ, regional-indicator, and spacing-mark cases.
2. Extract analysis-derived unit ranges in `underwood_parley`; attach every
   logical shaped record to exactly one range.
3. Lower visual slices and cursor transitions from units while retaining
   ligature-component reachability.
4. Project units and exact semantic slices through committed and composition
   scenes; migrate hit and selection geometry.
5. Generalize same-paragraph document range transactions without changing
   semantic leaf structure.
6. Exercise Backspace/Delete, pointer selection, mixed bidi, generated IME
   source, and paragraph-local invalidation through the native showcase path.
7. Run adversarial API/realness review, native visual inspection, all local
   gates, and the full protected remote matrix before landing.

## Execution graph

The versioned Beads graph makes the human gate and convergence points
executable:

```text
und-oh0.10.2.6.1  approve Design-0011
          |
          v
und-oh0.10.2.6.2  lock the trap corpus
          |
          +----------------------+
          v                      v
und-oh0.10.2.6.3        und-oh0.10.2.6.5
lower adapter units     multi-leaf transactions
          |
          v
und-oh0.10.2.6.4
project through scenes
          |                      |
          +-----------+----------+
                      v
             und-oh0.10.2.6.6
             native editor proof
                      |
                      v
             und-oh0.10.2.6.7
             review, prove, land
```

Only `.1` is ready before approval. Closing it makes the failing-first corpus
ready; adapter/scene migration and document transactions then remain separately
reviewable before converging at the native proof. `.7` owns the final
requirement-by-requirement audit and may not close the parent on partial
evidence.

## Proof matrix

| Case | Required observation |
|---|---|
| precomposed `é` | one unit, one move, one deletion |
| decomposed `e + U+0301` in one leaf | one unit despite several shaping records |
| decomposed sequence across two leaves | both sources hit/select/delete; both endpoint owners retained |
| OpenType `ffi` | three character units remain independently reachable |
| CRLF | one grapheme deletion; endpoint carets remain before and after the mandatory break |
| emoji ZWJ and regional indicator pair | one move/delete per extended grapheme |
| Arabic base plus marks | marks remain painted and delete with their grapheme |
| mixed bidi | reciprocal drags and visual movement retain identical source |
| composition-generated mark | projected unit contains snapshot and generated sources; cancel reuses committed work |
| committed IME text | one publication and only the owning paragraph reshapes |
| multiple carets | complete units delete atomically in one transaction |
| shared-leaf multicaret | original-coordinate conflict checks and resulting carets remain deterministic |
| stale/foreign/overlap/cross-paragraph | whole transaction fails before staging |

## Approval

Bruce Mitchener explicitly approved Design-0011 on 2026-07-23. That approval
authorizes the foundational paragraph-adapter and hit-source API migration and
broadens document replacement from same-leaf to same-paragraph ranges:

1. analysis-derived interaction units with visual slices;
2. source-complete committed and projected hit units;
3. non-structural same-paragraph multi-leaf replacement semantics.

It does not authorize a second Unicode segmenter, cross-paragraph structural
editing, semantic-leaf merging, new production dependencies, or `unsafe`.

## Implementation record

The accepted representation and transaction were implemented in `56ce91e` and
`7349677`. The native release-showcase proof is `4fcd18f`, and the unbundled
emoji/complex-script interaction corpus is `4c527d3`. The requirement-by-
requirement review, honest font-proof boundary, public migration handoff, and
gate record live in
`docs/proof/source-complete-grapheme-interaction-review-2026-07-24.md`.
The complete branch landed through protected PR and merge-group matrices as
PR #20, squash commit `23b94c6`.
