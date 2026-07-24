# Parley Engine line-formation proof — 2026-07-24

## Disposition

Underwood now consumes one exact Parley-family type universe at
`9c41a4d0b9aa1aae7b8fdad8cf31728c9c3476bb`. The workspace contains no
`parley_core` dependency, old fork pin, `apply_break`, or `apply_concat`.

The replacement is not a synthetic breaker or a copied shaper. Underwood
retains canonical Parley Engine analysis and shaping, chooses legal line
ranges, then re-itemizes and shapes committed ranges through public Engine
APIs. Fonts are resolved once during canonical shaping and reused by exact
`FontInstance` for line-final shaping.

## Executable correctness

The focused product corpus proves:

- a legal U+200B break changes real Arabic joining glyph output;
- no accepted glyph source crosses the committed line seam;
- a line-final Arabic shape whose advance grows past the constraint retries
  and commits the preceding legal opportunity;
- Latin ligature components remain exact interaction units;
- CR, LF, CRLF, U+2028, and U+2029 retain mandatory-break behavior;
- NBSP and unbreakable words overflow rather than inventing breaks;
- mixed LTR/RTL runs lower in visual order inside logical line source;
- line-height-only formation reuses accepted line glyphs;
- a single unwrapped line reuses canonical `ShapedText`;
- rejected and accepted line candidates report exact attempts, retained-font
  resolution, shaped runs, and shaped glyphs.

Explicit paragraph direction is separately trapped for RTL numeric/neutral
text, RTL empty text, LTR override of Arabic first-strong inference, and
product-path invalidation. A foreign paragraph-style identity is rejected.

## Work-accounting correction

The old `break_reshapes` counter represented only committed boundaries which a
temporary fork marked unsafe. Current public Parley Engine exposes no such
marker. Design-0013 therefore replaces it with:

- canonical `font_selection` and `shape` stages;
- line-final `line_font_resolution` and `line_shape` stages;
- `line_reshapes`, counting every accepted or rejected line-shaping attempt.

The adversarial review found one Must issue during implementation: canonical
glyph work was initially derived from accepted line output, and Parley's
array-only glyph slice does not include inline single glyphs. Both canonical
and line-final glyph counts now sum the cluster representation, including
inline glyphs and excluding zero-glyph ligature components.

## Wind tunnel

Command:

```sh
cargo run --profile wind-tunnel -p underwood_semantic_scene_benchmark --locked
```

Environment: Apple Silicon macOS, Rust/Cargo 1.96.0. Two complete process runs
of `main` at `c0e230e` and this branch were paired on the same machine. Each
cell is the midpoint. CPU scheduling was not pinned, so elapsed time is a
diagnostic screen; deterministic work assertions are the primary evidence.

| Workload | `main` ns/iteration | branch ns/iteration | Delta |
| --- | ---: | ---: | ---: |
| cold 64-paragraph scene | 6,764,377 | 6,553,130 | -3.1% |
| retained unchanged | 1,247,855 | 1,257,695 | +0.8% |
| paint only | 1,246,030 | 1,249,602 | +0.3% |
| alternating width | 5,346,108 | 5,714,956 | +6.9% |
| one-paragraph edit | 1,319,301 | 1,335,840 | +1.3% |

The width delta is expected and visible: the old fork could skip line shaping
when private HarfBuzz safety flags said a break was safe; current public Engine
requires the conservative correct path. The new corpus makes that work
concrete:

| Workload | ns/iteration | Line-shaping attempts / 100 iterations |
| --- | ---: | ---: |
| visible-space width churn, 64 paragraphs | 5,835,410 | 12,800 |
| Arabic cursive U+200B width churn, 64 paragraphs | 2,460,358 | 6,400 |

These fixtures have different text and line counts; their elapsed times are
not a safe-versus-unsafe ratio. They prove both ordinary and join-sensitive
line formation execute through the same public product path with exact work.
`und-oh0.2.10` owns any future reusable upstream break-safety or line-former
seam and must beat this evidence without weakening correctness.

The separate 2,048-label wind tunnel remains on the single-line canonical fast
path. The final local run recorded 36,991 ns per cold unique block and 8,458 ns
per retained unique block; the earlier Design-0012 observations were 38,701
and 9,005 respectively. Its constrained corpus recorded 1,024 wrapped
paragraphs and exactly 3,072 line-shaping attempts. Constrained labels therefore
make their line-final cost explicit while max-content labels avoid it.

## Lynx review

**Must**

- Fixed canonical and inline-glyph work misaccounting described above.

**Should**

- Fixed repeated Fontique queries by mapping line clusters to canonical font
  instances.
- Fixed silent acceptance of paragraph-style overrides belonging to another
  document.
- Grouped backend line counters into documented `LineShapingWork` instead of a
  ten-argument public constructor.

**Could / tracked**

- Selectively avoid provably safe line shaping if Parley Engine gains an
  upstream reusable seam: `und-oh0.2.10`.

**Unsafe watch**

No `unsafe` and no new production dependency were introduced. Foundational
crates remain `no_std + alloc`.

## Validation

The focused Underwood, adapter, direction, line-formation, source-interaction,
semantic-scene, and label wind tunnels pass. The complete workspace tests,
all-target/all-feature Clippy with warnings denied, warning-denied rustdoc,
Rust 1.88 workspace check, both `no_std` targets, repository policy, Beads
lint and cycle checks, exact visual snapshot, and deterministic 12,024-byte
PDF proof all pass. Protected remote checks remain the landing gate for this
proof.
