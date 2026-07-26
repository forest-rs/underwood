# Underwood

`underwood` is the small, renderer-independent foundation for immutable
semantic documents, retained single-paragraph blocks, intrinsic and
constrained paragraph formation, and text scenes.

The crate is `no_std + alloc`. It owns no shaping engine, platform host policy,
graphics backend, renderer, system fonts, or global state. Geometry and paint
use Kurbo and Peniko values. The separate `underwood_parley` crate adapts the
pre-stable [`adapter`] contract to the repository's pinned Parley revision.

The first draft public slice is deliberately complete end to end:

- [`Document`] publishes immutable [`DocumentSnapshot`] revisions through
  atomic staged edits and preserves body and heading paragraph roles without
  prescribing their visual styling;
- [`TextBlock`] hides document-editing ceremony for retained one-paragraph
  text while preserving the same source model, paragraph engine, caches, and
  [`TextScene`] as documents;
- [`LayoutEngine`] retains formed paragraphs and avoids analysis or canonical
  paragraph shaping for unchanged siblings, paint-value changes, and
  constraint-only changes; wrapped constraint changes expose their separate
  line-final shaping work;
  an explicit [`CacheBudget`] independently bounds committed and transient
  composition geometry plus coordinated backend state, and can separately
  bound exact identity-free preparation shared by equivalent labels, while
  release operations and [`CacheDiagnostics`] expose lifecycle facts to hosts;
- [`adapter::ParagraphFormation`] keeps legal line breaking, visual ordering,
  and font-derived metrics behind the paragraph-engine boundary instead of
  hiding text preparation in scene construction; formed lines retain complete
  source slices across semantic leaves and distinguish real glyphs from
  intentionally unrendered controls;
- [`ComputedInlineStyle`] keeps [`ShapingStyle`], [`InlineFlowStyle`], and
  [`PaintSlot`] values in separate invalidation partitions while [`StyleMap`]
  assigns complete inline styles to semantic text leaves and
  [`ParagraphStyle`] values to paragraphs;
- [`ShapingStyle`] carries backend-neutral family, weight, width, style,
  language, feature, and variation requests; the separate adapter resolves
  them without moving font matching into this crate;
- [`TextScene`] exposes real glyph resources, paint ownership, optional
  explicit partial-paint clips, source mapping, exact shaped-cluster hits and
  carets (including whitespace, ligature
  components, bidi affinities, and empty editable leaves), revision-bound
  logical and visual selection sets, complete-scene endpoints, represented
  leaf-local positions, logical word movement from retained analysis, and
  semantic observations;
- document IDs are opaque and document-scoped, while [`SnapshotTextRange`] and
  [`SnapshotTextPosition`] values are dense observations valid only for their
  named revision.

The API is unpublished and pre-stable. Snapshot positions expose validated
UTF-8 boundaries but have no raw constructor and are not durable anchors. The
crate still introduces no caller-constructed byte-offset mutation API,
persistence format, renderer, or compatibility promise. See the external
`examples/headless` workspace crate for the normative preparation call path.

## Exact scene interaction

Paragraph adapters provide analysis-derived extended-grapheme interaction
units separately from painted glyphs. Each unit retains every shaping slice,
including zero-advance marks and controls, while exposing only its two endpoint
carets. Exact hits therefore cover ligature components and whitespace without
pretending that ink bounds are cursor geometry. A committed hit returns a
[`SnapshotTextUnit`] whose ordered source ranges can cross semantic leaves;
`semantic_id()` still identifies the exact visual slice under the pointer.
Closest hits also clamp to an empty editable leaf:

```rust,ignore
let interaction = scene.interaction()?;
let hit = interaction
    .hit_test(point)
    .or_else(|| interaction.hit_test_closest(point));
if let Some(hit) = hit {
    let caret = interaction
        .caret(hit.position())
        .expect("a hit from this scene has a matching caret stop");
    assert_eq!(caret.position(), hit.position());
}
```

`SnapshotTextPosition` includes the exact document revision, semantic text
leaf, UTF-8 byte boundary, and upstream/downstream affinity. Passing a position
from another revision or scene to [`SceneInteraction::caret`] returns `None` rather
than silently relocating it.

## Selection sets and replacement

One [`SnapshotTextSelection`] is one insertion point. It can retain several
logically ordered ranges when a visually contiguous bidi selection is
logically disjoint. A [`SnapshotTextSelectionSet`] holds several independent
insertion points for multi-caret interaction; these two levels are not
flattened together.

```rust,ignore
use underwood::{TextMovement, TextSelectionMode};

let editing = scene.editing()?;
let anchor = editing.hit_test_closest(drag_start).unwrap();
let extent = editing.hit_test_closest(drag_end).unwrap();
let visual = editing.selection_between(
    anchor.position(),
    extent.position(),
    TextSelectionMode::Visual,
)?;
let selections = editing.selection_set([visual])?;
let selections = editing.move_selections(
    &selections,
    TextMovement::NextVisual,
    true,
)?;
let replacement = document.replace_selections(&selections, "typed once")?;
let (publication, selections) = replacement.into_parts();
```

Selection geometry preserves both selection and logical-range indices.
Replacement validates the complete set, deletes every range in one selection,
inserts once for that insertion point, repeats once per independent selection,
and publishes one revision. Canonical ranges may span semantic leaves within
one paragraph without removing, merging, or restyling those leaves.
Cross-paragraph replacement remains a structural operation and is rejected.
Snapshot selections remain dense revision-local values, not durable anchors.

## Retained text blocks and intrinsic metrics

Application call sites for small labels do not need to construct a paragraph,
text leaf, style map, or edit transaction. `TextBlock` performs the
one-paragraph document construction once internally. Blocks borrow a reusable
computed style and paint table, but still execute the document paragraph path:

```rust,ignore
use underwood::{
    BlockRequest, CacheBudget, DocumentId, LayoutEngine, TextBlock,
    TextConstraint,
};
use underwood_parley::ParleyParagraphEngine;

let mut layout = LayoutEngine::new(
    ParleyParagraphEngine::new(fonts),
    CacheBudget::new(4_096).with_shared_preparation_bytes(8 * 1024 * 1024),
);
let mut label =
    TextBlock::plain(DocumentId::from_bytes(*b"save-label-00001"), "Save")?;

let output = layout.prepare_block(
    &label.snapshot(),
    &BlockRequest::new(TextConstraint::MaxContent, &shared_style, &shared_paint),
)?;
let metrics = output.scene().metrics();
label.set_text("Open")?;

layout.release_document(label.id());
```

The same façade also exposes just enough revision-correct editing for a
toolkit-owned text control. It does not own keyboard bindings, focus, IME
policy, or widget state:

```rust,ignore
use underwood::TextSelectionMode;

let snapshot = label.snapshot();
let output = layout.prepare_block(
    &snapshot,
    &BlockRequest::new(TextConstraint::MaxContent, &shared_style, &shared_paint)
        .with_features(underwood::SceneFeatures::EDITABLE),
)?;
let scene = output.scene();
let editing = scene.editing()?;
let text = snapshot.text_id();
let start = editing.position_at(text, 0).unwrap();
let end = editing.position_at(text, 4).unwrap();
let selected = editing.selection_between(&start, &end, TextSelectionMode::Logical)?;
let selected = editing.selection_set([selected])?;

let rebound = label.replace_selections(&selected, "Open")?;
assert_eq!(label.text(), "Open");
let current = layout.prepare_block(
    &label.snapshot(),
    &BlockRequest::new(TextConstraint::MaxContent, &shared_style, &shared_paint),
)?;
assert_eq!(rebound.revision(), current.scene().revision());
```

The shared-preparation budget is opt-in and independent of the retained
geometry entry budget. An exact hit skips backend analysis, font selection,
shaping, and line formation, then rebuilds current document, revision,
semantic, interaction, paint, and geometry identity. `release_document`
releases identity-bound entries but preserves useful shared facts;
`clear_cache` releases both. The byte diagnostics are deterministic retention
charges, not allocator-exact heap measurements.

[`TextConstraint::MaxContent`] suppresses soft wrapping while preserving
mandatory breaks. [`TextConstraint::MinContent`] commits every legal soft
break through line-final shaping, and
[`TextConstraint::Wrap`] greedily fits legal breaks to one [`FiniteWidth`].
[`TextMetrics`] reports maximum actual line advance, total block extent, and
optional first/last baselines. Empty blocks have zero width, their resolved
line height, and no text baseline.

Line formation is observable independently of shaping.
[`WorkReport::line_candidates`] counts proposed candidates,
[`WorkReport::rejected_line_candidates`] exposes fit-changing retries, and
[`WorkReport::line_checkpoint_restores`] records rewinds of traversal and
provisional output. These counters are actual work from the current call;
they are not retained paragraph preparation.

`ComputedInlineStyle` clones share the owned family, feature, and variation
arrays. `BlockRequest` goes further and borrows one caller-owned style, so any
number of labels can reuse the same style and paint table without rebuilding
authored font requests.

`ParagraphStyle` keeps paragraph base direction and accepted-slot alignment
out of inline font style. Automatic direction and logical-start alignment
remain the defaults; explicit LTR/RTL and physical or logical alignment values
are available for markup, host, and authoring semantics:

```rust,ignore
use underwood::{BaseDirection, ParagraphStyle, TextAlignment};

styles.set_paragraph_style(
    paragraph,
    ParagraphStyle::new(BaseDirection::Rtl)
        .with_alignment(TextAlignment::Start),
);
```

Changing paragraph direction invalidates Unicode analysis for that paragraph.
Changing only alignment reuses analysis, selected fonts, canonical shaping,
line-final shaping, and accepted source boundaries; it recomputes immutable
line adjustment and scene geometry. `Start` and `End` consume the paragraph
direction already resolved by Unicode analysis. `Justify` expands explicit
Western inter-word spaces on eligible soft-wrapped lines; final and mandatory
lines remain start-aligned, and CJK and Arabic strategies remain separate.
[`SceneLineView::adjustment`] exposes the exact offset, hanging trailing
whitespace, and per-opportunity expansion. Changing only line height reuses
accepted line glyphs and recomputes metrics.

Computed text policy is partitioned by the earliest preparation stage it can
change. A host can lower its resolved style without constructing CSS or widget
objects in Underwood:

```rust,ignore
use underwood::{
    AnalysisStyle, ComputedInlineStyle, InlineFlowStyle, LineHeight,
    OverflowWrap, PaintSlot, TextSpacing, TextWrapMode, WordBreak,
};

let flow = InlineFlowStyle::new(LineHeight::metrics_relative(1.1)?)
    .with_spacing(TextSpacing::new(0.5, 2.0)?)
    .with_overflow_wrap(OverflowWrap::Anywhere)
    .with_text_wrap_mode(TextWrapMode::Wrap);
let style = ComputedInlineStyle::new(shaping, flow, PaintSlot::new(0))
    .with_analysis(AnalysisStyle::new(WordBreak::Normal));
```

`WordBreak` participates in Unicode analysis. Wrap and emergency-break policy
participate in line formation. Line-height changes recompute metrics. A
nonzero letter-spacing transition may reshape with retained fonts to disable
optional ligatures; changing only a nonzero spacing amount adjusts retained
advances without another font query.

## Source-complete text projection

[`ProjectedText`] is a small `no_std + alloc` transformation kernel independent
of documents, scenes, paint, and paragraph engines. It retains authored UTF-8,
presentation UTF-8, and compact monotonic relation runs for identity,
replacement, collapse, omission, and insertion. Position lookup uses explicit
[`TextAffinity`] at ambiguous transformed boundaries.

Identity projection keeps one string allocation rather than cloning it.
Custom transformations use [`ProjectionBuilder`], which rejects incomplete
source coverage and invalid UTF-8 boundaries:

```rust
use underwood::{ProjectedText, ProjectionBuilder, TextAffinity};

let mut builder = ProjectionBuilder::new("İ")?;
builder.push_replacement(2, "i\u{307}")?;
let projected = builder.finish()?;

assert_eq!(projected.text(), "i\u{307}");
assert_eq!(projected.source_range(0..1)?, 0..2);
assert_eq!(
    projected.source_position(1, TextAffinity::Downstream)?,
    0,
);
# Ok::<(), underwood::ProjectionError>(())
```

The document preparation path uses this same kernel. Whitespace preservation
remains the default. A host opts into paragraph-stream collapse with:

```rust
use underwood::{ParagraphStyle, WhitespaceCollapse};

let paragraph_style = ParagraphStyle::DEFAULT
    .with_whitespace_collapse(WhitespaceCollapse::Collapse);
```

Collapse recognizes space, tab, carriage return, line feed, and form feed,
turning each maximal run into one ASCII space without trimming line edges.
State crosses inline leaf boundaries. The first authored contributor owns the
collapsed unit's style and semantic identity, while hits, selections, edits,
accessibility records, and renderer/export provenance retain every
contributing leaf-local range.

## Composition epochs and editable surfaces

[`SceneEditing::begin_composition`] creates a transient [`CompositionSession`]
without editing its immutable snapshot. Each accepted [`CompositionUpdate`]
advances a checked epoch, carries generated UTF-8 text, selection, and optional
IME-authored clauses, and projects that text through the same paragraph engine
as committed content. Generated bytes have explicit composition provenance;
they are never mislabeled as authored snapshot ranges.

Scenes remain natively multi-selection. A committed [`EditableSurfaceSnapshot`]
exposes every independent selection and preserves every logical range within a
visual bidi selection. Native composition is the narrower case: because a
marked-text protocol exposes one replacement region, starting a session with
several selections or one disjoint visual selection explicitly collapses to
the primary extent and reports that change through
[`CompositionStart::selection_changed`]. It never silently flattens several
visual ranges into one logical range.

[`EditableSurface`] makes the focused semantic scope explicit, including any
read-only separators, then atomically binds text, selections, source mapping,
geometry, document revision, and composition epoch. A host adapter can answer
UTF-8, UTF-16, or Unicode-scalar range conversions, text queries, caret and
range rectangles, and point-to-offset hits without making platform offsets
global document positions. [`EditableSurfaceSnapshot::replacement_selection`]
maps one explicit host range back into a scene-validated authored selection,
closing the mutation side without exposing a raw position constructor. Cancel
publishes nothing and reveals the committed cache; commit publishes exactly one
validated selection replacement.

## Computed inline styles

Every text leaf receives one complete [`ComputedInlineStyle`]. Callers build
that value from independently invalidated shaping, inline-flow, and paint
partitions, then assign it to the [`TextId`] returned by an edit:

```rust
use underwood::{
    ComputedInlineStyle, Document, DocumentId, FontFamily, FontFeature, InlineFlowStyle,
    InlineRole, PaintSlot, ParagraphRole, ShapingStyle, StyleMap, Tag,
};

let mut document = Document::new(DocumentId::from_bytes(*b"style-example-01"));
let mut edit = document.edit();
let paragraph = edit.append_paragraph(ParagraphRole::BODY).unwrap();
let emphasis = edit
    .append_text(paragraph, InlineRole::EMPHASIS, "office")
    .unwrap();

let shaping = ShapingStyle::new(FontFamily::named("Roboto Flex"), 16.0).unwrap();
let body = ComputedInlineStyle::new(
    shaping.clone(),
    InlineFlowStyle::default(),
    PaintSlot::new(0),
);
let no_ligatures = body
    .clone()
    .with_shaping(shaping.with_features([
        FontFeature::new(Tag::new(b"liga"), 0),
    ]))
    .with_paint(PaintSlot::new(1));

let mut styles = StyleMap::new(body);
styles.set(emphasis, no_ligatures);
```

This replaces the pre-stable `TextStyle { font_size, paint }` shortcut and
`StyleMap::set_paint`: migrate by constructing the complete override from the
default style and assigning it with [`StyleMap::set`].

Font-family CSS source is parsed and owned when a shaping style is built.
Family, weight, width, and style changes reuse Unicode analysis but invalidate
font selection and shaping for the affected paragraph. Resolved scene
fragments retain exact font bytes, normalized variation coordinates, and
portable synthesis evidence; [`WorkReport::font_selection`] exposes the
clusters resolved instead of hiding that work under shaping.
