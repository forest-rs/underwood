# Design-0009: Interactive snapshot editing

- **Status:** Active; execution authorized 2026-07-22
- **Campaign:** interactive semantic document
- **Existing gate:** ADR-0001 position and canonical-storage contract
- **Existing obligation:** `und-oh0.2.3` cluster hit/caret geometry

## Goal

Turn the live retained-document proof into a genuinely interactive semantic
document. A pointer must resolve to a real shaped cluster and caret affinity;
dragging must produce bidi-correct selection geometry; keyboard input must
publish one validated document transaction; and IME preedit must use a
separate projection epoch without mutating committed text. That composition
model must support both a simple one-way event feed and richer native text
protocols that synchronously query and mutate host editor state.

The product proof is deliberately demanding: mixed English and Arabic,
combining marks, an OpenType ligature, soft and explicit line breaks, empty
editable text, resize reflow, selection, replacement, deletion, and IME
preedit/commit/cancel all execute through the same public preparation path.

## Fence

Parley owns paragraph-local cluster boundaries, bidi visual order, caret
affinity, and break-sensitive cursor mechanics; Underwood owns revision-bound
semantic positions, source projection, selection geometry, validated document
transactions, composition epochs, and retained work; the native showcase owns
pointer gestures, platform key/IME translation, focus, caret blinking, and
presentation policy. No layer may reconstruct another layer's private facts.

This campaign explicitly does not choose durable-anchor storage, add
collaboration, stabilize `DocumentSession`, implement undo/redo or clipboard
policy, add block flow, or move Winit and rendering dependencies into a
production crate.

## Position law

ADR-0001 distinguishes sparse durable anchors from dense, revision-bound
derived positions. This campaign uses the latter.

- A scene interaction position names one exact `DocumentRevision`, `TextId`,
  validated UTF-8 boundary, and upstream/downstream affinity.
- It is valid only against the named immutable snapshot. A stale position is
  rejected, never silently applied to a newer revision.
- A transaction consumes current snapshot positions and publishes replacement
  positions for its new revision. It does not claim that the old positions
  survived an unrelated edit.
- Persistent selections, collaboration presence, comments, and bookmarks still
  require the durable-anchor representation gated by `und-oh0.10.1.1`.

This is a final position *kind*, not a temporary byte-offset escape hatch.

## Integration

```text
parley_core ShapedText + formed line cluster ranges
                         |
                         v
underwood_parley portable visual clusters + caret affinities
                         |
                         v
Underwood projection -> snapshot positions -> TextScene interaction map
                         |                         |
                         |                         +-> caret / selection geometry
                         v
validated range transaction or composition projection epoch
             |                                      ^
             v                                      |
one-paragraph retained reprepare -> imaging overlay + native host
                                                    |
                    text/range/geometry/hit queries + edit callbacks
```

The adapter result carries portable visual cluster records for every formed
line, including whitespace and intentionally unrendered controls. Each record
contains its paragraph-local source range, bidi level, scene-independent
inline geometry, and the exact source boundary plus affinity reached from its
visual sides. Underwood maps paragraph-local boundaries through the projection
source map; it does not infer bidi direction from glyph order.

## Scene interaction contract

The current fragment-bounds hit test and query-point-derived caret are removed.
They cannot distinguish ligature components, whitespace, bidi boundaries, or
the two visual positions that can share one logical byte boundary.

The replacement contract has these invariants:

1. Exact hit testing returns no result outside selectable cluster geometry;
   closest hit testing clamps to a line boundary for pointer selection.
2. A hit returns a collapsed snapshot position and the source cluster it hit.
3. Caret lookup resolves the position and affinity against the current scene;
   it never uses the original pointer x-coordinate as caret geometry.
4. Visual left/right movement walks adapter-produced caret stops. Logical
   movement and deletion walk valid cluster boundaries, not UTF-8 bytes.
5. Selection geometry includes every visual cluster whose logical source lies
   between the endpoints, splits at bidi discontinuities and line boundaries,
   and merges only adjacent rectangles on the same line.
6. Every returned source position is a UTF-8 boundary in exactly one semantic
   text leaf. Leaf-boundary ownership follows affinity explicitly.
7. Empty editable text has a valid caret when its paragraph contains an empty
   text leaf; a structurally leafless paragraph remains non-editable.

## Transaction contract

The existing whole-leaf `replace_text` operation remains available. A new
validated range replacement consumes two current snapshot positions in one
text leaf and a replacement string.

- Wrong-document, wrong-revision, reversed, cross-leaf, and non-UTF-8 ranges
  fail before publication.
- Dropping the edit publishes nothing.
- Commit publishes one new immutable snapshot and reports the affected
  paragraph once.
- The edit result exposes the collapsed position immediately after inserted
  text in the new revision so a single-writer caller does not manufacture a
  position from a raw offset.
- Backspace, delete, and selection replacement obtain their range from the
  scene interaction map. The document layer never guesses grapheme or visual
  boundaries.

Cross-leaf and structural replacement remain later transaction operations.
The first live editor keeps its editable multilingual specimen in one semantic
leaf while hit testing and selection geometry remain document-wide.

## Composition contract

IME preedit is not a sequence of committed document edits.

- A composition names an ID, monotonically increasing epoch, current snapshot
  replacement range, preedit UTF-8, preedit selection, and optional clauses.
- Projection replaces the target range with generated composition text and
  records an explicit composition source segment. Generated bytes are not
  mislabeled as authored snapshot bytes.
- Only the affected paragraph receives composition analysis, shaping, flow,
  and geometry. Unaffected paragraphs retain their identities.
- Committed paragraph formation remains cached beside the transient
  composition formation. Changing or cancelling preedit does not evict it.
- Cancel reveals the unchanged committed scene with zero document
  publication. Commit publishes exactly one range-replacement transaction and
  removes the composition overlay.
- The showcase translates Winit IME events into this toolkit-independent
  state. Underwood does not interpret platform key conventions.

### Two protocol families, one editor model

The simple Winit model is a lossy adapter, not the shape of Underwood's IME
contract. `ui-events` is deliberately developing two complementary directions:

| Protocol family | Platform supplies | Platform asks the host for |
| --- | --- | --- |
| Event feed | preedit snapshots, preedit selection, commit/end | at most an externally updated cursor area |
| Host driven | replacement and marked-text callbacks, selection and edit commands | selection, marked range, surrounding/arbitrary text, offset conversion, range/caret rectangles, and point-to-offset hits |

Winit and the current Windows IMM32 adapter exercise the first family. AppKit's
`NSTextInputClient`, UIKit's `UITextInput`, Android's `InputConnection`, and
Windows TSF exercise the second family to different degrees. Underwood serves
both through one revisioned *editable surface*:

- The focus owner explicitly chooses which semantic text is exposed and how
  any leaves or paragraph separators are flattened. Platform offsets never
  silently mean global document offsets.
- The surface snapshot binds document text, selection, composition, source
  projection, and geometry to one document revision and one composition
  epoch. A synchronous callback cannot combine text from one snapshot with
  geometry from another.
- Underwood positions stay UTF-8 and semantic. The platform adapter converts
  UTF-16, code-point, or protocol-specific ranges at the boundary and rejects
  offsets that do not map to a valid surface boundary.
- A simple feed anchors its replacement range to the selection when a
  composition begins and retains that range across subsequent preedit
  snapshots. A host-driven callback may provide an explicit replacement range,
  but it is validated through the same surface mapping.
- Text queries, first-rectangle queries, caret rectangles, and point hits are
  read-only views of the same exact scene interaction map used by selection.
- Selection/text/layout changes produce explicit host-visible invalidation
  facts. The native adapter decides how to turn those into AppKit, UIKit,
  Android, TSF, Wayland, or candidate-window notifications and coordinate
  transforms.
- Platform locking, responder lifetimes, thread affinity, screen-coordinate
  conversion, and callback reentrancy remain adapter/host responsibilities.

Underwood does not take a production dependency on `ui-events` in this
campaign. A deterministic compatibility adapter in the external proof layer
will demonstrate that both the `TextInputEvent` feed and the experimental
`ui-text-input` reverse-query capabilities can be implemented from the same
Underwood state. This leaves those upstream APIs free to settle without
weakening the core boundary.

## Delivery slices

### A. Exact interaction map

Complete `und-oh0.2.3`: add portable prepared clusters, replace fragment hits
and point-derived carets, and prove ligature, combining, mixed-bidi,
soft/explicit-break, whitespace, empty-text, affinity, and round-trip cases.
This is the first independently landable PR.

### B. Snapshot transaction and selection

Add validated snapshot positions, range replacement with returned post-edit
position, visual/logical movement, and selection rectangles. The live proof
must click, drag, insert, replace, backspace, and delete while reporting one
affected paragraph and unchanged siblings.

### C. Composition epoch

Add explicit generated-source projection and a retained composition cache.
First pin the feed-versus-host-driven compatibility contract. Exercise Winit
preedit, selection movement within preedit, commit, cancel, and replacement
over a selected range, then run a synchronous host-query trace over that same
state for selection, text, range conversion, geometry, and hit testing.
Cancellation must demonstrate reuse of the committed paragraph formation.

### D. Product proof and review

Make the native showcase directly editable, retain a deterministic headless
interaction trace, and finish with exact semantic hover, pressed state, and
activation. Underwood returns the hit position and `SemanticId`; a
showcase-owned registry associates that identity with an action or URL, and the
host performs activation. This campaign does not stabilize a permanent link
schema or open URLs from core. Run correctness and real-versus-mirage review,
and land each coherent slice only after local and remote Definition of Done
gates pass.

## Migration

This repository is pre-stable, but public interaction changes still receive an
explicit migration record.

- `TextHit` changes from a whole-fragment source observation to an exact
  cluster hit with a collapsed revision-bound position.
- `TextScene::caret` becomes fallible when a hit or position does not belong to
  the scene's revision/source map.
- Callers that used `hit.point().x` as the caret x-coordinate migrate to the
  returned `SceneCaret` geometry.
- Whole-leaf replacement remains source-compatible; interactive callers move
  to validated snapshot-range replacement.
- No snapshot-local interaction type may be documented or serialized as a
  durable anchor.

## Proof gates

The campaign is complete only when:

- no fragment-bounds or query-point caret approximation remains;
- the public conformance corpus passes for Latin ligatures, combining text,
  Arabic RTL, mixed bidi, soft and explicit breaks, whitespace, and empty text;
- pointer hit/caret and caret-to-position round trips agree at every tested
  stop and both bidi affinities;
- selection-only changes perform zero document publication, analysis,
  shaping, line formation, and text geometry work;
- one committed edit and one IME commit reshape only the affected paragraph;
- IME preedit never mutates the committed snapshot, and cancel reuses the
  committed cached formation;
- one event-feed trace and one synchronous host-query trace drive the same
  composition state machine, with revision-consistent text, selection,
  geometry, hit testing, and UTF-8/UTF-16 conversion;
- semantic hover, press cancellation, and activation remain exact across
  wrapping and bidi boundaries, while action lookup and execution stay in the
  application and host;
- production crates remain `no_std + alloc`, gain no dependency, and contain
  no `unsafe`;
- public rustdoc, migration notes, focused tests, full workspace gates,
  adversarial review, and native visual inspection are green.
