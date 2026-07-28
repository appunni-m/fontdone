# fontdone-wasm

`fontdone-wasm` is the low-level WebAssembly ABI for the pure-Rust `fontdone`
engine. It is a raw linear-memory interface, not a `wasm-bindgen` package and
not a high-level text-layout API.

Version `2.14.3-alpha.1` requires exactly `fontdone = 2.14.3-alpha.1`.
The crate is not published yet and no prebuilt `.wasm` file is distributed;
build it from the workspace. No ABI compatibility is promised between
different alpha releases.

Commands beginning with `make` or `python3 scripts/` are repository verification
commands and require the complete checkout. The `cargo build` command below is
the package build available from an unpacked `.crate` archive.

## 1. Supported target and host

The only claimed compile target is:

```text
wasm32-unknown-unknown
```

The only maintained host integration claimed by this alpha is Node.js 20 or newer using
the built-in `WebAssembly` API. Browser, Deno, WASI, component-model, and
`wasm32-wasip*` packaging are not claimed. The target imports no host
functions; the instance exports its own `memory`.

Install and build:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p fontdone-wasm \
  --target wasm32-unknown-unknown \
  --release --locked
```

Artifact:

```text
target/wasm32-unknown-unknown/release/fontdone_wasm.wasm
```

Bzip2, LZW, and color-layer support are enabled by the matching default Cargo
features. `make optional-feature-contract` separately proves all 3 disabled
host-facade behaviors with `--no-default-features` against matching pinned
FreeType configurations, plus both unavailable LCD filter setters. It also
builds the host facade with `subpixel-rendering` and verifies all 7 LCD setter
routes and stored filter state against the corresponding pinned build.

## 2. Machine-readable ABI contract

The package contains 2 generated contracts:

1. `abi.json`: every `#[unsafe(no_mangle)]` export plus every public `#[repr(C)]`
   record's field order, wasm32 byte offset, width, alignment, pointer
   interpretation, and ownership class.
2. `fontdone_wasm.d.ts`: the directly callable Node host subset used by the
   maintained example.

Regenerate and reject drift:

```bash
python3 scripts/generate_wasm_contract.py
python3 scripts/generate_wasm_contract.py --check
```

On wasm32, pointers and `usize` are 32-bit byte offsets into the instance's
little-endian exported memory. Rust `u64`/`i64` parameters are JavaScript
`bigint`; other scalar parameters in the supported subset are JavaScript
`number`.

## 3. Maintained Node integration

Build and run the complete host path:

```bash
make test-wasm-consumer
```

The underlying copyable command is:

```bash
node fontdone-wasm/examples/node.mjs \
  target/wasm32-unknown-unknown/release/fontdone_wasm.wasm \
  tests/fixtures/input/fonts/DejaVuSans.ttf
```

The example:

1. instantiates the module and obtains exported memory;
2. allocates and copies font bytes;
3. exercises an invalid-open error;
4. opens a memory face and releases the copied input;
5. selects 16 ppem, maps and loads `A`, then renders it;
6. reads bitmap metadata and copies coverage out of linear memory;
7. exercises an invalid-handle render error;
8. destroys the face and releases every caller allocation.

## 4. Supported direct-host subset

| Export | Contract |
|---|---|
| `fontdone_wasm_malloc(size)` | Allocate caller-owned linear memory |
| `fontdone_wasm_free(ptr, size)` | Release exactly that allocation |
| `fontdone_wasm_open_face_handle(ptr, len, face_index, size_pt, out_error)` | Copy bytes and return a face handle |
| `fontdone_wasm_done_face(handle)` | Consume the face handle |
| `fontdone_wasm_set_pixel_sizes(handle, width, height)` | Select integer ppem |
| `fontdone_wasm_get_char_index(handle, codepoint)` | Map a Unicode value to a glyph |
| `fontdone_wasm_load_glyph(handle, glyph, flags)` | Replace the current face-owned slot |
| `fontdone_wasm_render_glyph(handle, mode)` | Replace the slot with a rendered slot |
| `fontdone_wasm_bitmap_{buffer,len,width,rows,pitch}(handle)` | Borrow scalar bitmap information |

Other exports exist to keep the repository's cross-facade parity harness
honest. They are inventoried in `abi.json`, but they are not all promoted as
ergonomic JavaScript calls. Struct-returning C ABI functions in particular can
use target ABI lowering that differs from a direct JavaScript scalar call; use
the promoted subset unless the schema and generated module signature are both
handled.

Export presence and parity-route evidence do not mean every function is
application-ready. Consult the repository's
[function adoption map](https://github.com/appunni-m/fontdone/blob/main/doc/FREETYPE_SUPPORT.md)
and compatibility snapshot before depending on a broader export.

## 5. Allocation and pointer rules

`fontdone_wasm_malloc` uses 8-byte alignment and allocates `max(size, 1)`
bytes. Therefore a zero-size request produces a releasable one-byte allocation.
Null reports layout/allocation failure.

`fontdone_wasm_free`:

- accepts null as a no-op;
- requires the identical `size` used for allocation, including zero;
- consumes a non-null allocation exactly once;
- does not validate arbitrary or already-freed offsets.

An invalid non-null pointer, double free, or size mismatch violates the ABI
precondition and can trap or corrupt that instance. It is not an `FT_Error`.

For every `(pointer, length)` input, the complete byte range must lie in the
current exported memory and remain readable or writable for the synchronous
call. Reacquire `memory.buffer` views after any call that can grow memory.

## 6. Handle and borrowed-output lifecycle

| Value | Ownership and validity | Release |
|---|---|---|
| face handle | Owned scalar returned on successful open | `fontdone_wasm_done_face` once |
| input font allocation | Caller-owned; bytes are copied by successful open | `fontdone_wasm_free` immediately after open |
| current glyph slot | Face-owned; replaced by the next load/render mutation | Released with face |
| bitmap buffer offset | Borrowed from current slot; invalid after slot replacement or face teardown | Never free |
| caller output record | Caller-allocated linear memory; fields follow `abi.json` | Caller frees its allocation |
| returned string/table offsets | Borrowed or explicitly owned exactly as named by the export's schema/lifecycle pair | Follow `abi.json` and matching free export |

All handles are instance-local. Never pass a handle or memory offset between
different `WebAssembly.Instance` objects.

## 7. Record layout

Every exported record is `#[repr(C)]`. `abi.json` is authoritative for:

- declaration field order;
- wasm32 byte offset and width;
- record size and alignment;
- pointer-as-linear-memory interpretation;
- borrowed/value ownership.

`FT_Pos`, outline coordinates, bearings, and ordinary advances use signed 26.6
units after scaling. `FT_Fixed` and matrix coefficients use signed 16.16.
Bitmap length is `abs(pitch) * rows`; pixel meaning is selected by
`pixel_mode`.

## 8. Packaging and license

The crate archive contains source, this README, `abi.json`,
`fontdone_wasm.d.ts`, the Node example, and `LICENSE`, `FTL.TXT`, and
`NOTICE.md`. It excludes test fonts, generated fixture outputs, C oracle source,
and local tooling.

Inspect it with:

```bash
cargo package -p fontdone-wasm --list
```
