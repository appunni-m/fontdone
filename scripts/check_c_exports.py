#!/usr/bin/env python3
"""Compare shared and static native exports with the maintained C header."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HEADER = ROOT / "fontdone-c-abi" / "include" / "fontdone_ffi.h"
LEDGER = ROOT / "target" / "api-abi-audit" / "c_export_ledger.json"


def header_functions() -> set[str]:
    text = re.sub(r"/\*.*?\*/", " ", HEADER.read_text(encoding="utf-8"), flags=re.S)
    functions: set[str] = set()
    for statement in text.split(";"):
        statement = " ".join(statement.split())
        if statement.startswith("typedef ") or "(" not in statement:
            continue
        match = re.search(r"\b((?:FT|FTC)_[A-Za-z0-9_]+)\s*\([^()]*\)$", statement)
        if match:
            functions.add(match.group(1))
    return functions


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def binary_exports(
    kind: str,
    system: str,
    release: Path,
    nm: str,
) -> tuple[Path, set[str]]:
    if system == "Windows":
        if kind == "static":
            library = release / "fontdone_c_abi.lib"
            command = ["dumpbin", "/nologo", "/linkermember:1", str(library)]
        else:
            library = release / "fontdone_c_abi.dll"
            command = ["dumpbin", "/nologo", "/exports", str(library)]
    elif kind == "static":
        library = release / "libfontdone_c_abi.a"
        command = [nm, "-g", str(library)]
    elif system == "Darwin":
        library = release / "libfontdone_c_abi.dylib"
        command = [nm, "-gU", str(library)]
    elif system == "Linux":
        library = release / "libfontdone_c_abi.so"
        command = [nm, "-D", "--defined-only", str(library)]
    else:
        raise SystemExit(f"binary export comparison is not supported on {system}")
    if not library.exists():
        raise SystemExit(f"missing native library: {library}")
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    output = completed.stdout
    if system == "Windows":
        result = set(re.findall(r"\b(?:FT|FTC)_[A-Za-z0-9_]+\b", output))
    else:
        result = set()
        for line in output.splitlines():
            symbol = line.split()[-1] if line.split() else ""
            symbol = symbol.removeprefix("_")
            if symbol.startswith(("FT_", "FTC_")):
                result.add(symbol)
    # Apple's system nm can reject LLVM-22 bitcode attributes in Rust standard
    # library members while still reading the crate member that owns every
    # exported C symbol.  Treat the command as usable only when it yielded the
    # complete filtered symbol surface; the exact comparison below remains the
    # authority.
    if completed.returncode != 0 and not result:
        raise SystemExit(
            f"failed to inspect {library}:\n"
            f"{completed.stderr or completed.stdout}"
        )
    return library, result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target")
    parser.add_argument("--nm")
    args = parser.parse_args()

    native_target_output = subprocess.run(
        ["rustc", "-vV"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    native_target = next(
        (
            line.split(":", 1)[1].strip()
            for line in native_target_output.splitlines()
            if line.startswith("host:")
        ),
        "unknown",
    )
    target = args.target or native_target
    if "windows" in target:
        system = "Windows"
    elif "apple-darwin" in target:
        system = "Darwin"
    elif "linux" in target:
        system = "Linux"
    else:
        raise SystemExit(f"binary export comparison is unsupported for {target}")
    nm = args.nm or "nm"
    if system != "Windows" and shutil.which(nm) is None:
        raise SystemExit(f"symbol inspector not found: {nm}")

    cargo_command = [
        "cargo",
        "build",
        "-p",
        "fontdone-c-abi",
        "--release",
        "--locked",
    ]
    if args.target:
        cargo_command.extend(("--target", target))
    subprocess.run(
        cargo_command,
        cwd=ROOT,
        env=os.environ.copy(),
        check=True,
    )
    release = (
        ROOT / "target" / target / "release"
        if args.target
        else ROOT / "target" / "release"
    )
    declared = header_functions()
    artifacts = {}
    failed = False
    for kind in ("shared", "static"):
        library, exported = binary_exports(kind, system, release, nm)
        missing = sorted(declared - exported)
        undocumented = sorted(exported - declared)
        artifacts[kind] = {
            "path": str(library.relative_to(ROOT)),
            "sha256": sha256(library),
            "declared": len(declared),
            "exported": len(exported),
            "missing": missing,
            "undocumented": undocumented,
            "status": "exact" if not missing and not undocumented else "mismatch",
        }
        if missing or undocumented:
            failed = True
            if missing:
                print(f"{kind} artifact is missing header declarations:")
                print("\n".join(missing))
            if undocumented:
                print(f"{kind} artifact exports undocumented C symbols:")
                print("\n".join(undocumented))
    ledger = {
        "schema_version": 1,
        "measurement": (
            "Every FT_/FTC_ symbol in each release shared/static artifact "
            "equals the maintained C header declaration set."
        ),
        "platform": {
            "system": system,
            "target": target,
            "native_system": platform.system(),
        },
        "artifacts": artifacts,
    }
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    LEDGER.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")
    if failed:
        raise SystemExit(1)
    print(
        f"C exports: shared and static artifacts each match "
        f"{len(declared)} header declarations"
    )


if __name__ == "__main__":
    main()
