#!/usr/bin/env python3
"""Build wasm32 and run the maintained Node host integration."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WASM = (
    ROOT
    / "target"
    / "wasm32-unknown-unknown"
    / "release"
    / "fontdone_wasm.wasm"
)
FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "DejaVuSans.ttf"


def main() -> None:
    if shutil.which("node") is None:
        raise SystemExit("Node.js 20 or newer is required")
    version = subprocess.run(
        ["node", "--version"], check=True, capture_output=True, text=True
    ).stdout.strip()
    major = int(version.removeprefix("v").split(".", 1)[0])
    if major < 20:
        raise SystemExit(f"Node.js 20 or newer is required, found {version}")
    installed = subprocess.run(
        ["rustup", "target", "list", "--installed"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    if "wasm32-unknown-unknown" not in installed:
        raise SystemExit(
            "missing wasm32-unknown-unknown; run "
            "`rustup target add wasm32-unknown-unknown`"
        )
    subprocess.run(
        [
            "cargo",
            "build",
            "-p",
            "fontdone-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--locked",
        ],
        cwd=ROOT,
        check=True,
    )
    subprocess.run(
        [
            "node",
            "fontdone-wasm/examples/check_exports.mjs",
            str(WASM),
            "fontdone-wasm/abi.json",
        ],
        cwd=ROOT,
        check=True,
    )
    subprocess.run(
        [
            "node",
            "fontdone-wasm/examples/node.mjs",
            str(WASM),
            str(FONT),
        ],
        cwd=ROOT,
        check=True,
    )
    print(f"WASM consumer: compiled and ran with Node {version}")


if __name__ == "__main__":
    main()
