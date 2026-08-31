# fontdone

`fontdone` is the browser npm package for the pure-Rust fontdone engine. It
ships a prebuilt `wasm32-unknown-unknown` module and a zero-dependency ESM
wrapper for opening font bytes and rasterizing individual glyphs.

This is version `2.14.3-alpha.1`. Different alpha releases are not API- or
ABI-compatible by promise, and this package is not a text-shaping or layout
engine.

## Install and render

```bash
npm install fontdone@2.14.3-alpha.1
```

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

The default initializer fetches `fontdone.wasm` relative to the ESM entry
point. Pass an explicit URL, `Response`, `ArrayBuffer`, typed-array view,
`WebAssembly.Module`, or `WebAssembly.Instance` when your asset pipeline needs
different loading behavior:

```js
const engine = await createFontdone("/assets/fontdone.wasm");
```

The loader uses streaming instantiation when available and falls back to an
`ArrayBuffer` when the server does not send `application/wasm`.

## Browser contract

The maintained package requires browser ESM, `fetch`, WebAssembly, and
WebAssembly JavaScript BigInt integration. The wrapper performs no font or
telemetry requests; the only implicit request is for its own Wasm asset.

Each initializer call owns a separate WebAssembly instance. Faces and memory
offsets are instance-local and cannot be transferred to another instance or
Worker. Create one engine per Worker and close every face deterministically.

`render` maps one Unicode scalar, loads the glyph, renders it in normal
grayscale mode, and copies the result out of linear memory. Its result has:

- `glyphIndex`: the mapped glyph index;
- `width` and `height`: bitmap dimensions in pixels;
- `pitch`: signed bytes per source row;
- `pixels`: an owned `Uint8Array` of `abs(pitch) * height` bytes.

The package does not shape text, apply bidi ordering, choose fallback fonts,
or retain the caller's font buffer. Use a shaping library before loading glyph
indices when those behaviors are required.

## API

- `createFontdone(source?)` / `init(source?)`: create an independent engine.
- `engine.openFace(bytes, { faceIndex?, pixelSize? })`: copy bytes, open a face,
  and select an integer pixels-per-EM size (16 by default).
- `face.setPixelSize(size)` or `setPixelSizes(width, height)`: select integer
  pixels per EM.
- `face.getCharIndex(character)`: map one number or one Unicode scalar string.
- `face.loadGlyph(index, loadFlags?)`: load an explicit glyph.
- `face.renderGlyph(renderMode?)`: render and copy the current glyph bitmap.
- `face.render(character, options?)`: map, load, and render in one call.
- `face.close()` / `engine.close()`: deterministic, idempotent wrapper cleanup.

Nonzero engine statuses throw `FontdoneError`, whose `code` is the preserved
FreeType-compatible `FT_Error` value and whose `operation` names the failed
wrapper call. Programmer errors such as an invalid size or use after close
throw standard JavaScript errors.

TypeScript declarations ship with the package. The complete low-level export
and record inventory is also distributed as
[`abi.json`](https://github.com/appunni-m/fontdone/blob/main/fontdone-wasm/abi.json);
only the wrapper above is promoted as the browser application API.

## Security and license

Fonts are untrusted binary input and can demand substantial CPU or memory even
inside WebAssembly. Apply application-level input-size and execution limits.
Report vulnerabilities through the repository's
[private security-advisory route](https://github.com/appunni-m/fontdone/security/advisories/new).

The package is distributed under the FreeType License (`FTL`). See
[`FTL.TXT`](FTL.TXT) and [`NOTICE.md`](NOTICE.md) for terms and attribution.
