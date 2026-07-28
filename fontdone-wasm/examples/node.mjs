import { readFile } from "node:fs/promises";

const [wasmPath, fontPath] = process.argv.slice(2);
if (!wasmPath || !fontPath) {
  throw new Error("usage: node node.mjs FONTDONE_WASM FONT_FILE");
}

const wasmBytes = await readFile(wasmPath);
const fontBytes = await readFile(fontPath);
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const api = instance.exports;
if (!(api.memory instanceof WebAssembly.Memory)) {
  throw new Error("fontdone-wasm did not export linear memory");
}

const errorPtr = api.fontdone_wasm_malloc(4);
if (errorPtr === 0) {
  throw new Error("cannot allocate FT_Error output");
}

let fontPtr = 0;
let face = 0;
try {
  const invalidFace = api.fontdone_wasm_open_face_handle(
    0,
    0,
    0n,
    16,
    errorPtr,
  );
  const invalidError = new DataView(api.memory.buffer).getInt32(errorPtr, true);
  if (invalidFace !== 0 || invalidError === 0) {
    throw new Error("invalid input did not produce a non-zero FT_Error");
  }

  fontPtr = api.fontdone_wasm_malloc(fontBytes.length);
  if (fontPtr === 0) {
    throw new Error("cannot allocate font input");
  }
  new Uint8Array(api.memory.buffer, fontPtr, fontBytes.length).set(fontBytes);

  face = api.fontdone_wasm_open_face_handle(
    fontPtr,
    fontBytes.length,
    0n,
    16,
    errorPtr,
  );
  const openError = new DataView(api.memory.buffer).getInt32(errorPtr, true);
  if (face === 0 || openError !== 0) {
    throw new Error(`fontdone_wasm_open_face_handle failed: ${openError}`);
  }

  api.fontdone_wasm_free(fontPtr, fontBytes.length);
  fontPtr = 0;

  const sizeError = api.fontdone_wasm_set_pixel_sizes(face, 0, 16);
  if (sizeError !== 0) {
    throw new Error(`fontdone_wasm_set_pixel_sizes failed: ${sizeError}`);
  }
  const glyph = api.fontdone_wasm_get_char_index(face, 65n);
  if (glyph === 0) {
    throw new Error("font has no glyph for U+0041");
  }
  const loadError = api.fontdone_wasm_load_glyph(face, glyph, 0);
  if (loadError !== 0) {
    throw new Error(`fontdone_wasm_load_glyph failed: ${loadError}`);
  }
  const renderError = api.fontdone_wasm_render_glyph(face, 0);
  if (renderError !== 0) {
    throw new Error(`fontdone_wasm_render_glyph failed: ${renderError}`);
  }

  const width = api.fontdone_wasm_bitmap_width(face);
  const rows = api.fontdone_wasm_bitmap_rows(face);
  const pitch = api.fontdone_wasm_bitmap_pitch(face);
  const bitmapPtr = api.fontdone_wasm_bitmap_buffer(face);
  const bitmapLen = api.fontdone_wasm_bitmap_len(face);
  if (
    width === 0 ||
    rows === 0 ||
    bitmapPtr === 0 ||
    bitmapLen !== Math.abs(pitch) * rows
  ) {
    throw new Error("rendered bitmap metadata is inconsistent");
  }
  const bitmap = new Uint8Array(
    api.memory.buffer,
    bitmapPtr,
    bitmapLen,
  ).slice();
  const coverageSum = bitmap.reduce((sum, byte) => sum + byte, 0);
  if (coverageSum === 0) {
    throw new Error("rendered bitmap contains no coverage");
  }

  const handledError = api.fontdone_wasm_render_glyph(0, 0);
  if (handledError === 0) {
    throw new Error("invalid face handle unexpectedly rendered");
  }
  console.log(
    `glyph=${glyph} bitmap=${width}x${rows} pitch=${pitch} bytes=${bitmapLen} coverage=${coverageSum}`,
  );
} finally {
  if (face !== 0) {
    const doneError = api.fontdone_wasm_done_face(face);
    if (doneError !== 0) {
      throw new Error(`fontdone_wasm_done_face failed: ${doneError}`);
    }
  }
  if (fontPtr !== 0) {
    api.fontdone_wasm_free(fontPtr, fontBytes.length);
  }
  api.fontdone_wasm_free(errorPtr, 4);
}
