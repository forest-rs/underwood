# Text-data wind-tunnel evidence — 2026-07-21

- **Capability:** international text data
- **Bead:** `und-oh0.10.1.3`
- **Implementation commit:** `9a8d5d5`
- **Upstream:** Parley `45da4a90248b1600277a4294b70d8bfde5ca8e97`
- **Tiers:** current compiled `minimal` and `complex-segmentation` paths
- **Proof effect:** evidence at `Specified`; no promotion to `Executable`

## Reproduction

```sh
cargo test -p underwood_text_data_wind_tunnel
cargo test -p underwood_text_data_wind_tunnel --features complex-scripts
bash benches/text-data/measure.sh
```

The recorded run used Rust 1.96.0
`ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96`, Cargo 1.96.0,
`wasm32-unknown-unknown`, Brotli 1.2.0 at quality 11, and Node.js 26.5.0 on
macOS 26.5.2 arm64. The Cargo lockfile SHA-256 was
`1d6da91683cf8359fb772f34e4b957239c9cc9b4e67976134944d3d95b844eba`.

Artifacts use the repository's `wind-tunnel` profile: `opt-level = "z"`, fat
LTO, one codegen unit, aborting panics, and stripped symbols. No post-link
WebAssembly optimizer was available or applied. The empty binary is built from
the same package and profile but does not reference Parley, so its difference
from the exercised binaries is the measured compiled-path increment.

## Artifact identity and size

| Artifact | Raw bytes | Brotli bytes | Increment from preceding tier | Proposed compressed gate | Verdict |
| --- | ---: | ---: | ---: | ---: | --- |
| Empty harness | 13,982 | 5,176 | — | — | baseline |
| `minimal` | 147,695 | 44,017 | +38,841 Brotli bytes from empty | at most 256 KiB | **PASS** |
| `complex-segmentation` | 3,943,410 | 2,111,702 | +2,067,685 Brotli bytes from `minimal` | at most a further 512 KiB | **FAIL** |

Exact raw artifact SHA-256 identities:

| Artifact | SHA-256 |
| --- | --- |
| Empty | `728f18c37b7c3cea984ca2781b166860815a6186cc5bbd70033684981b3ec031` |
| `minimal` | `43666d738dce6abb97e91e862fefd0a15dd56ebab28d03ef078b5972b788589b` |
| `complex-segmentation` | `5322603e2f9ada28bf7d92e8b74c87d305055a55a478d1766c31f33999567c3c` |

The complex tier misses its measured compressed budget by 1,543,397 bytes,
3.94 times the allowed increment. This result rejects the current compiled
feature as the proposed WebAssembly distribution unchanged; it does not
silently enlarge the tier budget.

## Linear-memory observation

Node instantiated each import-free module, recorded its exported memory before
execution, invoked exported `main` once over the multilingual corpus, and
recorded the resulting memory size.

| Artifact | Initial bytes | `data_end` | `heap_base` | After one warm run | Incremental warm bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Empty harness | 1,114,112 | 1,050,178 | 1,050,192 | 1,114,112 | — |
| `minimal` | 1,179,648 | 1,148,378 | 1,148,384 | 1,245,184 | +131,072 from empty |
| `complex-segmentation` | 4,980,736 | 4,929,402 | 4,929,408 | 5,046,272 | +3,801,088 from `minimal` |

The `minimal` warm increment passes the proposed 1 MiB resident gate. The
complex tier fails its further 2 MiB gate by 1,703,936 bytes. These are exact
WebAssembly linear-memory observations, but they do not separate static data,
allocator retention, provider-owned heap, or peak transient allocations.

## Compiled-path and trace evidence

The 543-byte corpus exercises Latin, combining normalization, Greek, Cyrillic,
Arabic, Hebrew, Thai, Khmer, Lao, Myanmar, Japanese, Chinese, Korean, emoji,
regional indicators, grapheme, word, line, and bidi inputs.

| Tier | Characters | Word boundaries | Line boundaries | Bidi entries | Emoji/pictographs | Forced normalization | Result digest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `minimal` | 226 | 25 | 34 | 226 | 9 | 30 | `8e03b79635dfb945` |
| `complex-segmentation` | 226 | 25 | 53 | 226 | 9 | 30 | `80fb9a79c34617bf` |

Three tests pass under each feature configuration. They check deterministic
analysis, primitive replay across a changed trace identity, rejection of a
data-dependent replay under that change, and explicit private diagnostics for
absent complex segmentation or hyphenation.

The private `TraceIdentity` is deliberately only a version/tier label used to
exercise the replay law. It is not the immutable content-digest manifest
required by ADR-0003. The current Parley compiled path exposes no bundle
identity or capability negotiation, so actual cache/replay identity remains a
**FAIL**, not a capability supplied by the wind tunnel.

In a debug `minimal` run, ICU4X printed unstructured missing-model messages:
Thai four times and Japanese, Khmer, Lao, and Myanmar twice each. Release mode
does not provide those messages, and Parley documents character-level fallback
for complex scripts when the feature is disabled. The actual compiled path
therefore does not meet ADR-0003's structured `missing_capability` requirement.
The private diagnostic test proves only the required Underwood-owned law.

## Throughput screen

One optimized native run of 1,000 iterations over a supported-script subset
observed 37.726 MiB/s for `minimal` and 35.780 MiB/s for
`complex-segmentation`. These are `SCREEN` observations only: they are native,
single-sample, system-allocator numbers without confidence intervals or
allocation instrumentation. They are not a WebAssembly throughput verdict.

## License inventory

The locked `parley_core` and `parley_data` packages declare
`Apache-2.0 OR MIT`. ICU4X 2.2.0 packages participating in the graph—collections,
locale, normalizer, properties, provider, segmenter, and their compiled data
packages—declare `Unicode-3.0`. This is an exact source-manifest inventory for
the measured graph, not a legal opinion or approval for future locale and
hyphenation packs.

## Known failures and exclusions

- The current compiled complex tier fails both proposed WebAssembly size gates.
- The current compiled path lacks content-digest bundle identity, capability
  negotiation, and release-mode missing-capability diagnostics.
- Peak transient memory, load latency, allocations, WebAssembly throughput,
  and provider-specific retained heap remain `NOT_RUN`.
- No locale-tailoring or hyphenation bundle exists.
- The corpus records deterministic observations, not yet a versioned external
  Unicode/CLDR conformance suite.
- `wasm-opt` or another post-link optimizer may change exact artifact sizes and
  requires a separately identified run; it cannot retroactively change this
  result.

These failures block admission of the current complex compiled feature as the
distribution tier, completion of the text-data bead, and proof promotion.
