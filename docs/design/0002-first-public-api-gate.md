# Design-0002: First semantic-to-scene public API gate

- **Status:** Accepted
- **Accepted:** 2026-07-21 by Bruce Mitchener
- **Bead:** `und-oh0.10.1.6`
- **Campaign:** `und-oh0.10.1`
- **Authority:** Design-0001, ADR-0001 through ADR-0004

## Decision

Approved by Bruce Mitchener on 2026-07-21. The dependency, crate, and draft
public-API gate below is authorized for immediate implementation as one
coherent semantic-to-scene slice.

## Approved scope

Approve one coherent implementation patch containing:

1. the draft public surface below in the existing `underwood` crate;
2. a new `underwood_parley` adapter crate behind the accepted dependency
   fence;
3. the exact production dependencies and features in this document;
4. an external `examples/headless` workspace crate that uses only public paths;
5. the `CHANGELOG.md` migration entry below.

Approval would authorize these draft APIs and dependencies. It would not make
the API stable, approve a Parley patch, select a wire format, publish either
crate, or promote any proof status before the external call path passes.

## Why this is one gate

The façade, adapter contract, geometry vocabulary, paint vocabulary, and
headless call site constrain one another. Approving only one type at a time
would encourage a locally tidy API that cannot carry the whole first slice.
The proposed patch is therefore reviewed as one end-to-end contract.

## Dependency proposal

All entries are production dependencies and remain absent from manifests until
this document is approved.

| Owning crate | Dependency | Exact selection | Purpose |
| --- | --- | --- | --- |
| `underwood` | `kurbo` | crates.io `0.13.1`, `default-features = false`, `features = ["libm"]` | shared `no_std` geometry vocabulary |
| `underwood` | `peniko` | crates.io `0.6.1`, `default-features = false`, `features = ["libm"]` | renderer-neutral brush and color values |
| `underwood_parley` | `parley_core` | git revision `45da4a90248b1600277a4294b70d8bfde5ca8e97`, `default-features = false`, `features = ["libm"]` | analysis, bidi, itemization, and shaping |
| `underwood_parley` | `fontique` | same git revision, `default-features = false`, `features = ["libm"]` | caller-supplied font blobs and instances |
| `underwood_parley` | `parlance` | same git revision, `default-features = false` | shaping feature and variation values |

Design-0005 supersedes only the immutable Parley selection in this approved
table: all three Parley dependencies now resolve together from
`6c81e1dd9b67793cdd959c65cc650c96a1262fb7`. Crate ownership, features, and the
no-system-font policy are unchanged.

Design-0007 subsequently supersedes the paint-coverage behavior in this first
slice. The live pin is `d12c801d8fd298ff095f1ec903b6adaa732fcef2`;
glyph clips use real outline bounds, and a paint boundary inside one shaped
glyph returns `UnsupportedPaintCoverage` instead of a proportional split. The
historical example and acceptance text below record what the initial gate
approved, not the current conformance claim.

The local forest consistently uses Kurbo 0.13.x without default features;
Overstory uses Kurbo 0.13.1 and Peniko 0.6.1 with `libm`. The audited releases
are Apache-2.0 OR MIT and have MSRVs below Underwood's Rust 1.92. The Parley
revision and licenses are already recorded by ADR-0004 and the retained-seam
wind tunnel.

`fontique/system` and every platform font backend stay disabled. The headless
path injects licensed font bytes. A later host-font adapter is a separate
dependency and ownership decision.

## Ownership map

```text
underwood
  document     immutable semantic snapshots and typed whole-leaf edits
  projection   paragraph UTF-8 plus snapshot-local source map
  style        shape/flow/paint partitions and paint slots
  prepared     Underwood-owned adapter input and validated shaped output
  flow         first finite-width, single-region private breaker
  scene        immutable geometry, paint, hit/caret, and semantic observations

underwood_parley
  owns conversion between prepared contracts and pinned Parley
  owns no document, flow, scene, renderer, or public Parley leakage

examples/headless
  external consumer; receives no private module, test hook, or fixture shortcut
```

`underwood` remains one crate. Modules stay private; the root re-exports only
the types required by the complete call site. The adapter-facing contracts are
public under `underwood::adapter`, explicitly pre-stable, because a separate
crate cannot implement a private trait.

## Complete external call site

The implementation patch must make this shape compile in
`examples/headless`. Exact error plumbing may use `main -> Result`, but the
Underwood calls and ownership shown here are normative.

```rust
use underwood::{
    Brush, Color, Document, DocumentId, FiniteWidth, InlineRole, LayoutEngine,
    PaintSlot, PaintTable, ParagraphRole, SceneRequest, StyleMap, TextStyle,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Document::new(DocumentId::from_bytes(*b"underwood-demo-1"));

    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_prefix = edit.append_text(first, InlineRole::TEXT, "of")?;
    let first_suffix = edit.append_text(first, InlineRole::EMPHASIS, "fice مرحبا")?;
    let second = edit.append_paragraph(ParagraphRole::BODY)?;
    let second_text = edit.append_text(second, InlineRole::TEXT, "unchanged sibling")?;
    let published = edit.commit()?;
    let old_snapshot = published.snapshot().clone();

    let base = TextStyle::new(16.0, PaintSlot::new(0))?;
    let mut styles = StyleMap::new(base);
    styles.set_paint(first_prefix, PaintSlot::new(0))?;
    styles.set_paint(first_suffix, PaintSlot::new(1))?;

    let paint = PaintTable::from_brushes([
        Brush::Solid(Color::from_rgb8(0x20, 0x20, 0x20)),
        Brush::Solid(Color::from_rgb8(0x20, 0x50, 0xa0)),
    ]);

    let fonts = FontSet::try_from_fonts([
        Font::from_bytes(
            "latin",
            include_bytes!("../fonts/RobotoFlex-VariableFont.ttf"),
        )?,
        Font::from_bytes(
            "arabic",
            include_bytes!("../fonts/NotoKufiArabic-Regular.otf"),
        )?,
    ])?;
    let data = TextData::compiled_minimal();
    let paragraphs = ParleyParagraphEngine::new(data, fonts)?;
    let mut layout = LayoutEngine::new(paragraphs);
    let request = SceneRequest::new(FiniteWidth::new(420.0)?, &styles, &paint);

    let first_scene = layout.prepare(published.snapshot(), &request)?;
    assert!(first_scene.scene().lines().len() >= 2);
    let fragment = &first_scene.scene().fragments()[0];
    assert!(!fragment.glyphs().is_empty());
    assert!(fragment.source().is_some());
    let hit = first_scene
        .scene()
        .hit_test((10.0, 10.0).into())
        .expect("the first line must be hittable");
    let caret = first_scene.scene().caret(&hit);
    assert!(caret.bounds().height() > 0.0);
    assert_eq!(hit.source().revision(), published.snapshot().revision());
    assert!(first_scene.scene().semantics().any(|fragment| {
        fragment.inline_role() == Some(InlineRole::EMPHASIS)
    }));

    let mut edit = document.edit();
    edit.replace_text(first_suffix, "fices مرحبا")?;
    let changed = edit.commit()?;
    assert_eq!(old_snapshot.text(second_text), Some("unchanged sibling"));

    let second_scene = layout.prepare(changed.snapshot(), &request)?;
    assert_eq!(second_scene.work().analysis().paragraphs(), 1);
    assert_eq!(second_scene.work().shape().paragraphs(), 1);
    assert_eq!(second_scene.work().reused_paragraphs(), 1);

    let recolored = paint.with_brush(
        PaintSlot::new(1),
        Brush::Solid(Color::from_rgb8(0xa0, 0x20, 0x20)),
    )?;
    let paint_request =
        SceneRequest::new(FiniteWidth::new(420.0)?, &styles, &recolored);
    let paint_scene = layout.prepare(changed.snapshot(), &paint_request)?;
    assert_eq!(paint_scene.work().analysis().paragraphs(), 0);
    assert_eq!(paint_scene.work().shape().paragraphs(), 0);
    assert_eq!(paint_scene.work().flow().paragraphs(), 0);
    assert_ne!(second_scene.scene().paint(), paint_scene.scene().paint());

    Ok(())
}
```

The example deliberately:

- builds a root with two stable paragraphs and semantic inline leaves;
- places a paint boundary inside the shaped `ffi` sequence without making the
  semantic boundary a shaping boundary;
- mixes Latin and Arabic;
- proves old-snapshot observability after a whole-leaf edit;
- proves sibling-paragraph reuse;
- proves paint-only reuse;
- consumes line, hit, and semantic observations.

The bundled font files live only in the example crate and retain their upstream
licenses and SHA-256 identities. Production crates contain no fonts.

## Proposed public surface

The exact representation of every opaque type stays private.

### Document and transaction

```rust
pub struct Document;
pub struct DocumentSnapshot;
pub struct DocumentId;
pub struct DocumentRevision;
pub struct ParagraphId;
pub struct TextId;
pub struct SemanticId;
pub struct Edit<'document>;
pub struct Publication;
pub struct ChangeSet;

pub struct ParagraphRole;
pub struct InlineRole;

impl ParagraphRole {
    pub const BODY: Self;
}

impl InlineRole {
    pub const TEXT: Self;
    pub const EMPHASIS: Self;
}

impl DocumentId {
    pub const fn from_bytes(value: [u8; 16]) -> Self;
}

impl Document {
    pub fn new(id: DocumentId) -> Self;
    pub fn snapshot(&self) -> DocumentSnapshot;
    pub fn edit(&mut self) -> Edit<'_>;
}

impl Edit<'_> {
    pub fn append_paragraph(
        &mut self,
        role: ParagraphRole,
    ) -> Result<ParagraphId, EditError>;
    pub fn append_text(
        &mut self,
        paragraph: ParagraphId,
        role: InlineRole,
        text: &str,
    ) -> Result<TextId, EditError>;
    pub fn replace_text(
        &mut self,
        text: TextId,
        replacement: &str,
    ) -> Result<(), EditError>;
    pub fn commit(self) -> Result<Publication, EditError>;
}

impl Publication {
    pub fn snapshot(&self) -> &DocumentSnapshot;
    pub fn changes(&self) -> &ChangeSet;
}

impl DocumentSnapshot {
    pub fn id(&self) -> DocumentId;
    pub fn revision(&self) -> DocumentRevision;
    pub fn text(&self, id: TextId) -> Option<&str>;
}
```

An `Edit` stages changes. Dropping it publishes nothing. Commit either
publishes one new immutable snapshot or leaves the document unchanged. IDs are
document-scoped and stable across ordinary edits, but this patch defines no
serialized, cross-document, or collaboration identity.

Whole-leaf replacement is intentional. No public byte-offset editing, stable
anchor, authored tracked range, or universal range type is introduced by this
slice.

### Style and paint

```rust
pub struct PaintSlot;
pub struct TextStyle;
pub struct StyleMap;
pub struct PaintTable;

impl PaintSlot {
    pub const fn new(index: u32) -> Self;
}

impl TextStyle {
    pub fn new(font_size: f32, paint: PaintSlot) -> Result<Self, StyleError>;
}

impl StyleMap {
    pub fn new(default: TextStyle) -> Self;
    pub fn set_paint(
        &mut self,
        text: TextId,
        paint: PaintSlot,
    ) -> Result<(), StyleError>;
}

impl PaintTable {
    pub fn from_brushes(values: impl IntoIterator<Item = Brush>) -> Self;
    pub fn with_brush(
        &self,
        slot: PaintSlot,
        value: Brush,
    ) -> Result<Self, StyleError>;
}
```

`StyleMap` is an immutable input once borrowed by a preparation call. Its
private stage keys separate shaping, flow, and paint. The first slice exposes
only the values used by the example; it does not add a generic property bag or
claim the complete style system.

### Preparation and scene

```rust
pub struct LayoutEngine;
pub struct FiniteWidth;
pub struct SceneRequest<'a>;
pub struct SceneOutput;
pub struct WorkReport;
pub struct StageWork;
pub struct TextScene;
pub struct SceneLine;
pub struct SceneFragment;
pub struct SemanticFragment;
pub struct TextHit;
pub struct SceneCaret;
pub struct SnapshotTextRange;
pub struct SceneGlyph;
pub struct SceneFragmentId;

impl FiniteWidth {
    pub fn new(width: f64) -> Result<Self, SceneError>;
}

impl<'a> SceneRequest<'a> {
    pub fn new(
        width: FiniteWidth,
        styles: &'a StyleMap,
        paint: &'a PaintTable,
    ) -> Self;
}

impl LayoutEngine {
    pub fn new(
        paragraphs: impl adapter::ParagraphPreparation + 'static,
    ) -> Self;
    pub fn prepare(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Result<SceneOutput, SceneError>;
}

impl SceneOutput {
    pub fn scene(&self) -> &TextScene;
    pub fn work(&self) -> &WorkReport;
}

impl WorkReport {
    pub fn analysis(&self) -> StageWork;
    pub fn itemization(&self) -> StageWork;
    pub fn shape(&self) -> StageWork;
    pub fn flow(&self) -> StageWork;
    pub fn geometry(&self) -> StageWork;
    pub fn paint(&self) -> StageWork;
    pub fn reused_paragraphs(&self) -> usize;
}

impl StageWork {
    pub fn paragraphs(self) -> usize;
    pub fn records(self) -> usize;
}

impl TextScene {
    pub fn lines(&self) -> &[SceneLine];
    pub fn fragments(&self) -> &[SceneFragment];
    pub fn paint(&self) -> &PaintTable;
    pub fn semantics(&self) -> impl Iterator<Item = &SemanticFragment>;
    pub fn hit_test(&self, point: Point) -> Option<TextHit>;
    pub fn caret(&self, hit: &TextHit) -> SceneCaret;
}

impl SceneLine {
    pub fn bounds(&self) -> Rect;
    pub fn source(&self) -> &SnapshotTextRange;
}

impl SceneFragment {
    pub fn id(&self) -> SceneFragmentId;
    pub fn glyphs(&self) -> &[SceneGlyph];
    pub fn paint(&self) -> PaintSlot;
    pub fn transform(&self) -> Affine;
    pub fn source(&self) -> Option<&SnapshotTextRange>;
}

impl SceneGlyph {
    pub fn id(&self) -> u32;
    pub fn position(&self) -> Point;
    pub fn advance(&self) -> Vec2;
    pub fn source(&self) -> &SnapshotTextRange;
}

impl SemanticFragment {
    pub fn semantic_id(&self) -> SemanticId;
    pub fn inline_role(&self) -> Option<InlineRole>;
    pub fn source(&self) -> Option<&SnapshotTextRange>;
    pub fn bounds(&self) -> Rect;
}

impl TextHit {
    pub fn source(&self) -> &SnapshotTextRange;
    pub fn point(&self) -> Point;
}

impl SceneCaret {
    pub fn source(&self) -> &SnapshotTextRange;
    pub fn bounds(&self) -> Rect;
}

impl SnapshotTextRange {
    pub fn revision(&self) -> DocumentRevision;
    pub fn text(&self) -> TextId;
    pub fn bytes(&self) -> core::ops::Range<u32>;
}
```

The concrete scene and adapter contract use re-exported Peniko
`Brush`/`Color`/`FontData` and Kurbo
`Affine`/`Point`/`Rect`/`Size`/`Vec2`; Underwood does not invent parallel
geometry, brush, or font-resource types.

`SnapshotTextRange` is dense and valid only for the exact
`DocumentSnapshot` revision named by the scene. It is returned through
semantic, hit, and caret observations but has no constructor from raw offsets
and cannot be stored as a durable anchor. This is the narrow snapshot-local
position form accepted by ADR-0001, not a public stable-position API.

### Adapter contract

`underwood::adapter` exposes only Underwood-owned types:

```rust
pub trait ParagraphPreparation {
    fn prepare(
        &mut self,
        input: ParagraphInput<'_>,
    ) -> Result<ParagraphPreparationOutput, PreparationError>;
}

pub struct ParagraphInput<'a>;
pub struct ParagraphPreparationOutput;
pub struct PreparationWork;
pub struct PreparedParagraph;
pub struct PreparedRun;
pub struct PreparedGlyph;
pub struct GlyphPaintCoverage;
pub struct GlyphPaintSegment;
pub struct PaintRun;
pub struct PreparationError;

impl ParagraphInput<'_> {
    pub fn paragraph(&self) -> ParagraphId;
    pub fn text(&self) -> &str;
    pub fn font_size(&self) -> f32;
    pub fn paint_runs(&self) -> &[PaintRun];
}

impl PaintRun {
    pub fn bytes(&self) -> core::ops::Range<u32>;
    pub fn slot(&self) -> PaintSlot;
}

impl ParagraphPreparationOutput {
    pub fn new(
        paragraph: PreparedParagraph,
        work: PreparationWork,
    ) -> Self;
    pub fn paragraph(&self) -> &PreparedParagraph;
    pub fn work(&self) -> PreparationWork;
}

impl PreparationWork {
    pub const fn new(
        analyzed: bool,
        itemized: bool,
        shaped_runs: u32,
        shaped_glyphs: u32,
    ) -> Self;
}

impl PreparedParagraph {
    pub fn try_from_runs(
        paragraph: ParagraphId,
        text_len: u32,
        runs: impl IntoIterator<Item = PreparedRun>,
    ) -> Result<Self, PreparationError>;
}

impl PreparedRun {
    pub fn try_new(
        source: core::ops::Range<u32>,
        bidi_level: u8,
        script: [u8; 4],
        font: FontData,
        font_size: f32,
        normalized_coords: impl IntoIterator<Item = i16>,
        glyphs: impl IntoIterator<Item = PreparedGlyph>,
    ) -> Result<Self, PreparationError>;
}

impl PreparedGlyph {
    pub fn try_new(
        id: u32,
        source: core::ops::Range<u32>,
        advance: Vec2,
        offset: Vec2,
        paint: GlyphPaintCoverage,
    ) -> Result<Self, PreparationError>;
}

impl GlyphPaintCoverage {
    pub fn try_from_segments(
        segments: impl IntoIterator<Item = GlyphPaintSegment>,
    ) -> Result<Self, PreparationError>;
}

impl GlyphPaintSegment {
    pub fn new(
        source: core::ops::Range<u32>,
        slot: PaintSlot,
        local_clip: Rect,
    ) -> Result<Self, PreparationError>;
}
```

The real patch must provide documented getters and checked constructors for
these records. It must not expose a Parley type, allow an invalid source range,
or let a glyph reference an absent run/font/paint slot. Inputs borrow only for
the call; successful output owns all retained data.

`PreparationWork` reports actual analyzer, itemizer, shaper-run, and glyph
activity observed by the adapter. The core aggregates it with paragraphs it
skips entirely through retained component revisions. Spy-adapter tests and the
real Parley path both exercise the counters; equal output is not accepted as
proof that work was avoided.

`GlyphPaintCoverage` represents one or more source-ordered paint segments and
their clip geometry for a shaped glyph. It preserves one shaping result across
paint-only boundaries, including ligatures. The adapter must return
`PreparationError::UnsupportedPaintCoverage` rather than silently assigning
the whole glyph one color when it cannot produce faithful coverage.

`underwood_parley` adds this exact façade over that contract:

```rust
pub struct Font;
pub struct FontSet;
pub struct TextData;
pub struct ParleyParagraphEngine;
pub struct AdapterError;

impl Font {
    pub fn from_bytes(
        diagnostic_name: &str,
        bytes: &[u8],
    ) -> Result<Self, AdapterError>;
}

impl FontSet {
    pub fn try_from_fonts(
        fonts: impl IntoIterator<Item = Font>,
    ) -> Result<Self, AdapterError>;
}

impl TextData {
    pub fn compiled_minimal() -> Self;
}

impl ParleyParagraphEngine {
    pub fn new(
        data: TextData,
        fonts: FontSet,
    ) -> Result<Self, AdapterError>;
}

impl underwood::adapter::ParagraphPreparation for ParleyParagraphEngine {
    // Implements the Underwood-owned method without exposing Parley.
}
```

`Font` copies the first headless fixtures into immutable owned storage. The
diagnostic name is not a cache identity. Prepared font identity derives from
the exact bytes, face index, synthesis, and normalized coordinates; the
implementation must not use the name as identity. `TextData::compiled_minimal`
is admitted only as the first measured current path and does not pretend to
provide the content-digest provider contract still missing under ADR-0003.

The adapter crate is also `no_std + alloc`; it uses no system-font feature and
implements `core::error::Error`.

## Errors

All public fallible constructors and operations return concrete, non-exhaustive
error structs with a documented `kind()` enum, `Display`, and
`core::error::Error`. Errors contain stable category, affected
document/paragraph identity when available, and source range when validated.
They do not expose backend error types or promise stable prose.

The first patch contains:

- `EditError` for wrong-document IDs, invalid structure, oversized text, and
  revision conflict;
- `StyleError` for non-finite/negative values, unknown text IDs, and absent
  paint slots;
- `PreparationError` for missing capabilities/fonts/glyph coverage, invalid
  adapter output, and cancelled work;
- `SceneError` for invalid finite width, preparation failure, source-coverage
  violation, and flow failure.

## Ownership and invalidation contract

- `DocumentSnapshot`, `TextScene`, and `PaintTable` are immutable, cheaply
  cloneable shared values.
- `Document` and `LayoutEngine` are explicit mutable owners and contain no
  hidden global state.
- `LayoutEngine` owns exactly one configured `ParagraphPreparation`, caches by
  document/paragraph/stage identity, and publishes a scene only after the whole
  request succeeds.
- Replacing fonts or text data requires constructing a new `LayoutEngine`, so
  its retained cache cannot outlive the preparation context that produced it.
- `ParagraphPreparation` may reuse private workspaces, but its fonts, text
  data, and shaping configuration are immutable after it enters
  `LayoutEngine`; no borrowed Parley output survives a call.
- Paint values are absent from analysis, itemization, shape, break, flow, and
  geometry keys.
- Paint-slot topology creates no itemization split and does not enter shaping
  physics or its cache key; the adapter may carry it as uninterpreted metadata
  only to reconstruct coverage for lowering.
- Width is absent from analysis, itemization, and shaping keys.
- A changed paragraph cannot invalidate an unchanged sibling's retained stages.

These are tested as negative work assertions, not inferred from equal pixels.

## Non-goals of the first patch

- byte-offset editing, stable anchors, dense authored tracked ranges, selection,
  undo, collaboration, or persistence;
- arbitrary regions, pagination, floats, tables, vertical text, inline objects,
  or general fragmentation;
- system font discovery, asynchronous loading, locale tailoring, hyphenation,
  or a production text-data bundle format;
- a serialized scene/delta protocol or stable identity representation;
- renderer, GPU atlas, accessibility toolkit, or host integration;
- publication to crates.io or a compatibility promise.

## Migration entry

The implementation patch adds this under `CHANGELOG.md` / `Unreleased`:

```text
### Draft API

- Added the first review-gated semantic-to-scene path through `Document`,
  `LayoutEngine`, `TextScene`, and `underwood_parley`.
- IDs are document-scoped and not serialized; scene source ranges are valid
  only for their named immutable snapshot.
- This pre-stable API intentionally replaces no prior public product API.
```

Every later draft-API change updates the external example and adds a migration
note naming the old and new call shape.

## Alternatives rejected

### Core-only document API first

It would make early progress look cleaner, but it would allow document IDs and
transactions to stabilize before scene source mapping and invalidation test
them. Rejected as an API-first half slice.

### `String -> TextScene`

It cannot prove semantic structure, paragraph identity, sibling reuse, or
source mapping. Rejected as a demo shortcut.

### Generic paint and geometry parameters

They avoid dependencies by pushing permanent complexity onto every caller.
Rejected in favor of the forest's existing Peniko/Kurbo vocabulary.

### High-level `parley::Layout<Brush>` in the façade

It couples paint payloads to the wrong retained identities and leaks Parley
ownership. Rejected by ADR-0004.

## Proof gate after approval

Landing the proposed API does not itself make the capability `Executable`.
Promotion requires:

- the external headless crate using only the documented public path;
- real Parley glyph IDs, advances, clusters, bidi, and source coverage;
- faithful multi-paint ligature coverage or an explicit failing diagnostic;
- old-snapshot, sibling-reuse, paint-only, and width-only negative-work tests;
- stable/MSRV, `x86_64-unknown-none`, `wasm32-unknown-unknown`, and all-host CI;
- a Lynx and Rook review with all Must findings resolved.
