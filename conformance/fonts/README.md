# Conformance test fonts

The font in this directory is a deterministic test resource, not a system-font
dependency.

`NotoSansDevanagari-Regular.subset.ttf` is derived from the hinted Noto Sans
Devanagari Regular face published by the
[`notofonts/devanagari`](https://github.com/notofonts/devanagari) project. It is
licensed under the SIL Open Font License 1.1; see
`LICENSE-NotoSansDevanagari.txt`.

The subset retains every OpenType layout feature so tests exercise real
Devanagari shaping. Its SHA-256 digest is:

```text
94d45487543c47fa41d94f23bfc6ceb277dbcbc3ae12c92b079772ec4a76b2bb
```

Regenerate a comparably small subset with fonttools:

```sh
uvx --from fonttools pyftsubset NotoSansDevanagari-Regular.ttf \
  --unicodes='U+0020-007E,U+00A0,U+0900-097F,U+25CC' \
  --layout-features='*' \
  --output-file=NotoSansDevanagari-Regular.subset.ttf
```
