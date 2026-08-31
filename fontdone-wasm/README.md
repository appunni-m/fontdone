# fontdone WebAssembly

This directory owns two synchronized WebAssembly surfaces for the pure-Rust
`fontdone` engine:

- the browser npm package named `fontdone`, with a prebuilt Wasm module and a
  typed ESM lifecycle wrapper;
- the `fontdone-wasm` Rust crate, which builds the raw linear-memory ABI used by
  that wrapper and by the repository parity harness.

Version `2.14.3-alpha.1` requires exactly `fontdone = 2.14.3-alpha.1`. Different
alpha releases are not API- or ABI-compatible by promise. Neither surface is a
text-shaping or layout engine.

## 1. Browser npm package

Install the public package:

```bash
npm install fontdone@2.14.3-alpha.1
```

Then initialize the packaged Wasm asset and render one glyph:

```js
import createFontdone from "fontdone";

const [engine, fontBytes] = await Promise.all([
  createFontdone(),
  fetch("/fonts/example.ttf").then((response) => response.arrayBuffer()),
]);
const face = engine.openFace(fontBytes, { pixelSize: 32 });

try {
  const bitmap = face.render("A");
  console.log(bitmap.width, bitmap.height, bitmap.pitch, bitmap.pixels);
} finally {
  face.close();
  engine.close();
}
```

The initializer accepts an explicit URL, `Request`, `Response`, buffer,
compiled `WebAssembly.Module`, or `WebAssembly.Instance`. With no argument it
fetches `fontdone.wasm` relative to the ESM entry point. Streaming
instantiation falls back to an `ArrayBuffer` when a server does not provide the
`application/wasm` content type.

The maintained browser contract requires ESM, `fetch`, WebAssembly, and
WebAssembly JavaScript BigInt integration. Each initializer call creates an
independent instance; its faces and memory offsets are not transferable to
another instance or Worker. The wrapper copies caller font bytes on open,
copies rendered bitmap bytes before returning, and provides idempotent
`close()` methods.

The complete browser API, error model, bitmap layout, and security boundary are
documented in the
[npm package guide](https://github.com/appunni-m/fontdone/blob/main/fontdone-wasm/npm/README.md).

Build, inspect, install, and execute the exact npm tarball:

```bash
make npm-package-verify
```

The verified archive is written to:

```text
target/npm-package/fontdone-2.14.3-alpha.1.tgz
```

`npm pack` runs the same Rust build, raw-export check, and wrapper tests through
the package's `prepack` lifecycle. No prebuilt Wasm file is committed.

## 2. Raw target and hosts

The raw crate's claimed compile target is:

```text
wasm32-unknown-unknown
```

The module imports no host functions and exports its own `memory`. The promoted
direct-host subset works with browser `WebAssembly` and with Node.js 20 or
newer. Browser applications should normally use the npm wrapper because it
owns allocator pairing and face cleanup.

Install and build the raw module:

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
features. `make optional-feature-contract` separately proves the disabled
host-facade behaviors against matching pinned FreeType configurations and
checks the subpixel-rendering feature routes.

## 3. Machine-readable raw ABI

The Rust package contains two generated contracts:

1. `abi.json`: every `#[unsafe(no_mangle)]` export plus every public
   `#[repr(C)]` record's field order, wasm32 byte offset, width, alignment,
   pointer interpretation, and ownership class.
2. `fontdone_wasm.d.ts`: the directly callable JavaScript host subset used by
   the maintained raw example and the browser wrapper.

Regenerate and reject drift:

```bash
python3 scripts/generate_wasm_contract.py
python3 scripts/generate_wasm_contract.py --check
```

On wasm32, pointers and `usize` are 32-bit byte offsets into the instance's
little-endian exported memory. Rust `u64` and `i64` parameters are JavaScript
`bigint`; other promoted scalar parameters are JavaScript `number`.

## 4. Maintained integrations

Run the raw Node host path:

```bash
make test-wasm-consumer
```

Its copyable command is:

```bash
node fontdone-wasm/examples/node.mjs \
  target/wasm32-unknown-unknown/release/fontdone_wasm.wasm \
  tests/fixtures/input/fonts/DejaVuSans.ttf
```

The example instantiates the module, checks an invalid open, copies a font into
linear memory, opens a face, selects 16 ppem, maps and renders `A`, copies the
bitmap, checks invalid-handle error transport, and releases every allocation
and handle.

`make npm-package-verify` additionally runs wrapper unit tests, inspects the
actual npm tarball, installs it into a temporary dependency consumer, reruns
the shipped verification scripts outside the checkout, and renders the same
glyph through the package import.

## 5. Supported direct-host subset

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
| `fontdone_wasm_bitmap_{buffer,len,width,rows,pitch}(handle)` | Borrow bitmap bytes and scalar metadata |

Other exports exist to keep the cross-facade parity harness honest. They are
inventoried in `abi.json`, but are not all promoted as ergonomic JavaScript
calls. Struct-returning C ABI functions in particular can use target ABI
lowering that differs from a direct JavaScript scalar call. Use the promoted
subset unless both the schema and generated module signature are handled.

Export presence and parity-route evidence do not mean every function is
application-ready. Consult the repository's
[function adoption map](https://github.com/appunni-m/fontdone/blob/main/doc/FREETYPE_SUPPORT.md)
and compatibility snapshot before depending on a broader export.

## 6. Allocation and pointer rules

`fontdone_wasm_malloc` uses 8-byte alignment and allocates `max(size, 1)`
bytes. A zero-size request therefore produces a releasable one-byte allocation.
Null reports layout or allocation failure.

`fontdone_wasm_free`:

- accepts null as a no-op;
- requires the identical `size` used for allocation, including zero;
- consumes a non-null allocation exactly once;
- does not validate arbitrary or already-freed offsets.

An invalid pointer, double free, or size mismatch violates the ABI precondition
and can trap or corrupt that instance. It is not an `FT_Error`.

For every pointer/length input, the complete byte range must lie in current
exported memory for the synchronous call. Reacquire `memory.buffer` views after
any call that can grow memory.

## 7. Handle and borrowed-output lifecycle

| Value | Ownership and validity | Release |
|---|---|---|
| face handle | Owned scalar returned on successful open | `fontdone_wasm_done_face` once |
| input font allocation | Caller-owned; bytes are copied by successful open | `fontdone_wasm_free` after open |
| current glyph slot | Face-owned; replaced by the next load/render mutation | Released with face |
| bitmap buffer offset | Borrowed from current slot; invalid after slot replacement or face teardown | Never free |
| caller output record | Caller-allocated linear memory; fields follow `abi.json` | Caller frees its allocation |
| returned string/table offsets | Borrowed or owned exactly as named by the export schema | Follow `abi.json` and the matching free export |

All handles are instance-local. Never pass a handle or memory offset between
different `WebAssembly.Instance` objects.

Every exported record is `#[repr(C)]`. `abi.json` is authoritative for field
order, wasm32 offsets and widths, record size and alignment, pointer meaning,
and ownership. `FT_Pos`, outline coordinates, bearings, and ordinary advances
use signed 26.6 units after scaling. `FT_Fixed` and matrix coefficients use
signed 16.16. Bitmap length is `abs(pitch) * rows`.

## 8. Packaging and license

The Cargo archive contains source, this README, the generated raw contracts,
the Node example, and `LICENSE`, `FTL.TXT`, and `NOTICE.md`. It excludes test
fonts, generated fixture outputs, C oracle source, the compiled Wasm binary,
and local tooling.

The npm archive contains the ESM wrapper, declarations, prebuilt Wasm binary,
ABI inventory, browser and Node examples, verification scripts, and the same
legal files. It excludes Rust/C source, fixture fonts, oracle material, and
repository tooling.

Inspect both artifact forms with:

```bash
cargo package -p fontdone-wasm --list
make npm-package-verify
```
