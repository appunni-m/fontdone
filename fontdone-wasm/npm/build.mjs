import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDirectory = dirname(fileURLToPath(import.meta.url));
const crateDirectory = dirname(packageDirectory);
const manifest = join(crateDirectory, "Cargo.toml");
const cargo = process.env.CARGO || "cargo";

function cargoOutput(arguments_) {
  return execFileSync(cargo, arguments_, {
    cwd: crateDirectory,
    encoding: "utf8",
    stdio: ["inherit", "pipe", "inherit"],
  });
}

const packageManifest = JSON.parse(
  readFileSync(join(packageDirectory, "package.json"), "utf8"),
);
if (packageManifest.name !== "fontdone") {
  throw new Error("npm package must be named fontdone");
}
const packagedWasm = join(packageDirectory, "fontdone.wasm");
const packagedAbi = join(packageDirectory, "abi.json");

if (existsSync(manifest)) {
  const metadata = JSON.parse(
    cargoOutput([
      "metadata",
      "--manifest-path",
      manifest,
      "--format-version",
      "1",
      "--no-deps",
    ]),
  );
  const crate = metadata.packages.find(
    (candidate) => candidate.name === "fontdone-wasm",
  );
  if (crate === undefined) {
    throw new Error("cargo metadata did not contain fontdone-wasm");
  }
  const sourceAbi = JSON.parse(
    readFileSync(join(crateDirectory, "abi.json"), "utf8"),
  );
  if (packageManifest.version !== crate.version) {
    throw new Error(
      `npm/Cargo version mismatch: ${packageManifest.version} != ${crate.version}`,
    );
  }
  if (sourceAbi.package_version !== crate.version) {
    throw new Error(
      `abi/Cargo version mismatch: ${sourceAbi.package_version} != ${crate.version}`,
    );
  }

  execFileSync(
    cargo,
    [
      "build",
      "--manifest-path",
      manifest,
      "--package",
      "fontdone-wasm",
      "--target",
      "wasm32-unknown-unknown",
      "--release",
      "--locked",
    ],
    { cwd: crateDirectory, stdio: "inherit" },
  );

  const builtWasm = join(
    metadata.target_directory,
    "wasm32-unknown-unknown",
    "release",
    "fontdone_wasm.wasm",
  );
  copyFileSync(builtWasm, packagedWasm);
  copyFileSync(join(crateDirectory, "abi.json"), packagedAbi);
}

const abi = JSON.parse(readFileSync(packagedAbi, "utf8"));
if (abi.package_version !== packageManifest.version) {
  throw new Error(
    `packaged abi/npm version mismatch: ${abi.package_version} != ${packageManifest.version}`,
  );
}

const module = new WebAssembly.Module(readFileSync(packagedWasm));
const imports = WebAssembly.Module.imports(module);
if (imports.length !== 0) {
  throw new Error(`fontdone Wasm unexpectedly imports ${imports.length} items`);
}
const moduleExports = new Set(
  WebAssembly.Module.exports(module).map((entry) => `${entry.kind}:${entry.name}`),
);
for (const entry of abi.exports) {
  if (!moduleExports.has(`function:${entry.name}`)) {
    throw new Error(`fontdone Wasm is missing ABI export ${entry.name}`);
  }
}
if (!moduleExports.has("memory:memory")) {
  throw new Error("fontdone Wasm does not export memory");
}

const size = statSync(packagedWasm).size;
console.log(`npm Wasm asset: ${size} bytes at ${packagedWasm}`);
