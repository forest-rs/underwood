<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# CJK line-breaking review

- **Date:** 2026-07-25
- **Bead:** `und-oh0.13.10`
- **Design:** Design-0014 and ADR-0005
- **Scope:** ordinary CJK line opportunities, Japanese kinsoku traps, authored
  word-break policy, optional dictionary data, reusable formation, public
  scene preparation, and exact non-claims

## First read: result

Underwood's retained path now has executable CJK line-breaking evidence rather
than a general promise. The real Parley Engine analysis and Underwood line
former agree on:

- Japanese closing and opening punctuation;
- small kana and iteration marks;
- ideographic space;
- hiragana, katakana, Chinese, and Korean runs;
- mixed Latin and Han text;
- emoji ZWJ grapheme atomicity;
- mandatory breaks; and
- `WordBreak::Normal`, `BreakAll`, and `KeepAll`.

The public `LayoutEngine` path additionally proves that normal Han text exposes
an inter-ideograph min-content opportunity while `KeepAll` suppresses it. That
test uses the native macOS fallback catalog because the deterministic bundled
fonts intentionally contain no Han face. The portable analyzer and line-former
corpus remains independent of installed fonts.

This slice adds no dependency and no `unsafe`. It exposes the existing Parley
Engine `complex-scripts` feature through `underwood_parley`, but keeps it
non-default because its data cost is material.

## What the tests mean

| Trap | Expected first legal result |
| --- | --- |
| `漢。字` | break after `。`, never before it |
| `漢「字` | break before `「`, never after it |
| `漢ゃ字` | small kana stays with the preceding ideograph |
| `漢々字` | iteration mark stays with the preceding ideograph |
| `漢　字` | break after the ideographic space |
| `かなカナ` | ordinary kana boundaries remain available |
| `你好世界` | ordinary Chinese ideograph boundaries remain available |
| `한글문자` | ordinary Korean syllable boundaries remain available |
| `abc漢字` | the Latin/Han seam and Han boundary remain available |
| `漢👩‍💻字` | no break occurs inside the emoji ZWJ sequence |
| `漢\n字` | the mandatory break survives `KeepAll` |

The punctuation assertions are byte-exact. `reusable_line_formation_commits_only_analyzed_cjk_boundaries`
passes the real analysis into the production line former and proves that
min-content commits `漢。` as the first line, then `字`. It does not substitute
a handwritten breaker or infer behavior from glyph widths.

## Ownership and support matrix

| Capability | Current state | Owner |
| --- | --- | --- |
| ordinary Unicode CJK line opportunities | executable | Parley Engine ICU4X analysis |
| `Normal`, `BreakAll`, `KeepAll` projection | executable | Underwood computed analysis style |
| resumable candidate formation | executable | `underwood_parley` line former |
| source-complete scene geometry | executable | Underwood scene preparation |
| dictionary data selection | explicit, non-default | `underwood_parley/complex-scripts` |
| locale-specific line tailoring | not represented | future analysis-policy work |
| dictionary-quality CJK word navigation | blocked by merged upstream facts | `und-oh0.2.11` |
| CJK justification and punctuation compression | not claimed | future line adjustment |

Unicode line breaking, browser compatibility, and layout adjustment are
different claims. [Unicode Standard Annex #14] defines legal line-break
opportunities; it does not choose a line, expand it for justification, or make
Chromium the conformance oracle. [CSS Text Level 3]'s `word-break` values are
authored policy over those opportunities. Underwood keeps each at its owning
stage.

[Unicode Standard Annex #14]: https://www.unicode.org/reports/tr14/
[CSS Text Level 3]: https://www.w3.org/TR/css-text-3/#word-break-property

## Second read: implementation detail

### One analysis source

`AnalysisStyle` is projected across semantic text leaves and becomes Parley
Engine `AnalysisOptions::word_break` ranges. A change invalidates analysis,
itemization, font selection, shaping, and formation. Underwood does not run a
second Unicode line segmenter and does not maintain a kinsoku table.

The portable tests inspect Parley Engine's published boundary facts and then
feed those same facts, plus shaped advances, through `collect_logical_clusters`
and `choose_line`. The native-fallback test executes
`LayoutEngine::prepare`, Fontique selection, Parley Engine shaping, reusable
formation, and scene geometry.

### Optional dictionary data

Without `complex-scripts`, Parley Engine constructs ICU4X segmenters for
non-complex scripts. With it, Parley Engine constructs dictionary-backed word
and line segmenters. Ordinary CJK line opportunities and the three
`WordBreak` policies pass in both configurations. The feature is needed for
dictionary-sensitive segmentation and contextual breaking in scripts such as
Thai, Lao, Khmer, and Myanmar.

The same locked release headless example measured:

| Build | Binary bytes |
| --- | ---: |
| default adapter features | 3,952,176 |
| `underwood_parley/complex-scripts` | 7,805,504 |
| delta | 3,853,328 (3.67 MiB, +97.5%) |

This is a single local linked-binary comparison, not a universal download-size
forecast. Linker, platform, optimization, compression, and which other
features already retain ICU data all affect a product binary. It is enough to
reject a hidden default cost.

### The boundary-fact limit

Parley Engine currently computes word, grapheme, and line boundary positions
independently, then publishes one `Boundary` value per character with line
boundaries taking precedence over word boundaries. A dictionary word boundary
that coincides with an ordinary CJK line boundary is therefore not observable
as a word fact downstream.

That representation is sufficient for line formation. It is insufficient for
an honest claim that Underwood's word-movement methods follow dictionary words
in CJK. Enabling dictionary data cannot recover a fact erased by the published
representation. `und-oh0.2.11` tracks a small upstream Parley Engine change to
preserve independently observable word and line facts, followed by Underwood
adapter consumption. No local duplicate segmenter is planned.

### Locale and language

Parley Engine's shaping options accept language, but its current
`AnalysisOptions` expose base direction, word-break ranges, and a line-break
override—not a locale input for ICU line tailoring. Underwood therefore makes
no locale-tailored Japanese strictness claim. A shaping language must not be
misrepresented as segmentation locale.

### Chromium corpus

Parley's browser recorder is valuable methodology: record the first-line
source boundary and the smallest preserving width, then replay a deterministic
corpus. Its current generated corpus is printable ASCII and answers Chromium
compatibility questions. Underwood borrowed the source-boundary style of
assertion, not the browser as a Unicode oracle and not another test dependency.
A later CSS Text compatibility campaign can extend the recorder independently.

## Public migration

The `complex-scripts` feature is additive and disabled by default. Existing
call sites and default binaries keep their behavior and data footprint. Hosts
that need dictionary-sensitive segmentation enable:

```toml
underwood_parley = { features = ["complex-scripts"] }
```

This does not yet upgrade Underwood CJK word navigation; that requires the
separate boundary-fact work above. It does upgrade the data available to the
existing Parley Engine analyzer without changing Underwood's API types or
adding another engine.

## Glossary

- **Kinsoku:** Japanese rules that prohibit particular characters at the
  beginning or end of a line.
- **Line opportunity:** a legal source boundary where a line may end; it is
  not a command to break there.
- **Word-break policy:** authored `Normal`, `BreakAll`, or `KeepAll` behavior
  that changes legal line opportunities.
- **Dictionary segmentation:** data-backed boundary selection for scripts
  whose useful words or line opportunities cannot be derived only from spaces.
- **Locale tailoring:** language- or locale-specific changes to otherwise
  generic segmentation rules.
- **Formation:** Underwood's selection and commitment of lines from analyzed,
  shaped facts under width or region constraints.

## Evidence

- `underwood_parley/src/tests/cjk_line_break.rs`
- `underwood_parley/src/line_former.rs`
- `underwood_parley/src/line_break.rs`
- `underwood_parley/src/shaping.rs`
- `underwood_parley/Cargo.toml`
- `underwood_parley/README.md`
- `und-oh0.2.11`

The focused suite passes with default features and with all features. The full
workspace and portability matrices remain the landing gates rather than being
inferred from these focused tests.
