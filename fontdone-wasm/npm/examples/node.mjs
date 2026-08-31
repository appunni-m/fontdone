import { readFile } from "node:fs/promises";

import createFontdone from "fontdone";

const [fontPath] = process.argv.slice(2);
if (!fontPath) {
  throw new Error("usage: node node.mjs FONT_FILE");
}

const wasmUrl = new URL(import.meta.resolve("fontdone/wasm"));
const [wasm, font] = await Promise.all([readFile(wasmUrl), readFile(fontPath)]);
const engine = await createFontdone(wasm);
const face = engine.openFace(font, { pixelSize: 16 });

try {
  const bitmap = face.render("A");
  const coverage = bitmap.pixels.reduce((sum, value) => sum + value, 0);
  if (coverage === 0) {
    throw new Error("rendered glyph has no coverage");
  }
  console.log(
    `glyph=${bitmap.glyphIndex} bitmap=${bitmap.width}x${bitmap.height} ` +
      `pitch=${bitmap.pitch} bytes=${bitmap.pixels.length} coverage=${coverage}`,
  );
} finally {
  face.close();
  engine.close();
}
