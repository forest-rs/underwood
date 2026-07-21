# Design-0003: Computed inline-style spine

- **Status:** Approved
- **Approved:** 2026-07-21 by Bruce Mitchener
- **Beads:** `und-oh0.4.1`, `und-oh0.4.2`
- **Authority:** `UNDERWOOD_HANDOVER.md` §§13.1–13.7, 16.1–16.4

## Goal

Replace the first-slice `TextStyle { font_size, paint }` shortcut with the
permanent computed-inline-style spine. One semantic document must be able to
carry heterogeneous shaping, inline-flow, and paint values through the real
Parley adapter and into `TextScene`, while retained work reports distinguish
the stages those values actually invalidate.

## Fence

The `underwood` style layer owns validated complete computed inline styles,
their partitioned paragraph-run projection, and Underwood's invalidation
identities; it explicitly does not own authored declarations, cascade, font
matching, Unicode analysis, shaping algorithms, paragraph-break policy, block
flow, or pixel production.

`underwood_parley` owns conversion from Underwood shaping runs into the pinned
`parley_core` inputs; it explicitly does not reinterpret style semantics or
move Parley types into the document model.

## Invariants

1. A complete style has three explicit partitions: shaping, inline flow, and
   paint.
2. Paint values never enter analysis, itemization, shaping, or flow identity.
3. Line height never enters analysis, itemization, or shaping identity.
4. Language, OpenType features, variation coordinates, and the current
   backend's font size split shaping items and enter shaping identity.
5. Changing a shaping style without changing text reuses Unicode analysis.
6. Every exposed property executes through the public path and has a
   deterministic behavioral or work-report test. Placeholder properties are
   prohibited.
7. Settings are owned, canonicalized by tag, and independent of caller
   lifetimes. For duplicate tags, the last supplied value wins.
8. Non-finite numbers are rejected before preparation. An unsupported feature
   or variation tag is a deterministic no-op for a font; supported variation
   coordinates are normalized and clamped by the shaping backend.
9. An absent explicit `opsz` coordinate means the font's default optical-size
   position in this slice. Automatic optical sizing is not implied.
10. Core crates remain `no_std + alloc`, gain no dev-dependencies, and contain
    no `unsafe`.

## Options considered

### Add only features and variations to `TextStyle`

Smallest diff, but it preserves a monolithic identity and leaves the next
layout property to force another public rewrite. Rejected.

### Revive `styled_text` as a new foundational crate

The compact complete-style/run idea is sound, but a separate crate has no
second proven consumer and would expose storage before the style boundary is
earned. Rejected for now; extraction remains possible if real reuse appears.

### Establish computed partitions in `underwood`

Chosen. The public API accepts complete computed values, while paragraph
projection interns shaping and flow partitions privately and emits contiguous
runs. This is the permanent seam onto which a future cascade can resolve.

## Shared vocabulary decision

`underwood` adds `parlance` with default features disabled and re-exports
`Tag`, `Language`, `FontFeature`, and `FontVariation`. Parlance is the
dependency-free, `no_std` vocabulary crate already used by the pinned adapter;
duplicating these four types would add conversions and allow their semantics
to drift. No `parley` or `parley_core` engine type crosses the Underwood
facade.

This production dependency edge and its public use were approved on
2026-07-21.

## Approved public direction

The exact rustdoc may grow during implementation, but the ownership and
callsite shape are fixed:

```rust
pub use parlance::{FontFeature, FontVariation, Language, Tag};

pub struct ShapingStyle;
pub struct LineHeight;
pub struct InlineFlowStyle;
pub struct ComputedInlineStyle;
pub struct StyleMap;

impl ShapingStyle {
    pub fn new(font_size: f32) -> Result<Self, StyleError>;
    pub fn with_language(self, language: Option<Language>) -> Self;
    pub fn with_features(
        self,
        features: impl IntoIterator<Item = FontFeature>,
    ) -> Self;
    pub fn with_variations(
        self,
        variations: impl IntoIterator<Item = FontVariation>,
    ) -> Result<Self, StyleError>;
}

impl LineHeight {
    pub const NORMAL: Self;
    pub fn from_multiplier(multiplier: f32) -> Result<Self, StyleError>;
}

impl InlineFlowStyle {
    pub const fn new(line_height: LineHeight) -> Self;
}

impl ComputedInlineStyle {
    pub const fn new(
        shaping: ShapingStyle,
        inline_flow: InlineFlowStyle,
        paint: PaintSlot,
    ) -> Self;
    pub fn with_shaping(self, shaping: ShapingStyle) -> Self;
    pub const fn with_inline_flow(self, inline_flow: InlineFlowStyle) -> Self;
    pub const fn with_paint(self, paint: PaintSlot) -> Self;
}

impl StyleMap {
    pub fn new(default: ComputedInlineStyle) -> Self;
    pub fn set(&mut self, text: TextId, style: ComputedInlineStyle);
}
```

Read-only accessors for every public value are part of the implementation.
Dense shaping and inline-flow IDs are paragraph-local implementation details,
not public stable identities or serialized values.

### Before

```rust
let base = TextStyle::new(16.0, PaintSlot::new(0))?;
let mut styles = StyleMap::new(base);
styles.set_paint(emphasis, PaintSlot::new(1))?;
```

### After

```rust
let body_shaping = ShapingStyle::new(16.0)?;
let body = ComputedInlineStyle::new(
    body_shaping.clone(),
    InlineFlowStyle::default(),
    PaintSlot::new(0),
);
let emphasis_style = body.clone()
    .with_shaping(body_shaping.with_features([
        FontFeature::new(Tag::new(b"liga"), 0),
    ]))
    .with_paint(PaintSlot::new(1));

let mut styles = StyleMap::new(body);
styles.set(emphasis, emphasis_style);
```

`TextStyle` and `StyleMap::set_paint` are removed rather than retained as a
second construction path. The workspace is unpublished and pre-stable; every
checked-in caller migrates in the same change.

## Property and invalidation matrix

| Property | Analysis | Itemize | Shape | Flow/geometry | Paint |
| --- | ---: | ---: | ---: | ---: | ---: |
| Font size | no | yes | yes today | yes | no |
| Language | no | yes | yes | downstream | no |
| OpenType features | no | yes | yes | downstream | no |
| Variation coordinates | no | yes | yes | downstream | no |
| Line-height multiplier | no | no | no | yes | no |
| Paint-slot assignment | no | no | no | fragment lowering only | yes |
| Brush value | no | no | no | no | yes |

Font size remains a shaping input because the approved pinned `parley_core`
passes point size to HarfRust. Reclassifying size requires the separate
font-unit shaping evidence described by the handover; this slice does not make
that future result implicit.

## Adapter plumbing

```text
DocumentSnapshot + StyleMap
          |
          v
paragraph projection
  complete source runs
  -> shaping table + runs
  -> inline-flow table + runs
  -> paint runs
          |
          v
ParagraphInput ----------------------+
          |                           |
          v                           v
underwood_parley                  Underwood flow
          |                     (line-height only)
          v                           |
parley_core analysis/itemize/shape   |
          |                           |
          +------> PreparedParagraph-+
                          |
                          v
                      TextScene
```

The adapter receives complete, contiguous shaping runs. It creates
per-character style indices, asks Parley to split items exactly where shaping
inputs change, and passes the selected run's size, language, features, and
variations to `ShapeOptions`. Paint remains carried metadata and never becomes
a shaping split.

## Executable proof

The production change is not complete until the public path proves all of the
following:

- one semantic document contains mixed font sizes, line heights, paints,
  feature settings, and variable-font coordinates;
- `liga` on/off produces observably different real glyph output;
- explicit `wght` and `opsz` settings produce distinct normalized coordinates
  in the checked-in Roboto Flex font;
- a shaping-only change reshapes the affected paragraph without reanalysis;
- a line-height-only change rebuilds flow/geometry without analysis,
  itemization, or shaping;
- a brush-only change performs paint work without rebuilding flow or text
  physics;
- the CPU visual proof turns these facts into a compelling variable-type
  specimen rather than a synthetic diagram.

## Deferred properties

Font family/weight/width/style requests wait for an executable font-matching
contract. Letter and word spacing wait for the Parley tracking-topology seam so
Underwood does not invent ligature behavior. Baseline shift waits for explicit
cluster behavior at style boundaries. Word breaking, overflow wrapping,
hyphenation, whitespace transformation, decorations, block styles, cascade,
and authored expressions remain in their owning campaigns.

## Extension points

- A future specified/cascade layer resolves into `ComputedInlineStyle` without
  changing paragraph preparation.
- More shaping properties extend `ShapingStyle` and its topology equality.
- More executable used values extend `InlineFlowStyle` and the geometry key.
- A second real consumer may justify extracting the private complete-run IR;
  this design does not assume that extraction.
- Font request values can be added when `underwood_parley` can execute them
  through an approved resolver seam.

## Validation

```sh
cargo fmt --all --check
taplo fmt --check --diff
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo xtask check
typos
bd lint --status all
bd dep cycles
```
