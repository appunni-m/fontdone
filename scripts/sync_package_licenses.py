#!/usr/bin/env python3
"""Copy root license and notice texts into independently packaged facades."""

from __future__ import annotations

import argparse
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGES = (
    ROOT / "fontdone-c-abi",
    ROOT / "fontdone-wasm",
    ROOT / "fontdone-wasm" / "npm",
)
FILES = ("LICENSE", "FTL.TXT", "NOTICE.md")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    stale: list[Path] = []
    for package in PACKAGES:
        for name in FILES:
            source = ROOT / name
            destination = package / name
            data = source.read_bytes()
            if destination.exists() and destination.read_bytes() == data:
                continue
            stale.append(destination)
            if not args.check:
                destination.write_bytes(data)
                print(destination.relative_to(ROOT))
    if args.check and stale:
        for path in stale:
            print(f"stale package legal text: {path.relative_to(ROOT)}")
        raise SystemExit(1)
    if args.check:
        print("package legal texts: clean")


if __name__ == "__main__":
    main()
