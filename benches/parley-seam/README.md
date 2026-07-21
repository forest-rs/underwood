# Underwood retained-Parley seam wind tunnel

This unpublished benchmark crate copies current `parley_core` callback results
into private Underwood-owned observation records. It proves the usable
analysis, itemization, font-selection, and horizontal-shaping seams without
turning their current Rust types into a public adapter contract.

```text
corpus + licensed Parley test fonts
                |
                v
       pinned parley_core
                |
                v
private owned observations + explicit gap matrix
```

```sh
cargo test -p underwood_parley_seam_wind_tunnel
cargo run -p underwood_parley_seam_wind_tunnel
```

The Roboto Flex and Noto Kufi Arabic font files and their licenses come from
the exact pinned `parley_dev` git source. The executable's compact FNV digests
are deterministic fixture checksums, not security or supply-chain identities;
the evidence record carries SHA-256 source identities.

This crate is evidence only: it is not `underwood_parley`, a production
dependency approval, an owned shaped-text design, or a replacement for
upstream retained output. Copying callback-borrowed glyph data is deliberately
measured as a prototype seam, not presented as the desired retained contract.
