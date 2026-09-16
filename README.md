# fontdone

<!-- release:summary -->
**Latest release: [2.14.3-alpha.11](https://github.com/appunni-m/fontdone/releases/tag/v2.14.3-alpha.11).**
<!-- /release:summary -->

A pure Rust font engine for glyph rendering and metrics, with selected
FreeType-compatible APIs. Use it from Rust, C/C++, Node.js, or a browser.
This alpha has [documented compatibility limits](https://appunni-m.github.io/fontdone/maturity/).

[Documentation](https://appunni-m.github.io/fontdone/) ·
[Supported features](https://appunni-m.github.io/fontdone/maturity/) ·
[Benchmark results](https://appunni-m.github.io/fontdone/benchmarks/)

## Install

| Your application | Distribution | Guide |
| --- | --- | --- |
| Rust | Cargo: `fontdone` | [Rust guide](https://appunni-m.github.io/fontdone/rust/) |
| Node.js or browser | npm: `fontdone` | [JavaScript guide](https://appunni-m.github.io/fontdone/javascript/) |
| C/C++ | SDK archive on GitHub Releases | [C guide](https://appunni-m.github.io/fontdone/c/) |

There is one public Cargo crate, one npm package, and no PyPI package.
The Rust crate requires Rust 1.87 or newer.

<!-- release:cargo -->
```toml
[dependencies]
fontdone = "=2.14.3-alpha.11"
```
<!-- /release:cargo -->

## Render a glyph in Rust

```rust
use fontdone::Font;

fn render_a(font_bytes: &[u8]) -> Result<Vec<u8>, fontdone::FontError> {
    let font = Font::truetype(font_bytes, 16.0)?;
    let mask = font.getmask("A")?;
    assert_eq!(mask.pixels.len() as u64, u64::from(mask.width) * u64::from(mask.height));
    Ok(mask.pixels)
}
```

Pass bytes from a font you are licensed to use. The font copies the input;
the mask owns its pixels. `getmask` and `getbbox` process the first Unicode
scalar. `getlength` sums advances across the string without kerning.
See the [Rust guide](https://appunni-m.github.io/fontdone/rust/) for units,
errors, ownership, and explicit glyph loading.

## Is it right for my application?

- Render glyph masks and obtain metrics through the compact Rust or JavaScript APIs.
- Use supported FreeType-shaped functions when migrating Rust or native C code.
- Add a separate shaping/layout layer for ligatures, bidi text, fallback fonts, and paragraphs.
- Check the [compatibility guide](https://appunni-m.github.io/fontdone/maturity/) for font formats and partial features.

This alpha does not promise full FreeType replacement or ABI stability.
Create a separate face per thread and verify the fonts and operations you use.

## Performance

[View benchmark results](https://appunni-m.github.io/fontdone/benchmarks/) for
per-operation timings. Historical and timing-only measurements remain labeled.
They do not establish a universal speedup or performance on every font.

## Contribute and get help

[Contributing](https://appunni-m.github.io/fontdone/contributing/) covers builds,
tests, benchmark development, and reporting problems. See the contributor
[documentation index](https://appunni-m.github.io/fontdone/guides/) and
[measured compatibility evidence](https://appunni-m.github.io/fontdone/evidence/) for details.
[Releases](https://github.com/appunni-m/fontdone/releases) ·
[Changelog](https://appunni-m.github.io/fontdone/changelog/) ·
[Security](https://appunni-m.github.io/fontdone/security/) ·
[Code of conduct](https://appunni-m.github.io/fontdone/conduct/)

Fontdone is distributed under the [FreeType License](FTL.TXT).
See [NOTICE.md](NOTICE.md) for attribution.

## Acknowledgements

Thank you to [FreeType](https://freetype.org/) and its contributors for the
reference implementation and public contracts, and to the authors who provide
licensed test fonts. Thank you also to [Puhu](https://github.com/bgunebakan/puhu)
for the Rust/Python imaging work that informed the parent project's early
exploration, and [Pillow](https://python-pillow.org/) for its image and font
interfaces and reference behavior.
