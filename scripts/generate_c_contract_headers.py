#!/usr/bin/env python3
"""Generate the FreeType-compatible public include-path wrappers."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PINNED_INCLUDE = ROOT / "freetype" / "include"
OUTPUT = ROOT / "fontdone-c-abi" / "include"


def public_header_paths() -> list[Path]:
    paths = [
        Path("freetype") / path.name
        for path in sorted((PINNED_INCLUDE / "freetype").glob("*.h"))
    ]
    paths.append(Path("ft2build.h"))
    if len(paths) != 47:
        raise SystemExit(f"pinned public header denominator drift: {len(paths)} != 47")
    return sorted(paths)


def header_macros() -> list[tuple[str, str]]:
    source = (
        PINNED_INCLUDE / "freetype" / "config" / "ftheader.h"
    ).read_text()
    rows = re.findall(
        r"^\s*#\s*define\s+(FT_[A-Z0-9_]+_H)\s+"
        r"(<freetype/[^>]+>)",
        source,
        re.M,
    )
    public = [
        (name, value)
        for name, value in rows
        if "/config/" not in value
    ]
    if not public:
        raise SystemExit("no public ft2build include macros discovered")
    return public


def guard_for(path: Path) -> str:
    return "FONTDONE_" + re.sub(r"[^A-Za-z0-9]", "_", path.as_posix()).upper()


def render_wrapper(path: Path) -> str:
    guard = guard_for(path)
    return "\n".join(
        (
            f"#ifndef {guard}",
            f"#define {guard}",
            "",
            "/* Generated FreeType 2.14.3 compatibility include path. */",
            '#include "../fontdone_ffi.h"',
            "",
            "#endif",
            "",
        )
    )


def render_ft2build() -> str:
    lines = [
        "#ifndef FONTDONE_FT2BUILD_H",
        "#define FONTDONE_FT2BUILD_H",
        "",
        "/* Generated FreeType 2.14.3 compatibility include selectors. */",
    ]
    for name, value in header_macros():
        lines.extend((f"#ifndef {name}", f"#define {name} {value}", "#endif"))
    lines.extend(("", "#endif", ""))
    return "\n".join(lines)


def expected_files() -> dict[Path, str]:
    return {
        OUTPUT / path: (
            render_ft2build()
            if path == Path("ft2build.h")
            else render_wrapper(path)
        )
        for path in public_header_paths()
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = expected_files()
    if args.check:
        stale = [
            path
            for path, content in expected.items()
            if not path.exists() or path.read_text() != content
        ]
        if stale:
            raise SystemExit(
                "generated C include paths are stale: "
                + ", ".join(str(path) for path in stale)
            )
        print(f"checked {len(expected)} generated C include paths")
        return
    for path, content in expected.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
    print(f"generated {len(expected)} C include paths")


if __name__ == "__main__":
    main()
