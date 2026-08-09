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


def svg_payload_records(records: tuple[tuple[int, int, bytes], ...]) -> bytes:
    """Return an SVG document list with the supplied inclusive glyph ranges."""

    document_list_offset = 10
    document_offset = 2 + 12 * len(records)
    data = bytearray()
    data.extend(struct.pack(">HLL", 0, document_list_offset, 0))
    data.extend(struct.pack(">H", len(records)))
    for start_glyph, end_glyph, document in records:
        data.extend(
            struct.pack(">HHLL", start_glyph, end_glyph, document_offset, len(document))
        )
        document_offset += len(document)
    for _, _, document in records:
        data.extend(document)
    return bytes(data)


def svg_payload(document: bytes = SVG_DOCUMENT) -> bytes:
    """Return one SVG document-list record covering glyph index 1."""

    return svg_payload_records(((1, 1, document),))


def svg_table(data: bytes | None = None) -> DefaultTable:
    """Return an OpenType SVG table from reviewed raw payload bytes."""

    table = DefaultTable("SVG ")
    table.data = svg_payload() if data is None else data
    return table


def malformed_svg_payloads() -> dict[str, bytes]:
    """Return malformed SVG controls whose tables are ignored at face open."""

    valid = bytearray(svg_payload())

    list_before_header = bytearray(valid)
    list_before_header[2:6] = struct.pack(">L", 9)

    list_out_of_range = bytearray(valid)
    list_out_of_range[2:6] = struct.pack(">L", 0xFFFF_FFFF)

    records_truncated = bytearray(struct.pack(">HLLH", 0, 10, 0, 2))
    records_truncated.extend(b"\0" * 12)

    document_out_of_bounds = bytearray(valid)
    document_out_of_bounds[16:20] = struct.pack(">L", 0xFFFF_FFF0)

    gzip_document = b"\x1f\x8b\x08" + b"\0" * (len(SVG_DOCUMENT) - 3)

    return {
        "svg-list-offset-before-header.ttf": bytes(list_before_header),
        "svg-list-offset-out-of-range.ttf": bytes(list_out_of_range),
        "svg-document-records-truncated.ttf": bytes(records_truncated),
        "svg-document-out-of-bounds.ttf": bytes(document_out_of_bounds),
        "svg-gzip-document.ttf": svg_payload(gzip_document),
        "svg-short-table.ttf": b"\0\0",
    }


def write_font(path: Path, payload: bytes) -> None:
    """Write one deterministic base font with the supplied SVG payload."""

    if not BASE_FONT.is_file():
        raise SystemExit(f"missing reviewed synthetic base font: {BASE_FONT}")
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    add_vertical_metrics(font)
    font["SVG "] = svg_table(payload)
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    font.save(path, reorderTables=True)
    font.close()


def main() -> None:
    write_font(OUTPUT, svg_payload())
    write_font(
        OUTPUT.parent / "otsvg-glyph-range-gap.ttf",
        svg_payload_records(((1, 1, SVG_DOCUMENT), (3, 3, SVG_DOCUMENT))),
    )
    for name, payload in malformed_svg_payloads().items():
        write_font(OUTPUT.parent / name, payload)


if __name__ == "__main__":
    main()
