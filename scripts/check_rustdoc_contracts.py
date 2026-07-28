#!/usr/bin/env python3
"""Enforce useful public documentation at each supported Rust boundary."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SURFACE_FILES = (ROOT / "src" / "api.rs", ROOT / "src" / "font.rs", ROOT / "src" / "render.rs")
CRATE_ROOTS = (
    ROOT / "src" / "lib.rs",
    ROOT / "fontdone-c-abi" / "src" / "lib.rs",
    ROOT / "fontdone-wasm" / "src" / "lib.rs",
)
IMPLEMENTATION_DOC_EXCEPTIONS = {
    ROOT / "src" / "ffi" / "constants.rs",
    ROOT / "src" / "ffi" / "convert.rs",
    ROOT / "src" / "ffi" / "handles.rs",
    ROOT / "src" / "ffi" / "types.rs",
    ROOT / "fontdone-c-abi" / "src" / "implementation.rs",
    ROOT / "fontdone-wasm" / "src" / "implementation.rs",
}


def preceding_docs(lines: list[str], index: int) -> list[str]:
    docs: list[str] = []
    cursor = index - 1
    while cursor >= 0 and not lines[cursor].strip():
        cursor -= 1
    while cursor >= 0 and lines[cursor].lstrip().startswith("#["):
        cursor -= 1
        while cursor >= 0 and not lines[cursor].strip():
            cursor -= 1
    while cursor >= 0 and lines[cursor].lstrip().startswith("///"):
        docs.append(lines[cursor])
        cursor -= 1
    return docs


def result_functions_without_errors(path: Path) -> list[tuple[int, str]]:
    lines = path.read_text(encoding="utf-8").splitlines()
    missing: list[tuple[int, str]] = []
    for index, line in enumerate(lines):
        match = re.search(r"\bpub fn\s+([A-Za-z0-9_]+)", line)
        if match is None:
            continue
        signature = line
        cursor = index + 1
        while "{" not in signature and cursor < len(lines):
            signature += " " + lines[cursor]
            cursor += 1
        if re.search(r"->\s*(?:[A-Za-z0-9_:]+::)?Result\s*<", signature) is None:
            continue
        if not any("# Errors" in doc for doc in preceding_docs(lines, index)):
            missing.append((index + 1, match.group(1)))
    return missing


def public_functions(path: Path) -> set[str]:
    text = path.read_text(encoding="utf-8")
    return set(
        re.findall(
            r"(?m)^pub\s+(?:unsafe\s+)?(?:extern\s+\"C\"\s+)?fn\s+"
            r"([A-Za-z_][A-Za-z0-9_]*)",
            text,
        )
    )


def macro_names(path: Path, macro_name: str) -> set[str]:
    """Return identifiers in every invocation of one simple list macro."""
    text = path.read_text(encoding="utf-8")
    names: set[str] = set()
    pattern = re.compile(rf"\b{re.escape(macro_name)}!\s*\(")
    for match in pattern.finditer(text):
        start = match.end()
        depth = 1
        cursor = start
        while cursor < len(text) and depth:
            if text[cursor] == "(":
                depth += 1
            elif text[cursor] == ")":
                depth -= 1
            cursor += 1
        if depth:
            raise ValueError(f"{path}: unterminated {macro_name}! invocation")
        names.update(re.findall(r"\b[A-Za-z_][A-Za-z0-9_]*\b", text[start : cursor - 1]))
    return names


def check_documented_function_facade(
    errors: list[str],
    *,
    label: str,
    implementation_paths: tuple[Path, ...],
    facade: Path,
    macros: tuple[str, ...],
    individually_documented: set[str],
) -> None:
    defined: set[str] = set()
    for path in implementation_paths:
        defined.update(public_functions(path))
    documented = set(individually_documented)
    for macro in macros:
        documented.update(macro_names(facade, macro))
    for name in sorted(defined - documented):
        errors.append(f"{label}: public callable {name} lacks a facade rustdoc contract")
    for name in sorted(documented - defined):
        errors.append(f"{label}: documented callable {name} has no implementation")


def rust_sources() -> list[Path]:
    roots = (ROOT / "src", ROOT / "fontdone-c-abi" / "src", ROOT / "fontdone-wasm" / "src")
    return sorted(path for root in roots for path in root.rglob("*.rs"))


def main() -> int:
    errors: list[str] = []
    for path in SURFACE_FILES:
        for line, name in result_functions_without_errors(path):
            errors.append(
                f"{path.relative_to(ROOT)}:{line}: public Result function {name} lacks # Errors"
            )

    implementation_suppression = re.compile(
        r"#!\s*\[(?:allow|expect)\s*\([^]]*\bmissing_docs\b",
        re.DOTALL,
    )
    for path in rust_sources():
        text = path.read_text(encoding="utf-8")
        if not implementation_suppression.search(text):
            continue
        if path in CRATE_ROOTS:
            errors.append(f"{path.relative_to(ROOT)}: crate-root missing_docs suppression is forbidden")
        elif path not in IMPLEMENTATION_DOC_EXCEPTIONS:
            errors.append(
                f"{path.relative_to(ROOT)}: missing_docs suppression is outside an audited ABI implementation"
            )
        elif "#![expect(" not in text:
            errors.append(
                f"{path.relative_to(ROOT)}: ABI implementation must use an explained expect, not allow"
            )

    check_documented_function_facade(
        errors,
        label="fontdone::ffi",
        implementation_paths=(ROOT / "src" / "ffi" / "handles.rs", ROOT / "src" / "ffi" / "convert.rs"),
        facade=ROOT / "src" / "ffi" / "mod.rs",
        macros=("export_freetype_routes", "export_parity_helpers"),
        individually_documented={
            "FT_Init_FreeType",
            "FT_LOAD_TARGET_MODE",
            "glyph_format_from_core",
            "load_flags_to_core",
            "pixel_mode_from_core",
            "render_mode_to_core",
        },
    )
    check_documented_function_facade(
        errors,
        label="fontdone-c-abi",
        implementation_paths=(ROOT / "fontdone-c-abi" / "src" / "implementation.rs",),
        facade=ROOT / "fontdone-c-abi" / "src" / "lib.rs",
        macros=("document_c_entry_points", "document_abi_test_support"),
        individually_documented={"FT_Init_FreeType"},
    )
    check_documented_function_facade(
        errors,
        label="fontdone-wasm",
        implementation_paths=(ROOT / "fontdone-wasm" / "src" / "implementation.rs",),
        facade=ROOT / "fontdone-wasm" / "src" / "lib.rs",
        macros=("document_wasm_entry_points", "document_wasm_test_support"),
        individually_documented=set(),
    )

    c_readme = (ROOT / "fontdone-c-abi" / "README.md").read_text(encoding="utf-8")
    wasm_readme = (ROOT / "fontdone-wasm" / "README.md").read_text(encoding="utf-8")
    for heading in ("lifecycle", "pointer", "ownership"):
        if heading not in c_readme.lower():
            errors.append(f"fontdone-c-abi/README.md: missing raw-ABI {heading} contract")
    for heading in ("memory", "allocation", "ownership", "layout"):
        if heading not in wasm_readme.lower():
            errors.append(f"fontdone-wasm/README.md: missing raw-ABI {heading} contract")
    if errors:
        for error in errors:
            print(f"rustdoc contract error: {error}", file=sys.stderr)
        return 1
    print("rustdoc contracts: Result, facade-callable, and scoped implementation policies clean")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
