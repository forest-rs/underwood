# Underwood

`underwood` is the small, renderer-independent foundation for immutable
semantic documents, retained paragraph preparation, finite-width flow, and
text scenes.

The crate is `no_std + alloc`. It owns no shaping engine, platform host policy,
graphics backend, renderer, system fonts, or global state. Geometry and paint
use Kurbo and Peniko values. The separate `underwood_parley` crate adapts the
pre-stable [`adapter`] contract to the repository's pinned Parley revision.

The first draft public slice is deliberately complete end to end:

- [`Document`] publishes immutable [`DocumentSnapshot`] revisions through
  atomic staged edits;
- [`LayoutEngine`] retains prepared paragraphs and avoids analysis or shaping
  for unchanged siblings, paint-value changes, and width-only changes;
- [`ComputedInlineStyle`] keeps [`ShapingStyle`], [`InlineFlowStyle`], and
  [`PaintSlot`] values in separate invalidation partitions while [`StyleMap`]
  assigns complete styles to semantic text leaves;
- [`TextScene`] exposes real glyph resources, paint clips, source mapping, hit
  testing, caret geometry, and semantic observations;
- document IDs are opaque and document-scoped, while [`SnapshotTextRange`]
  values are dense observations valid only for their named revision.

The API is unpublished and pre-stable. It introduces no byte-offset editing,
durable anchors, persistence format, renderer, or compatibility promise. See
the external `examples/headless` workspace crate for the normative call path.

## Computed inline styles

Every text leaf receives one complete [`ComputedInlineStyle`]. Callers build
that value from independently invalidated shaping, inline-flow, and paint
partitions, then assign it to the [`TextId`] returned by an edit:

```rust
use underwood::{
    ComputedInlineStyle, Document, DocumentId, FontFeature, InlineFlowStyle,
    InlineRole, PaintSlot, ParagraphRole, ShapingStyle, StyleMap, Tag,
};

let mut document = Document::new(DocumentId::from_bytes(*b"style-example-01"));
let mut edit = document.edit();
let paragraph = edit.append_paragraph(ParagraphRole::BODY).unwrap();
let emphasis = edit
    .append_text(paragraph, InlineRole::EMPHASIS, "office")
    .unwrap();

let shaping = ShapingStyle::new(16.0).unwrap();
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
