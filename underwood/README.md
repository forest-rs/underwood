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
- [`TextScene`] exposes real glyph resources, paint clips, source mapping, hit
  testing, caret geometry, and semantic observations;
- document IDs are opaque and document-scoped, while [`SnapshotTextRange`]
  values are dense observations valid only for their named revision.

The API is unpublished and pre-stable. It introduces no byte-offset editing,
durable anchors, persistence format, renderer, or compatibility promise. See
the external `examples/headless` workspace crate for the normative call path.
