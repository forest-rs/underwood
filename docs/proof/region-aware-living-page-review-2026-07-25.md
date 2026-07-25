# Region-aware living page review — 2026-07-25

## Status

The complete reusable text-preparation path now has one coherent native product
proof and one deterministic headless artifact. The page is executable through
public Underwood document, style, region, preparation, scene, interaction, and
trace APIs; presentation remains in the external showcase crate.

This proof closes `und-oh0.13.12`. It does not promote the future living agent
document itself: the showcase is sustained evidence for that product scenario,
not the product.

## One document, one preparation path

The visible page is one `DocumentSnapshot`. It contains semantic headings and
body paragraphs, preserved and collapsed whitespace, mixed Latin/Arabic text,
Japanese/Chinese/Korean policy specimens, variable-font instances, OpenType
feature differences, actionable text, and an editable mixed-direction
paragraph. There are no renderer-painted strings and no separately shaped
cards.

```text
DocumentSnapshot + StyleMap + PaintTable
                    |
                    v
           responsive RegionFlow
                    |
                    v
       LayoutEngine + ParleyParagraphEngine
                    |
                    v
 TextScene + RegionTranscript + PreparationTrace
                    |
          +---------+---------+
          |                   |
          v                   v
 imaging_vello_cpu      exact interaction
 native/headless RGBA   selection/edit/IME/action
```

The showcase owns responsive page geometry and decorative art. Underwood owns
the exact line slots, resumable cursor, height rejection, adjustment, portable
scene geometry, and retained work. Imaging consumes the prepared scene without
repositioning text.

## Real flow

Every wide frame begins with a deliberately eleven-unit-high probe. The title
candidate is measured, rejected for height, and retried without losing source
progress. The transcript preserves both attempts.

The accepted document then:

- flows around a right float in the hero region;
- traverses three equal bounded columns at wide widths;
- traverses two equal bounded columns at medium widths;
- uses one long region at narrow widths;
- flows around an authored exclusion in the second column;
- continues into a distinct off-page overflow region if edits outgrow the
  visible wide-page columns.

Visible float and exclusion art is inset inside larger flow obstacles. The
gutter is therefore preparation geometry, not a painted margin that text can
invade. The regression
`living_page_consumes_retry_float_exclusion_and_all_wide_columns` proves the
initial height rejection, accepted text in every region, and reduced slots
intersecting both obstacles.

The first visual iteration exposed two Must-fix product defects: the float had
no text gutter, and a 1,600-unit escape region allowed the second column to run
through the page frame. The float now owns real flow padding, and every visible
column is bounded. A separate full-width continuation begins below the visible
columns; it is unused by the authored default page but prevents an edit or a
slightly taller platform font result from exhausting `RegionFlow` and killing
the host with `InvalidOutput`. The final default page uses three short balanced
columns and reports no clipping. A viewport too short for its flow or content
that reaches the continuation reports `CLIPPED`; scrolling is not claimed.

## Source and script evidence

The source/presentation specimen executes both whitespace policies. Its
collapsed leaf authors a space, tab, newline, and multiple spaces. The prepared
paragraph forms one visual line while its line provenance covers the complete
authored byte range. No source byte is discarded merely because presentation
uses one visible space.

The showcase enables the adapter's explicit `complex-scripts` feature and
executes:

- Japanese `WordBreak::Normal`;
- Chinese `WordBreak::BreakAll`;
- Korean `WordBreak::KeepAll`.

All three semantic leaves resolve their language-matched bundled proof font and
produce non-`.notdef` glyphs. The assets are small subsets of the official Noto
Sans CJK JP, SC, and KR Regular 2.004 faces. Their official repository, release
tag, commit, source paths, source and subset SHA-256 values, and exact OFL are
checked in at
`examples/showcase/assets/fonts/README.md`. No sibling checkout or developer
machine path is part of the provenance.

## Guided diagnostics

`F4` cycles:

1. flow slots and the rejected probe;
2. accepted line boxes;
3. shaped fragments colored by script and bidi level;
4. glyph origins and multi-source ownership;
5. semantic geometry.

The overlays read `TextScene` and `RegionTranscript`; they do not alter
preparation. While a guide is active, the title reports cold, formation,
adjustment, and paint invalidation counts; region attempts and height
rejections; scene-output and cache capacity; and scratch growth. Preparation
and rendering retain separate host clocks.

The normal live path still proves paint-only reuse, variable-axis
paragraph-local invalidation, local edit isolation, exact semantic activation,
visual bidi selection, independent carets, revision-rebound transactions,
extended-grapheme movement/deletion, and transient IME composition.

## Deterministic artifact

The same showcase content renders headlessly at 1100×800 with bundled fonts
only:

```sh
cargo run --release -p underwood_showcase -- --write-snapshot
```

The committed artifact is
`examples/showcase/snapshots/underwood-living-page.png`. A focused test renders
the same public path and compares every RGBA byte. It therefore catches font,
line-break, region, paint, and presentation drift rather than merely checking
that an image file exists.

Native release inspection on this macOS host reported approximately 0.6 ms of
preparation and 4.6–4.8 ms of CPU rendering for the default 1100×800 frame.
Those title-bar observations are product responsiveness evidence, not a
cross-platform benchmark. The preparation and label wind tunnels remain the
performance authorities.

## Exact non-claims

The page deliberately does not claim:

- `uppercase`, `lowercase`, `capitalize`, full-width, or locale-tailored text
  transformation;
- leading/trailing whitespace trimming, `break-spaces`, tab sizing, or a full
  CSS Text whitespace profile;
- hanging punctuation, optical margin alignment, Japanese kinsoku compression,
  or locale-specific CJK adjustment;
- CJK inter-character justification or Arabic kashida/tatweel generation;
- dictionary quality beyond the enabled Parley Engine data;
- general CJK font coverage from the proof-only subset;
- pagination, scrolling, native accessibility projection, or widget policy;
- universal PDF extraction behavior from the native raster proof.

Western U+0020 expansion is the only justification policy demonstrated.
Authored Arabic tatweel remains ordinary source text when present; Underwood
does not synthesize it.

## Validation

Focused validation:

```sh
cargo fmt --all -- --check
cargo clippy -p underwood_showcase --all-targets --all-features -- -D warnings
cargo test -p underwood_showcase --all-features
cargo run -p underwood_showcase --release -- --write-snapshot
```

The focused suite passes 40 tests. Full workspace, MSRV, portability, rustdoc,
repository, Beads, PDF, and remote protected checks belong to the final
`und-oh0.13.13` campaign review.
