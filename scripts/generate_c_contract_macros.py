#!/usr/bin/env python3
"""Generate the pinned FreeType public macro compatibility surface."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
from pathlib import Path

import audit_api_abi


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "fontdone-c-abi" / "include" / "fontdone_macros.h"

def pinned_contract_definitions() -> tuple[dict[str, str], dict[str, int]]:
    inventory = audit_api_abi.parse_c_headers(ROOT / "freetype" / "include")
    names = set(inventory["macros"])
    command = audit_api_abi.clang_base_command(inventory, local=False)
    command.extend(("-dM", "-E", "-x", "c", os.devnull))
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    definitions: dict[str, str] = {}
    for line in completed.stdout.splitlines():
        match = re.match(r"#define\s+([A-Za-z_][A-Za-z0-9_]*)(.*)", line)
        if match and match.group(1) in names:
            definitions[match.group(1)] = match.group(2)
    definitions.update(
        {
            name: replacement
            for name, replacement in audit_api_abi.TRANSIENT_MACRO_DEFAULTS.items()
            if name in names
        }
    )
    missing = sorted(names - set(definitions))
    extra = sorted(set(definitions) - names)
    if missing or extra:
        raise SystemExit(
            f"public macro inventory mismatch: missing={missing}, extra={extra}"
        )
    if len(definitions) != audit_api_abi.PINNED_COUNTS["c_macros"]:
        raise SystemExit(
            "public macro denominator drift: "
            f"{len(definitions)} != "
            f"{audit_api_abi.PINNED_COUNTS['c_macros']}"
        )
    error_names = set(inventory["error_codes"])
    error_values = audit_api_abi.enum_constant_values(
        audit_api_abi.clang_ast(inventory, local=False),
        error_names,
    )
    missing_errors = sorted(error_names - set(error_values))
    if missing_errors:
        raise SystemExit(
            f"pinned error value inventory incomplete: {missing_errors}"
        )
    return definitions, {
        name: row["value"] for name, row in error_values.items()
    }


def render(
    definitions: dict[str, str],
    error_values: dict[str, int],
) -> str:
    lines = [
        "#ifndef FONTDONE_FREETYPE_2_14_3_MACROS_H",
        "#define FONTDONE_FREETYPE_2_14_3_MACROS_H",
        "",
        "/* Generated from the pinned FreeType 2.14.3 public macro surface.",
        " * Regenerate with scripts/generate_c_contract_macros.py.",
        " * FreeType-compatible definitions are distributed under FTL.TXT.",
        " */",
        "",
    ]
    for name, replacement in sorted(definitions.items()):
        lines.extend(
            (
                f"#ifndef {name}",
                f"#define {name}{replacement}",
                "#endif",
            )
        )
    lines.extend(
        (
            "",
            "/* Pinned public error and module-error values. */",
            "",
        )
    )
    for name, value in sorted(error_values.items()):
        lines.extend(
            (
                f"#ifndef {name}",
                f"#define {name} {value}",
                "#endif",
            )
        )
    lines.extend(("", "#endif", ""))
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    output = args.output if args.output.is_absolute() else ROOT / args.output
    definitions, error_values = pinned_contract_definitions()
    rendered = render(definitions, error_values)
    if args.check:
        if not output.exists() or output.read_text() != rendered:
            raise SystemExit(f"generated C macro surface is stale: {output}")
        print(f"checked {output}")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered)
    print(output)


if __name__ == "__main__":
    main()
