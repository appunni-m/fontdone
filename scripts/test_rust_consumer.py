#!/usr/bin/env python3
"""Compile and run a minimal Cargo consumer outside the workspace."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "tests" / "external" / "rust-consumer"
OUTPUT = ROOT / "target" / "external-consumers" / "rust"
FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "DejaVuSans.ttf"


def fontdone_version() -> str:
    metadata = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    packages = json.loads(metadata.stdout)["packages"]
    for package in packages:
        if package["name"] == "fontdone":
            return package["version"]
    raise RuntimeError("cargo metadata did not report the fontdone package")


def main() -> None:
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    shutil.copytree(TEMPLATE, OUTPUT)
    template = (OUTPUT / "Cargo.toml.in").read_text(encoding="utf-8")
    manifest = template.replace("@FONTDONE_VERSION@", fontdone_version())
    manifest = manifest.replace("@FONTDONE_PATH@", json.dumps(str(ROOT))[1:-1])
    (OUTPUT / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (OUTPUT / "Cargo.toml.in").unlink()
    subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=OUTPUT, check=True)
    subprocess.run(
        ["cargo", "run", "--locked", "--offline", "--", str(FONT)],
        cwd=OUTPUT,
        check=True,
    )
    print(f"external Rust consumer: passed outside workspace at {OUTPUT}")


if __name__ == "__main__":
    main()
