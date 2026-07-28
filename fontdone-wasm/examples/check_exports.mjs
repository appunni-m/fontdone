import { readFile } from "node:fs/promises";

const [wasmPath, contractPath] = process.argv.slice(2);
if (!wasmPath || !contractPath) {
  throw new Error("usage: node check_exports.mjs FONTDONE_WASM ABI_JSON");
}

const module = await WebAssembly.compile(await readFile(wasmPath));
const contract = JSON.parse(await readFile(contractPath, "utf8"));
const expected = new Set(contract.exports.map((entry) => entry.name));
const moduleExports = WebAssembly.Module.exports(module);
const actual = new Set(
  moduleExports
    .filter(
      (entry) =>
        entry.kind === "function" && entry.name.startsWith("fontdone_wasm_"),
    )
    .map((entry) => entry.name),
);
const missing = [...expected].filter((name) => !actual.has(name)).sort();
const undocumented = [...actual].filter((name) => !expected.has(name)).sort();
if (missing.length !== 0 || undocumented.length !== 0) {
  throw new Error(
    `WASM export mismatch: missing=${missing.join(",")} undocumented=${undocumented.join(",")}`,
  );
}
if (!moduleExports.some((entry) => entry.kind === "memory" && entry.name === "memory")) {
  throw new Error("WASM module does not export memory");
}
console.log(`WASM exports: ${actual.size} functions and memory match abi.json`);
