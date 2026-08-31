# Rust integration guide

This guide covers the two safe Rust surfaces:

- the compact `Font` API for masks and metrics;
- the FreeType-shaped `fontdone::ffi` facade for migration work.

C consumers should use the
[`fontdone-c-abi` guide](../fontdone-c-abi/README.md). Browser consumers should
use the [`fontdone` npm guide](../fontdone-wasm/npm/README.md); raw JavaScript
hosts should use the [`fontdone-wasm` guide](../fontdone-wasm/README.md).

## 1. Select and install the exact alpha

All workspace packages use `2.14.3-alpha.1`. They have not been published to
crates.io, so evaluate a checkout through a versioned path dependency. Keeping
the version requirement is important: Cargo rejects a path-only dependency
when the downstream crate is packaged or published.

```toml
[dependencies]
fontdone = { version = "=2.14.3-alpha.1", path = "../fontdone" }
```

Once the root package is published, registry consumers should use the exact
prerelease:

```toml
[dependencies]
fontdone = { version = "=2.14.3-alpha.1" }
```

After the repository and matching tag are public, a Git consumer may pin the
exact tag or an immutable 40-character revision:

```toml
[dependencies]
fontdone = { git = "https://github.com/appunni-m/fontdone", tag = "v2.14.3-alpha.1" }
```

This alpha requires Rust 1.87 or newer. Different `alpha.N` releases are not
API- or ABI-compatible by promise.

## 2. Compact Rust API

Use this surface when the application needs font bytes, one-glyph masks,
metrics, or direct glyph loading without preserving FreeType names.

### 2.1 Open and render

```rust
use fontdone::Font;

fn render_a(bytes: &[u8]) -> Result<Vec<u8>, fontdone::FontError> {
    let font = Font::truetype(bytes, 16.0)?;
    let mask = font.getmask("A")?;

    assert_eq!(mask.pixels.len(), mask.width * mask.height);
    Ok(mask.pixels)
}
```

Constructors consume bytes rather than paths. The input is copied into owned
font data, so the caller can release or reuse its buffer after the constructor
returns.

### 2.2 Input and format boundary

`Font::truetype`, `Font::truetype_face`, and their load-mode variants expose
the measured SFNT route for standalone TrueType/OpenType fonts and
TrueType/OpenType collections. The parser inspects bytes and tables; a filename
extension has no effect.

The method name `truetype` is historical API naming. It is not a claim that
every format handled elsewhere in the parity engine is accepted by this
constructor. The FreeType-shaped memory-face path has additional measured
routes, including BDF, WinFNT, Type 1, CFF, PCF, and PFR behavior. Check the
[function adoption map](FREETYPE_SUPPORT.md) before relying on a format or
operation.

The compact API does not discover system fonts, open paths itself, decompress
WOFF/WOFF2 as a declared application contract, or perform network I/O.

### 2.3 Text and rendering scope

The compact helpers do not perform shaping or text layout:

| Call | Input consumed | Kerning | Result |
|---|---|---|---|
| `getmask(text)` | First Unicode scalar | No | Owned 8-bit coverage and placement |
| `getbbox(text)` | First Unicode scalar | No | Integer-pixel bitmap box |
| `getlength(text)` | Every Unicode scalar | No | `f32` pixel advance |
| `getkerning(left, right)` | One Unicode pair | The pair itself | Signed 26.6 adjustment |
| `glyph_metrics(codepoint)` | One mapped glyph | No | Signed 26.6 metrics |
| `Face::load_glyph` | One glyph index | Controlled by flags | FreeType-shaped slot |

An empty string produces an empty mask and zero bbox. For script shaping,
ligatures, bidi ordering, fallback, grapheme handling, or OpenType
positioning, shape the run with another library and load the resulting glyph
indices.

`GlyphMask::pixels` is owned, tightly packed, row-major 8-bit coverage:

- `pixels.len() == width * height`;
- each row contains exactly `width` bytes;
- `0` is transparent and `255` is fully covered;
- `xmin` is the signed left placement in pixels;
- `ymin` is the signed lower bitmap-box coordinate in a y-up coordinate
  system;
- `advance_width` is the rounded horizontal advance in pixels.

LCD, LCD-V, mono, SDF, signed pitch, pixel mode, and unrounded slot advances
are available through explicit glyph loading rather than `GlyphMask`.

### 2.4 Sizes and units

Compact constructors take typographic points at an initial 72 DPI. Thus
`16.0` initially selects 16 ppem. The explicit face API can set DPI or pixel
sizes:

- 1 point is 1/72 inch;
- ppem is pixels per EM after point-size and DPI conversion;
- 26.6 values have 6 fractional bits and divide by 64 to produce pixels;
- 16.16 values have 16 fractional bits and divide by 65,536.

For 12 points at 96 DPI, `ppem = 12 × 96 / 72 = 16`.
`set_char_size(0, 12 * 64, 96, 96)` and `set_pixel_sizes(0, 16)` select that
same ppem through different unit contracts.

### 2.5 Ownership, concurrency, allocation, and errors

- A cloned `Font` shares immutable parsed data and selected caches but clones
  face state. It does not reread input.
- `Font` and `Face` are neither `Send` nor `Sync` in this alpha because
  rendering and hinting state uses single-threaded shared mutability. Open a
  separate face per thread.
- Opening and rendering allocate owned tables, outlines, caches, and bitmap
  storage. The safe API has no caller-supplied allocator.
- Public operations return `FontError` for invalid data, unsupported routes,
  invalid glyphs, malformed outlines, and allocation guards. Ordinary invalid
  input is not a panic-handling contract.

Handle invalid input explicitly:

```rust
use fontdone::{Font, FontError};

fn main() -> Result<(), FontError> {
    match Font::truetype(b"not a font", 16.0) {
        Err(FontError::InvalidFont(message)) => assert!(!message.is_empty()),
        Err(other) => return Err(other),
        Ok(_) => panic!("invalid bytes must not open"),
    }

    Ok(())
}
```

### 2.6 Maintained examples

```bash
cargo run --example render_mask -- tests/fixtures/input/fonts/DejaVuSans.ttf
cargo run --example load_glyph -- tests/fixtures/input/fonts/DejaVuSans.ttf
cargo run --example handle_error
make test-rust-consumer
make doc-test
```

`make test-rust-consumer` creates a temporary downstream Cargo project, uses
`fontdone` through a path dependency, exercises an error, and renders a
maintained fixture.

## 3. FreeType-shaped safe Rust

Use `fontdone::ffi` when porting code that depends on `FT_*` concepts but can
adopt Rust ownership. This is not the native C ABI package.

```rust
use fontdone::ffi::*;

fn render(bytes: &[u8]) -> Result<FT_GlyphSlot, FT_Error> {
    let library = FT_Init_FreeType();
    let mut face = FT_New_Memory_Face(&library, bytes, 0, 16.0)?;

    let error = FT_Set_Pixel_Sizes(&mut face, 0, 16);
    if error != FT_Err_Ok as FT_Error {
        return Err(error);
    }

    let glyph = FT_Get_Char_Index(&face, 'A' as FT_ULong);
    let slot = FT_Load_Glyph(&face, glyph, FT_LOAD_DEFAULT)?;
    let slot = FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL)?;

    assert_eq!(FT_Done_Face(Some(face)), FT_Err_Ok as FT_Error);
    assert_eq!(FT_Done_FreeType(Some(library)), FT_Err_Ok as FT_Error);
    Ok(slot)
}
```

The facade preserves measured names, constants, error numbers, units, and
record concepts while replacing raw C mutation:

| C pattern | Safe Rust pattern |
|---|---|
| nullable input pointer | `Option<&T>` or `Option<&mut T>` |
| output pointer | returned value, `Result`, or explicit mutable reference |
| `face->glyph` mutation | owned `FT_GlyphSlot` snapshot |
| teardown function | consuming function or `Drop` |
| caller-owned output array | `Vec<T>` where the mapping declares ownership |
| C-layout record | Rust record with semantic fields; no C layout promise |

Representative mappings:

| FreeType C | Safe Rust |
|---|---|
| `FT_Init_FreeType` | `FT_Init_FreeType() -> FT_Library` |
| `FT_New_Memory_Face` | `FT_New_Memory_Face(&FT_Library, &[u8], index, points) -> Result<FT_Face, FT_Error>` |
| `FT_Set_Pixel_Sizes` | `FT_Set_Pixel_Sizes(&mut FT_Face, width, height) -> FT_Error` |
| `FT_Get_Char_Index` | `FT_Get_Char_Index(&FT_Face, codepoint) -> FT_UInt` |
| `FT_Load_Glyph` | `FT_Load_Glyph(&FT_Face, glyph, flags) -> Result<FT_GlyphSlot, FT_Error>` |
| `FT_Render_Glyph` | `FT_Render_Glyph(FT_GlyphSlot, mode) -> Result<FT_GlyphSlot, FT_Error>` |
| `FT_Done_Face` | `FT_Done_Face(Option<FT_Face>) -> FT_Error` |
| `FT_Done_FreeType` | `FT_Done_FreeType(Option<FT_Library>) -> FT_Error` |

These examples are not an exhaustive compatibility list. The generated
[adoption map](FREETYPE_SUPPORT.md) is authoritative for all 218 pinned
functions. Safe Rust records do not promise C layout; use `fontdone-c-abi` when
raw pointers, headers, exported symbols, or exact native record layout are part
of the consumer contract.

Run the complete migration walkthrough with:

```bash
cargo run --example ffi_migration -- \
  tests/fixtures/input/fonts/DejaVuSans.ttf
```

## 4. Decide whether the current alpha is sufficient

Use all three evidence layers:

1. Find the operation in the [function adoption map](FREETYPE_SUPPORT.md).
2. Check the exact executed denominators in the
   [compatibility snapshot](compatibility_snapshot.json), or run
   `make test-parity` for the current worktree.
3. If native C replacement matters, inspect the 12-category scorecard produced
   by `make c-abi-contract`.

A function route can have runtime evidence without supporting every successful
input, state transition, ownership edge, artifact, or target. Version similarity
alone is not a compatibility claim.
