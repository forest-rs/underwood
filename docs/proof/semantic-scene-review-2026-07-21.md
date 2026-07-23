# Semantic-scene Lynx and Rook review — 2026-07-21

> **Superseded coverage claim (2026-07-22):** Design-0007 deleted the measured
> ASCII/proportional ligature split described below. Design-0010 subsequently
> removed mandatory outline clips as incomplete paint truth. Live code draws
> ordinary source-complete glyphs unclipped and returns
> `UnsupportedPaintCoverage` when one glyph crosses paint runs. This review
> remains historical evidence for the first slice.

- **Scope:** Design-0002 implementation, external headless path, and product benchmark
- **Review modes:** Lynx adversarial correctness; Rook real-versus-mirage audit
- **Unsafe watch:** no `unsafe` in Underwood-owned Rust; no system-font feature
- **Result:** all Must findings resolved; Should and Could limits remain explicit
- **Remote gate:** GitHub Actions run `29820819717`; all eight required jobs passed
- **Proof effect:** first semantic-to-scene spine promoted from `Specified` to
  `Executable`

## Lynx findings

### Must — resolved

1. **Synthetic implementations were presented under `benches/`.** The four
   pre-product wind tunnels now live under `experiments/`, use test-class crate
   fences, and explicitly cannot support product performance claims.
   `benches/semantic-scene` calls only the real public product crates.
2. **Paint topology used a 64-bit digest as a correctness identity.** The
   retained core now compares exact source ranges and slots. A hash collision
   cannot reuse incorrect glyph coverage.
3. **A third-party adapter could return ranges inside a UTF-8 scalar.**
   `LayoutEngine` now validates every run, glyph, and paint segment against the
   projected string and leaf boundary before publishing. A malicious-adapter
   regression test proves rejection and diagnostic context.
4. **Font fallback was selected by script and list position.** The Parley
   adapter now evaluates each grapheme cluster against each validated font's
   real character map and reports `MissingFont` when none covers it.
5. **Fragment identities collided across documents.** Document identity now
   participates in the opaque retained fragment identity, with a two-document
   regression test.
6. **Prepared paragraph validation allowed gaps between runs.** Checked
   construction now requires exact contiguous paragraph coverage; a regression
   test covers the former gap.
7. **Multi-paint coverage implied more fidelity than the adapter could prove.**
   The first slice supports measured ASCII ligature components, including the
   required `ffi` boundary. It explicitly returns
   `UnsupportedPaintCoverage` for multi-paint clusters whose partition cannot
   yet be justified, including combining and non-ASCII multi-source glyphs.
8. **Scene glyphs retained a font resource but not all data required to render
   its instance.** Fragments now expose font size and normalized variation
   coordinates along with font bytes, face index, glyph IDs, positions,
   advances, paint clips, and transforms.

### Should — explicit first-slice limits

- Flow is a deterministic finite-width greedy glyph-boundary breaker. It does
  not claim word-quality line breaking, pagination, regions, or hyphenation.
- A `SceneLine` exposes one representative snapshot range rather than a public
  cross-leaf authored range; durable and universal positions remain excluded.
- The retained engine keeps one geometry width per paragraph. Multi-width
  history and bounded cache policy remain later performance work.

### Could

- Add statistical sampling and machine baselines around the dependency-free
  benchmark runner once representative corpora and regression budgets are
  accepted.
- Add a dedicated renderer conformance consumer after a renderer boundary is
  approved.

## Rook audit

The implementation earns the capability it presents:

- `examples/headless` is an external workspace crate using only public paths;
- the test suite executes that full binary path with real licensed fonts;
- font character maps choose fallback, while pinned Parley performs analysis,
  bidi, itemization, and shaping;
- the scene retains render-relevant font-instance, glyph, geometry, paint,
  source, hit, caret, and semantic observations;
- one shaped `ffi` glyph is observed through multiple paint clips without a
  shaping split;
- work reports are asserted for old-snapshot, sibling, paint-only, and
  width-only behavior;
- the only product benchmark executes the production crates and fails if those
  work invariants regress.

The following are intentionally not claimed: a production-quality line
breaker, durable source anchors, arbitrary paint partitioning, a renderer,
system fonts, a complete text-data provider, stable IDs, or a wire format.
Those absences are visible in public types, documentation, and errors rather
than concealed behind placeholder APIs.
