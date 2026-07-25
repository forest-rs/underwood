<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `underwood_parley`

`underwood_parley` is the pinned, `no_std + alloc` Parley Engine adapter for
Underwood's pre-stable paragraph-formation contract. Its default feature set
accepts only caller-supplied font bytes and never enables system discovery. A
native host can explicitly enable `system-fonts` and call
`FontSet::with_system_fonts` to add one fixed Fontique platform-catalog snapshot
before constructing the paragraph engine. Linux uses Fontique's dynamic
Fontconfig loading so compiling the optional feature does not require
Fontconfig development headers; if the runtime library is absent, no system
fallback is added.

`FontSet::empty` supports missing-font proofs and system-font-only hosts.
`FontSet::registered_family_names` reports only sorted, deduplicated embedded
families, so the result is stable across machines. Registered bytes always use
shared blob backing. Enabling the explicit `std` feature additionally gives
Fontique's collection and source cache synchronized shared backing; the
`system-fonts` feature implies it. This makes clones suitable for constructing
many UI-local paragraph engines without catalog copies, font re-registration,
or repeated file loads. Default `no_std + alloc` builds retain shared font bytes
but clone Fontique's catalog records locally because Fontique's synchronized
stores require `std`.

All generic-family and fallback configuration must be completed before a
`FontSet` is cloned into paragraph engines. The set is then an application
resource snapshot: per-engine analyzers, shapers, query state, and paragraph
caches remain local, while the font universe stays coherent.

Implementation ownership is deliberately private and narrow:

- `engine` coordinates paragraph identities, invalidation, and retained physics;
- `font` owns immutable Fontique catalog construction and validation;
- `shaping` projects Underwood styles into Parley analysis, itemization, font
  selection, initial shaping, and line-final shaping with retained fonts;
- `line_former` owns reversible candidates, fit checks, retries, and
  checkpoints over Parley Engine cluster facts without importing Underwood
  document or scene types;
- `line_break` adapts Underwood constraints and line metrics to that reusable
  kernel, then owns line-local bidi ordering;
- `lowering` produces portable glyph, source, synthesis, and paint records;
- `interaction` produces source-complete grapheme units and cursor movement;
- `validation` rejects incomplete or non-canonical adapter inputs.

The crate root contains only documentation, private module declarations, and
the stable public re-exports.

The adapter owns analysis and shaping scratch, retains Parley Engine's native
`ShapedText` across reusable formations, and lowers it into Underwood's
portable formed-line records without maintaining a second shaped-run model.
`ParleyParagraphEngine::new(fonts)` is infallible; Unicode analysis data comes
from the pinned Parley Engine implementation rather than an empty configuration
placeholder. Paragraph physics are indexed by stable paragraph identity, and
Underwood's cache release and budget eviction propagate into this adapter so
dead blocks do not leave shaped text retained here.
Parley Engine boundary classes select legal and mandatory breaks. Explicit
max-content formation ignores soft opportunities, min-content formation
commits each legal opportunity through line-final re-itemization and shaping,
and constrained formation greedily fits a validated finite width. If a
line-final shape no longer fits, the line former rejects that candidate and
backs up to the preceding legal opportunity without advancing its traversal
cursor. Checkpoints restore both traversal and caller-owned provisional output.
Candidate, rejection, and restoration counts remain transient call work rather
than retained cache state. A single unwrapped line reuses the retained
canonical shape. Line boxes use the selected fonts' scaled metrics, and each
line's runs are reordered visually only after its logical source range is
fixed. Paint
boundaries remain source and clip metadata rather than shaping inputs. Complete
Underwood shaping runs supply family, weight, width, style, font size,
language, OpenType features, and variable-font coordinates.

`FontSet` is a deterministic Fontique catalog, not an Underwood matcher.
`FontSet::try_from_fonts` registers caller-owned memory fonts with system fonts
disabled; builders configure named generic families and script/language
fallbacks. The optional native-host builder adds platform fonts without making
them part of deterministic proof. For every itemized run, Fontique owns
attribute matching, coverage, fallback, and synthesis. The adapter performs
only the cluster callback needed to pass the selected `FontInstance` to Parley
Engine, then retains exact resource, synthesis, final normalized-coordinate, and
work evidence in Underwood values.

Parley stores shaped clusters in logical order. The adapter lowers LTR clusters
forward and RTL clusters backward so scene glyphs remain in visual order, and
applies line-local UAX #9 L2 run reordering for mixed-direction text. A
ligature glyph owns the complete source range represented by its start and
continuation clusters. Parley's `contributes_to_shaping` analysis identifies
controls and format characters which intentionally produce no glyphs; their
source remains explicit while shaping-only sentinel glyphs are discarded.

Formed lines also retain a separate visual interaction stream. The adapter
consumes Parley's `Analysis::is_grapheme_start` facts once, groups every shaped
record into its owning extended grapheme, and retains those records as ordered
visual slices. Ligature components remain separate interaction units, while
combining source, style-split marks, CRLF, whitespace, and controls stay
source-complete without internal caret stops. Unit sides map to explicit
paragraph-local boundaries and upstream/downstream affinities. Underwood can
therefore project exact semantic hits and caret stops without reconstructing
bidi direction from glyphs or using ink clips as interaction geometry. Soft
wraps retain both affinities for their shared logical boundary, and mandatory
break endpoints can occupy distinct lines.

Paint coverage records source-to-paint ownership, not universal glyph ink. A
glyph wholly owned by one paint run lowers without a per-glyph clip, leaving
outline, bitmap, color-graph, and synthesis extent to the renderer. A glyph
crossing paint runs returns `UnsupportedPaintCoverage` until a
conformance-backed component rule can provide explicit source-complete clips.
Advances and character counts are never substituted for component geometry.
Hit testing, carets, and selections use the separate interaction-unit stream.

Fontique synthesis variations precede explicit `ShapingStyle` variations at
the Parley Engine seam. An explicit coordinate therefore wins for the same axis.
Synthetic skew is retained for capable renderers and does not alter
Underwood's layout advances. Synthetic emboldening is likewise retained and no
longer fails preparation merely because outline-derived extent is unavailable.
Renderer support for either synthesis operation is an independent fidelity
capability and must not be inferred from successful preparation.
