#!/usr/bin/env python3
"""Generate the wasm32 ABI inventory, record layouts, and Node declarations."""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fontdone-wasm" / "src" / "implementation.rs"
JSON_OUTPUT = ROOT / "fontdone-wasm" / "abi.json"
TS_OUTPUT = ROOT / "fontdone-wasm" / "fontdone_wasm.d.ts"

SCALARS = {
    "bool": (1, 1),
    "i8": (1, 1),
    "u8": (1, 1),
    "c_uchar": (1, 1),
    "i16": (2, 2),
    "u16": (2, 2),
    "c_short": (2, 2),
    "i32": (4, 4),
    "u32": (4, 4),
    "f32": (4, 4),
    "usize": (4, 4),
    "isize": (4, 4),
    "i64": (8, 8),
    "u64": (8, 8),
    "f64": (8, 8),
}
HOST_EXPORTS = {
    "fontdone_wasm_malloc",
    "fontdone_wasm_free",
    "fontdone_wasm_open_face_handle",
    "fontdone_wasm_done_face",
    "fontdone_wasm_set_pixel_sizes",
    "fontdone_wasm_get_char_index",
    "fontdone_wasm_load_glyph",
    "fontdone_wasm_render_glyph",
    "fontdone_wasm_bitmap_buffer",
    "fontdone_wasm_bitmap_len",
    "fontdone_wasm_bitmap_width",
    "fontdone_wasm_bitmap_rows",
    "fontdone_wasm_bitmap_pitch",
}


@dataclass(frozen=True)
class Layout:
    size: int
    align: int


def normalize_type(value: str) -> str:
    return " ".join(value.replace("\n", " ").split())


def split_top_level(value: str) -> list[str]:
    result: list[str] = []
    start = 0
    depth = 0
    for index, character in enumerate(value):
        if character in "(<[":
            depth += 1
        elif character in ")>]":
            depth -= 1
        elif character == "," and depth == 0:
            result.append(value[start:index].strip())
            start = index + 1
    tail = value[start:].strip()
    if tail:
        result.append(tail)
    return result


def aliases(text: str) -> dict[str, str]:
    result = {}
    for match in re.finditer(r"(?ms)^pub type\s+(\w+)\s*=\s*(.*?);", text):
        result[match.group(1)] = normalize_type(match.group(2))
    return result


def structs(text: str) -> dict[str, list[tuple[str, str]]]:
    result = {}
    pattern = re.compile(
        r"(?ms)#\[repr\(C\)\].*?^pub struct\s+(\w+)\s*\{(.*?)^\}"
    )
    for match in pattern.finditer(text):
        fields = []
        for field in re.finditer(r"(?m)^\s*pub\s+(\w+)\s*:\s*([^,]+),", match.group(2)):
            fields.append((field.group(1), normalize_type(field.group(2))))
        result[match.group(1)] = fields
    return result


def type_layout(
    type_name: str,
    alias_map: dict[str, str],
    struct_layouts: dict[str, Layout],
    stack: tuple[str, ...] = (),
) -> Layout | None:
    type_name = normalize_type(type_name)
    if type_name in SCALARS:
        return Layout(*SCALARS[type_name])
    if type_name == "c_void":
        return None
    if type_name.startswith(("*const ", "*mut ")):
        return Layout(4, 4)
    if type_name.startswith("Option<") or 'extern "C" fn' in type_name:
        return Layout(4, 4)
    array = re.fullmatch(r"\[(.+);\s*(\d+)\]", type_name)
    if array:
        element = type_layout(array.group(1), alias_map, struct_layouts, stack)
        return None if element is None else Layout(element.size * int(array.group(2)), element.align)
    if type_name in struct_layouts:
        return struct_layouts[type_name]
    if type_name in alias_map and type_name not in stack:
        return type_layout(
            alias_map[type_name], alias_map, struct_layouts, stack + (type_name,)
        )
    return None


def align_up(value: int, alignment: int) -> int:
    return (value + alignment - 1) // alignment * alignment


def record_contracts(
    records: dict[str, list[tuple[str, str]]], alias_map: dict[str, str]
) -> list[dict[str, object]]:
    layouts: dict[str, Layout] = {}
    unresolved = set(records)
    while unresolved:
        progress = False
        for name in sorted(unresolved):
            offset = 0
            alignment = 1
            for _, field_type in records[name]:
                layout = type_layout(field_type, alias_map, layouts)
                if layout is None:
                    break
                offset = align_up(offset, layout.align) + layout.size
                alignment = max(alignment, layout.align)
            else:
                layouts[name] = Layout(align_up(offset, alignment), alignment)
                unresolved.remove(name)
                progress = True
                break
        if not progress:
            details = {
                name: [
                    field_type
                    for _, field_type in records[name]
                    if type_layout(field_type, alias_map, layouts) is None
                ]
                for name in sorted(unresolved)
            }
            raise SystemExit(f"cannot compute wasm32 record layouts: {details}")

    result = []
    for name, fields in records.items():
        offset = 0
        output_fields = []
        for field_name, field_type in fields:
            layout = type_layout(field_type, alias_map, layouts)
            if layout is None:
                raise AssertionError(field_type)
            offset = align_up(offset, layout.align)
            pointer = pointer_like(field_type, alias_map)
            output_fields.append(
                {
                    "name": field_name,
                    "type": field_type,
                    "offset": offset,
                    "width": layout.size,
                    "alignment": layout.align,
                    "pointer_interpretation": (
                        "i32 byte offset into this instance's exported memory; 0 is null"
                        if pointer
                        else None
                    ),
                    "ownership": (
                        "borrowed; see the producing function and lifecycle table"
                        if pointer
                        else "value"
                    ),
                }
            )
            offset += layout.size
        result.append(
            {
                "name": name,
                "representation": "C",
                "size": layouts[name].size,
                "alignment": layouts[name].align,
                "fields": output_fields,
            }
        )
    return result


def pointer_like(
    type_name: str, alias_map: dict[str, str], stack: tuple[str, ...] = ()
) -> bool:
    type_name = normalize_type(type_name)
    if type_name.startswith(("*const ", "*mut ", "Option<")):
        return True
    if 'extern "C" fn' in type_name:
        return True
    if type_name in alias_map and type_name not in stack:
        return pointer_like(alias_map[type_name], alias_map, stack + (type_name,))
    return False


def exports(text: str) -> list[dict[str, object]]:
    result = []
    marker = re.compile(r'#\[unsafe\(no_mangle\)\](?:\s*///[^\n]*)*\s*pub\s+extern\s+"C"\s+fn\s+(\w+)\s*\(')
    for match in marker.finditer(text):
        open_paren = match.end() - 1
        depth = 0
        close_paren = -1
        for index in range(open_paren, len(text)):
            character = text[index]
            if character == "(":
                depth += 1
            elif character == ")":
                depth -= 1
                if depth == 0:
                    close_paren = index
                    break
        if close_paren < 0:
            raise SystemExit(f"unclosed export signature: {match.group(1)}")
        return_match = re.match(r"\s*(?:->\s*([^\{]+))?\{", text[close_paren + 1 :])
        if return_match is None:
            raise SystemExit(f"cannot parse export return: {match.group(1)}")
        parameters = []
        for parameter in split_top_level(text[open_paren + 1 : close_paren]):
            if ":" not in parameter:
                continue
            name, type_name = parameter.split(":", 1)
            parameters.append(
                {"name": name.strip(), "type": normalize_type(type_name)}
            )
        return_type = normalize_type(return_match.group(1) or "()")
        result.append(
            {
                "name": match.group(1),
                "parameters": parameters,
                "return": return_type,
                "supported_host_contract": match.group(1) in HOST_EXPORTS,
            }
        )
    names = [row["name"] for row in result]
    if len(names) != len(set(names)):
        raise SystemExit("duplicate no_mangle WebAssembly export")
    return result


def render_typescript() -> str:
    return """// Generated by scripts/generate_wasm_contract.py. Do not edit.
// This is the directly callable Node host subset. abi.json inventories every
// no_mangle export and every repr(C) record.

export interface FontdoneWasmExports extends WebAssembly.Exports {
  readonly memory: WebAssembly.Memory;
  fontdone_wasm_malloc(size: number): number;
  fontdone_wasm_free(pointer: number, size: number): void;
  fontdone_wasm_open_face_handle(
    fileBase: number,
    fileSize: number,
    faceIndex: bigint,
    sizePt: number,
    outError: number,
  ): number;
  fontdone_wasm_done_face(handle: number): number;
  fontdone_wasm_set_pixel_sizes(
    handle: number,
    pixelWidth: number,
    pixelHeight: number,
  ): number;
  fontdone_wasm_get_char_index(handle: number, charCode: bigint): number;
  fontdone_wasm_load_glyph(
    handle: number,
    glyphIndex: number,
    loadFlags: number,
  ): number;
  fontdone_wasm_render_glyph(handle: number, renderMode: number): number;
  fontdone_wasm_bitmap_buffer(handle: number): number;
  fontdone_wasm_bitmap_len(handle: number): number;
  fontdone_wasm_bitmap_width(handle: number): number;
  fontdone_wasm_bitmap_rows(handle: number): number;
  fontdone_wasm_bitmap_pitch(handle: number): number;
}
"""


def contract(text: str) -> dict[str, object]:
    alias_map = aliases(text)
    records = structs(text)
    export_rows = exports(text)
    if not export_rows:
        raise SystemExit("no WebAssembly exports found")
    return {
        "schema_version": 1,
        "package": "fontdone-wasm",
        "package_version": "2.14.3-alpha.1",
        "target": "wasm32-unknown-unknown",
        "pointer_width": 4,
        "endianness": "little",
        "supported_hosts": ["Node.js >= 20"],
        "integer_lowering": {
            "i32/u32/pointers/usize": "JavaScript number",
            "i64/u64": "JavaScript bigint",
        },
        "memory_contract": {
            "pointers": "byte offsets into the module's exported WebAssembly.Memory",
            "null": 0,
            "caller_allocations": "fontdone_wasm_malloc/free with identical size",
            "borrowed_results": "valid until the documented owning handle mutation or teardown",
        },
        "exports": export_rows,
        "records": record_contracts(records, alias_map),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    text = SOURCE.read_text(encoding="utf-8")
    json_content = json.dumps(contract(text), indent=2) + "\n"
    ts_content = render_typescript()
    expected = ((JSON_OUTPUT, json_content), (TS_OUTPUT, ts_content))
    stale = [
        path
        for path, content in expected
        if not path.exists() or path.read_text(encoding="utf-8") != content
    ]
    if args.check:
        if stale:
            for path in stale:
                print(f"stale generated WebAssembly contract: {path.relative_to(ROOT)}")
            raise SystemExit(1)
        data = json.loads(json_content)
        print(
            "WASM contract: "
            f"{len(data['exports'])} exports, {len(data['records'])} repr(C) records clean"
        )
        return
    for path, content in expected:
        path.write_text(content, encoding="utf-8")
        print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
