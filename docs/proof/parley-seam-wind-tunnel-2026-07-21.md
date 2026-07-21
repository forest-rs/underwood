# Retained-Parley seam wind-tunnel evidence — 2026-07-21

- **Capability:** Parley alignment
- **Bead:** `und-oh0.10.1.4`
- **Implementation commit:** `4b43ca2`
- **Upstream:** Parley `45da4a90248b1600277a4294b70d8bfde5ca8e97`
- **Path:** current upstream `parley_core`; no patch
- **Proof effect:** evidence at `Specified`; no promotion to `Executable`

## Reproduction

```sh
cargo test -p underwood_parley_seam_experiment
cargo run -p underwood_parley_seam_experiment
```

The recorded run used Rust 1.96.0
`ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96`, Cargo 1.96.0, and macOS
26.5.2 arm64. The implementation commit's Cargo lockfile SHA-256 was
`f042b13ce4bec4d5f2674b7e36f50415b405a55076a6efe0fc7ea949e1851fc2`.

The executable emitted:

```text
parley=45da4a90248b1600277a4294b70d8bfde5ca8e97 analysis=cfdddc2f29b0a50b items=f92022fbdf3a152a physics=e88c05c9f03ac26e slots=f7b2494a3759e8a3 paint=b118c17a87887d7a items_count=2 runs=2 glyphs=29 gaps=5
fonts=RobotoFlex-VariableFont.ttf:aecc087879796a24,NotoKufiArabic-Regular.otf:4e4306d5a30a1500
```

The compact hexadecimal values are deterministic FNV fixture checksums. They
are not cryptographic source, cache, or supply-chain identities.

## Fixture identity and licensing

The crate loads both fonts through `parley_dev::font_dirs()` from the exact
locked git source. No font binary is copied into Underwood.

| Asset in pinned `parley_dev` | SHA-256 | Recorded license |
| --- | --- | --- |
| `RobotoFlex-VariableFont.ttf` | `2bd17942bba38048b2f86e00cdd6fb2242035c0b2c843d5979bb24f13d5512ef` | Apache-2.0 |
| `LICENSE-RobotoFlex-VariableFont.txt` | `9ad8709b97c6b9f8e241a44ce0c2c4a81ac09a8954ff00da740ac623af89dd6e` | license text identity |
| `NotoKufiArabic-Regular.otf` | `da737b6c2187af99294269aa5e439dca28c48e70f4a95755e705300931fa2540` | SIL OFL-1.1 |
| Noto `LICENSE.txt` | `fa27a4641d00020ca4522b71d7e9d2eddafcd5031a2e55b4cfb3eedfe33a865a` | license text identity |

This is a source and license inventory, not a legal opinion or approval for a
future production font bundle.

## Executed conformance slice

Six deterministic tests execute against the actual pinned callbacks and copy
their borrowed results into private records containing only owned source
ranges, bidi/script observations, font fixture identity, normalized variation
coordinates, glyphs, advances, offsets, clusters, and paint-slot coverage.

| Law | Observation | Verdict |
| --- | --- | --- |
| Callback ownership | Two independent preparations compare equal after every Parley callback and borrow has ended | **PASS** for private copy-out |
| Mixed-direction itemization | The corpus emits a Latin LTR item and an Arabic RTL item with distinct selected fonts | **PASS** for this corpus |
| Source coverage | Items tile all source bytes; every glyph maps to a nonempty in-range byte/character span | **PASS** for this corpus |
| Weight invalidation | Roboto Flex `wght=700` preserves analysis and item digests while changing normalized coordinates and shaped physics | **PASS** |
| Feature invalidation | Disabling `kern` over `AV` preserves analysis and item digests while changing shaped physics | **PASS** |
| Paint-value separation | Changing only the paint table changes lowering but not analysis, items, glyphs, or advances | **PASS** |
| Paint-topology separation | Splitting slots inside a real Roboto Flex ligature and Arabic cursive text leaves the physics digest unchanged | **PASS** for this corpus |
| Ligature slot coverage | One retained glyph spans multiple source characters and carries both source slots `[0, 1]` | **PASS** for private reconstruction |

The paint-slot observation is an Underwood-side reconstruction from shaped
glyph cluster/source ranges. Current `ShapedRun` does not retain
`char_style_indices` as owned shaped metadata. This proves that the adapter can
recover the required coverage for the corpus; it does not claim that current
Parley supplies the final retained paint-slot contract.

## Current-main seam matrix

| Required seam | Current observation | Status |
| --- | --- | --- |
| Owned Unicode analysis and bidi | `Analysis` is retained and reusable | exercised, limited corpus |
| Itemization | source ranges, bidi level, and script are observed | exercised, limited corpus |
| Font selection | callback selects one licensed fixture per script | exercised; fallback not tested |
| Horizontal shaping | glyphs, clusters, positions, and coordinates are copied from callback-borrowed runs | exercised; copy required |
| Retained shaped output | `ShapedRun` remains callback-borrowed | **GAP** |
| Bounded break reshaping and concat equivalence | no current-main seam | **GAP** |
| Vertical shaping and orientation | no current-main input/output contract | **GAP** |
| Core inline objects | no first-class core itemization contract | **GAP** |
| Text-data capability/content identity | no injectable identified provider seam | **GAP** |

The executable keeps those five gaps in an exact enum-backed matrix, with a
test that fails if they are silently removed or renamed. Closing a gap requires
new executable evidence, not deletion of the assertion.

## Architectural result

The current core seam is sufficient to begin private analysis, itemization,
font-selection, and horizontal-shaping integration work. It is not sufficient
to stabilize a production `underwood_parley` contract:

- copying callback-borrowed glyph data is prototype overhead and storage
  ownership, not the desired retained upstream result;
- the adapter currently chooses fonts by script and does not exercise a real
  fallback resolver;
- no Parley type crosses a stable Underwood façade because no such façade was
  created;
- this benchmark-only git dependency is not a production pin or dependency
  approval;
- no upstream patch or divergence lifecycle exists in this run.

## Known failures and exclusions

- Devanagari, Thai, CJK, emoji, broader mixed bidi, fallback chains, missing
  glyphs, normalization edges, tracking, and language changes are `NOT_RUN`.
- Hyphenation, discretionary breaking, break/concat equivalence, disjoint line
  intervals, U+FFFC objects, vertical mixed-script text, and accessibility
  linkage are `NOT_RUN` because the required core seams are absent or not yet
  connected.
- Only the unpatched upstream path ran; there is no divergence to dual-run.
- Cross-target deterministic prepared digests and fixed external expected
  artifacts are `NOT_RUN`.
- Allocation cost, copy cost, preparation throughput, and retained size are
  `NOT_RUN`.
- The FNV observations are screens, not collision-resistant identities.

These gaps block completion of the Parley conformance bead, creation of the
production adapter without its human gates, and proof promotion.

## Retained-result follow-up — 2026-07-22

Design-0005 reran this wind tunnel after `ShapedText` landed, against exact
Parley revision `6c81e1dd9b67793cdd959c65cc650c96a1262fb7`. The executable now
shapes into owned `ShapedText`, reads retained source clusters, fonts, glyphs,
positions, and normalized coordinates, and keeps only deterministic
observation records for comparison. It no longer receives a borrowed shaped
run callback.

```text
parley=6c81e1dd9b67793cdd959c65cc650c96a1262fb7 analysis=cfdddc2f29b0a50b items=f92022fbdf3a152a physics=3e1d17966180319d slots=f7b2494a3759e8a3 paint=b118c17a87887d7a items_count=2 runs=2 glyphs=29 gaps=4
fonts=RobotoFlex-VariableFont.ttf:aecc087879796a24,NotoKufiArabic-Regular.otf:4e4306d5a30a1500
```

All six deterministic tests pass. The analysis, item, paint-slot, and paint
digests remain unchanged. The physics digest changes because it now records
Parley's scaled `f32` retained glyph values rather than raw HarfRust integer
positions. The retained-shaped-output gap is removed from the enum-backed
matrix. The remaining four gaps are bounded break reshaping, vertical shaping,
core inline objects, and text-data identity.

Production conformance additionally proves exact `ffi` source union, logical-
to-visual RTL lowering, glyphless newline handling, family/fallback/synthesis
selection, and unchanged CPU poster pixels through `underwood_parley` and the
public Underwood scene path.
