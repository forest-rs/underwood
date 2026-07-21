# Parley paragraph-breaking seam audit — 2026-07-22

## Result

The current Underwood breaker is provisional and must be deleted. Pinned Parley
Core already supplies owned logical clusters, Unicode line/mandatory boundary
classes, bidi levels, glyphs, and scaled font metrics. It does not yet supply
bounded break/concat reshaping. High-level Parley supplies mature line formation
but cannot accept Underwood's retained `ShapedText` through a public seam.

The viable architecture is therefore a narrow Core-backed formation policy in
`underwood_parley`, portable formed-line output, high-level Parley as a private
oracle, and a precise upstream request for break-sensitive reshaping.

## Revisions inspected

- Underwood base: `888214701f6770c600b699df3ba56521beae1a5e`
- Parley main: `6c81e1dd9b67793cdd959c65cc650c96a1262fb7`
- Draft reshape design: [Parley PR #634](https://github.com/linebender/parley/pull/634),
  head `d222b7ce9a297d495f4cc11b01e5ee61a023acd3` at audit time

## Current product behavior

`underwood/src/scene.rs::build_geometry` iterates prepared glyphs, takes the
absolute horizontal glyph advance, and flushes immediately before whichever
glyph first exceeds the requested width. It does not inspect Unicode break
opportunities, mandatory breaks, whitespace, graphemes, or line-local bidi.
For every contributing glyph it derives a requested line height and assigns
80% above the baseline and 20% below it.

Consequences:

- ordinary words, graphemes, and shaped units may break at illegal positions;
- newline controls have no glyph and therefore cannot create an explicit line;
- CRLF behavior is undefined;
- line baselines ignore actual font ascent/descent;
- mixed bidi is flattened before line ranges are known;
- no chosen break can trigger bounded reshaping;
- the scene module silently owns text physics.

## Available Core evidence

At the pinned revision:

- `ClusterData.info.boundary()` returns `None`, `Word`, `Line`, or `Mandatory`;
- cluster storage is logical even for RTL runs;
- glyph storage is visual and Underwood already reverses logical RTL clusters
  when lowering;
- `ShapedRun` retains source ranges, bidi level, total advance, font instance,
  normalized coordinates, font size, and scaled `FontMetrics`;
- a control-only run can validly contain no glyphs.

These primitives are sufficient for legal greedy selection, explicit breaks,
font-derived line boxes, line-local visual ordering, and retained width-only
formation. They are not sufficient to commit an unsafe break correctly.

## High-level Parley evidence

`parley::Layout::break_lines` / `break_all_lines` provide:

- legal and mandatory greedy breaks;
- CRLF coalescing;
- trailing-whitespace and overflow handling;
- font/line-height box accumulation;
- per-line bidi run reordering;
- resumable in-process `BreakerState` and variable line geometry.

The breaker operates on private `LayoutData`, `RunData`, line items, and style
records. There is no constructor from a caller-owned Core `ShapedText`.
Production adoption would therefore shape twice or replace Underwood's
paragraph boundary with high-level Parley's entire style/layout model.

The executable oracle also found one policy divergence worth preserving. At a
one-unit width, pinned high-level Parley forms `alpha\u{00A0}|beta`: its
overflowing-whitespace path treats NBSP like a hanging space and emits a
regular break after it. Underwood records this exact observation but does not
adopt it as the correct policy; Core's boundary classification remains the
source of legal opportunities. This is a concrete reason the high-level path
is an oracle, not a golden implementation.

## Draft PR #634 evidence

The draft describes caller-owned wrapping over Core clusters followed by
`ShapeContext::apply_break`, and `apply_concat` to restore a removed seam. Its
tests cover Arabic joining and splitting a Latin `fi` ligature. The current
main API has neither unsafe-region flags nor these operations. The PR is a
useful contract source, not a production revision Underwood can pin wholesale:
it is a 67-file experimental umbrella based on older main.

## Decision

Proceed with Design-0006's formed-line contract and every capability supported
by current main. Keep the campaign branch and bead open until the bounded
reshape slice lands upstream or an explicitly reviewed narrow patch is carried
under ADR-0004's lifecycle rules. Do not merge a safe-break-only implementation
under the full capability name.
