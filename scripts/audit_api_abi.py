#!/usr/bin/env python3
"""Generate a FreeType C-to-fontdone API/ABI surface audit.

The audit compares:

1. Pinned FreeType C public headers under ``freetype/include/freetype``.
2. This crate's public Rust surface plus ``tests/data/interface_map.json``.

The output is intentionally written to ``target/`` because it is a generated
diagnostic artifact, not a committed oracle fixture.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "target" / "api-abi-audit"
LOCAL_C_HEADER = ROOT / "fontdone-c-abi" / "include" / "fontdone_ffi.h"
LOCAL_C_MACROS = ROOT / "fontdone-c-abi" / "include" / "fontdone_macros.h"
ROUTE_AUDIT = OUTPUT_DIR / "route_audit.json"
EXTERNAL_C_FUNCTION_LEDGER = OUTPUT_DIR / "external_c_function_ledger.json"
EXACT_ERROR_LEDGER = OUTPUT_DIR / "exact_error_ledger.json"
CONTRACT_INVENTORY = ROOT / "tests" / "data" / "c_contract_inventory.json"
C_CONSUMER_LEDGER = OUTPUT_DIR / "c_consumer_ledger.json"
C_EXPORT_LEDGER = OUTPUT_DIR / "c_export_ledger.json"
RUST_C_LAYOUT_LEDGER = OUTPUT_DIR / "rust_c_layout_ledger.json"
PLATFORM_CONTRACT_DIR = OUTPUT_DIR / "platform-contract"
PINNED_FREETYPE_VERSION = "2.14.3"
PLATFORM_TARGET: str | None = None
PLATFORM_RUNNER: list[str] = []
PLATFORM_LINKER: str | None = None
PLATFORM_CLANG_TARGET: str | None = None
PLATFORM_CLANG_SYSROOT: Path | None = None
PINNED_COUNTS = {
    "c_functions": 218,
    "c_macros": 891,
    "c_typedefs": 62,
    "c_callbacks": 39,
    "c_structs": 78,
    "c_enums": 20,
    "c_enum_variants": 158,
    "c_error_codes": 119,
}
TRANSIENT_MACRO_DEFAULTS = {
    "FT_DEPRECATED_ATTRIBUTE": " __attribute__((deprecated))",
    "FT_ERRORDEF": "(e,v,s) e = v,",
    "FT_ERRORDEF_": (
        "(e,v,s) FT_ERRORDEF(FT_ERR_CAT(FT_ERR_PREFIX, e), "
        "v + FT_ERR_BASE, s)"
    ),
    "FT_ERROR_END_LIST": " FT_ERR_CAT(FT_ERR_PREFIX, Max) };",
    "FT_ERROR_START_LIST": " enum {",
    "FT_ERR_BASE": " 0",
    "FT_ERR_PREFIX": " FT_Err_",
    "FT_INCLUDE_ERR_PROTOS": "",
    "FT_MODERRDEF": "(e,v,s) FT_Mod_Err_ ## e = 0,",
    "FT_MODERR_END_LIST": " FT_Mod_Err_Max };",
    "FT_MODERR_START_LIST": " enum {",
    "FT_NEED_EXTERN_C": "",
    "FT_NOERRORDEF_": (
        "(e,v,s) FT_ERRORDEF(FT_ERR_CAT(FT_ERR_PREFIX, e), v, s)"
    ),
}


def read_text(path: Path) -> str:
    return path.read_text(errors="ignore") if path.exists() else ""


def strip_c_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"//.*", " ", text)


def parse_c_headers(include_root: Path) -> dict:
    inventory = {
        "functions": {},
        "macros": {},
        "typedefs": {},
        "callbacks": {},
        "structs": {},
        "enums": {},
        "enum_variants": {},
        "error_codes": {},
        "fields": {},
    }
    header_root = include_root / "freetype"
    for path in sorted(header_root.rglob("*.h")):
        if not is_public_header(path, include_root):
            continue
        raw = read_text(path)
        text = strip_c_comments(raw)
        rel = str(path.relative_to(include_root))

        for match in re.finditer(
            r"FT_EXPORT\s*\(([^)]*)\)\s*([A-Za-z_][A-Za-z0-9_]*)\s*\((.*?)\)\s*;",
            text,
            re.S,
        ):
            ret, name, params = match.groups()
            inventory["functions"][name] = {
                "return": normalize_ws(ret),
                "params": normalize_ws(params),
                "file": rel,
            }

        for match in re.finditer(r"^\s*#\s*define\s+([A-Za-z_][A-Za-z0-9_]*)\b([^\n]*)", raw, re.M):
            name, value = match.groups()
            if c_public_name(name):
                inventory["macros"][name] = {"value": value.strip(), "file": rel}

        for match in re.finditer(r"typedef\s+([^;{}]+?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*;", text):
            definition, name = match.groups()
            if c_public_name(name):
                inventory["typedefs"][name] = {
                    "definition": normalize_ws(definition),
                    "file": rel,
                }

        for match in re.finditer(
            r"typedef\s+([^;{}]+?)\(\s*\*\s*"
            r"([A-Za-z_][A-Za-z0-9_]*)\s*\)\s*\((.*?)\)\s*;",
            text,
            re.S,
        ):
            return_type, name, params = match.groups()
            if c_public_name(name):
                inventory["callbacks"][name] = {
                    "return": normalize_ws(return_type),
                    "params": normalize_ws(params),
                    "file": rel,
                }

        for kind in ("struct", "enum"):
            pattern = (
                r"typedef\s+"
                + kind
                + r"\s*(?:[A-Za-z_][A-Za-z0-9_]*)?\s*\{(.*?)\}\s*([A-Za-z_][A-Za-z0-9_]*)\s*;"
            )
            for match in re.finditer(pattern, text, re.S):
                body, name = match.groups()
                if not c_public_name(name):
                    continue
                bucket = "structs" if kind == "struct" else "enums"
                inventory[bucket][name] = {"file": rel}
                fields = parse_c_fields(kind, body)
                inventory["fields"][name] = fields
                if kind == "enum":
                    for variant in fields:
                        inventory["enum_variants"][variant] = {
                            "enum": name,
                            "file": rel,
                        }

        parse_error_code_header(inventory, raw, rel)

    return inventory


def is_public_header(path: Path, include_root: Path) -> bool:
    try:
        rel = path.relative_to(include_root)
    except ValueError:
        return False
    parts = rel.parts
    if len(parts) != 2 or parts[0] != "freetype":
        return False
    return parts[1] not in {"ftchapters.h"}


def parse_c_fields(kind: str, body: str) -> list[str]:
    if kind == "enum":
        return re.findall(r"\b([A-Z][A-Z0-9_]+)\b\s*(?:=|,|$)", body)
    fields = []
    for part in body.split(";"):
        part = normalize_ws(part)
        if not part or part.startswith("#"):
            continue
        declarators = part.split(",")
        if not declarators:
            continue
        first = declarators[0]
        first_match = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*(?:\[[^]]+\])?$", first)
        if first_match:
            fields.append(first_match.group(1))
        for declarator in declarators[1:]:
            match = re.search(r"^\s*\*?\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:\[[^]]+\])?$", declarator)
            if match:
                fields.append(match.group(1))
    return fields


def parse_error_code_header(inventory: dict, raw: str, rel: str) -> None:
    if rel == "freetype/fterrdef.h":
        text = strip_c_comments(raw)
        for macro, prefix in (("FT_NOERRORDEF_", "FT_Err_"), ("FT_ERRORDEF_", "FT_Err_")):
            pattern = r"^\s*" + macro + r"\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*,\s*([^,\n]+)"
            for label, value in re.findall(pattern, text, re.M):
                inventory["error_codes"][prefix + label] = {
                    "kind": "error",
                    "value": normalize_ws(value),
                    "file": rel,
                }
    elif rel == "freetype/ftmoderr.h":
        text = strip_c_comments(raw)
        pattern = r"^\s*FT_MODERRDEF\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*,\s*([^,\n]+)"
        for label, value in re.findall(pattern, text, re.M):
            inventory["error_codes"]["FT_Mod_Err_" + label] = {
                "kind": "module_error",
                "value": normalize_ws(value),
                "file": rel,
            }


def parse_fontdone(src_root: Path) -> dict:
    inventory = {"functions": {}, "consts": {}, "structs": {}, "enums": {}, "fields": {}, "modules": {}}
    for path in sorted(src_root.rglob("*.rs")):
        text = read_text(path)
        rel = str(path.relative_to(src_root))
        for match in re.finditer(r"pub\s+mod\s+([A-Za-z_][A-Za-z0-9_]*)", text):
            inventory["modules"][match.group(1)] = {"file": rel}
        for match in re.finditer(
            r"pub(?:\([^)]*\))?\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\((.*?)\)\s*(?:->\s*([^\n{]+))?",
            text,
            re.S,
        ):
            name, params, ret = match.groups()
            inventory["functions"][name] = {
                "params": normalize_ws(params),
                "return": normalize_ws(ret or ""),
                "file": rel,
            }
        for match in re.finditer(r"pub\s+const\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", text):
            inventory["consts"][match.group(1)] = {"file": rel}
        for kind, bucket in (("struct", "structs"), ("enum", "enums")):
            for match in re.finditer(
                r"pub\s+" + kind + r"\s+([A-Za-z_][A-Za-z0-9_]*)(?:<[^>{]+>)?\s*\{(.*?)\n\}",
                text,
                re.S,
            ):
                name, body = match.groups()
                inventory[bucket][name] = {"file": rel}
                if kind == "struct":
                    fields = re.findall(r"pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", body)
                else:
                    fields = re.findall(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:,|\(|\{)", body, re.M)
                inventory["fields"][name] = fields
    return inventory


def load_interface_map(path: Path) -> dict:
    if not path.exists():
        return {}
    data = json.loads(path.read_text())
    mapped = {}
    for group in data["paths"]:
        for symbol, meta in group["symbols"].items():
            mapped[symbol] = {
                "path": group["path"],
                "status": meta.get("status", "unknown"),
                "rust": meta.get("rust"),
                "reason": meta.get("reason"),
            }
    return mapped


def c_public_name(name: str) -> bool:
    return name.startswith(("FT_", "TT_", "T1_", "FTC_"))


def normalize_ws(text: str) -> str:
    return " ".join(text.split())


TYPE_MAP = {
    "FT_BBox": "BBox",
    "FT_Bitmap": "RenderedBitmap",
    "FT_CharMapRec": "CharmapInfo",
    "FT_FaceRec": "FaceInfo/Face",
    "FT_GlyphSlotRec": "GlyphSlot",
    "FT_Glyph_Metrics": "GlyphSlotMetrics",
    "FT_Outline": "Outline",
    "FT_Pixel_Mode": "PixelMode",
    "FT_Render_Mode": "RenderMode",
    "FT_Size_Metrics": "SizeMetrics",
    "FT_Vector": "Vector",
}


def classify_function(symbol: str, c: dict, interface: dict) -> dict:
    meta = interface.get(symbol, {})
    status = meta.get("status", "unknown")
    rust = meta.get("rust")
    exactness = "unmapped"
    if status == "complete":
        exactness = "semantic_mapped"
    elif status == "partial":
        exactness = "partial_semantic"
    elif status == "planned":
        exactness = "not_implemented"
    elif status == "out_of_scope":
        exactness = "intentionally_excluded"
    return {
        "symbol": symbol,
        "c_return": c["functions"][symbol]["return"],
        "c_params": c["functions"][symbol]["params"],
        "c_file": c["functions"][symbol]["file"],
        "fontdone_status": status,
        "fontdone_mapping": rust or "",
        "exactness": exactness,
    }


def classify_type(name: str, c: dict, fontdone: dict) -> dict:
    mapped = TYPE_MAP.get(name, "")
    fields = c["fields"].get(name, [])
    our_fields = fontdone["fields"].get(mapped, []) if mapped in fontdone["fields"] else []
    field_exact = bool(mapped) and fields == our_fields
    source = c["structs"].get(name) or c["enums"].get(name) or c["typedefs"].get(name) or {}
    return {
        "type": name,
        "kind": "struct" if name in c["structs"] else "enum" if name in c["enums"] else "typedef",
        "c_file": source.get("file", ""),
        "fontdone_mapping": mapped,
        "c_field_count": len(fields),
        "fontdone_field_count": len(our_fields),
        "field_order_exact": field_exact,
        "c_fields": fields,
        "fontdone_fields": our_fields,
    }


def classify_constant(name: str, c: dict) -> dict:
    mapped = ""
    if name.startswith("FT_LOAD_"):
        mapped = "LoadFlags subset"
    elif name.startswith("FT_RENDER_MODE_"):
        mapped = "RenderMode"
    elif name.startswith("FT_PIXEL_MODE_"):
        mapped = "PixelMode"
    return {
        "constant": name,
        "c_value": c["macros"][name]["value"],
        "c_file": c["macros"][name]["file"],
        "fontdone_mapping": mapped,
    }


def classify_enum_variant(name: str, c: dict) -> dict:
    source = c["enum_variants"][name]
    return {
        "constant": name,
        "kind": "enum_variant",
        "enum": source["enum"],
        "c_value": "",
        "c_file": source["file"],
        "fontdone_mapping": "",
    }


def classify_error_code(name: str, c: dict) -> dict:
    source = c["error_codes"][name]
    return {
        "constant": name,
        "kind": source["kind"],
        "enum": "",
        "c_value": source["value"],
        "c_file": source["file"],
        "fontdone_mapping": "",
    }


def classify_callback(name: str, c: dict) -> dict:
    source = c["callbacks"][name]
    return {
        "callback": name,
        "c_return": source["return"],
        "c_params": source["params"],
        "c_file": source["file"],
    }


def parse_local_c_header() -> dict:
    raw = read_text(LOCAL_C_HEADER) + "\n" + read_text(LOCAL_C_MACROS)
    text = strip_c_comments(raw)
    surface = {
        "functions": {},
        "macros": {},
        "typedefs": set(),
        "callbacks": {},
        "structs": {},
        "enums": {},
        "enum_variants": set(),
    }
    for match in re.finditer(
        r"^\s*([A-Za-z_][A-Za-z0-9_\s*]*?)\s+"
        r"((?:FT|FTC)_[A-Za-z0-9_]+)\s*\(([^;{}]*)\)\s*;",
        text,
        re.M,
    ):
        return_type, name, params = match.groups()
        if not return_type.strip().startswith("typedef"):
            surface["functions"][name] = {
                "return": normalize_ws(return_type),
                "params": normalize_ws(params),
            }
    for name, value in re.findall(
        r"^\s*#\s*define\s+([A-Za-z_][A-Za-z0-9_]*)\b([^\n]*)",
        raw,
        re.M,
    ):
        if c_public_name(name):
            surface["macros"][name] = normalize_ws(value)
    for match in re.finditer(
        r"typedef\s+([^;{}]+?)\(\s*\*\s*"
        r"([A-Za-z_][A-Za-z0-9_]*)\s*\)\s*\((.*?)\)\s*;",
        text,
        re.S,
    ):
        return_type, name, params = match.groups()
        if c_public_name(name):
            surface["callbacks"][name] = {
                "return": normalize_ws(return_type),
                "params": normalize_ws(params),
            }
            surface["typedefs"].add(name)
    for definition, name in re.findall(
        r"typedef\s+([^;{}]+?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*;",
        text,
    ):
        if c_public_name(name):
            surface["typedefs"].add(name)
    for kind, bucket in (("struct", "structs"), ("enum", "enums")):
        pattern = (
            r"typedef\s+"
            + kind
            + r"\s*(?:[A-Za-z_][A-Za-z0-9_]*)?\s*\{(.*?)\}\s*"
            r"([A-Za-z_][A-Za-z0-9_]*)\s*;"
        )
        for body, name in re.findall(pattern, text, re.S):
            if not c_public_name(name):
                continue
            fields = parse_c_fields(kind, body)
            surface[bucket][name] = fields
            surface["typedefs"].add(name)
            if kind == "enum":
                surface["enum_variants"].update(fields)
    return surface


def walk_ast(node: object):
    if not isinstance(node, dict):
        return
    yield node
    for child in node.get("inner", []):
        yield from walk_ast(child)


def clang_base_command(c: dict, *, local: bool) -> list[str]:
    compiler = os.environ.get("CLANG", "clang")
    if shutil.which(compiler) is None:
        raise SystemExit(f"C ABI contract requires Clang: {compiler!r} not found")
    target_flags = []
    if PLATFORM_CLANG_TARGET:
        target_flags.append(f"--target={PLATFORM_CLANG_TARGET}")
    if PLATFORM_CLANG_SYSROOT:
        target_flags.extend(
            (
                f"--sysroot={PLATFORM_CLANG_SYSROOT}",
                "-isystem",
                str(PLATFORM_CLANG_SYSROOT / "include"),
            )
        )
    if local:
        return [
            compiler,
            *target_flags,
            "-std=c11",
            "-Werror",
            f"-I{LOCAL_C_HEADER.parent}",
            "-include",
            LOCAL_C_HEADER.name,
        ]
    include_root = ROOT / "freetype" / "include"
    header_paths = {
        row["file"]
        for bucket in (
            "functions",
            "macros",
            "typedefs",
            "callbacks",
            "structs",
            "enums",
            "enum_variants",
            "error_codes",
        )
        for row in c[bucket].values()
        if row.get("file")
    }
    # These two headers cannot be consumed directly on the native hosts used
    # by the contract audit. fterrdef.h is intentionally reincluded through
    # fterrors.h with caller-provided macros; ftmac.h needs classic Mac SDK
    # types and contributes no declaration to the pinned portable inventory.
    header_paths -= {"freetype/fterrdef.h", "freetype/ftmac.h"}
    command = [
        compiler,
        *target_flags,
        "-std=c11",
        "-Werror",
        f"-I{include_root}",
        "-include",
        "ft2build.h",
    ]
    for header in sorted(header_paths):
        command.extend(("-include", header))
    return command


def clang_ast(c: dict, *, local: bool) -> dict:
    command = clang_base_command(c, local=local)
    command.extend(
        (
            "-Xclang",
            "-ast-dump=json",
            "-fsyntax-only",
            "-x",
            "c",
            os.devnull,
        )
    )
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def clang_macro_definitions(c: dict, *, local: bool) -> dict[str, str]:
    command = clang_base_command(c, local=local)
    command.extend(("-dM", "-E", "-x", "c", os.devnull))
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    definitions = {}
    for line in completed.stdout.splitlines():
        match = re.match(
            r"#define\s+([A-Za-z_][A-Za-z0-9_]*)(.*)",
            line,
        )
        if match:
            definitions[match.group(1)] = match.group(2)
    if not local:
        definitions.update(TRANSIENT_MACRO_DEFAULTS)
    return definitions


def canonical_macro_definition(definition: str) -> str:
    return normalize_ws(definition)


def enum_constant_values(ast: dict, names: set[str]) -> dict[str, dict]:
    values = {}
    for enum in walk_ast(ast):
        if enum.get("kind") != "EnumDecl":
            continue
        previous = -1
        for child in enum.get("inner", []):
            if child.get("kind") != "EnumConstantDecl":
                continue
            value_node = next(
                (
                    node
                    for node in walk_ast(child)
                    if node.get("kind") == "ConstantExpr"
                    and node.get("value") is not None
                ),
                None,
            )
            value = (
                int(value_node["value"])
                if value_node is not None
                else previous + 1
            )
            previous = value
            name = child.get("name")
            if name in names:
                values[name] = {
                    "value": value,
                    "type": child.get("type", {}).get("qualType", ""),
                }
    return values


def clang_integer_expression_values(
    c: dict,
    *,
    local: bool,
    names: set[str],
) -> dict[str, int]:
    ordered = sorted(names)
    source = "\n".join(
        f"enum {{ FONTDONE_VALUE_{index} = ({name}) }};"
        for index, name in enumerate(ordered)
    )
    command = clang_base_command(c, local=local)
    command.extend(
        (
            "-Xclang",
            "-ast-dump=json",
            "-fsyntax-only",
            "-x",
            "c",
            "-",
        )
    )
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        input=source,
        capture_output=True,
        text=True,
    )
    ast = json.loads(completed.stdout)
    indexed = {}
    for node in walk_ast(ast):
        name = node.get("name", "")
        match = re.fullmatch(r"FONTDONE_VALUE_(\d+)", name)
        if node.get("kind") != "EnumConstantDecl" or match is None:
            continue
        value_node = next(
            (
                child
                for child in walk_ast(node)
                if child.get("kind") == "ConstantExpr"
                and child.get("value") is not None
            ),
            None,
        )
        if value_node is not None:
            indexed[int(match.group(1))] = int(value_node["value"])
    return {
        name: indexed[index]
        for index, name in enumerate(ordered)
        if index in indexed
    }


def first_function_proto(node: dict) -> dict | None:
    return next(
        (
            child
            for child in walk_ast(node)
            if child.get("kind") == "FunctionProtoType"
        ),
        None,
    )


def field_contract(node: dict) -> dict:
    field_type = node.get("type", {})
    bit_width = None
    if node.get("isBitfield"):
        width_node = next(
            (
                child
                for child in walk_ast(node)
                if child.get("kind") in {"ConstantExpr", "IntegerLiteral"}
                and child.get("value") is not None
            ),
            None,
        )
        bit_width = width_node.get("value") if width_node else "unknown"
    return {
        "name": node.get("name", ""),
        "type": field_type.get("qualType", ""),
        "desugared_type": field_type.get(
            "desugaredQualType",
            field_type.get("qualType", ""),
        ),
        "bit_width": bit_width,
    }


def compiler_surface(ast: dict, names: dict[str, set[str]]) -> dict:
    surface = {
        "functions": {},
        "typedefs": {},
        "callbacks": {},
        "records": {},
    }
    for node in walk_ast(ast):
        kind = node.get("kind")
        name = node.get("name")
        if kind == "FunctionDecl" and name in names["functions"]:
            surface["functions"][name] = node.get("type", {}).get("qualType", "")
        elif kind == "TypedefDecl" and name in names["typedefs"]:
            type_data = node.get("type", {})
            typedef_type = {
                "type": type_data.get("qualType", ""),
                "desugared_type": type_data.get(
                    "desugaredQualType",
                    type_data.get("qualType", ""),
                ),
            }
            surface["typedefs"][name] = typedef_type
            proto = first_function_proto(node)
            if proto is not None:
                surface["callbacks"][name] = {
                    **typedef_type,
                    "calling_convention": proto.get("cc", "cdecl"),
                }
        elif (
            kind == "RecordDecl"
            and name
            and node.get("completeDefinition")
            and name in names["record_tags"]
        ):
            surface["records"][name] = [
                field_contract(child)
                for child in node.get("inner", [])
                if child.get("kind") == "FieldDecl"
            ]
    return surface


def parse_layout_dump(text: str, record_tags: set[str]) -> dict:
    layouts = {}
    for raw_block in text.split("*** Dumping AST Record Layout"):
        lines = [line for line in raw_block.splitlines() if line.strip()]
        if not lines:
            continue
        header = re.match(r"^\s*0\s+\|\s+(?:struct|union)\s+(\S+)\s*$", lines[0])
        if header is None or header.group(1) not in record_tags:
            continue
        tag = header.group(1)
        fields = {}
        size = None
        align = None
        for line in lines[1:]:
            summary = re.match(
                r"^\s*\|\s+\[sizeof=(\d+),\s+align=(\d+)\]\s*$",
                line,
            )
            if summary:
                size = int(summary.group(1))
                align = int(summary.group(2))
                continue
            field = re.match(r"^\s*([^|]+?)\s+\|(\s+)(.+?)\s*$", line)
            if field is None or len(field.group(2)) != 3:
                continue
            field_name = re.search(
                r"([A-Za-z_][A-Za-z0-9_]*)\s*$",
                field.group(3),
            )
            if field_name:
                fields[field_name.group(1)] = normalize_ws(field.group(1))
        if size is not None and align is not None:
            layouts[tag] = {
                "size": size,
                "align": align,
                "field_offsets": fields,
            }
    return layouts


def clang_layouts(c: dict, *, local: bool, record_tags: set[str]) -> dict:
    command = clang_base_command(c, local=local)
    command.extend(
        (
            "-Xclang",
            "-fdump-record-layouts-complete",
            "-fsyntax-only",
            "-x",
            "c",
            os.devnull,
        )
    )
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return parse_layout_dump(
        completed.stdout + completed.stderr,
        record_tags,
    )


def typedef_record_tag(typedef_type: dict | None) -> str | None:
    if typedef_type is None:
        return None
    match = re.fullmatch(
        r"(?:struct|union)\s+(\S+)",
        typedef_type["type"],
    )
    return match.group(1) if match else None


def compiler_contract(c: dict, data: dict) -> dict:
    function_names = {row["symbol"] for row in data["functions"]}
    all_type_names = {row["type"] for row in data["types"]}
    contract_type_names = {
        row["type"]
        for row in data["types"]
        if row["kind"] in {"typedef", "enum"}
    }
    callback_names = {row["callback"] for row in data["callbacks"]}
    macro_names = {row["constant"] for row in data["constants"]}
    enum_variant_names = {
        row["constant"] for row in data["enum_variants"]
    }
    error_names = {row["constant"] for row in data["error_codes"]}
    struct_names = {
        row["type"] for row in data["types"] if row["kind"] == "struct"
    }
    names = {
        "functions": function_names,
        "typedefs": all_type_names | callback_names,
        "record_tags": set(),
    }
    pinned_ast = clang_ast(c, local=False)
    local_ast = clang_ast(c, local=True)
    pinned = compiler_surface(pinned_ast, names)
    local = compiler_surface(local_ast, names)
    pinned_sets = {
        "functions": set(pinned["functions"]),
        "types": set(pinned["typedefs"]),
        "callbacks": set(pinned["callbacks"]),
    }
    expected_sets = {
        "functions": function_names,
        "types": all_type_names | callback_names,
        "callbacks": callback_names,
    }
    for label, expected in expected_sets.items():
        missing = sorted(expected - pinned_sets[label])
        extra = sorted(pinned_sets[label] - expected)
        if missing or extra:
            raise SystemExit(
                f"Clang pinned {label} inventory mismatch: "
                f"missing={missing}, extra={extra}"
            )

    pinned_record_tags = {
        tag
        for name in all_type_names
        if (tag := typedef_record_tag(pinned["typedefs"].get(name)))
    }
    local_record_tags = {
        tag
        for name in all_type_names
        if (tag := typedef_record_tag(local["typedefs"].get(name)))
    }
    names["record_tags"] = pinned_record_tags | local_record_tags
    # Record definitions are selected only after their tags are known from the
    # typedef declarations, so collect the two ASTs once more without rerunning
    # Clang.
    pinned["records"] = compiler_surface(pinned_ast, names)["records"]
    local["records"] = compiler_surface(local_ast, names)["records"]
    pinned_layouts = clang_layouts(
        c,
        local=False,
        record_tags=pinned_record_tags,
    )
    local_layouts = clang_layouts(
        c,
        local=True,
        record_tags=local_record_tags,
    )
    missing_record_evidence = []
    for name in sorted(struct_names):
        tag = typedef_record_tag(pinned["typedefs"].get(name))
        if tag is None:
            missing_record_evidence.append(f"{name}: no record tag")
        elif tag not in pinned["records"]:
            missing_record_evidence.append(f"{name}: no Clang record definition")
        elif tag not in pinned_layouts:
            missing_record_evidence.append(f"{name}: no Clang record layout")
    if missing_record_evidence:
        raise SystemExit(
            "Clang pinned record inventory incomplete: "
            + ", ".join(missing_record_evidence)
        )

    function_debt = []
    exact_functions = 0
    for name in sorted(function_names):
        expected = pinned["functions"].get(name)
        actual = local["functions"].get(name)
        if expected is not None and expected == actual:
            exact_functions += 1
        else:
            function_debt.append(
                f"{name}: pinned={expected or '<missing>'}; "
                f"local={actual or '<missing>'}"
            )

    type_debt = []
    exact_types = 0
    for name in sorted(contract_type_names):
        expected = pinned["typedefs"].get(name)
        actual = local["typedefs"].get(name)
        if (
            expected is not None
            and actual is not None
            and expected["desugared_type"] == actual["desugared_type"]
        ):
            exact_types += 1
        else:
            type_debt.append(
                f"{name}: pinned={expected or '<missing>'}; "
                f"local={actual or '<missing>'}"
            )

    callback_debt = []
    exact_callbacks = 0
    for name in sorted(callback_names):
        expected = pinned["callbacks"].get(name)
        actual = local["callbacks"].get(name)
        expected_abi = (
            {
                "type": expected["desugared_type"],
                "calling_convention": expected["calling_convention"],
            }
            if expected is not None
            else None
        )
        actual_abi = (
            {
                "type": actual["desugared_type"],
                "calling_convention": actual["calling_convention"],
            }
            if actual is not None
            else None
        )
        if expected_abi is not None and expected_abi == actual_abi:
            exact_callbacks += 1
        else:
            callback_debt.append(
                f"{name}: pinned={expected_abi or '<missing>'}; "
                f"local={actual_abi or '<missing>'}"
            )

    layout_debt = []
    exact_layouts = 0
    for name in sorted(struct_names):
        pinned_tag = typedef_record_tag(pinned["typedefs"].get(name))
        local_tag = typedef_record_tag(local["typedefs"].get(name))
        expected_layout = pinned_layouts.get(pinned_tag or "")
        actual_layout = local_layouts.get(local_tag or "")
        expected_fields = pinned["records"].get(pinned_tag or "")
        actual_fields = local["records"].get(local_tag or "")
        expected_abi_fields = (
            [
                {
                    "name": field["name"],
                    "type": field["desugared_type"],
                    "bit_width": field["bit_width"],
                }
                for field in expected_fields
            ]
            if expected_fields is not None
            else None
        )
        actual_abi_fields = (
            [
                {
                    "name": field["name"],
                    "type": field["desugared_type"],
                    "bit_width": field["bit_width"],
                }
                for field in actual_fields
            ]
            if actual_fields is not None
            else None
        )
        if (
            expected_layout is not None
            and expected_layout == actual_layout
            and expected_abi_fields is not None
            and expected_abi_fields == actual_abi_fields
        ):
            exact_layouts += 1
        else:
            layout_debt.append(
                f"{name}: pinned_tag={pinned_tag or '<missing>'}, "
                f"local_tag={local_tag or '<missing>'}, "
                f"layout_match={expected_layout is not None and expected_layout == actual_layout}, "
                f"fields_match={expected_abi_fields is not None and expected_abi_fields == actual_abi_fields}"
            )

    callback_aliases = {
        name: row["value"]
        for name, row in c["macros"].items()
        if row["value"] in callback_names
    }
    if len(callback_aliases) != 16:
        raise SystemExit(
            "pinned callback-alias inventory drift: "
            f"found {len(callback_aliases)}, expected 16"
        )
    local_macros = parse_local_c_header()["macros"]
    alias_debt = [
        f"{name}: pinned={target}; local={local_macros.get(name, '<missing>')}"
        for name, target in sorted(callback_aliases.items())
        if local_macros.get(name) != target
    ]

    pinned_macro_definitions = clang_macro_definitions(c, local=False)
    local_macro_definitions = clang_macro_definitions(c, local=True)
    macro_debt = []
    exact_macros = 0
    for name in sorted(macro_names):
        expected = pinned_macro_definitions.get(name)
        actual = local_macro_definitions.get(name)
        if (
            expected is not None
            and actual is not None
            and canonical_macro_definition(expected)
            == canonical_macro_definition(actual)
        ):
            exact_macros += 1
        else:
            macro_debt.append(
                f"{name}: pinned={expected or '<missing>'}; "
                f"local={actual or '<missing>'}"
            )

    pinned_enum_values = enum_constant_values(
        pinned_ast,
        enum_variant_names,
    )
    local_enum_values = enum_constant_values(
        local_ast,
        enum_variant_names,
    )
    enum_value_debt = []
    exact_enum_values = 0
    for name in sorted(enum_variant_names):
        expected = pinned_enum_values.get(name)
        actual = local_enum_values.get(name)
        if expected is not None and expected == actual:
            exact_enum_values += 1
        else:
            enum_value_debt.append(
                f"{name}: pinned={expected or '<missing>'}; "
                f"local={actual or '<missing>'}"
            )

    pinned_error_values = clang_integer_expression_values(
        c,
        local=False,
        names=error_names,
    )
    local_error_values = clang_integer_expression_values(
        c,
        local=True,
        names=error_names,
    )
    error_value_debt = []
    exact_error_values = 0
    for name in sorted(error_names):
        expected = pinned_error_values.get(name)
        actual = local_error_values.get(name)
        if expected is not None and expected == actual:
            exact_error_values += 1
        else:
            error_value_debt.append(
                f"{name}: pinned={expected if expected is not None else '<missing>'}; "
                f"local={actual if actual is not None else '<missing>'}"
            )

    return {
        "function_signatures": {
            "complete": exact_functions,
            "total": len(function_names),
            "debt": function_debt,
        },
        "types": {
            "complete": exact_types,
            "total": len(contract_type_names),
            "debt": type_debt,
        },
        "layouts": {
            "complete": exact_layouts,
            "total": len(struct_names),
            "debt": layout_debt,
            "pinned": {
                name: pinned_layouts.get(
                    typedef_record_tag(pinned["typedefs"].get(name)) or ""
                )
                for name in sorted(struct_names)
            },
        },
        "callbacks": {
            "complete": exact_callbacks,
            "total": len(callback_names),
            "debt": callback_debt,
        },
        "callback_aliases": {
            "complete": len(callback_aliases) - len(alias_debt),
            "total": len(callback_aliases),
            "debt": alias_debt,
        },
        "constant_values": {
            "complete": exact_macros + exact_enum_values,
            "total": len(macro_names) + len(enum_variant_names),
            "debt": macro_debt + enum_value_debt,
            "macro_complete": exact_macros,
            "macro_total": len(macro_names),
            "enum_complete": exact_enum_values,
            "enum_total": len(enum_variant_names),
        },
        "error_values": {
            "complete": exact_error_values,
            "total": len(error_names),
            "debt": error_value_debt,
        },
        "clang": {
            "path": shutil.which(os.environ.get("CLANG", "clang")),
            "target": (
                PLATFORM_CLANG_TARGET
                or subprocess.run(
                    [os.environ.get("CLANG", "clang"), "-dumpmachine"],
                    cwd=ROOT,
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip()
            ),
        },
    }


def contract_metric(
    metric_id: str,
    label: str,
    complete: int | None,
    total: int | None,
    evidence: str,
    *,
    blocking: bool = True,
    debt: list[str] | None = None,
) -> dict:
    is_complete = (
        complete is not None
        and total is not None
        and total > 0
        and complete == total
    )
    return {
        "id": metric_id,
        "label": label,
        "complete": complete,
        "total": total,
        "blocking": blocking,
        "is_complete": is_complete,
        "evidence": evidence,
        "debt": debt or [],
    }


def complete_interface_paths() -> tuple[int, int]:
    data = json.loads(
        (ROOT / "tests" / "data" / "interface_map.json").read_text()
    )
    paths = data["paths"]
    complete = sum(
        all(contract["status"] == "complete" for contract in group["symbols"].values())
        for group in paths
    )
    return complete, len(paths)


def drop_in_header_paths(c: dict) -> set[str]:
    pinned = {
        "ft2build.h",
        *{
            row["file"]
            for bucket in (
                "functions",
                "macros",
                "typedefs",
                "callbacks",
                "structs",
                "enums",
                "enum_variants",
                "error_codes",
            )
            for row in c[bucket].values()
            if row.get("file")
        },
    }
    # The pinned distribution contains public headers with no declarations in
    # the parsed buckets. Include every direct public FreeType header.
    pinned.update(
        f"freetype/{path.name}"
        for path in (ROOT / "freetype" / "include" / "freetype").glob("*.h")
    )
    return pinned


def matching_drop_in_headers(c: dict) -> tuple[int, int]:
    pinned = drop_in_header_paths(c)
    local = {
        path.relative_to(LOCAL_C_HEADER.parent).as_posix()
        for path in LOCAL_C_HEADER.parent.rglob("*.h")
    }
    return len(pinned & local), len(pinned)


def compile_drop_in_headers(c: dict) -> dict:
    headers = sorted(drop_in_header_paths(c))
    results: dict[str, dict[str, bool]] = {
        header: {} for header in headers
    }
    debt = []
    for language, compiler, standard in (
        ("c", os.environ.get("CLANG", "clang"), "c11"),
        ("c++", os.environ.get("CLANGXX", "clang++"), "c++17"),
    ):
        if shutil.which(compiler) is None:
            raise SystemExit(
                f"C header contract requires {language} compiler: "
                f"{compiler!r} not found"
            )
        for header in headers:
            completed = subprocess.run(
                [
                    compiler,
                    f"-std={standard}",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    f"-I{LOCAL_C_HEADER.parent}",
                    "-include",
                    header,
                    "-fsyntax-only",
                    "-x",
                    language,
                    os.devnull,
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
            )
            passed = completed.returncode == 0
            results[header][language] = passed
            if not passed:
                diagnostic = normalize_ws(
                    completed.stderr or completed.stdout
                )
                debt.append(
                    f"{header} ({language}): "
                    f"{diagnostic or 'compiler failed without diagnostics'}"
                )
    complete = sum(all(row.values()) for row in results.values())
    return {
        "complete": complete,
        "total": len(headers),
        "debt": debt,
        "results": results,
    }


def rust_binary_record_layouts(
    struct_rows: dict[str, dict],
    expected_layouts: dict[str, dict],
) -> dict:
    """Measure the actual Rust records, not just the generated C header."""

    cargo_command = [
        "cargo",
        "build",
        "--release",
        "-p",
        "fontdone-c-abi",
        "--locked",
    ]
    if PLATFORM_TARGET:
        cargo_command.extend(("--target", PLATFORM_TARGET))
    environment = os.environ.copy()
    if PLATFORM_TARGET and PLATFORM_LINKER:
        linker_key = (
            "CARGO_TARGET_"
            + re.sub(r"[^A-Za-z0-9]", "_", PLATFORM_TARGET).upper()
            + "_LINKER"
        )
        environment.setdefault(linker_key, PLATFORM_LINKER)
    subprocess.run(
        cargo_command,
        cwd=ROOT,
        env=environment,
        check=True,
    )
    release_dir = (
        ROOT / "target" / PLATFORM_TARGET / "release"
        if PLATFORM_TARGET
        else ROOT / "target" / "release"
    )
    rlibs = sorted(
        (release_dir / "deps").glob("libfontdone_c_abi*.rlib"),
        key=lambda path: path.stat().st_mtime_ns,
    )
    if not rlibs:
        raise SystemExit("fontdone-c-abi release rlib is missing")
    rlib = rlibs[-1]
    probe_source = OUTPUT_DIR / "rust_c_layout_probe.rs"
    target_suffix = f"-{PLATFORM_TARGET}" if PLATFORM_TARGET else ""
    executable_suffix = (
        ".exe"
        if (PLATFORM_TARGET or rust_host_target()).endswith("-windows-msvc")
        else ""
    )
    probe_binary = (
        OUTPUT_DIR / f"rust_c_layout_probe{target_suffix}{executable_suffix}"
    )
    field_names = {"type": "type_"}
    source = [
        "#![allow(non_snake_case)]",
        "use fontdone_c_abi::*;",
        "use std::mem::{align_of, offset_of, size_of};",
        "fn main() {",
    ]
    for name, row in sorted(struct_rows.items()):
        source.append(
            f'println!("T\\t{name}\\t{{}}\\t{{}}", '
            f"size_of::<{name}>(), align_of::<{name}>());"
        )
        for field in row["c_fields"]:
            rust_field = field_names.get(field, field)
            source.append(
                f'println!("F\\t{name}\\t{field}\\t{{}}", '
                f"offset_of!({name}, {rust_field}));"
            )
    source.append("}")
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    probe_source.write_text("\n".join(source) + "\n")
    command = [
        os.environ.get("RUSTC", "rustc"),
        "--edition=2024",
        str(probe_source),
    ]
    if PLATFORM_TARGET:
        command.extend(("--target", PLATFORM_TARGET))
    command.extend(
        [
            "--extern",
            f"fontdone_c_abi={rlib}",
            "-L",
            f"dependency={release_dir / 'deps'}",
            "-o",
            str(probe_binary),
        ]
    )
    if PLATFORM_LINKER:
        command.extend(("-C", f"linker={PLATFORM_LINKER}"))
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    completed = subprocess.run(
        [*PLATFORM_RUNNER, str(probe_binary)],
        cwd=ROOT,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    )
    actual: dict[str, dict] = {}
    for line in completed.stdout.splitlines():
        parts = line.split("\t")
        if parts[0] == "T" and len(parts) == 4:
            actual[parts[1]] = {
                "size": int(parts[2]),
                "align": int(parts[3]),
                "field_offsets": {},
            }
        elif parts[0] == "F" and len(parts) == 4:
            actual[parts[1]]["field_offsets"][parts[2]] = int(parts[3])
        else:
            raise SystemExit(f"invalid Rust C-layout probe row: {line}")

    debt = []
    exact = 0
    for name in sorted(struct_rows):
        expected = expected_layouts.get(name)
        observed = actual.get(name)
        if expected is None or observed is None:
            debt.append(
                f"{name}: pinned={expected or '<missing>'}; "
                f"rust={observed or '<missing>'}"
            )
            continue
        normalized_expected = {
            "size": expected["size"],
            "align": expected["align"],
            "field_offsets": {
                field: int(offset)
                for field, offset in expected["field_offsets"].items()
            },
        }
        if normalized_expected == observed:
            exact += 1
        else:
            debt.append(
                f"{name}: pinned={normalized_expected}; rust={observed}"
            )
    ledger = {
        "schema_version": 1,
        "measurement": (
            "Rust size_of, align_of, and offset_of for every public C record "
            "compared with the pinned Clang layout on the active target"
        ),
        "target": PLATFORM_TARGET or rust_host_target(),
        "execution_runner": PLATFORM_RUNNER,
        "rlib": str(rlib.relative_to(ROOT)),
        "rlib_sha256": file_sha256(rlib),
        "complete": exact,
        "total": len(struct_rows),
        "debt": debt,
        "records": actual,
    }
    (OUTPUT_DIR / "rust_c_layout_ledger.json").write_text(
        json.dumps(ledger, indent=2) + "\n"
    )
    return ledger


def route_measurements(route_audit: dict, exact_error_evidence: dict) -> dict:
    rows = [
        row
        for row in route_audit["rows"]
        if row.get("contract_scope", "pinned_c") == "pinned_c"
    ]
    category_counts = Counter(row["category"] for row in rows)
    fallback_categories = {
        category
        for category in category_counts
        if "fallback" in category or category == "explicit-unsupported"
    }
    exact_runtime = (
        category_counts["real-parity"] + category_counts["real-null-validation"]
    )
    error_rows = [row for row in rows if row["expect_error"]]
    searchable = lambda row: " ".join(
        (
            row["case_id"],
            row["subject"],
            row["case"],
            row["reason"],
        )
    ).lower()
    ownership_rows = [row for row in rows if "ownership" in searchable(row)]
    state_rows = [
        row
        for row in rows
        if re.search(r"\b(lifecycle|state|transition)\b", searchable(row))
    ]
    return {
        "total": len(rows),
        "exact_runtime": exact_runtime,
        "compile_contract": category_counts["compile-contract"],
        "pending": category_counts["pending-route"],
        "fallback": sum(category_counts[category] for category in fallback_categories),
        "fallback_categories": fallback_categories,
        "error_total": len(error_rows),
        "exact_errors": exact_error_evidence["complete"],
        "ownership_rows": len(ownership_rows),
        "state_rows": len(state_rows),
    }


def exact_error_ledger_evidence(route_audit: dict) -> dict:
    error_rows = [
        row
        for row in route_audit["rows"]
        if row["expect_error"]
        and row.get("contract_scope", "pinned_c") == "pinned_c"
    ]
    exact_categories = {"real-parity", "real-null-validation"}
    runnable = {
        row["runtime_id"]: row
        for row in error_rows
        if row["category"] in exact_categories
    }
    all_error_ids = {row["runtime_id"] for row in error_rows}

    def rejected(reason: str) -> dict:
        return {
            "complete": 0,
            "total": len(error_rows),
            "exact_ids": set(),
            "debt": sorted(
                f"{row['runtime_id']}: {row['category']}"
                for row in error_rows
            ),
            "evidence": f"rejected exact-error ledger: {reason}",
        }

    try:
        ledger = json.loads(EXACT_ERROR_LEDGER.read_text())
    except (OSError, json.JSONDecodeError) as error:
        return rejected(str(error))
    if ledger.get("schema_version") != 1:
        return rejected("schema_version must be 1")
    if ledger.get("pinned_freetype_version") != "2.14.3":
        return rejected("pinned FreeType version is not 2.14.3")

    identity = ledger.get("identity")
    if not isinstance(identity, dict):
        return rejected("identity is missing")
    identity_files = (
        ("route_audit", "route_audit_sha256"),
        ("pinned_oracle", "pinned_oracle_sha256"),
        ("test_executable", "test_executable_sha256"),
    )
    for path_key, hash_key in identity_files:
        relative = identity.get(path_key)
        expected_hash = identity.get(hash_key)
        if not isinstance(relative, str) or not isinstance(expected_hash, str):
            return rejected(f"{path_key} identity is missing")
        path = (ROOT / relative).resolve()
        try:
            path.relative_to(ROOT.resolve())
        except ValueError:
            return rejected(f"{path_key} escapes the repository")
        if not path.is_file():
            return rejected(f"{path_key} does not exist: {relative}")
        if file_sha256(path) != expected_hash:
            return rejected(f"{path_key} identity is stale")

    cases = ledger.get("cases")
    exact_ids_raw = ledger.get("exact_case_ids")
    mismatch_ids_raw = ledger.get("mismatch_case_ids")
    totals = ledger.get("totals")
    if (
        not isinstance(cases, dict)
        or not isinstance(exact_ids_raw, list)
        or not isinstance(mismatch_ids_raw, list)
        or not isinstance(totals, dict)
    ):
        return rejected("case rows or totals are missing")
    if not all(isinstance(case_id, str) for case_id in exact_ids_raw + mismatch_ids_raw):
        return rejected("case ID lists contain non-string values")
    exact_ids = set(exact_ids_raw)
    mismatch_ids = set(mismatch_ids_raw)
    if len(exact_ids) != len(exact_ids_raw) or len(mismatch_ids) != len(mismatch_ids_raw):
        return rejected("case ID lists contain duplicates")
    if exact_ids & mismatch_ids:
        return rejected("exact and mismatch case sets overlap")
    if set(cases) != exact_ids | mismatch_ids:
        return rejected("case rows do not partition exact and mismatch IDs")
    if set(cases) != set(runnable):
        return rejected("ledger does not cover the current runnable expected-error routes")
    for case_id, row in cases.items():
        expected_status = "exact" if case_id in exact_ids else "mismatch"
        if not isinstance(row, dict) or row.get("status") != expected_status:
            return rejected(f"{case_id} has inconsistent status")
    if (
        totals.get("runnable_expected_error_cases") != len(cases)
        or totals.get("exact_cases") != len(exact_ids)
        or totals.get("mismatch_cases") != len(mismatch_ids)
    ):
        return rejected("totals do not match case rows")

    pending_ids = all_error_ids - set(runnable)
    debt = sorted(
        [f"{case_id}: strict mismatch" for case_id in mismatch_ids]
        + [
            f"{case_id}: {next(row['category'] for row in error_rows if row['runtime_id'] == case_id)}"
            for case_id in pending_ids
        ]
    )
    return {
        "complete": len(exact_ids),
        "total": len(error_rows),
        "exact_ids": exact_ids,
        "debt": debt,
        "evidence": (
            "target/api-abi-audit/exact_error_ledger.json: forced exact "
            "pinned-C status and post-call output across Rust FFI, C ABI, and WASM"
        ),
    }


def load_contract_inventory() -> dict:
    try:
        inventory = json.loads(CONTRACT_INVENTORY.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"invalid C contract inventory: {error}") from error
    if inventory.get("schema_version") != 1:
        raise SystemExit("C contract inventory schema_version must be 1")
    if inventory.get("pinned_freetype_version") != "2.14.3":
        raise SystemExit("C contract inventory must pin FreeType 2.14.3")
    required = (
        "ownership_rules",
        "state_transitions",
        "default_modules",
        "optional_components",
        "binary_artifacts",
        "platform_behaviors",
    )
    for name in required:
        rows = inventory.get(name)
        if not isinstance(rows, list) or not rows:
            raise SystemExit(f"C contract inventory {name} must be a non-empty list")
        identities = [
            str(row.get("id", row.get("class", "")))
            for row in rows
            if isinstance(row, dict)
        ]
        if len(identities) != len(rows) or any(not value for value in identities):
            raise SystemExit(f"C contract inventory {name} has an invalid item identity")
        if len(set(identities)) != len(identities):
            raise SystemExit(f"C contract inventory {name} has duplicate item identities")
    return inventory


def check_contract_inventory(route_audit: dict) -> dict:
    inventory = load_contract_inventory()
    route_ids = {row["case_id"] for row in route_audit["rows"]}
    missing_routes = []
    for section in (
        "ownership_rules",
        "state_transitions",
        "optional_components",
    ):
        for item in inventory[section]:
            routes = item.get("routes")
            if not isinstance(routes, list) or not routes:
                missing_routes.append(
                    f"{section}:{item['id']}: empty route list"
                )
                continue
            missing_routes.extend(
                f"{section}:{item['id']}: {case_id}"
                for case_id in routes
                if case_id not in route_ids
            )
    expected_modules = pinned_default_modules()
    maintained_modules = [
        {"class": row["class"], "kind": row["kind"]}
        for row in inventory["default_modules"]
    ]
    errors = []
    if missing_routes:
        errors.append(
            "inventory routes missing from route audit: "
            + ", ".join(missing_routes)
        )
    if maintained_modules != expected_modules:
        errors.append(
            f"default module inventory differs from pinned ftmodule.h: "
            f"pinned={expected_modules}; maintained={maintained_modules}"
        )
    known_artifact_probes = {
        "consumer:shared",
        "consumer:static",
        "exports:shared",
        "exports:static",
        "loader_identity",
        "pkg_config",
        "install_layout",
        "windows_import_library",
    }
    unknown_probes = sorted(
        row["probe"]
        for row in inventory["binary_artifacts"]
        if row.get("probe") not in known_artifact_probes
    )
    if unknown_probes:
        errors.append("unknown artifact probes: " + ", ".join(unknown_probes))
    for row in inventory["platform_behaviors"]:
        missing_fields = [
            field
            for field in (
                "target",
                "os",
                "data_model",
                "endianness",
                "ci_marker",
            )
            if not row.get(field)
        ]
        if missing_fields:
            errors.append(
                f"platform {row['id']} lacks fields: {','.join(missing_fields)}"
            )
    if errors:
        raise SystemExit("\n".join(errors))
    counts = {
        name: len(inventory[name])
        for name in (
            "ownership_rules",
            "state_transitions",
            "default_modules",
            "optional_components",
            "binary_artifacts",
            "platform_behaviors",
        )
    }
    print(
        "C contract inventory: "
        + ", ".join(f"{name}={count}" for name, count in counts.items())
    )
    return inventory


def route_inventory_measurement(
    items: list[dict], route_audit: dict, exact_error_ids: set[str]
) -> dict:
    by_case: dict[str, list[dict]] = {}
    for row in route_audit["rows"]:
        by_case.setdefault(row["case_id"], []).append(row)
    complete = 0
    debt = []
    exact_categories = {"real-parity", "real-null-validation"}
    for item in items:
        issues = []
        routes = item.get("routes")
        if not isinstance(routes, list) or not routes:
            issues.append("route list is empty")
            routes = []
        for case_id in routes:
            rows = by_case.get(case_id, [])
            if not rows:
                issues.append(f"{case_id}: missing route")
                continue
            categories = sorted({row["category"] for row in rows})
            if any(row["category"] not in exact_categories for row in rows):
                issues.append(f"{case_id}: evidence={','.join(categories)}")
            error_gaps = sum(
                row["expect_error"] and row["runtime_id"] not in exact_error_ids
                for row in rows
            )
            if error_gaps:
                issues.append(
                    f"{case_id}: {error_gaps} error route(s) lack exact output evidence"
                )
        if issues:
            debt.append(
                f"{item['id']} {item['subject']}: " + "; ".join(issues)
            )
        else:
            complete += 1
    return {"complete": complete, "total": len(items), "debt": debt}


def semantic_function_measurement(
    function_names: set[str], route_audit: dict, exact_error_ids: set[str]
) -> dict:
    by_function: dict[str, list[dict]] = {name: [] for name in function_names}
    for row in route_audit["rows"]:
        if row.get("contract_scope", "pinned_c") != "pinned_c":
            continue
        symbol = row["subject"].rsplit(".", 1)[-1]
        if symbol in by_function:
            by_function[symbol].append(row)
    complete = 0
    debt = []
    exact_categories = {"real-parity", "real-null-validation"}
    for symbol, rows in sorted(by_function.items()):
        exact = [row for row in rows if row["category"] in exact_categories]
        unresolved = [
            row
            for row in rows
            if row["category"] == "pending-route"
            or "fallback" in row["category"]
            or row["category"] == "explicit-unsupported"
            or (row["expect_error"] and row["runtime_id"] not in exact_error_ids)
        ]
        if exact and not unresolved:
            complete += 1
            continue
        reasons = Counter(row["category"] for row in unresolved)
        missing_error_outputs = sum(
            row["expect_error"] and row["runtime_id"] not in exact_error_ids
            for row in unresolved
        )
        detail = ", ".join(
            f"{name}={count}" for name, count in sorted(reasons.items())
        )
        if missing_error_outputs:
            detail += (
                (", " if detail else "")
                + f"exact-error-output-gaps={missing_error_outputs}"
            )
        if not exact:
            detail += (", " if detail else "") + "no exact runtime route"
        debt.append(f"{symbol}: {detail}")
    return {"complete": complete, "total": len(function_names), "debt": debt}


def pinned_default_modules() -> list[dict]:
    path = ROOT / "freetype" / "include" / "freetype" / "config" / "ftmodule.h"
    text = strip_c_comments(read_text(path))
    kind_names = {
        "FT_Module_Class": "module",
        "FT_Driver_ClassRec": "driver",
        "FT_Renderer_Class": "renderer",
    }
    rows = []
    for kind, class_name in re.findall(
        r"FT_USE_MODULE\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*,\s*"
        r"([A-Za-z_][A-Za-z0-9_]*)\s*\)",
        text,
    ):
        if kind not in kind_names:
            raise SystemExit(f"unknown pinned default module kind: {kind}")
        rows.append({"class": class_name, "kind": kind_names[kind]})
    if not rows:
        raise SystemExit("pinned ftmodule.h yielded no default modules")
    return rows


def default_module_measurement(
    inventory: dict,
    external_evidence: dict,
) -> dict:
    expected = pinned_default_modules()
    maintained = [
        {"class": row["class"], "kind": row["kind"]}
        for row in inventory["default_modules"]
    ]
    issues = []
    if maintained != expected:
        issues.append(
            f"ftmodule.h inventory mismatch: pinned={expected}; maintained={maintained}"
        )
    probe_path = (
        ROOT
        / "tests"
        / "fixtures"
        / "inputs"
        / "public-api"
        / "ftmodapi.FT_Add_Default_Modules.json"
    )
    try:
        probe_data = json.loads(probe_path.read_text())
        probe_case = next(
            row
            for row in probe_data["cases"]
            if row["case_id"]
            == "ftmodapi.FT_Add_Default_Modules.installs_default_module_table"
        )
        probe_names = probe_case["inputs"]["params"]["probe_names"]
    except (OSError, json.JSONDecodeError, KeyError, StopIteration, TypeError) as error:
        issues.append(f"default-module probe is invalid: {error}")
        probe_names = []
    runtime_names = [row["runtime_name"] for row in inventory["default_modules"]]
    if probe_names != runtime_names:
        issues.append(
            f"default-module probe order mismatch: expected={runtime_names}; "
            f"actual={probe_names}"
        )
    case_exact = False
    if not issues and external_evidence["complete"] == external_evidence["total"]:
        try:
            ledger = json.loads(EXTERNAL_C_FUNCTION_LEDGER.read_text())
            case_exact = any(
                row.get("case_id")
                == "ftmodapi.FT_Add_Default_Modules.installs_default_module_table"
                and row.get("status") == "exact"
                and "FT_Add_Default_Modules" in row.get("entered_symbols", [])
                for row in ledger["cases"]
            )
        except (OSError, json.JSONDecodeError, KeyError, TypeError):
            case_exact = False
    if not case_exact:
        issues.append("full default-module external C probe lacks fresh exact evidence")
    total = len(inventory["default_modules"])
    return {
        "complete": total if not issues else 0,
        "total": total,
        "debt": issues,
    }


def valid_artifact_ledger(path: Path, key: str) -> tuple[dict | None, str | None]:
    try:
        ledger = json.loads(path.read_text())
        if ledger.get("schema_version") != 1:
            return None, f"{path.name}: schema_version is not 1"
        row = ledger["artifacts"][key]
        artifact = ROOT / row["path"]
        if not artifact.is_file():
            return None, f"{path.name}: artifact is missing: {row['path']}"
        if file_sha256(artifact) != row.get("sha256"):
            return None, f"{path.name}: artifact hash is stale: {row['path']}"
        if row.get("status") != "exact":
            return None, f"{path.name}: {key} status is not exact"
        return row, None
    except (OSError, json.JSONDecodeError, KeyError, TypeError) as error:
        return None, f"{path.name}: {error}"


def loader_identity_probe() -> tuple[bool, str]:
    system = os.uname().sysname if hasattr(os, "uname") else ""
    release = ROOT / "target" / "release"
    if system == "Darwin":
        library = release / "libfontdone_c_abi.dylib"
        if not library.is_file() or shutil.which("otool") is None:
            return False, "Darwin shared library or otool is missing"
        output = subprocess.run(
            ["otool", "-D", str(library)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.splitlines()
        identity = output[1].strip() if len(output) > 1 else ""
        exact = identity in {
            "@rpath/libfontdone_c_abi.dylib",
            "libfontdone_c_abi.dylib",
        }
        return exact, f"Mach-O install-name={identity or '<missing>'}"
    if system == "Linux":
        library = release / "libfontdone_c_abi.so"
        if not library.is_file() or shutil.which("readelf") is None:
            return False, "Linux shared library or readelf is missing"
        output = subprocess.run(
            ["readelf", "-d", str(library)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        match = re.search(r"\(SONAME\).*?\[([^]]+)\]", output)
        identity = match.group(1) if match else ""
        return (
            identity == "libfontdone_c_abi.so",
            f"ELF SONAME={identity or '<missing>'}",
        )
    if os.name == "nt":
        library = release / "fontdone_c_abi.dll"
        return library.is_file(), f"PE DLL={library.name}"
    return False, f"unsupported loader identity host: {system or os.name}"


def artifact_inventory_measurement(items: list[dict]) -> dict:
    probes: dict[str, tuple[bool, str]] = {}
    for prefix, ledger_path in (
        ("consumer", C_CONSUMER_LEDGER),
        ("exports", C_EXPORT_LEDGER),
    ):
        for kind in ("shared", "static"):
            row, error = valid_artifact_ledger(ledger_path, kind)
            probes[f"{prefix}:{kind}"] = (
                row is not None,
                error or f"{ledger_path.name}:{kind}",
            )
    probes["loader_identity"] = loader_identity_probe()
    pkg_config = ROOT / "fontdone-c-abi" / "fontdone2.pc"
    pkg_config_text = read_text(pkg_config)
    pkg_config_exact = pkg_config.is_file() and all(
        row in pkg_config_text
        for row in (
            "Name: fontdone",
            "Version: 2.14.3-alpha.1",
            "Libs: -L${libdir} -lfontdone_c_abi",
            "Cflags: -I${includedir}",
        )
    ) and '"/fontdone2.pc"' in read_text(
        ROOT / "fontdone-c-abi" / "Cargo.toml"
    )
    probes["pkg_config"] = (
        pkg_config_exact,
        (
            f"{pkg_config.relative_to(ROOT)} exact and packaged"
            if pkg_config_exact
            else f"{pkg_config.relative_to(ROOT)} missing, incomplete, or unpackaged"
        ),
    )
    makefile = read_text(ROOT / "Makefile")
    install_targets = all(
        marker in makefile for marker in ("c-abi-install:", "c-abi-install-check:")
    )
    install_ledger_exact = False
    try:
        consumer = json.loads(C_CONSUMER_LEDGER.read_text())
        installation = consumer["installation"]
        install_ledger_exact = (
            installation.get("status") == "exact"
            and installation.get("header_count")
            == len(list((ROOT / "fontdone-c-abi" / "include").rglob("*.h")))
            and installation.get("shared_sha256")
            == consumer["artifacts"]["shared"]["sha256"]
            and installation.get("static_sha256")
            == consumer["artifacts"]["static"]["sha256"]
        )
    except (OSError, json.JSONDecodeError, KeyError, TypeError):
        pass
    probes["install_layout"] = (
        install_targets and install_ledger_exact,
        (
            "Make install targets and staged C consumer are exact"
            if install_targets and install_ledger_exact
            else "Make install targets or fresh staged-install evidence is missing"
        ),
    )
    probes["windows_import_library"] = windows_import_library_probe()
    complete = 0
    debt = []
    for item in items:
        probe = item["probe"]
        passed, evidence = probes.get(probe, (False, f"unknown probe {probe}"))
        if passed:
            complete += 1
        else:
            debt.append(f"{item['id']} {item['subject']}: {evidence}")
    return {"complete": complete, "total": len(items), "debt": debt}


def platform_contract_source_files() -> list[Path]:
    """Return the maintained inputs that can affect a platform proof."""

    files = [
        ROOT / "Cargo.lock",
        ROOT / "Cargo.toml",
        ROOT / "Makefile",
        ROOT / ".github" / "workflows" / "ci.yml",
        ROOT / "scripts" / "audit_api_abi.py",
        ROOT / "scripts" / "check_c_exports.py",
        ROOT / "scripts" / "test_c_consumer.py",
        ROOT / "tests" / "data" / "c_contract_inventory.json",
        ROOT
        / "tests"
        / "fixtures"
        / "input"
        / "fonts"
        / "DejaVuSans.ttf",
    ]
    for directory in (
        ROOT / "src",
        ROOT / "fontdone-c-abi" / "examples",
        ROOT / "fontdone-c-abi" / "include",
        ROOT / "fontdone-c-abi" / "src",
    ):
        files.extend(path for path in directory.rglob("*") if path.is_file())
    files.extend(
        path
        for path in (
            ROOT / "fontdone-c-abi" / "Cargo.toml",
            ROOT / "fontdone-c-abi" / "build.rs",
            ROOT / "fontdone-c-abi" / "fontdone2.pc",
        )
        if path.is_file()
    )
    unique = sorted(
        set(files),
        key=lambda path: path.relative_to(ROOT).as_posix(),
    )
    missing = [path for path in unique if not path.is_file()]
    if missing:
        raise SystemExit(
            "platform contract source input is missing: "
            + ", ".join(str(path.relative_to(ROOT)) for path in missing)
        )
    return unique


def platform_contract_source_identity() -> dict:
    digest = hashlib.sha256()
    relative_paths = []
    for path in platform_contract_source_files():
        relative = path.relative_to(ROOT).as_posix()
        relative_paths.append(relative)
        encoded_path = relative.encode("utf-8")
        digest.update(len(encoded_path).to_bytes(8, "big"))
        digest.update(encoded_path)
        with path.open("rb") as handle:
            for block in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(block)
    return {
        "source_sha256": digest.hexdigest(),
        "source_files": relative_paths,
    }


def rust_host_target() -> str:
    output = subprocess.run(
        ["rustc", "-vV"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    for line in output.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    raise SystemExit("rustc -vV did not report a host target")


def active_platform_facts(consumer: dict) -> dict:
    measured = consumer.get("platform", {})
    target = PLATFORM_TARGET or rust_host_target()
    if measured.get("rust_host") != target:
        raise SystemExit(
            "C platform probe target differs from the requested Rust target"
        )
    system = measured.get("system")
    os_name = "macOS" if system == "Darwin" else system
    required = (
        "pointer_bits",
        "long_bits",
        "int_bits",
        "data_model",
        "endianness",
        "probe_output",
    )
    missing = [key for key in required if measured.get(key) is None]
    if missing:
        raise SystemExit(
            "C platform probe lacks fields: " + ", ".join(missing)
        )
    return {
        "target": target,
        "os": os_name,
        **{key: measured[key] for key in required},
    }


def load_json_file(path: Path, label: str) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"invalid {label}: {error}") from error
    if not isinstance(value, dict):
        raise SystemExit(f"invalid {label}: root must be an object")
    return value


def record_platform_contract(
    items: list[dict],
    rust_layouts: dict,
    compiler: dict,
) -> Path:
    consumer = load_json_file(C_CONSUMER_LEDGER, "C consumer ledger")
    facts = active_platform_facts(consumer)
    matches = [row for row in items if row["target"] == facts["target"]]
    if len(matches) != 1:
        raise SystemExit(
            f"native target {facts['target']} is not exactly one fixed "
            "platform-contract lane"
        )
    inventory_row = matches[0]
    for key in ("os", "data_model", "endianness"):
        if facts[key] != inventory_row[key]:
            raise SystemExit(
                f"native {key} differs from the fixed platform inventory: "
                f"observed={facts[key]}, expected={inventory_row[key]}"
            )
    if (
        rust_layouts.get("complete") != rust_layouts.get("total")
        or rust_layouts.get("total") != PINNED_COUNTS["c_structs"]
        or rust_layouts.get("debt")
    ):
        raise SystemExit("native Rust/C record-layout evidence is incomplete")

    exports = load_json_file(C_EXPORT_LEDGER, "C export ledger")
    for ledger_path in (C_CONSUMER_LEDGER, C_EXPORT_LEDGER):
        for kind in ("shared", "static"):
            _, error = valid_artifact_ledger(ledger_path, kind)
            if error:
                raise SystemExit(error)
    if consumer.get("platform", {}).get("rust_host") != facts["target"]:
        raise SystemExit("C consumer ledger was produced for a different Rust target")
    if exports.get("platform", {}).get("target") != facts["target"]:
        raise SystemExit("C export ledger was produced for a different Rust target")
    if rust_layouts.get("target") != facts["target"]:
        raise SystemExit("Rust/C layout ledger was produced for a different target")
    if consumer.get("installation", {}).get("status") != "exact":
        raise SystemExit("C installed-layout consumer evidence is not exact")
    for kind in ("shared", "static"):
        consumer_row = consumer["artifacts"][kind]
        export_row = exports["artifacts"][kind]
        if consumer_row.get("sha256") != export_row.get("sha256"):
            raise SystemExit(
                f"{kind} consumer and export ledgers describe different artifacts"
            )

    identity = platform_contract_source_identity()
    bundle = {
        "schema_version": 1,
        "pinned_freetype_version": PINNED_FREETYPE_VERSION,
        "inventory": inventory_row,
        "platform": facts,
        "identity": {
            **identity,
            "ci_revision": os.environ.get("GITHUB_SHA"),
        },
        "layout": {
            "status": "exact",
            "complete": rust_layouts["complete"],
            "total": rust_layouts["total"],
            "debt": rust_layouts["debt"],
            "ledger_sha256": file_sha256(RUST_C_LAYOUT_LEDGER),
            "rlib_sha256": rust_layouts["rlib_sha256"],
            "clang_target": compiler["clang"]["target"],
        },
        "consumer": consumer,
        "exports": exports,
    }
    PLATFORM_CONTRACT_DIR.mkdir(parents=True, exist_ok=True)
    destination = PLATFORM_CONTRACT_DIR / f"{facts['target']}.json"
    destination.write_text(json.dumps(bundle, indent=2) + "\n", encoding="utf-8")
    print(f"platform contract recorded: {destination.relative_to(ROOT)}")
    return destination


def validate_platform_contract_bundle(
    path: Path,
    inventory_by_target: dict[str, dict],
    source_identity: dict,
) -> tuple[str | None, list[str]]:
    try:
        bundle = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return None, [f"{path}: invalid JSON: {error}"]
    if not isinstance(bundle, dict):
        return None, [f"{path}: root must be an object"]

    platform_row = bundle.get("platform")
    target = platform_row.get("target") if isinstance(platform_row, dict) else None
    errors = []
    if not isinstance(target, str) or target not in inventory_by_target:
        return None, [f"{path}: target is absent from the fixed inventory"]
    expected = inventory_by_target[target]
    if bundle.get("schema_version") != 1:
        errors.append("schema_version is not 1")
    if bundle.get("pinned_freetype_version") != PINNED_FREETYPE_VERSION:
        errors.append(
            f"pinned FreeType version is not {PINNED_FREETYPE_VERSION}"
        )
    if bundle.get("inventory") != expected:
        errors.append("embedded inventory row differs from the maintained row")
    for key in ("target", "os", "data_model", "endianness"):
        if platform_row.get(key) != expected[key]:
            errors.append(
                f"platform {key}={platform_row.get(key)!r}, "
                f"expected {expected[key]!r}"
            )

    identity = bundle.get("identity", {})
    if identity.get("source_sha256") != source_identity["source_sha256"]:
        errors.append("source digest is stale")
    if identity.get("source_files") != source_identity["source_files"]:
        errors.append("source-file inventory is stale")
    current_revision = os.environ.get("GITHUB_SHA")
    if (
        current_revision is not None
        and identity.get("ci_revision") != current_revision
    ):
        errors.append("CI revision differs from the aggregate checkout")

    layout = bundle.get("layout", {})
    if (
        layout.get("status") != "exact"
        or layout.get("complete") != PINNED_COUNTS["c_structs"]
        or layout.get("total") != PINNED_COUNTS["c_structs"]
        or layout.get("debt") != []
        or not isinstance(layout.get("ledger_sha256"), str)
        or not isinstance(layout.get("rlib_sha256"), str)
        or not isinstance(layout.get("clang_target"), str)
    ):
        errors.append("record-layout evidence is incomplete")

    consumer = bundle.get("consumer", {})
    exports = bundle.get("exports", {})
    if consumer.get("schema_version") != 1:
        errors.append("C consumer ledger schema is not 1")
    if exports.get("schema_version") != 1:
        errors.append("C export ledger schema is not 1")
    if exports.get("platform", {}).get("target") != target:
        errors.append("C export target differs from the bundle target")
    consumer_platform = consumer.get("platform", {})
    if consumer_platform.get("rust_host") != target:
        errors.append("C consumer Rust target differs from the bundle target")
    for key in (
        "pointer_bits",
        "long_bits",
        "int_bits",
        "data_model",
        "endianness",
        "probe_output",
    ):
        if consumer_platform.get(key) != platform_row.get(key):
            errors.append(
                f"C consumer {key} differs from platform evidence"
            )
    expected_probe_output = (
        f"pointer_bits={platform_row.get('pointer_bits')} "
        f"long_bits={platform_row.get('long_bits')} "
        f"int_bits={platform_row.get('int_bits')} "
        f"endianness={platform_row.get('endianness')}"
    )
    if platform_row.get("probe_output") != expected_probe_output:
        errors.append("target-executed C platform probe output is inconsistent")
    expected_widths = {
        "ILP32": (32, 32, 32),
        "LP64": (64, 64, 32),
        "LLP64": (64, 32, 32),
    }.get(expected["data_model"])
    observed_widths = (
        platform_row.get("pointer_bits"),
        platform_row.get("long_bits"),
        platform_row.get("int_bits"),
    )
    if observed_widths != expected_widths:
        errors.append(
            f"target C widths {observed_widths} do not prove "
            f"{expected['data_model']}"
        )
    if consumer.get("installation", {}).get("status") != "exact":
        errors.append("installed-layout C consumer is not exact")

    consumer_artifacts = consumer.get("artifacts", {})
    export_artifacts = exports.get("artifacts", {})
    outputs = []
    for kind in ("shared", "static"):
        consumer_row = consumer_artifacts.get(kind, {})
        export_row = export_artifacts.get(kind, {})
        artifact_hash = consumer_row.get("sha256")
        if (
            consumer_row.get("status") != "exact"
            or not isinstance(artifact_hash, str)
            or len(artifact_hash) != 64
        ):
            errors.append(f"{kind} C consumer evidence is not exact")
        if (
            export_row.get("status") != "exact"
            or export_row.get("declared") != PINNED_COUNTS["c_functions"]
            or export_row.get("exported") != PINNED_COUNTS["c_functions"]
            or export_row.get("missing") != []
            or export_row.get("undocumented") != []
        ):
            errors.append(f"{kind} export evidence is not exact")
        if export_row.get("sha256") != artifact_hash:
            errors.append(
                f"{kind} consumer and export evidence hashes differ"
            )
        outputs.append(consumer_row.get("output"))
    if any(not isinstance(output, str) or not output for output in outputs):
        errors.append("shared/static C consumer output is absent")
    elif len(set(outputs)) != 1:
        errors.append("shared/static C consumer outputs differ")

    installation = consumer.get("installation", {})
    if expected["os"] == "Windows":
        import_row = consumer_artifacts.get("import", {})
        import_hash = import_row.get("sha256")
        import_path = import_row.get("path")
        if (
            import_row.get("status") != "exact"
            or not isinstance(import_hash, str)
            or re.fullmatch(r"[0-9a-f]{64}", import_hash) is None
            or not isinstance(import_path, str)
            or not import_path.replace("\\", "/").endswith(
                "/fontdone_c_abi.dll.lib"
            )
            or import_row.get("output") != consumer_artifacts.get("shared", {}).get(
                "output"
            )
        ):
            errors.append(
                "Windows import-library link evidence is not exact and hash-bound"
            )
        if (
            installation.get("import_library") != "fontdone_c_abi.dll.lib"
            or installation.get("import_library_sha256") != import_hash
        ):
            errors.append(
                "installed Windows import-library evidence differs from the "
                "linked release artifact"
            )
    elif consumer_artifacts.get("import") is not None:
        errors.append("non-Windows bundle contains Windows import-library evidence")

    return target, [f"{path}: {error}" for error in errors]


def platform_contract_evidence(
    items: list[dict],
) -> tuple[dict[str, Path], list[str]]:
    inventory_by_target = {row["target"]: row for row in items}
    source_identity = platform_contract_source_identity()
    evidence_by_target: dict[str, Path] = {}
    invalid_debt = []
    duplicate_targets = set()
    if PLATFORM_CONTRACT_DIR.is_dir():
        for path in sorted(PLATFORM_CONTRACT_DIR.rglob("*.json")):
            target, errors = validate_platform_contract_bundle(
                path,
                inventory_by_target,
                source_identity,
            )
            invalid_debt.extend(errors)
            if target is not None and not errors:
                if target in evidence_by_target:
                    duplicate_targets.add(target)
                    invalid_debt.append(
                        f"{path}: duplicate evidence for {target}; "
                        f"first seen at {evidence_by_target[target]}"
                    )
                else:
                    evidence_by_target[target] = path
    for target in duplicate_targets:
        evidence_by_target.pop(target, None)
    return evidence_by_target, invalid_debt


def windows_import_library_probe() -> tuple[bool, str]:
    items = load_contract_inventory()["platform_behaviors"]
    evidence_by_target, _ = platform_contract_evidence(items)
    target = "x86_64-pc-windows-msvc"
    path = evidence_by_target.get(target)
    if path is None:
        return (
            False,
            "no fresh exact Windows platform bundle with hash-bound "
            "import-library link/install evidence",
        )
    bundle = load_json_file(path, "Windows platform contract")
    import_row = bundle["consumer"]["artifacts"]["import"]
    return (
        True,
        f"{path.relative_to(ROOT)} import sha256={import_row['sha256']}",
    )


def platform_inventory_measurements(items: list[dict]) -> dict:
    required_fields = (
        "id",
        "target",
        "os",
        "data_model",
        "endianness",
        "ci_marker",
    )
    malformed = [
        str(row)
        for row in items
        if any(not row.get(field) for field in required_fields)
    ]
    targets = [row.get("target") for row in items]
    if (
        malformed
        or len(items) != 5
        or len(set(targets)) != len(targets)
    ):
        raise SystemExit(
            "invalid fixed platform inventory: "
            f"expected 5 unique lanes; malformed={malformed}, targets={targets}"
        )
    ci = read_text(ROOT / ".github" / "workflows" / "ci.yml")
    configured = [row for row in items if row["ci_marker"] in ci]
    configured_debt = [
        f"{row['id']} {row['target']}: CI marker {row['ci_marker']} is missing"
        for row in items
        if row["ci_marker"] not in ci
    ]
    evidence_by_target, invalid_debt = platform_contract_evidence(items)

    runtime_debt = list(invalid_debt)
    for row in items:
        if row["target"] not in evidence_by_target:
            runtime_debt.append(
                f"{row['id']} {row['target']}: no fresh exact platform bundle "
                "with layout, shared/static consumer, installed-layout, and "
                "shared/static export evidence"
            )
    return {
        "configured": len(configured),
        "total": len(items),
        "configured_debt": configured_debt,
        "runtime_complete": len(evidence_by_target),
        "runtime_debt": runtime_debt,
        "evidenced_targets": sorted(evidence_by_target),
    }


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def external_c_function_evidence(function_names: set[str]) -> dict:
    rejected = {
        "complete": 0,
        "total": len(function_names),
        "debt": sorted(function_names),
        "evidence": (
            "missing target/api-abi-audit/external_c_function_ledger.json; "
            "run make external-c-abi-audit"
        ),
    }
    if not EXTERNAL_C_FUNCTION_LEDGER.exists():
        return rejected
    try:
        ledger = json.loads(EXTERNAL_C_FUNCTION_LEDGER.read_text())
    except (OSError, json.JSONDecodeError) as error:
        rejected["evidence"] = f"rejected external C ledger: {error}"
        return rejected

    errors = []
    if ledger.get("schema_version") != 1:
        errors.append("schema_version is not 1")
    if ledger.get("pinned_freetype_version") != "2.14.3":
        errors.append("pinned FreeType version is not 2.14.3")
    rows = ledger.get("functions")
    if not isinstance(rows, dict) or set(rows) != function_names:
        errors.append("function key set does not equal the 218 pinned functions")
        rows = {}
    totals = ledger.get("totals", {})
    if totals.get("expected_functions") != len(function_names):
        errors.append("expected function denominator is stale")

    identity = ledger.get("identity", {})
    build = identity.get("build_manifest", {})
    identities = [
        (
            identity.get("pinned_oracle"),
            identity.get("pinned_oracle_sha256"),
            "pinned oracle",
        ),
        (
            identity.get("external_oracle"),
            identity.get("external_oracle_sha256"),
            "external oracle",
        ),
        (
            build.get("source"),
            build.get("source_sha256"),
            "C consumer source",
        ),
        (
            build.get("linked_fontdone_library"),
            build.get("linked_fontdone_library_sha256"),
            "linked Fontdone library",
        ),
    ]
    for relative, expected_digest, label in identities:
        if not isinstance(relative, str) or not isinstance(expected_digest, str):
            errors.append(f"{label} identity is missing")
            continue
        path = ROOT / relative
        if not path.is_file():
            errors.append(f"{label} is missing at {relative}")
            continue
        if file_sha256(path) != expected_digest:
            errors.append(f"{label} hash is stale")
    if build.get("traced_functions") != len(function_names):
        errors.append("tracing wrapper denominator is stale")
    dependencies = build.get("dependency_inspection", {})
    if dependencies.get("fontdone_c_abi_present") is not True:
        errors.append("external executable dependency proof lacks fontdone_c_abi")
    if dependencies.get("libfreetype_present") is not False:
        errors.append("external executable dependency proof includes libfreetype")

    verified = {
        symbol
        for symbol, row in rows.items()
        if isinstance(row, dict)
        and row.get("status") == "verified"
        and bool(row.get("verified_cases"))
        and bool(row.get("entered_cases"))
    }
    if totals.get("verified_functions") != len(verified):
        errors.append("verified function total disagrees with function rows")
    selection = ledger.get("selection", {})
    unique_cases = selection.get("unique_argv_cases")
    exact_cases = totals.get("exact_unique_cases")
    mismatched_cases = totals.get("mismatched_unique_cases")
    execution_error_cases = totals.get("execution_error_unique_cases")
    if not all(
        isinstance(value, int) and value >= 0
        for value in (
            unique_cases,
            exact_cases,
            mismatched_cases,
            execution_error_cases,
        )
    ):
        errors.append("selected-case totals are missing or invalid")
        unique_cases = 0
        exact_cases = 0
        mismatched_cases = 0
        execution_error_cases = 0
    elif exact_cases + mismatched_cases + execution_error_cases != unique_cases:
        errors.append("selected-case result totals do not equal the unique-case denominator")
    mismatch_debt = ledger.get("mismatched_unique_case_ids", [])
    execution_debt = [
        row.get("case_id", "unknown execution error")
        for row in ledger.get("execution_errors", [])
        if isinstance(row, dict)
    ]
    if len(mismatch_debt) != mismatched_cases:
        errors.append("mismatched selected-case debt does not equal its total")
    if len(execution_debt) != execution_error_cases:
        errors.append("execution-error selected-case debt does not equal its total")

    probes = ledger.get("explicit_probes", [])
    exact_probes = [
        row
        for row in probes
        if isinstance(row, dict)
        and row.get("status") == "exact"
        and row.get("entered") is True
    ]
    probe_total = totals.get("explicit_probe_cases")
    if not isinstance(probe_total, int) or probe_total <= 0:
        errors.append("explicit-probe denominator is missing or invalid")
        probe_total = 0
    elif len(probes) != probe_total:
        errors.append("explicit-probe rows do not equal their denominator")
    probe_debt = sorted(
        str(row.get("symbol", "unknown probe"))
        for row in probes
        if not (
            isinstance(row, dict)
            and row.get("status") == "exact"
            and row.get("entered") is True
        )
    )
    if errors:
        rejected.update(
            {
                "exact_cases": 0,
                "case_total": unique_cases,
                "case_debt": sorted(
                    str(row.get("case_id", "unknown selected case"))
                    for row in ledger.get("cases", [])
                    if isinstance(row, dict)
                ),
                "exact_probes": 0,
                "probe_total": probe_total,
                "probe_debt": sorted(
                    str(row.get("symbol", "unknown probe"))
                    for row in probes
                    if isinstance(row, dict)
                ),
            }
        )
        rejected["evidence"] = "rejected external C ledger: " + "; ".join(errors)
        return rejected
    return {
        "complete": len(verified),
        "total": len(function_names),
        "debt": sorted(function_names - verified),
        "exact_cases": exact_cases,
        "case_total": unique_cases,
        "case_debt": sorted(
            [str(case_id) for case_id in mismatch_debt] + execution_debt
        ),
        "exact_probes": len(exact_probes),
        "probe_total": probe_total,
        "probe_debt": probe_debt,
        "evidence": (
            "target/api-abi-audit/external_c_function_ledger.json: exact "
            "same-input output plus generated per-call tracing in a C executable "
            "linked only to fontdone-c-abi"
        ),
    }


def contract_categories(data: dict, c: dict, route_audit: dict) -> list[dict]:
    local = parse_local_c_header()
    exact_errors = exact_error_ledger_evidence(route_audit)
    routes = route_measurements(route_audit, exact_errors)
    inventory = load_contract_inventory()
    compiler = compiler_contract(c, data)
    rust_layouts = rust_binary_record_layouts(
        {
            row["type"]: row
            for row in data["types"]
            if row["kind"] == "struct"
        },
        compiler["layouts"]["pinned"],
    )
    clang_evidence = (
        f"Clang AST/layout comparison for {compiler['clang']['target']}"
    )
    functions = {row["symbol"]: row for row in data["functions"]}
    function_total = len(functions)
    local_function_names = set(local["functions"])
    function_names = set(functions)
    semantic_functions = semantic_function_measurement(
        function_names, route_audit, exact_errors["exact_ids"]
    )
    exact_runtime_categories = {"real-parity", "real-null-validation"}
    runtime_contract_rows = [
        row
        for row in route_audit["rows"]
        if row.get("contract_scope", "pinned_c") == "pinned_c"
        and row["category"] != "compile-contract"
    ]
    exact_runtime_rows = [
        row
        for row in runtime_contract_rows
        if row["category"] in exact_runtime_categories
    ]
    runtime_route_debt = sorted(
        f"{row['runtime_id']}: {row['category']}"
        for row in runtime_contract_rows
        if row["category"] not in exact_runtime_categories
    )
    external_function_evidence = external_c_function_evidence(function_names)
    ownership = route_inventory_measurement(
        inventory["ownership_rules"], route_audit, exact_errors["exact_ids"]
    )
    transitions = route_inventory_measurement(
        inventory["state_transitions"], route_audit, exact_errors["exact_ids"]
    )
    default_modules = default_module_measurement(
        inventory, external_function_evidence
    )
    optional_components = route_inventory_measurement(
        inventory["optional_components"], route_audit, exact_errors["exact_ids"]
    )
    artifact_consumers = artifact_inventory_measurement(
        inventory["binary_artifacts"][:2]
    )
    artifacts = artifact_inventory_measurement(inventory["binary_artifacts"])
    platforms = platform_inventory_measurements(
        inventory["platform_behaviors"]
    )

    macro_names = {row["constant"] for row in data["constants"]}
    variant_names = {row["constant"] for row in data["enum_variants"]}
    error_names = {row["constant"] for row in data["error_codes"]}
    typedef_names = {
        row["type"] for row in data["types"] if row["kind"] == "typedef"
    }
    struct_rows = {
        row["type"]: row for row in data["types"] if row["kind"] == "struct"
    }
    enum_names = {row["type"] for row in data["types"] if row["kind"] == "enum"}
    callback_names = {row["callback"] for row in data["callbacks"]}

    matching_field_order = sum(
        name in local["structs"]
        and local["structs"][name] == row["c_fields"]
        for name, row in struct_rows.items()
    )
    complete_paths, total_paths = complete_interface_paths()
    local_value_names = set(local["macros"]) | local["enum_variants"]
    drop_in_headers, pinned_headers = matching_drop_in_headers(c)
    header_compilation = compile_drop_in_headers(c)

    ci = read_text(ROOT / ".github" / "workflows" / "ci.yml").lower()
    named_platforms = ("ubuntu", "macos", "windows")
    ci_platforms = sum(name in ci for name in named_platforms)
    missing_ci_platforms = [
        name for name in named_platforms if name not in ci
    ]

    artifact_manifest = read_text(ROOT / "fontdone-c-abi" / "Cargo.toml")
    configured_artifacts = sum(
        f'"{kind}"' in artifact_manifest for kind in ("cdylib", "staticlib")
    )

    categories = [
        {
            "id": "C01",
            "name": "functions",
            "metrics": [
                contract_metric(
                    "C01.1",
                    "functions without unresolved function-subject routes",
                    semantic_functions["complete"],
                    semantic_functions["total"],
                    "all maintained routes in target/api-abi-audit/route_audit.json",
                    debt=semantic_functions["debt"],
                ),
                contract_metric(
                    "C01.2",
                    "pinned function names declared by the shipped C header",
                    len(function_names & local_function_names),
                    function_total,
                    "fontdone_ffi.h parsed against pinned headers",
                ),
                contract_metric(
                    "C01.3",
                    "source-compatible C signatures",
                    compiler["function_signatures"]["complete"],
                    compiler["function_signatures"]["total"],
                    clang_evidence,
                    debt=compiler["function_signatures"]["debt"],
                ),
                contract_metric(
                    "C01.4",
                    (
                        "functions with traced, exact same-input parity through "
                        "an independent external C executable"
                    ),
                    external_function_evidence["complete"],
                    function_total,
                    external_function_evidence["evidence"],
                    debt=external_function_evidence["debt"],
                ),
                contract_metric(
                    "C01.5",
                    "selected independent external-C cases with exact same-input output",
                    external_function_evidence.get("exact_cases", 0),
                    external_function_evidence.get("case_total", function_total),
                    external_function_evidence["evidence"],
                    debt=external_function_evidence.get("case_debt", []),
                ),
                contract_metric(
                    "C01.6",
                    "explicit external-C ABI probes with exact traced output",
                    external_function_evidence.get("exact_probes", 0),
                    external_function_evidence.get("probe_total", function_total),
                    external_function_evidence["evidence"],
                    debt=external_function_evidence.get("probe_debt", []),
                ),
                contract_metric(
                    "C01.7",
                    "pinned-C runtime contract rows with exact route evidence",
                    len(exact_runtime_rows),
                    len(runtime_contract_rows),
                    "all non-compile pinned-C rows in route_audit.json",
                    debt=runtime_route_debt,
                ),
            ],
        },
        {
            "id": "C02",
            "name": "constants",
            "metrics": [
                contract_metric(
                    "C02.1",
                    "pinned macros present in the shipped C header",
                    len(macro_names & set(local["macros"])),
                    len(macro_names),
                    "fontdone_ffi.h macro names parsed against pinned headers",
                ),
                contract_metric(
                    "C02.2",
                    "pinned enum value names present in the shipped C header",
                    len(variant_names & local_value_names),
                    len(variant_names),
                    "fontdone_ffi.h defines/enums parsed against pinned headers",
                ),
                contract_metric(
                    "C02.3",
                    "constant values proven C-expression equivalent",
                    compiler["constant_values"]["complete"],
                    compiler["constant_values"]["total"],
                    (
                        "Clang preprocessor macro definitions and enum "
                        f"constant evaluation for {compiler['clang']['target']}"
                    ),
                    debt=compiler["constant_values"]["debt"],
                ),
            ],
        },
        {
            "id": "C03",
            "name": "types",
            "metrics": [
                contract_metric(
                    "C03.1",
                    "pinned typedef names present in the shipped C header",
                    len(typedef_names & local["typedefs"]),
                    len(typedef_names),
                    "fontdone_ffi.h typedef names parsed against pinned headers",
                ),
                contract_metric(
                    "C03.2",
                    "pinned enum type names present in the shipped C header",
                    len(enum_names & set(local["enums"])),
                    len(enum_names),
                    "fontdone_ffi.h enum names parsed against pinned headers",
                ),
                contract_metric(
                    "C03.3",
                    "typedef and enum source/ABI equivalence",
                    compiler["types"]["complete"],
                    compiler["types"]["total"],
                    clang_evidence,
                    debt=compiler["types"]["debt"],
                ),
            ],
        },
        {
            "id": "C04",
            "name": "layouts",
            "metrics": [
                contract_metric(
                    "C04.1",
                    "records with matching field names and order",
                    matching_field_order,
                    len(struct_rows),
                    "fontdone_ffi.h records parsed against pinned headers",
                ),
                contract_metric(
                    "C04.2",
                    "records with exact native size, alignment, fields, and offsets",
                    compiler["layouts"]["complete"],
                    compiler["layouts"]["total"],
                    clang_evidence,
                    debt=compiler["layouts"]["debt"],
                ),
                contract_metric(
                    "C04.3",
                    "actual Rust C records with exact native size, alignment, and offsets",
                    rust_layouts["complete"],
                    rust_layouts["total"],
                    "target/api-abi-audit/rust_c_layout_ledger.json",
                    debt=rust_layouts["debt"],
                ),
            ],
        },
        {
            "id": "C05",
            "name": "callbacks",
            "metrics": [
                contract_metric(
                    "C05.1",
                    "pinned callback names present in the shipped C header",
                    len(callback_names & set(local["callbacks"])),
                    len(callback_names),
                    "callback typedefs parsed from both header sets",
                ),
                contract_metric(
                    "C05.2",
                    "callback typedefs with exact native type and calling convention",
                    compiler["callbacks"]["complete"],
                    compiler["callbacks"]["total"],
                    clang_evidence,
                    debt=compiler["callbacks"]["debt"],
                ),
                contract_metric(
                    "C05.3",
                    "callback alias macros resolving to the pinned callback",
                    compiler["callback_aliases"]["complete"],
                    compiler["callback_aliases"]["total"],
                    "pinned macro inventory resolved against Clang-classified callback typedefs",
                    debt=compiler["callback_aliases"]["debt"],
                ),
            ],
        },
        {
            "id": "C06",
            "name": "ownership rules",
            "metrics": [
                contract_metric(
                    "C06.1",
                    "ownership rules with exact runtime evidence",
                    ownership["complete"],
                    ownership["total"],
                    "tests/data/c_contract_inventory.json checked against route_audit.json",
                    debt=ownership["debt"],
                )
            ],
        },
        {
            "id": "C07",
            "name": "state transitions",
            "metrics": [
                contract_metric(
                    "C07.1",
                    "state transitions with exact runtime evidence",
                    transitions["complete"],
                    transitions["total"],
                    "tests/data/c_contract_inventory.json checked against route_audit.json",
                    debt=transitions["debt"],
                )
            ],
        },
        {
            "id": "C08",
            "name": "errors",
            "metrics": [
                contract_metric(
                    "C08.1",
                    "pinned error-code names present in the shipped C header",
                    len(error_names & set(local["macros"])),
                    len(error_names),
                    "fontdone_ffi.h macros parsed against pinned error inventory",
                ),
                contract_metric(
                    "C08.2",
                    "expected-error routes comparing exact error and output",
                    exact_errors["complete"],
                    exact_errors["total"],
                    exact_errors["evidence"],
                    debt=exact_errors["debt"],
                ),
                contract_metric(
                    "C08.3",
                    "routes without generic fallback evidence",
                    routes["total"] - routes["fallback"],
                    routes["total"],
                    "target/api-abi-audit/route_audit.json",
                    debt=sorted(
                        row["case_id"]
                        for row in route_audit["rows"]
                        if row["category"] in routes["fallback_categories"]
                    ),
                ),
                contract_metric(
                    "C08.4",
                    "pinned error-code values proven equivalent",
                    compiler["error_values"]["complete"],
                    compiler["error_values"]["total"],
                    (
                        "Clang constant-expression evaluation for "
                        f"{compiler['clang']['target']}"
                    ),
                    debt=compiler["error_values"]["debt"],
                ),
            ],
        },
        {
            "id": "C09",
            "name": "supported modules",
            "metrics": [
                contract_metric(
                    "C09.1",
                    "pinned default modules present in exact registration order",
                    default_modules["complete"],
                    default_modules["total"],
                    "pinned ftmodule.h plus the full external FT_Add_Default_Modules probe",
                    debt=default_modules["debt"],
                ),
                contract_metric(
                    "C09.2",
                    "optional public components with exact enabled/disabled behavior",
                    optional_components["complete"],
                    optional_components["total"],
                    "tests/data/c_contract_inventory.json checked against route_audit.json",
                    debt=optional_components["debt"],
                ),
            ],
        },
        {
            "id": "C10",
            "name": "headers",
            "metrics": [
                contract_metric(
                    "C10.1",
                    "drop-in public header paths shipped",
                    drop_in_headers,
                    pinned_headers,
                    "fontdone-c-abi/include compared with pinned include paths",
                ),
                contract_metric(
                    "C10.2",
                    "public headers compiled independently in C and C++",
                    header_compilation["complete"],
                    header_compilation["total"],
                    "independent Clang C11 and C++17 compilation per header",
                    debt=header_compilation["debt"],
                ),
            ],
        },
        {
            "id": "C11",
            "name": "binary artifacts",
            "metrics": [
                contract_metric(
                    "C11.1",
                    "configured shared and static library kinds",
                    configured_artifacts,
                    2,
                    "fontdone-c-abi/Cargo.toml",
                    blocking=False,
                ),
                contract_metric(
                    "C11.2",
                    "external C consumers linked and run against shared and static artifacts",
                    artifact_consumers["complete"],
                    artifact_consumers["total"],
                    "target/api-abi-audit/c_consumer_ledger.json",
                    debt=artifact_consumers["debt"],
                ),
                contract_metric(
                    "C11.3",
                    "binary and installation artifact contract items",
                    artifacts["complete"],
                    artifacts["total"],
                    "tests/data/c_contract_inventory.json plus generated consumer/export ledgers",
                    debt=artifacts["debt"],
                ),
            ],
        },
        {
            "id": "C12",
            "name": "platform behaviors",
            "metrics": [
                contract_metric(
                    "C12.1",
                    "named native OS families in C-consumer CI",
                    ci_platforms,
                    len(named_platforms),
                    ".github/workflows/ci.yml (Linux, macOS, Windows)",
                    debt=missing_ci_platforms,
                ),
                contract_metric(
                    "C12.2",
                    "inventoried target and data-model lanes configured in CI",
                    platforms["configured"],
                    platforms["total"],
                    "tests/data/c_contract_inventory.json and .github/workflows/ci.yml",
                    debt=platforms["configured_debt"],
                ),
                contract_metric(
                    "C12.3",
                    "inventoried target lanes with fresh layout, consumer, and export evidence",
                    platforms["runtime_complete"],
                    platforms["total"],
                    "generated per-target Rust layout and C artifact ledgers",
                    debt=platforms["runtime_debt"],
                ),
            ],
        },
    ]
    for category in categories:
        blockers = [metric for metric in category["metrics"] if metric["blocking"]]
        category["is_complete"] = bool(blockers) and all(
            metric["is_complete"] for metric in blockers
        )
    return categories


def write_contract_report(
    data: dict,
    c: dict,
    route_audit: dict,
    output_dir: Path,
) -> dict:
    categories = contract_categories(data, c, route_audit)
    report = {
        "schema_version": 1,
        "pinned_freetype_version": "2.14.3",
        "completion_rule": (
            "Every blocking metric must have a closed non-zero denominator "
            "and complete == total."
        ),
        "categories_complete": sum(row["is_complete"] for row in categories),
        "categories_total": len(categories),
        "is_complete": all(row["is_complete"] for row in categories),
        "categories": categories,
    }
    category_ids = [row["id"] for row in categories]
    metric_ids = [
        metric["id"] for row in categories for metric in row["metrics"]
    ]
    if len(categories) != 12 or len(set(category_ids)) != len(category_ids):
        raise SystemExit("C ABI contract must contain 12 unique categories")
    if len(set(metric_ids)) != len(metric_ids):
        raise SystemExit("C ABI contract contains duplicate measurement IDs")
    for row in categories:
        for metric in row["metrics"]:
            complete = metric["complete"]
            total = metric["total"]
            if complete is not None and complete < 0:
                raise SystemExit(f"{metric['id']}: negative numerator")
            if total is not None and total <= 0:
                raise SystemExit(f"{metric['id']}: denominator must be positive")
            if complete is not None and total is not None and complete > total:
                raise SystemExit(f"{metric['id']}: numerator exceeds denominator")
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "c_abi_contract_status.json"
    md_path = output_dir / "c_abi_contract_status.md"
    json_path.write_text(json.dumps(report, indent=2) + "\n")
    lines = [
        "# FreeType 2.14.3 C ABI Contract Status",
        "",
        "> Generated by `make c-abi-contract`; do not edit.",
        "",
        f"Complete categories: **{report['categories_complete']} / {report['categories_total']}**.",
        "",
        "A category is complete only when every blocking metric has a closed,",
        "non-zero denominator and its numerator equals its denominator.",
        "",
        "| ID | Category | Complete |",
        "|---|---|---:|",
    ]
    for category in categories:
        lines.append(
            f"| {category['id']} | {category['name']} | "
            f"{'yes' if category['is_complete'] else 'no'} |"
        )
    lines.extend(
        [
            "",
            "## Measurements",
            "",
            "| ID | Measurement | Result | Debt | Blocking | Evidence |",
            "|---|---|---:|---:|---:|---|",
        ]
    )
    for category in categories:
        for metric in category["metrics"]:
            result = (
                "? / ?"
                if metric["complete"] is None and metric["total"] is None
                else f"{metric['complete'] if metric['complete'] is not None else '?'} / "
                f"{metric['total'] if metric['total'] is not None else '?'}"
            )
            evidence = metric["evidence"].replace("|", "\\|")
            lines.append(
                f"| {metric['id']} | {metric['label']} | {result} | "
                f"{len(metric['debt'])} | "
                f"{'yes' if metric['blocking'] else 'no'} | {evidence} |"
            )
    debt_metrics = [
        metric
        for category in categories
        for metric in category["metrics"]
        if metric["debt"]
    ]
    if debt_metrics:
        lines.extend(["", "## Exact debt inventories", ""])
        for metric in debt_metrics:
            lines.extend(
                [
                    f"### {metric['id']} — {metric['label']}",
                    "",
                    f"Debt items: **{len(metric['debt'])}**",
                    "",
                ]
            )
            lines.extend(
                f"- `{item.replace('`', '')}`" for item in metric["debt"]
            )
            lines.append("")
    lines.extend(
        [
            "",
            "Code coverage is reported separately. It cannot satisfy a C contract",
            "metric because executing Rust lines does not prove C source, ABI,",
            "ownership, lifecycle, packaging, or platform equivalence.",
            "",
        ]
    )
    md_path.write_text("\n".join(lines))
    print(md_path)
    print(json_path)
    return report


def markdown_table(headers: list[str], rows: list[dict], limit: int | None = None) -> str:
    selected = rows if limit is None else rows[:limit]
    lines = ["| " + " | ".join(headers) + " |", "| " + " | ".join(["---"] * len(headers)) + " |"]
    for row in selected:
        values = []
        for header in headers:
            value = str(row.get(header, ""))
            value = value.replace("|", "\\|").replace("\n", " ")
            if len(value) > 180:
                value = value[:177] + "..."
            values.append(value)
        lines.append("| " + " | ".join(values) + " |")
    return "\n".join(lines)


def write_report(data: dict, output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "api_abi_audit.json").write_text(json.dumps(data, indent=2))

    functions = data["functions"]
    types = data["types"]
    constants = data["constants"]
    enum_variants = data["enum_variants"]
    error_codes = data["error_codes"]
    callbacks = data["callbacks"]
    counts = data["counts"]
    status_counts = {}
    for row in functions:
        status_counts[row["fontdone_status"]] = status_counts.get(row["fontdone_status"], 0) + 1

    critical_types = [
        row
        for row in types
        if row["type"]
        in {
            "FT_FaceRec",
            "FT_Size_Metrics",
            "FT_GlyphSlotRec",
            "FT_Glyph_Metrics",
            "FT_Bitmap",
            "FT_Outline",
            "FT_BBox",
            "FT_Vector",
            "FT_Pixel_Mode",
            "FT_Render_Mode",
        }
    ]

    md = [
        "# FreeType C-to-fontdone API/ABI Audit",
        "",
        "This generated diagnostic report compares pinned FreeType C headers with local `fontdone`.",
        "",
        "## Key Point",
        "",
        "`fontdone-c-abi` separately exports `FT_*` symbols and `repr(C)` records. The safe Rust API can be semantically compatible without exposing the raw C ABI.",
        "",
        "## Counts",
        "",
        markdown_table(["metric", "count"], [{"metric": k, "count": v} for k, v in counts.items()]),
        "",
        "## Function Status",
        "",
        markdown_table(["status", "count"], [{"status": k, "count": v} for k, v in sorted(status_counts.items())]),
        "",
        "## Critical Record Exactness",
        "",
        markdown_table(
            [
                "type",
                "fontdone_mapping",
                "c_field_count",
                "fontdone_field_count",
                "field_order_exact",
                "c_fields",
                "fontdone_fields",
            ],
            critical_types,
        ),
        "",
        "## Functions",
        "",
        markdown_table(
            [
                "symbol",
                "fontdone_status",
                "fontdone_mapping",
                "exactness",
                "c_return",
                "c_params",
                "c_file",
            ],
            functions,
        ),
        "",
        "## Types / Structs / Enums",
        "",
        markdown_table(
            [
                "type",
                "kind",
                "fontdone_mapping",
                "c_field_count",
                "fontdone_field_count",
                "field_order_exact",
                "c_fields",
                "fontdone_fields",
            ],
            types,
        ),
        "",
        "## Constants / Macros",
        "",
        markdown_table(["constant", "fontdone_mapping", "c_value", "c_file"], constants),
        "",
        "## Enum Variants",
        "",
        markdown_table(["constant", "enum", "c_file"], enum_variants),
        "",
        "## Error Codes",
        "",
        markdown_table(["constant", "kind", "c_value", "c_file"], error_codes),
        "",
        "## Callbacks",
        "",
        markdown_table(["callback", "c_return", "c_params", "c_file"], callbacks),
    ]
    (output_dir / "api_abi_audit.md").write_text("\n".join(md))


def main() -> int:
    global PLATFORM_TARGET
    global PLATFORM_RUNNER
    global PLATFORM_LINKER
    global PLATFORM_CLANG_TARGET
    global PLATFORM_CLANG_SYSROOT

    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, default=OUTPUT_DIR)
    parser.add_argument("--c-contract", action="store_true")
    parser.add_argument("--check-contract-inventory", action="store_true")
    parser.add_argument("--record-platform-contract", action="store_true")
    parser.add_argument("--check-platform-contract", action="store_true")
    parser.add_argument("--platform-target")
    parser.add_argument("--platform-runner", default="")
    parser.add_argument("--platform-linker")
    parser.add_argument("--platform-clang-target")
    parser.add_argument("--platform-clang-sysroot", type=Path)
    parser.add_argument(
        "--route-audit-json",
        type=Path,
        default=ROUTE_AUDIT,
    )
    parser.add_argument("--require-c-contract-complete", action="store_true")
    args = parser.parse_args()
    PLATFORM_TARGET = args.platform_target
    PLATFORM_RUNNER = shlex.split(args.platform_runner)
    PLATFORM_LINKER = args.platform_linker
    PLATFORM_CLANG_TARGET = args.platform_clang_target
    PLATFORM_CLANG_SYSROOT = args.platform_clang_sysroot
    if PLATFORM_TARGET and not PLATFORM_RUNNER:
        raise SystemExit("cross platform recording requires --platform-runner")
    if PLATFORM_TARGET and not PLATFORM_LINKER:
        raise SystemExit("cross platform recording requires --platform-linker")
    if PLATFORM_TARGET and not PLATFORM_CLANG_TARGET:
        raise SystemExit(
            "cross platform recording requires --platform-clang-target"
        )
    if PLATFORM_TARGET and PLATFORM_CLANG_SYSROOT is None:
        raise SystemExit(
            "cross platform recording requires --platform-clang-sysroot"
        )

    include_root = ROOT / "freetype" / "include"
    if not (include_root / "freetype" / "freetype.h").exists():
        subprocess.run(["bash", str(ROOT / "scripts" / "fetch_ft.sh")], check=True)

    c = parse_c_headers(include_root)
    fontdone = parse_fontdone(ROOT / "src")
    interface = load_interface_map(ROOT / "tests" / "data" / "interface_map.json")

    functions = [classify_function(symbol, c, interface) for symbol in sorted(c["functions"])]
    types = [
        classify_type(name, c, fontdone)
        for name in sorted(set(c["typedefs"]) | set(c["structs"]) | set(c["enums"]))
    ]
    constants = [classify_constant(name, c) for name in sorted(c["macros"])]
    enum_variants = [classify_enum_variant(name, c) for name in sorted(c["enum_variants"])]
    error_codes = [classify_error_code(name, c) for name in sorted(c["error_codes"])]
    callbacks = [classify_callback(name, c) for name in sorted(c["callbacks"])]

    data = {
        "counts": {
            "c_functions": len(c["functions"]),
            "c_macros": len(c["macros"]),
            "c_typedefs": len(c["typedefs"]),
            "c_callbacks": len(c["callbacks"]),
            "c_structs": len(c["structs"]),
            "c_enums": len(c["enums"]),
            "c_enum_variants": len(c["enum_variants"]),
            "c_error_codes": len(c["error_codes"]),
            "fontdone_pub_functions": len(fontdone["functions"]),
            "fontdone_pub_consts": len(fontdone["consts"]),
            "fontdone_pub_structs": len(fontdone["structs"]),
            "fontdone_pub_enums": len(fontdone["enums"]),
        },
        "functions": functions,
        "types": types,
        "constants": constants,
        "enum_variants": enum_variants,
        "error_codes": error_codes,
        "callbacks": callbacks,
    }
    for metric, expected in PINNED_COUNTS.items():
        actual = data["counts"][metric]
        if actual != expected:
            raise SystemExit(
                f"pinned FreeType 2.14.3 inventory drift: "
                f"{metric}={actual}, expected {expected}"
            )
    write_report(data, args.output_dir)
    if args.record_platform_contract:
        inventory = load_contract_inventory()
        compiler = compiler_contract(c, data)
        rust_layouts = rust_binary_record_layouts(
            {
                row["type"]: row
                for row in data["types"]
                if row["kind"] == "struct"
            },
            compiler["layouts"]["pinned"],
        )
        record_platform_contract(
            inventory["platform_behaviors"],
            rust_layouts,
            compiler,
        )
    if args.check_platform_contract:
        inventory = load_contract_inventory()
        platforms = platform_inventory_measurements(
            inventory["platform_behaviors"]
        )
        import_library_complete, import_library_evidence = (
            windows_import_library_probe()
        )
        print(
            "platform contract: "
            f"configured={platforms['configured']}/{platforms['total']}, "
            f"runtime={platforms['runtime_complete']}/{platforms['total']}, "
            "Windows import library="
            f"{int(import_library_complete)}/1"
        )
        for debt in (
            platforms["configured_debt"] + platforms["runtime_debt"]
        ):
            print(f"platform contract debt: {debt}")
        if not import_library_complete:
            print(
                "platform contract debt: Windows import library: "
                f"{import_library_evidence}"
            )
        if (
            platforms["configured"] != platforms["total"]
            or platforms["runtime_complete"] != platforms["total"]
            or platforms["runtime_debt"]
            or not import_library_complete
        ):
            return 1
    if (
        args.c_contract
        or args.check_contract_inventory
        or args.require_c_contract_complete
    ):
        if not args.route_audit_json.exists():
            raise SystemExit(
                f"missing route audit: {args.route_audit_json}; "
                "run make api-abi-check"
            )
        route_audit = json.loads(args.route_audit_json.read_text())
        check_contract_inventory(route_audit)
    if args.c_contract or args.require_c_contract_complete:
        contract = write_contract_report(data, c, route_audit, args.output_dir)
        if args.require_c_contract_complete and not contract["is_complete"]:
            incomplete = ", ".join(
                f"{row['id']} {row['name']}"
                for row in contract["categories"]
                if not row["is_complete"]
            )
            print(f"C ABI contract incomplete: {incomplete}")
            return 1
    print(args.output_dir / "api_abi_audit.md")
    print(args.output_dir / "api_abi_audit.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
