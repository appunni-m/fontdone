#!/usr/bin/env python3
"""Build compact, project-authored Type 42 fixture fonts.

The embedded TrueType font, its outlines, names, dictionaries, and Type 42
wrapper are synthetic.  No third-party font bytes are read or copied.
"""

from __future__ import annotations

from io import BytesIO
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "type42"


def glyph(points: list[tuple[int, int]]) -> object:
    pen = TTGlyphPen(None)
    if points:
        pen.moveTo(points[0])
        for point in points[1:]:
            pen.lineTo(point)
        pen.closePath()
    return pen.glyph()


def embedded_sfnt() -> bytes:
    builder = FontBuilder(1000, isTTF=True)
    glyph_order = [".notdef", "A"]
    builder.setupGlyphOrder(glyph_order)
    builder.setupCharacterMap({0x41: "A"})
    builder.setupGlyf(
        {
            ".notdef": glyph([]),
            "A": glyph([(50, 0), (300, 700), (550, 0)]),
        }
    )
    builder.setupHorizontalMetrics({".notdef": (600, 0), "A": (600, 50)})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupNameTable(
        {
            "familyName": "Fontdone Type42",
            "styleName": "Regular",
            "uniqueFontIdentifier": "FontdoneType42-Regular",
            "fullName": "Fontdone Type42 Regular",
            "psName": "FontdoneType42-Regular",
            "version": "Version 1.000",
        }
    )
    builder.setupPost(keepGlyphNames=True)
    builder.setupMaxp()
    builder.font["head"].created = 0
    builder.font["head"].modified = 0
    builder.font.recalcTimestamp = False
    output = BytesIO()
    builder.font.save(output, reorderTables=True)
    return output.getvalue()


def type42_wrapper(sfnt: bytes) -> bytes:
    hex_rows = [
        sfnt.hex().upper()[offset : offset + 128]
        for offset in range(0, len(sfnt) * 2, 128)
    ]
    lines = [
        "%!PS-TrueTypeFont-1.0: FontdoneType42 1.0",
        "11 dict begin",
        "/FontName /FontdoneType42 def",
        "/PaintType 0 def",
        "/FontType 42 def",
        "/FontMatrix [1 0 0 1 0 0] def",
        "/FontBBox [0 0 600 700] def",
        "/Encoding 256 array",
        "0 1 255 {1 index exch /.notdef put} for",
        "dup 65 /A put",
        "readonly def",
        "/FontInfo 9 dict dup begin",
        "/version (1.0) def",
        "/Notice (Project-authored synthetic fontdone Type 42 fixture) def",
        "/FullName (Fontdone Type42 Regular) def",
        "/FamilyName (Fontdone Type42) def",
        "/Weight (Regular) def",
        "/ItalicAngle 0 def",
        "/isFixedPitch false def",
        "/UnderlinePosition -100 def",
        "/UnderlineThickness 50 def",
        "end readonly def",
        "/sfnts [",
        *[f"<{row}>" for row in hex_rows],
        "] def",
        "/CharStrings 2 dict dup begin",
        "/.notdef 0 def",
        "/A 1 def",
        "end readonly def",
        "FontName currentdict end definefont pop",
        "",
    ]
    return "\n".join(lines).encode("ascii")


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "fontinfo-and-glyph-names.t42").write_bytes(
        type42_wrapper(embedded_sfnt())
    )


if __name__ == "__main__":
    main()
