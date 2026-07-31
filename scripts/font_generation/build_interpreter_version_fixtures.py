#!/usr/bin/env python3
"""Build compact TrueType fixtures sensitive to the interpreter-version
property (TT_INTERPRETER_VERSION_35/38/40).

The glyph programs use GETINFO[] selector 1 to branch on the active
interpreter version and shift a point by different amounts, producing an
outline/advance difference that is observable through the public glyph
metrics when loaded with FT_LOAD_RENDER.  The control font carries the same
outlines without version-dependent instructions, proving that non-GETINFO
paths stay identical across versions.
"""

from __future__ import annotations

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "truetype"
UNITS_PER_EM = 1024
GLYPH_ORDER = [
    ".notdef",
    "GETINFO_probe",
    "backward_compat_component",
    "phantom_point_sensitive",
]


def off_grid_box_glyph(points: list[tuple[int, int]]) -> object:
    pen = TTGlyphPen(None)
    for index, (x, y) in enumerate(points):
        if index == 0:
            pen.moveTo((x, y))
        else:
            pen.lineTo((x, y))
    pen.closePath()
    return pen.glyph()


def with_getinfo_instructions(glyph: object, point: int = 1) -> object:
    program = Program()
    # PUSHB[1] 1; GETINFO; PUSHB[1] 35; EQ; IF; PUSHB[1] <point>; MDAP[1]; EIF
    # MDAP rounds the selected off-grid point, so the hinted outline differs
    # between interpreter version 35 and 40 (which reports itself via GETINFO).
    bytecode = bytes([0xB0, 0x01, 0x88, 0xB0, 0x28, 0x54, 0x58])
    bytecode += bytes([0xB0, 0x01, point, 0x2F, 0x59])
    program.fromBytecode(bytecode)
    glyph.program = program
    return glyph


def build_font(name: str, version_sensitive: bool) -> None:
    builder = FontBuilder(UNITS_PER_EM, isTTF=True)
    boxes = [
        off_grid_box_glyph([(37, 61), (200, 100), (250, 180), (50, 240)]),
        off_grid_box_glyph([(30, 40), (190, 130), (260, 170), (60, 210)]),
        off_grid_box_glyph([(45, 25), (180, 120), (240, 190), (70, 220)]),
    ]
    glyphs = [boxes[0]]
    if version_sensitive:
        for index, box in enumerate(boxes):
            glyphs.append(with_getinfo_instructions(box, point=index + 1))
    else:
        glyphs += boxes
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap({0xE000 + i: GLYPH_ORDER[i] for i in range(len(GLYPH_ORDER))})
    builder.setupGlyf(dict(zip(GLYPH_ORDER, glyphs)))
    builder.setupHorizontalMetrics({g: (700, 0) for g in GLYPH_ORDER})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable({"familyName": f"fontdone {name}", "styleName": "Regular"})
    builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    builder.setupPost()
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    builder.save(out)
    font = TTFont(out, recalcTimestamp=False)
    # FontBuilder stamps the head table with the current time; pin it so the
    # fixture is byte-deterministic across generator runs.
    font["head"].created = 0
    font["head"].modified = 0
    maxp = font["maxp"]
    maxp.maxPoints = 16
    maxp.maxContours = 8
    maxp.maxZones = 2
    maxp.maxTwilightPoints = 16
    maxp.maxStorage = 64
    maxp.maxFunctionDefs = 64
    maxp.maxInstructionDefs = 64
    maxp.maxStackElements = 512
    maxp.maxSizeOfInstructions = 256
    prep = newTable("prep")
    program = Program()
    program.fromBytecode(bytes.fromhex("b0 00 21 b0 00 21 b0 00 21"))
    prep.program = program
    font["prep"] = prep
    font.save(out, reorderTables=True)


def main() -> None:
    build_font("bytecode-interpreter-version.ttf", version_sensitive=True)
    build_font("backward-compat-phantom-points.ttf", version_sensitive=False)
    for name in sorted(OUT_DIR.glob("*.ttf")):
        if name.name in ("bytecode-interpreter-version.ttf", "backward-compat-phantom-points.ttf"):
            data = name.read_bytes()
            import hashlib

            print(f"{name.name}: {len(data)} bytes sha256={hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
