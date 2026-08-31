import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import test from "node:test";

import createFontdone, { FontdoneError } from "./index.js";

const wasm = await readFile(new URL("./fontdone.wasm", import.meta.url));
const fontUrl = new URL(
  "../../tests/fixtures/input/fonts/DejaVuSans.ttf",
  import.meta.url,
);
const font = existsSync(fontUrl) ? await readFile(fontUrl) : undefined;

test("renders a copied grayscale bitmap from bytes", {
  skip: font === undefined ? "source fixture is not shipped in the npm package" : false,
}, async () => {
  const engine = await createFontdone(wasm);
  const face = engine.openFace(font, { pixelSize: 16 });
  const bitmap = face.render("A");

  assert.ok(bitmap.glyphIndex > 0);
  assert.ok(bitmap.width > 0);
  assert.ok(bitmap.height > 0);
  assert.equal(bitmap.pixels.length, Math.abs(bitmap.pitch) * bitmap.height);
  assert.ok(bitmap.pixels.some((value) => value !== 0));
  assert.throws(() => face.getCharIndex("\ud800"), /Unicode scalar value/);

  face.close();
  assert.equal(face.closed, true);
  face.close();
  engine.close();
  assert.equal(engine.closed, true);
});

test("accepts a compiled module and closes owned faces with the engine", {
  skip: font === undefined ? "source fixture is not shipped in the npm package" : false,
}, async () => {
  const module = await WebAssembly.compile(wasm);
  const engine = await createFontdone(module);
  const face = engine.openFace(font);

  engine.close();
  assert.equal(engine.closed, true);
  assert.equal(face.closed, true);
  assert.throws(() => face.render("A"), /fontdone face is closed/);
});

test("falls back when a Wasm response has a non-streaming MIME type", async () => {
  const response = new Response(wasm, {
    headers: { "content-type": "application/octet-stream" },
  });
  const engine = await createFontdone(response);

  assert.equal(engine.closed, false);
  engine.close();
});

test("reports invalid font input as a typed FT_Error", async () => {
  const engine = await createFontdone(wasm);

  assert.throws(
    () => engine.openFace(new Uint8Array([1, 2, 3, 4])),
    (error) =>
      error instanceof FontdoneError &&
      error.operation === "openFace" &&
      error.code !== 0,
  );
  engine.close();
});
