import { readFile } from "node:fs/promises";

import browserCreateFontdone from "./index.js";

const bundledWasm = new URL("./fontdone.wasm", import.meta.url);

/**
 * Node entrypoint for the single fontdone package. The browser entry keeps
 * using fetch for its adjacent asset; Node reads the same asset from disk so
 * a no-argument initializer never attempts fetch(file://…).
 */
export async function createFontdoneNode(source) {
  if (source === undefined) {
    source = await readFile(bundledWasm);
  }
  return browserCreateFontdone(source);
}

export const createFontdone = createFontdoneNode;
export const init = createFontdoneNode;
export default createFontdoneNode;

// Re-export the shared wrapper's public classes, constants, and types. The
// explicit initializer exports above take precedence over the star export.
export * from "./index.js";
