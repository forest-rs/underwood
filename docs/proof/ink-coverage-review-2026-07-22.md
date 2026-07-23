# Ink-coverage conformance review — 2026-07-22

> **Historical result, superseded by Design-0010:** This review proved that
> advance rectangles and proportional ligature splits were wrong, and it fixed
> the Arabic-dot snapshot. It did not prove that outline bounds are complete
> painted-ink truth. Native Han fallback with synthetic emboldening exposed the
> mandatory outline clip as a preparation bug. Ordinary glyphs now render
> without a per-glyph clip; this record remains evidence for the original
> failure and the abandoned Parley ink candidate.

- **Scope:** Parley Core ink-metrics candidate, `underwood_parley` lowering,
  public headless path, semantic-scene benchmark, and CPU visual proof
- **Design:** Design-0007
- **Bead:** `und-oh0.2.4`
- **Parley candidate:**
  [`d12c801d8fd298ff095f1ec903b6adaa732fcef2`](https://github.com/waywardmonkeys/parley/commit/d12c801d8fd298ff095f1ec903b6adaa732fcef2)
- **Parley branch:**
  [`waywardmonkeys/ink-coverage-primitives`](https://github.com/waywardmonkeys/parley/tree/ink-coverage-primitives)
- **Snapshot:** 1600 × 1000 RGBA8, PNG SHA-256
  `be7eaf53d4cccd72ae253433ffe7ee82a74c431a70f74c8c13787110499c4a11`
- **Remote gate:** GitHub Actions run
  [`29900013909`](https://github.com/forest-rs/underwood/actions/runs/29900013909);
  all eight jobs passed on PR #10
- **Unsafe watch:** no `unsafe` added to Underwood or the Parley candidate
- **Dependency watch:** existing transitive `libm` promoted to a direct
  Underwood edge; no package added to the dependency graph

## Result

The malformed Arabic rendering is fixed at the renderer-neutral boundary.
Noto Kufi's zero-advance dot glyph now has a non-empty clip derived from its
real outline, the CPU renderer produces exact gold pixels inside that clip, and
the committed poster visibly contains the dots. A Roboto Flex `j` proves that
ink can also extend outside the shaped advance.

The old ASCII character-count split is deleted. `ffi` substitution and complete
source ownership remain executable when one paint owns the glyph. A paint
boundary inside that glyph now returns the stable
`UnsupportedPaintCoverage` category instead of publishing invented component
geometry.

## Failure chain and repair

The independent Arabic review correctly identified missing dots. Source text,
bidi order, font fallback, and shaping were already correct: Noto Kufi emits a
dotless body and a separate visible dot whose advance is zero. The adapter then
made an advance-width clip, scene lowering preserved the empty rectangle, and
the imaging adapter correctly clipped the dot away.

Candidate `d12c801` moves the reusable fact to Parley Core:

- `RunGlyphMetrics` queries many glyphs after parsing a run's exact font once;
- bounds use the shaped size and normalized variation coordinates;
- a valid no-outline glyph returns empty bounds;
- an out-of-range glyph returns `None`; and
- the API remains `no_std + alloc` through the `libm` feature path.

`underwood_parley` maps those bounds into Underwood coverage, applies the same
canonical `FontSynthesis::skew_transform` as the renderer, and keeps paint
ownership outside Parley.

## Lynx review

### Must — resolved

1. **Advance was treated as ink.** Zero-advance Arabic marks and Latin
   overhangs now have structural public-path regressions.
2. **A source-count ratio was presented as ligature truth.** The approximation
   is gone; the unsupported path has an exact error assertion.
3. **Variable coordinates were accepted but not proven.** The Parley candidate
   shapes the same Roboto Flex `H` at `wght=100` and `wght=900` and requires
   different queried bounds.
4. **Coverage and rendering could disagree about faux oblique math.** Both now
   call the same portable `FontSynthesis::skew_transform`.
5. **The new float path compiled only when `std` happened to be present.** The
   optimized build exposed the defect; shared Kurbo `libm` math now keeps the
   foundational crates `no_std`. Underwood calls the already-present `libm`
   package directly because Kurbo's compatibility trait disappears when a
   workspace renderer unifies Kurbo's `std` feature.
6. **A pixel snapshot alone could preserve a malformed render.** The poster
   requires an exact gold pixel inside the structurally identified mark clip in
   addition to matching the committed RGBA image.

### Explicit limits

- Synthetic emboldening is rejected because Fontique does not expose the
  expansion needed for trustworthy bounds.
- A glyph crossing paint runs is rejected. The bundled Roboto Flex and Roboto
  Regular fonts have no GDEF `LigCaretList`, and caret data would still require
  an accepted paint-ownership rule.
- Outline rectangles are conservative paint clips, not outline paths or hit
  shapes.

## Rook audit

### Real

- Parley Core owns reusable font-instance metric lookup without requiring
  high-level layout.
- Underwood retains document, paint, scene, and invalidation policy.
- Arabic mark coverage, Latin overhang, mixed bidi, variable instances,
  `liga=1/0`, exact source ownership, and explicit unsupported behavior all run
  through production crates.
- The poster uses the public Underwood → Parley Core → `TextScene` → imaging →
  `imaging_vello_cpu` path and asserts actual output pixels.

### Not claimed

- Underwood does not yet conformantly color separate logical components of one
  ligature glyph.
- GDEF caret positions are not claimed to be exact outline-component geometry.
- The example adapter is not a general renderer package.
- This work does not make Underwood's document flow or retained cache model a
  Parley responsibility.

`und-oh0.2.8` is the explicit removal gate for the first limit. It requires a
normative or cross-engine oracle before enabling any supported case.

## Product-path observation

One local optimized run of the 64-paragraph semantic-scene benchmark produced:

```text
cold_scene            6,009,156 ns/iteration
retained_unchanged       95,282 ns/iteration
paint_only               92,911 ns/iteration
width_only            4,710,644 ns/iteration
one_paragraph_edit      179,450 ns/iteration
```

These are a smoke observation, not accepted performance thresholds. Every
workload asserted its retained work counters; importantly, paint-value changes
still perform zero shaping and zero flow work after ink-accurate coverage.

## Validation

The Parley candidate passes 23 `parley_core` tests plus its doctest,
`clippy -D warnings`, and `no-default-features + libm` checking. Underwood passes
workspace formatting, all-target/all-feature clippy with warnings denied, and
the complete workspace test suite including the external headless example,
product benchmark, and exact visual snapshot. Repository policy, rustdoc, Rust
1.92 workspace checking, and `x86_64-unknown-none` plus
`wasm32-unknown-unknown` portability checks also pass locally. PR #10 repeats
the complete suite successfully on Linux, macOS, and Windows.

## Migration

Adapter consumers must handle `PreparationErrorKind::UnsupportedPaintCoverage`
when one shaped glyph crosses paint runs or synthetic emboldening lacks exact
bounds. Renderers and adapters should replace local skew-angle conversion with
`FontSynthesis::skew_transform`; `skew_degrees` remains available.
