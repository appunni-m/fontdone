#!/usr/bin/env python3
"""Build the project-authored OpenType SVG glyph fixture."""

from __future__ import annotations

from pathlib import Path
import struct

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.DefaultTable import DefaultTable


ROOT = Path(__file__).resolve().parents[2]
BASE_FONT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "fonts"
    / "glyf"
    / "hinter-control-matrix.ttf"
)
OUTPUT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "fonts"
    / "svg"
    / "otsvg-glyph.ttf"
)

SVG_DOCUMENT = (
    b'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000">'
    b'<rect id="glyph1" x="100" y="100" width="800" height="800"/>'
    b"</svg>"
)


def add_vertical_metrics(font: TTFont) -> None:
    """Give the SVG route deterministic horizontal and vertical advances."""

    glyph_order = font.getGlyphOrder()
    vmtx = newTable("vmtx")
    vmtx.metrics = {name: (1000, 120) for name in glyph_order}
    font["vmtx"] = vmtx

    vhea = newTable("vhea")
    vhea.tableVersion = 0x00010000
    vhea.ascent = 880
    vhea.descent = -120
    vhea.lineGap = 20
    vhea.advanceHeightMax = 1000
    vhea.minTopSideBearing = 120
    vhea.minBottomSideBearing = 0
    vhea.yMaxExtent = 1000
    vhea.caretSlopeRise = 1
    vhea.caretSlopeRun = 0
    vhea.caretOffset = 0
    vhea.reserved1 = 0
    vhea.reserved2 = 0
    vhea.reserved3 = 0
    vhea.reserved4 = 0
    vhea.metricDataFormat = 0
    vhea.numberOfVMetrics = len(glyph_order)
    font["vhea"] = vhea


def svg_table() -> DefaultTable:
    """Return one SVG document-list record covering glyph index 1."""

    document_list_offset = 10
    document_offset = 2 + 12
    data = bytearray()
    data.extend(struct.pack(">HLL", 0, document_list_offset, 0))
    data.extend(struct.pack(">H", 1))
    data.extend(struct.pack(">HHLL", 1, 1, document_offset, len(SVG_DOCUMENT)))
    data.extend(SVG_DOCUMENT)
    table = DefaultTable("SVG ")
    table.data = bytes(data)
    return table


def main() -> None:
    if not BASE_FONT.is_file():
        raise SystemExit(f"missing reviewed synthetic base font: {BASE_FONT}")
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    add_vertical_metrics(font)
    font["SVG "] = svg_table()
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    if OUTPUT.exists() or OUTPUT.is_symlink():
        OUTPUT.unlink()
    font.save(OUTPUT, reorderTables=True)


if __name__ == "__main__":
    main()
