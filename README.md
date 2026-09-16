# fontdone

A pure Rust font engine with measured compatibility against FreeType 2.14.3.
Use it from Rust, through a native C SDK, or through one npm package for
Node.js and browsers.

[Documentation](https://appunni-m.github.io/fontdone/) ·
[Function support](https://appunni-m.github.io/fontdone/api-support/) ·
[Benchmarks](https://appunni-m.github.io/fontdone/benchmarks/) ·
[Rust API](https://docs.rs/fontdone/2.14.3-alpha.10/fontdone/)

**Current release: 2.14.3-alpha.10.** This is an alpha with a deliberately
limited adoption contract. The maintained map classifies 52 of 218 functions
as complete; the other functions are mapped incompletely, partial, planned,
or outside scope. Passing 20,357 runnable comparisons does not establish
every FreeType behavior. Start with [maturity](doc/MATURITY.md).

## Choose an interface

| Application | Distribution | Guide |
| --- | --- | --- |
| Rust | One Cargo crate: `fontdone` | [Rust integration](doc/INTEGRATION.md) |
| C/C++ | Native SDK attached to GitHub Releases | [C integration](fontdone-c-abi/README.md) |
| Node.js or browser | One npm package: `fontdone` | [JavaScript guide](fontdone-wasm/npm/README.md) |
| Custom WASM host | Raw WASM ABI | [Host integration](fontdone-wasm/README.md) |

The C and raw-WASM workspace members are private Cargo build packages.
There is no second public Cargo crate and no PyPI package.

## Render a glyph in Rust

```toml
[dependencies]
fontdone = "=2.14.3-alpha.10"
```

```rust
use fontdone::Font;

fn render_a(font_bytes: &[u8]) -> Result<Vec<u8>, fontdone::FontError> {
    let font = Font::truetype(font_bytes, 16.0)?;
    let mask = font.getmask("A")?;
    assert_eq!(mask.pixels.len() as u64, u64::from(mask.width) * u64::from(mask.height));
    Ok(mask.pixels)
}
```

Pass bytes from a font you are licensed to use. The font owns its copied data;
the returned mask owns its pixels. The compact helper processes the first
Unicode scalar, not a shaped string. Read the
[integration contract](doc/INTEGRATION.md) for units, formats, ownership, and
fallible operations. The declared Rust minimum is 1.87; the workspace uses
pinned Rust 1.96.1 and has a separate MSRV lane.

## What is and is not established

- Rust owns font parsing, glyph loading, hinting, metrics, outlines, and
  rasterization. FreeType C is an offline oracle, never a runtime fallback.
- The [function map](doc/FREETYPE_SUPPORT.md) classifies application behavior.
  A declared symbol, successful header compile, or null-input test does not
  establish complete replacement.
- The [evidence guide](doc/EVIDENCE.md) separates runtime parity, source
  coverage, and the twelve-category C contract. Three undefined-C inputs
  remain explicitly pending.
- Text shaping, bidi ordering, font fallback, and paragraph layout are outside
  the compact API. Use an appropriate shaping/layout layer.
- Alpha releases do not promise API or ABI stability. Test the application's
  own fonts, glyphs, features, and target platforms before upgrading.

## Measure performance

The [benchmark site](https://appunni-m.github.io/fontdone/benchmarks/) shows
per-operation latency, sample counts, and source identity. Timing-only rows
remain labeled, and historical measurements do not stand in for the current
release. [Methodology](doc/BENCHMARKING.md) explains process memory, artifact
size, and the still-unset regression thresholds.

## Contribute

Start with [Contributing](CONTRIBUTING.md) and the
[documentation index](doc/README.md). Small reproductions and licensed parity
inputs are especially useful.

```sh
make help
make build
make test-fast
make check-docs
```

Use `make docs-setup`, `make docs-build`, and `make docs-serve` for the
public site. Each repository publishes its own GitHub Pages artifact from CI.

## Project information

[Release process](doc/RELEASING.md) · [Changelog](CHANGELOG.md) ·
[Security](SECURITY.md) · [Code of conduct](CODE_OF_CONDUCT.md)

Fontdone is distributed under the [FreeType License](FTL.TXT).
[Notices](NOTICE.md) and [fixture provenance](tests/fixtures/input/fonts/PROVENANCE.md)
retain the authorship, license, and transformation history of reference assets.

## Acknowledgements

Thank you to [FreeType](https://freetype.org/) and its contributors for the
reference implementation and public contracts, and to the authors who provide
licensed test fonts. Thank you also to [Puhu](https://github.com/bgunebakan/puhu)
for the Rust/Python imaging work that informed the parent project's early
exploration, and [Pillow](https://python-pillow.org/) for its image and font
interfaces and reference behavior.
