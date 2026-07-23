# Selection-set transaction adversarial review — 2026-07-22

## Summary judgment

The selection slice is executable and preserves the distinction required by
visual bidi editing: a `SnapshotTextSelectionSet` contains independent
insertion points, while one `SnapshotTextSelection` can contain several
logically disjoint ranges. Scene movement and geometry operate on the complete
set. Document replacement validates the complete revision-bound set before
staging, deletes every range in one selection, inserts once for that selection,
repeats once per independent selection, and publishes one revision.

The real Parley-backed proof crosses an Arabic/Latin visual boundary, retains a
logical gap, produces separately owned geometry, moves two independent carets,
and inserts twice for two selections rather than once per logical range. A
multi-paragraph transaction reshapes exactly the two affected paragraphs and
reuses the untouched sibling. The slice adds no dependency and no `unsafe`.

Good catch: the first revision keyed selection sets only by revision number.
Two distinct documents can have the same revision, so the set now carries and
validates `DocumentId` as well as `DocumentRevision`.

## Must fix

All Must findings are resolved.

- **Visual selection cannot be flattened to a logical union.** Visual movement
  follows adapter-owned cursor transitions and collects the source cluster
  crossed by each step. Canonicalization merges only adjacent or overlapping
  leaf-local ranges and preserves true bidi gaps.
- **Several ranges cannot imply several insertions.** Replacement plans are
  keyed by selection. All ranges are deleted, but only the first logical range
  receives the replacement, so one selection remains one insertion point.
- **Independent edits cannot depend on caller order.** The complete set is
  validated for duplicate insertion points and overlapping ranges, then source
  operations are applied in reverse document order and committed once.
- **Dense positions cannot cross snapshots.** Scene movement and geometry
  reject foreign documents and revisions. Document replacement independently
  rejects foreign, stale, malformed, cross-leaf, overlapping, and non-UTF-8
  input before creating a staged edit.
- **Adapter cursor facts cannot smuggle invalid positions into a scene.** The
  prepared boundary requires complete, unique movement records, valid targets,
  crossed-source ranges that equal an actual prepared cluster, finite in-line
  carets, and known lines. Scene projection additionally rejects cursor offsets
  inside a UTF-8 scalar.
- **Multi-selection publication must retain honest work.** A three-paragraph
  Parley proof inserts through carets in the first and last paragraphs, reports
  those paragraphs in document order, reshapes exactly two, and reuses the
  unchanged middle formation and geometry.

## Should

- Keep bidi, affinity, soft-wrap, and cursor-side mechanics inside
  `underwood_parley`. `underwood` should continue to consume portable movement
  and crossed-source facts without reconstructing Unicode behavior.
- Preserve the same-leaf-per-selection fence until structural replacement is
  designed. The current transaction supports several selections and several
  paragraphs, but explicitly rejects one selection whose ranges span semantic
  leaves.
- Feed composition from the primary selection without discarding the complete
  selection-set model. Starting IME with several selections needs an explicit,
  host-visible collapse policy rather than an implicit flattening.
- Continue returning post-edit selections from the transaction. Callers should
  not manufacture byte offsets for the new revision.

## Could

- Upstream the reusable cursor-transition derivation to Parley Core once its
  ownership and API are agreed, then delete the corresponding adapter logic.
- Add indexed position and range lookup only after an interaction benchmark
  demonstrates that the current linear tables are material.
- Extend one selection across semantic leaves together with the later
  structural transaction design; do not weaken current validation as a
  shortcut.

## Real-vs-mirage boundary

**Real:** the immutable scene and document transaction APIs execute against
actual Parley-derived clusters, bidi levels, affinities, cursor transitions,
and retained paragraph caches. The tests assert exact logical ranges and exact
stage work, not merely the presence of plausible records.

**Mirage if overclaimed:** this is not yet product-proven editing. The native
showcase does not consume selection geometry or transactions on this branch,
the mixed-bidi corpus is still narrow, and cursor-transition derivation in
`underwood_parley` is one upstream review deep. The ledger therefore calls the
slice Executable, not Measured, Conformant, or Product-proven. Those promotions
belong to the interaction benchmark, broader differential corpus, Parley
upstreaming, and editable-showcase slices.

Logical cluster movement is also a primitive, not a claim about every
platform's word or deletion policy. It follows source-complete Parley shaping
records and keeps OpenType ligature components such as those in `ffi`
independently reachable. Full Unicode extended-grapheme movement and deletion,
especially when one grapheme crosses authored semantic leaves, requires the
separately tracked multi-source interaction representation.

The most dangerous remaining gap is host integration: a polished caret painted
by example-only state would be theater. The next product slice must render the
public `SceneSelectionRect` output and route pointer and keyboard actions
through `TextScene` and `Document::replace_selections` without a parallel
example-owned selection model.

## Suggested tests

The focused suite now includes:

- forward and reverse mixed-bidi visual selection with identical disjoint
  logical ranges;
- selection geometry tagged with both independent-selection and logical-range
  ownership;
- two independent carets moving together without merging;
- duplicate insertion-point, wrong-document, stale-revision, cross-leaf, and
  interior-UTF-8 rejection;
- one insertion for a multi-range visual selection and one for a second
  selection;
- logical forward-delete and backspace over multibyte and combining-mark
  clusters;
- visual and logical movement across semantic paragraph boundaries;
- a two-of-three-paragraph transaction with exact retained-work evidence; and
- malformed adapter cursor targets, caret lines, and UTF-8 offsets failing at
  the preparation boundary.
