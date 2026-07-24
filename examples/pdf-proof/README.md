# Underwood PDF proof

This executable prepares one real semantic Underwood document and lowers its
public `TextScene` through `underwood_pdf` and Krilla.

```sh
cargo run -p underwood_pdf_proof --release
```

The default artifact is `target/underwood-proof.pdf`; pass a path as the first
argument to write it elsewhere.

The specimen exercises:

- mixed Latin and right-to-left Arabic in one flowing paragraph;
- an authored `ffi` ligature (`office` reaches the scene as four glyphs);
- one decomposed grapheme split across semantic text leaves;
- multiple semantic paragraphs, sizes, line heights, and solid paints;
- embedded Roboto Flex at its default location and static Noto Kufi Arabic.

This is visual PDF evidence, not yet a tagged PDF or PDF/UA claim. Logical text
extraction for complex scripts is tracked separately.
