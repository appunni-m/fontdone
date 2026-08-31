#!/usr/bin/env python3
"""Build, inspect, install, and execute the browser-oriented npm package."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = ROOT / "fontdone-wasm" / "npm"
OUTPUT = ROOT / "target" / "npm-package"
EVIDENCE = ROOT / "target" / "release-evidence"
FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "DejaVuSans.ttf"
REQUIRED = {
    "README.md",
    "LICENSE",
    "FTL.TXT",
    "NOTICE.md",
    "package.json",
    "index.js",
    "index.d.ts",
    "build.mjs",
    "test.mjs",
    "fontdone.wasm",
    "abi.json",
    "examples/browser.html",
    "examples/browser.mjs",
    "examples/node.mjs",
}


def run(command: list[str], *, cwd: Path) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, check=True)


def require_tools() -> str:
    for tool in ("node", "npm", "cargo", "rustup"):
        if shutil.which(tool) is None:
            raise ValueError(f"{tool} is required for npm package verification")
    node_version = subprocess.run(
        ["node", "--version"], check=True, capture_output=True, text=True
    ).stdout.strip()
    major = int(node_version.removeprefix("v").split(".", 1)[0])
    if major < 20:
        raise ValueError(f"Node.js 20 or newer is required, found {node_version}")
    installed = subprocess.run(
        ["rustup", "target", "list", "--installed"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    if "wasm32-unknown-unknown" not in installed:
        raise ValueError(
            "missing wasm32-unknown-unknown; run "
            "`rustup target add wasm32-unknown-unknown`"
        )
    return node_version


def inspect_archive(archive: Path, version: str) -> None:
    prefix = PurePosixPath("package")
    files: dict[str, bytes] = {}
    with tarfile.open(archive, "r:gz") as package:
        for member in package.getmembers():
            if not member.isfile():
                continue
            path = PurePosixPath(member.name)
            if not path.parts or path.parts[0] != prefix.name:
                raise ValueError(f"unexpected npm archive root: {member.name}")
            relative = PurePosixPath(*path.parts[1:]).as_posix()
            source = package.extractfile(member)
            if source is None:
                raise ValueError(f"cannot read npm archive member: {member.name}")
            files[relative] = source.read()

    missing = sorted(REQUIRED - files.keys())
    unexpected = sorted(files.keys() - REQUIRED)
    if missing or unexpected:
        raise ValueError(
            f"npm package content mismatch: missing={missing}, unexpected={unexpected}"
        )
    for relative in files:
        if relative.lower().endswith((".ttf", ".otf", ".ttc", ".woff", ".woff2")):
            raise ValueError(f"font fixture leaked into npm package: {relative}")

    manifest = json.loads(files["package.json"])
    if manifest.get("name") != "fontdone" or manifest.get("version") != version:
        raise ValueError(
            "npm archive identity mismatch: "
            f"{manifest.get('name')}@{manifest.get('version')}"
        )
    if manifest.get("private") is True:
        raise ValueError("fontdone npm package is unexpectedly private")
    publish = manifest.get("publishConfig", {})
    if publish.get("access") != "public" or publish.get("tag") != "next":
        raise ValueError("npm alpha must publish publicly under the next dist-tag")
    if files["fontdone.wasm"][:4] != b"\0asm":
        raise ValueError("packaged fontdone.wasm has an invalid magic header")
    if b'from "node:' in files["index.js"] or b"from 'node:" in files["index.js"]:
        raise ValueError("browser entry point imports a Node-only module")

    EVIDENCE.mkdir(parents=True, exist_ok=True)
    inventory = EVIDENCE / f"fontdone-npm-{version}.inventory.txt"
    inventory.write_text("\n".join(sorted(files)) + "\n", encoding="utf-8")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (EVIDENCE / f"fontdone-npm-{version}.sha256").write_text(
        f"{digest}  {archive.name}\n", encoding="utf-8"
    )
    checksum_path = EVIDENCE / "SHA256SUMS"
    checksum_lines = []
    if checksum_path.is_file():
        checksum_lines = [
            line
            for line in checksum_path.read_text(encoding="utf-8").splitlines()
            if line and not line.endswith(f"  {archive.name}")
        ]
    checksum_lines.append(f"{digest}  {archive.name}")
    checksum_path.write_text("\n".join(checksum_lines) + "\n", encoding="utf-8")
    print(
        f"npm archive: {len(files)} files, {archive.stat().st_size} bytes, "
        f"sha256={digest}"
    )


def test_installed_archive(archive: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="fontdone-npm-consumer-") as temporary:
        consumer = Path(temporary)
        (consumer / "package.json").write_text(
            json.dumps(
                {"name": "fontdone-package-consumer", "private": True, "type": "module"},
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        shutil.copy2(PACKAGE_ROOT / "examples" / "node.mjs", consumer / "node.mjs")
        run(
            [
                "npm",
                "install",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                str(archive),
            ],
            cwd=consumer,
        )
        installed = consumer / "node_modules" / "fontdone"
        run(["npm", "run", "verify"], cwd=installed)
        run(["node", "node.mjs", str(FONT)], cwd=consumer)


def main() -> None:
    node_version = require_tools()
    manifest = json.loads((PACKAGE_ROOT / "package.json").read_text(encoding="utf-8"))
    version = manifest["version"]
    OUTPUT.mkdir(parents=True, exist_ok=True)
    archive = OUTPUT / f"fontdone-{version}.tgz"
    archive.unlink(missing_ok=True)
    run(
        ["npm", "pack", "--pack-destination", str(OUTPUT)],
        cwd=PACKAGE_ROOT,
    )
    if not archive.is_file():
        raise ValueError(f"npm did not create {archive}")
    inspect_archive(archive, version)
    test_installed_archive(archive)
    print(f"npm consumer: packed and installed with Node {node_version}")
    print(f"publishable archive: {archive.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
