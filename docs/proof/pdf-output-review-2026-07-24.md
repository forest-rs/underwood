# Prepared-scene PDF output review — 2026-07-24

## Overview

The goal of this slice is one honest portable-output path: prepare a real
Underwood semantic document, then lower its public renderer-neutral
`TextScene` through a replaceable Krilla adapter into a deterministic PDF.
Underwood continues to own shaping, bidi, fallback, layout, source
provenance, and paint partitioning.

This is not a pagination model, tagged PDF, PDF/UA, a second shaping engine, or
a claim that complex-script copy/paste is complete. The adapter deliberately
rejects scene features it cannot yet reproduce faithfully.

The executable example is:

```sh
cargo run --release -p underwood_pdf_proof
```

It writes `target/underwood-proof.pdf` by default.

## Concepts and glossary

- **Prepared scene:** immutable glyph, geometry, paint, and source observations
  produced by Underwood after shaping and layout.
- **Visual lowering:** preserving the prepared page appearance without
  re-shaping or inferring Unicode from glyph identifiers.
- **Source mapping:** the authored Unicode ranges retained by each scene glyph.
- **Logical extraction:** recovering authored reading order when copying or
  extracting PDF text.
- **Krilla adapter:** the host-side crate translating supported scene
  observations into PDF operations.

The dependency direction is:

```text
semantic document → Underwood TextScene → underwood_pdf → Krilla → PDF bytes
```

Neither Krilla nor its Rust-version floor enters `underwood` or
`underwood_parley`.

## What the proof establishes

The proof document contains semantic headings and paragraphs, five solid
paints, mixed Latin and Arabic in one paragraph, a real `ffi` ligature, an
Arabic lam-alef ligature, and a decomposed Latin accent split across semantic
text leaves. Assertions inspect the prepared scene before export:

- at least one Arabic fragment has an odd bidi level;
- `office` reaches the scene as four shaped glyphs;
- one glyph retains source across more than one semantic leaf;
- all instances stay inside the adapter's supported default/static subset.

The adapter then:

- reuses each scene font's shared byte backing rather than copying the blob;
- preserves the selected face index, glyph identifier, absolute origin,
  advance, fragment transform, and explicit partial-paint clip;
- uses each line's explicit scene-fragment range instead of inferring
  line ownership from overlapping source ranges;
- groups compatible glyph observations so Krilla can encode RTL reversal and
  complex clusters, while a shared shaped-glyph identity prevents duplicate
  Unicode from partial-paint fragments;
- supplies glyph source Unicode to Krilla instead of reverse-mapping glyph
  identifiers;
- rejects non-solid paint, non-default normalized variation coordinates,
  synthetic emboldening or skew, invalid resources, and unrepresentable
  geometry before serialization.

Two complete preparations produce byte-identical PDFs. The inspected artifact
is an unencrypted, one-page PDF 1.7 document with a 720 by 720 point media box.
Raster inspection confirms that Arabic dots and marks, ligatures, the
cross-leaf accent, line wrapping, and authored colors are visible and placed
correctly.

### Additive scene API note

This slice adds `SceneLine::fragment_range`,
`SceneGlyphInstanceId`, and `SceneGlyph::instance_id`. Existing renderer
callers require no migration. Adapters that associated lines by source
containment should move to the explicit fragment range; adapters that
deduplicated partial-painted glyphs by identifier and geometry should use the
instance identity instead.

## Portable-output lessons and backend rules

Portable output is a conformance audit of `TextScene`, not merely another
place to paint glyphs. This slice establishes the following rules for future
PDF, SVG, remote-scene, accessibility, and renderer adapters:

1. Consume the public prepared scene and its matching immutable snapshot.
   Never re-shape text or recover Unicode from glyph identifiers.
2. Preserve exact font bytes and face indices, glyph identifiers, normalized
   locations, positions, advances, transforms, paint partitions, and source
   ranges. Reject any unsupported observation before producing partial output.
3. Treat Unicode provenance, line/run topology, cluster ranges, bidi levels,
   and shaped-glyph identity as separate required facts. Source ranges alone
   are not enough to reconstruct portable text.
4. Emit semantic text once per shaped glyph instance even when several
   clipped paint observations render that glyph. Paint multiplicity is not
   text multiplicity.
5. Test visual rendering, Unicode extraction, and selection independently.
   Success in one contract is not evidence for the others.
6. Compare suspicious viewer behavior with independent producers before
   changing Underwood's architecture. Platform reconstruction bugs must not
   become permanent core workarounds.
7. Share font backing and prepared resources instead of establishing another
   font catalog or source cache in the output backend.
8. Keep backend dependencies and their compiler floors in replaceable host
   crates. Foundational crates remain renderer-neutral and retain their lower
   MSRV.
9. Put pagination, links, reading order, tagged structure, and accessibility
   claims in a document-level export layer above the one-page visual adapter.
   A PDF viewer does not consume Underwood's editor-selection model.

These rules make an exporter valuable even before it supports every PDF
feature: each new backend either consumes the same explicit contract or
reveals a narrowly stated observation that all renderers can use.

## Usage and extension points

Callers prepare a scene normally, retain its matching immutable snapshot, and
choose PDF page geometry:

```rust,ignore
let page = PdfPage::new(720.0, 720.0)?
    .with_origin(underwood::Point::new(72.0, 68.0))?;
let bytes = underwood_pdf::to_pdf(&scene, &snapshot, page)?;
```

The snapshot is required because the scene carries revision-bound source
ranges rather than duplicate strings. Future extensions can add multipage
assembly, more Krilla paint mappings, exact variable instances, synthesis, and
logical text structures without changing the foundational scene's ownership
boundary.

## Gotchas and risks

Visual correctness does not imply viewer-independent PDF selection. The
adapter now emits compatible glyphs as runs, gives Krilla exact ranges into one
logical line buffer, preserves decreasing RTL ranges, and emits Unicode only
once for a partial-painted shaped glyph. Plain Arabic therefore extracts
without the earlier duplicated marks. Mixed-direction runs can still be
reordered by the viewer's geometry heuristics.

This was tested in macOS Preview against three independent producers:
Underwood/Krilla, headless Chrome, and Apple's own AppKit/CoreText/Quartz PDF
path. Preview reordered or corrupted mixed Arabic in all three. It also did
not reliably honor a line-level `/ActualText` override. Invisible or
nearly-transparent duplicate text carriers were rejected because they either
were not selectable or introduced invalid font mappings. The adapter therefore
keeps one honest visual/text layer and makes no Preview-specific correctness
claim.

`und-oh0.9.3` remains the conformance follow-up: build a viewer matrix for
Acrobat, Chrome, Poppler, and PDFKit; decide whether tagged logical structure
belongs in this adapter; and separate standards-correct output from individual
viewer limitations. Underwood's own bidi selection remains represented by its
scene geometry and is not serialized as PDF editor state.

Krilla's selected minimal configuration supports outline and COLR glyph
programs. SVG glyphs need a renderer callback that is not configured here, and
embedded bitmap glyphs need the disabled `raster-images` feature. Non-default
variable-font instances and synthetic transforms are also outside this slice
and fail explicitly where the scene exposes them.

The remaining work is explicit in Beads:

- `und-oh0.9.2`: exact normalized variable-font locations;
- `und-oh0.9.3`: mixed-bidi extraction and viewer conformance;
- `und-oh0.9.4`: remaining paint, synthesis, SVG, and bitmap coverage;
- `und-oh0.9.5`: document-level pagination, links, tagging, and accessibility.
