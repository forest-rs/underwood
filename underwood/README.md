# Underwood

`underwood` is the small, renderer-independent foundation for immutable
semantic documents, retained paragraph formation, finite-width flow, and
text scenes.

The crate is `no_std + alloc`. It owns no shaping engine, platform host policy,
graphics backend, renderer, system fonts, or global state. Geometry and paint
use Kurbo and Peniko values. The separate `underwood_parley` crate adapts the
pre-stable [`adapter`] contract to the repository's pinned Parley revision.

The first draft public slice is deliberately complete end to end:

- [`Document`] publishes immutable [`DocumentSnapshot`] revisions through
  atomic staged edits and preserves body and heading paragraph roles without
  prescribing their visual styling;
- [`LayoutEngine`] retains formed paragraphs and avoids analysis or shaping
  for unchanged siblings, paint-value changes, and width-only changes;
- [`adapter::ParagraphFormation`] keeps legal line breaking, visual ordering,
  and font-derived metrics behind the paragraph-engine boundary instead of
  hiding text physics in scene construction; formed lines retain complete
  source slices across semantic leaves and distinguish real glyphs from
  intentionally unrendered controls;
- [`ComputedInlineStyle`] keeps [`ShapingStyle`], [`InlineFlowStyle`], and
  [`PaintSlot`] values in separate invalidation partitions while [`StyleMap`]
  assigns complete styles to semantic text leaves;
- [`ShapingStyle`] carries backend-neutral family, weight, width, style,
  language, feature, and variation requests; the separate adapter resolves
  them without moving font matching into this crate;
- [`TextScene`] exposes real glyph resources, paint clips, source mapping,
  exact shaped-cluster hits and carets (including whitespace, ligature
  components, bidi affinities, and empty editable leaves), and semantic
  observations;
- document IDs are opaque and document-scoped, while [`SnapshotTextRange`] and
  [`SnapshotTextPosition`] values are dense observations valid only for their
  named revision.

The API is unpublished and pre-stable. Snapshot positions expose validated
UTF-8 boundaries but have no raw constructor and are not durable anchors. The
crate still introduces no byte-offset mutation API, persistence format,
renderer, or compatibility promise. See the external `examples/headless`
workspace crate for the normative call path.

## Exact scene interaction

Paragraph adapters provide source-complete visual clusters separately from
painted glyphs. Exact hits therefore cover glyph interiors, ligature
components, and whitespace without pretending that ink bounds are cursor
geometry. Closest hits also clamp to an empty editable leaf:

```rust,ignore
let hit = scene.hit_test(point).or_else(|| scene.hit_test_closest(point));
if let Some(hit) = hit {
    let caret = scene
        .caret(hit.position())
        .expect("a hit from this scene has a matching caret stop");
    assert_eq!(caret.position(), hit.position());
}
```

`SnapshotTextPosition` includes the exact document revision, semantic text
leaf, UTF-8 byte boundary, and upstream/downstream affinity. Passing a position
from another revision or scene to [`TextScene::caret`] returns `None` rather
than silently relocating it.

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
