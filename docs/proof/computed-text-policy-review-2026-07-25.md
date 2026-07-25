<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Computed text-policy review

- **Date:** 2026-07-25
- **Bead:** `und-oh0.13.5`
- **Design:** Design-0014, Design-0015, ADR-0005
- **Scope:** line height, spacing, word breaking, emergency wrapping, stage
  ownership, retained invalidation, and the exact Overstory lowering boundary

## Result

The public document and `TextBlock` paths now carry complete computed policy
for the Overstory label gap:

| Policy | Representation | Execution owner |
| --- | --- | --- |
| metrics-relative line height | `LineHeight::metrics_relative` | line metrics |
| font-size-relative line height | `LineHeight::font_size_relative` | line metrics |
| absolute line height | `LineHeight::absolute` | line metrics |
| letter and word spacing | `TextSpacing` | retained shaping policy and shaped advances |
| word-breaking policy | `AnalysisStyle` | Parley Engine analysis |
| ordinary versus emergency wrapping | `InlineFlowStyle::overflow_wrap` | candidate formation |
| wrap versus no-wrap | `InlineFlowStyle::text_wrap_mode` | candidate formation |

Analysis values are projected and interned separately from shaping, inline
flow, and paint. `ParagraphInput` exposes each partition explicitly. A
`FormationKey` includes the projected analysis values and ranges, so source
style changes cannot reuse analysis from a semantically different paragraph.

## Spacing and cursive safety

Spacing is applied after canonical shaping and before line formation. The
adapter retains the unspaced cluster and glyph advances, so changing only a
nonzero spacing amount restores and adjusts those facts without another shape
or font query. Moving between zero and nonzero letter spacing reshapes with
the already selected fonts because optional `liga` and `clig` behavior can
change.

Authored OpenType feature values take precedence over the automatic optional
ligature policy. Joining text does not receive spacing between joining
grapheme units and does not have optional ligatures disabled merely because
tracking is nonzero.

The joining decision is data-derived. Parley Engine integration commit
`97b874719f810c375025f3fa727b245530a87f9f` retains the Unicode
`Joining_Type` property in its compact character facts and exposes it through
`CharInfo`. Underwood consumes those facts at grapheme-unit granularity. The
test corpus includes Latin, Arabic with a transparent mark, and Manichaean;
there is no script-tag allowlist.

This is script-safe executable Underwood behavior, not a complete CSS
letter-spacing claim. In particular, CSS line-edge spacing and every
mixed-bidi placement case are not yet claimed. Those belong in the selected
CSS-profile corpus rather than being implied by the API name.

## Exact invalidation

The retained adapter compares semantic values across differently interned run
partitions rather than treating table identities as behavior:

| Change | Analysis | Font query | Shape | Formation |
| --- | ---: | ---: | ---: | ---: |
| `WordBreak` | yes | yes | yes | yes |
| zero ↔ nonzero letter spacing | no | no | retained-font reshape | yes |
| nonzero spacing amount | no | no | no | yes |
| word spacing | no | no | no | yes |
| line-height basis or value | no | no | no | line metrics only |
| wrap/no-wrap | no | no | no | yes |
| `OverflowWrap` | no | no | no | yes |
| paint | no | no | no | no |

The public `WorkReport` assertions make those boundaries observable. A
line-height change reuses accepted line glyphs. `OverflowWrap::Anywhere`
changes constrained wrapping and min-content width; `BreakWord` changes only
constrained emergency wrapping. `NoWrap` suppresses both ordinary and
emergency soft breaks while mandatory breaks remain.

## Product-path evidence

The focused regressions execute through `LayoutEngine::prepare`, real
Fontique selection, Parley Engine analysis and shaping, reusable line
formation, and portable scene geometry:

- `word_break_is_range_projected_and_invalidates_from_analysis`
- `all_line_height_bases_recompute_metrics_without_reshaping`
- `spacing_reuses_fonts_and_keeps_joining_text_connected`
- `wrap_and_overflow_policy_reach_product_formation`
- `wrap_policy_distinguishes_soft_emergency_and_intrinsic_breaks`
- `joining_units_come_from_unicode_data_not_script_lists`

The existing Arabic joining, marks, ligature components, mixed bidi, CRLF,
intrinsic sizing, source mapping, editing, PDF, and paint regressions remain
green. Empty text selects no font, so metrics-relative empty-block height uses
the computed font size as an explicit deterministic fallback until a future
paragraph strut owns selected font metrics.

## Overstory consumer proof

The parked Overstory checkpoint
`75e22e5d0c4141767d131d237e781bc5ee1ac16f` was checked in a disposable
worktree against this Underwood worktree. After patching both consumers to one
Parlance source, its real `computed_style` call site type-checked with:

```rust,ignore
let line_height = match style.line_height {
    TextLineHeight::MetricsRelative(value) => LineHeight::metrics_relative(value),
    TextLineHeight::FontSizeRelative(value) => LineHeight::font_size_relative(value),
    TextLineHeight::Absolute(value) => LineHeight::absolute(value),
}?;
let flow = InlineFlowStyle::new(line_height)
    .with_spacing(TextSpacing::new(style.letter_spacing, style.word_spacing)?)
    .with_overflow_wrap(style.overflow_wrap)
    .with_text_wrap_mode(style.text_wrap_mode);
let computed = ComputedInlineStyle::new(shaping, flow, PaintSlot::new(0))
    .with_analysis(AnalysisStyle::new(style.word_break));
```

No conversion enum or substitute text engine is required. The editable
`TextBlock` operations tracked in `und-oh0.13.9` have since landed locally and
the complete parked library type-checks after its two stale Understory
presentation patterns are updated. The disposable proof is not a maintained
fork and contains no production change. See
`docs/proof/editable-text-block-operations-2026-07-25.md` for the current
consumer result.

## Browser and specification follow-up

Parley's `parley_tests/linebreaking_browser_recorder` records Chromium's first
line boundary and the tightest preserving width for deterministic generated
cases. Its current corpus is printable ASCII and primarily exercises browser
line-breaking equivalence classes. `und-oh0.13.10` records extending that
model to CJK, `WordBreak`, and `OverflowWrap` at Underwood's public formation
seam.

That evidence must keep three claims separate:

1. Unicode and selected specification conformance;
2. Chromium compatibility;
3. deliberate Underwood semantics or deviations.

## Public migration

This is an approved pre-stable public API migration:

- `LineHeight::from_multiplier(value)` remains available and now explicitly
  means font-size-relative line height.
- The removed `LineHeight::multiplier()` accessor becomes `basis()` plus
  `value()`; callers that need a resolved scalar use
  `resolve(font_size, metrics_height)`.
- `LineHeight::NORMAL` changes from a `1.25 × font-size` policy to
  `1.0 × preferred-font-metrics`.
- Existing `ComputedInlineStyle::new(shaping, flow, paint)` calls remain
  source-compatible and default to `WordBreak::Normal`.
- Hosts add analysis policy with `with_analysis` and add spacing/wrapping with
  the `InlineFlowStyle` builders.

There is no compatibility layer that folds analysis policy into shaping or
turns absolute line height back into a multiplier.

## Dependency and portability boundary

The workspace changes only the existing Fontique, Parlance, and Parley Engine
revision together. It adds no production dependency and no `unsafe`. The
Unicode property still fits in Parley's existing compact data word. The core
implementation uses `alloc` and remains available with default features
disabled.

## Gate results

The full workspace gate first exposed a retained-cache error: failed font
selection cleared shaped output but left the prior successful shaping key,
allowing a later paint-only request to report false reuse. The adapter now
invalidates that key before fallible shaping, and the existing headless
public-path regression proves the recovery reshapes exactly one paragraph.

Changing `LineHeight::NORMAL` intentionally moved specimens that had relied on
the default `1.25 × font-size` policy. The committed CPU poster was regenerated
only after visual comparison showed that font-preferred baselines remained
clear, unclipped, and collision-free. The explicitly authored variable-font
specimen did not move.

The completed local matrix is green:

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
cargo +1.88.0 check --workspace --all-targets --all-features --locked \
    --exclude underwood_showcase \
    --exclude underwood_visual_proof \
    --exclude underwood_pdf \
    --exclude underwood_pdf_proof
cargo check -p underwood -p underwood_parley \
    --target x86_64-unknown-none --locked
cargo check -p underwood -p underwood_parley \
    --target wasm32-unknown-unknown --locked
```

`cargo tree -d` reports one coherent dependency universe.
