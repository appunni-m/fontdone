#!/usr/bin/env python3
"""Build compact synthetic OpenType 'sbix' fixtures.

The pinned FreeType 2.14.3 oracle build has `FT_CONFIG_OPTION_USE_PNG`
disabled, so an 'sbix' table with a `png ` graphic record is accepted for
face flags and strike metrics but returns `Unimplemented_Feature` from
`TT_Load_Glyph`.  The fixture glyphs therefore carry a minimal project-authored
PNG signature payload that the oracle never decodes; this keeps the fixture
fully synthetic and deterministic while preserving the exact public
`FT_Open_Face` parameter behavior under test.
"""

from __future__ import annotations

from pathlib import Path
import struct

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables.DefaultTable import DefaultTable


ROOT = Path(__file__).resolve().parents[2]
BASE_FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "sbix"

# One byte for each sfnt table checksum/type record.
STRIKE_PPEM = 24
STRIKE_PPI = 72


def raw_table(tag: str, data: bytes) -> DefaultTable:
    table = DefaultTable(tag)
    table.data = data
    return table


def graphic_record(graphic_type: bytes, payload: bytes = b"") -> bytes:
    """Build one sbix glyph record with deterministic zero origins."""

    if len(graphic_type) != 4:
        raise ValueError(f"sbix graphic type must be four bytes: {graphic_type!r}")
    return struct.pack(">hh4s", 0, 0, graphic_type) + payload


def sbix_table(
    num_glyphs: int,
    flags: int,
    records: dict[int, bytes] | None = None,
) -> bytes:
    """Build a one-strike 'sbix' table from per-glyph records.

    Layout (OpenType 'sbix'):
      u16 version
      u16 flags
      u32 numStrikes
      u32 strikeOffset[numStrikes]
      strike: u16 ppem, u16 ppi, u32 glyphDataOffset[numGlyphs + 1], data...
    """
    strike_offset = 8 + 4 * 1
    strike_header = struct.pack(">HH", STRIKE_PPEM, STRIKE_PPI)
    offsets_count = num_glyphs + 1
    first_data_offset = len(strike_header) + offsets_count * 4
    if records is None:
        # Project-authored payload: a PNG signature is all the pinned C loader
        # inspects before returning Unimplemented_Feature (PNG support
        # disabled).  Keeping this as the default preserves the original
        # maintained sbix fixture bytes.
        payload = b"\x89PNG\r\n\x1a\n" + b"\0" * 8
        records = {1: graphic_record(b"png ", payload)}

    offsets = [first_data_offset] * offsets_count
    data = bytearray()
    for glyph_index in range(num_glyphs):
        offsets[glyph_index] = first_data_offset + len(data)
        data.extend(records.get(glyph_index, b""))
        offsets[glyph_index + 1] = first_data_offset + len(data)

    table = (
        struct.pack(">HHI", 1, flags, 1)
        + struct.pack(">I", strike_offset)
        + strike_header
        + struct.pack(f">{offsets_count}I", *offsets)
        + data
    )
    return table


def sbix_table_with_ranges(
    num_glyphs: int,
    flags: int,
    ranges: dict[int, tuple[int, int]],
) -> bytes:
    """Build a valid sbix header with deliberately malformed glyph ranges."""

    strike_offset = 8 + 4 * 1
    strike_header = struct.pack(">HH", STRIKE_PPEM, STRIKE_PPI)
    offsets_count = num_glyphs + 1
    first_data_offset = len(strike_header) + offsets_count * 4
    offsets = [first_data_offset] * offsets_count
    for glyph_index, (start, end) in ranges.items():
        offsets[glyph_index] = start
        offsets[glyph_index + 1] = end
    strike = strike_header + struct.pack(f">{offsets_count}I", *offsets)
    return (
        struct.pack(">HHI", 1, flags, 1)
        + struct.pack(">I", strike_offset)
        + strike
    )


def sbix_header_payload(version: int, flags: int, num_strikes: int) -> bytes:
    return struct.pack(">HHI", version, flags, num_strikes)


def save_sbix_font(
    name: str,
    flags: int,
    *,
    records: dict[int, bytes] | None = None,
    drop_outlines: bool = False,
) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    num_glyphs = font["maxp"].numGlyphs
    font["sbix"] = raw_table("sbix", sbix_table(num_glyphs, flags, records))
    if drop_outlines:
        del font["glyf"]
        del font["loca"]
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def save_sbix_payload(
    name: str,
    payload: bytes,
    *,
    drop_outlines: bool = False,
) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["sbix"] = raw_table("sbix", payload)
    if drop_outlines:
        del font["glyf"]
        del font["loca"]
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def reference_record(graphic_type: bytes, glyph_index: int) -> bytes:
    return graphic_record(graphic_type, struct.pack(">H", glyph_index))


def write_error_fixtures() -> None:
    num_glyphs = TTFont(BASE_FONT, recalcTimestamp=False)["maxp"].numGlyphs
    records = {
        1: graphic_record(b"png ", b"\x89PNG\r\n\x1a\n"),
        2: graphic_record(b"jpg "),
        3: graphic_record(b"tiff"),
        4: graphic_record(b"rgbl"),
        5: graphic_record(b"abcd"),
        6: reference_record(b"dupe", 1),
        7: reference_record(b"flip", 1),
        8: reference_record(b"dupe", 0),
        9: reference_record(b"dupe", 10),
        10: reference_record(b"dupe", 11),
        11: reference_record(b"dupe", 12),
        12: reference_record(b"dupe", 14),
        13: graphic_record(b"abcd"),
        # This closes the recursion chain at depth four, where pinned C
        # rejects another `dupe`/`flip` hop before reading its target.
        14: reference_record(b"dupe", 1),
        # A reference record with no two-byte target exercises the truncated
        # payload read; pinned C treats the missing target as a missing bitmap
        # and falls back to the outline on this scalable companion font.
        15: graphic_record(b"dupe"),
        # A valid outer glyph may still reference a target outside maxp; the
        # recursive loader validates that target before looking up its record.
        16: reference_record(b"dupe", 0xFFFF),
    }
    save_sbix_font("sbix-error-matrix.ttf", flags=1, records=records)

    # The loader validates the glyph-offset array against the selected glyph
    # before attempting to read a pair of offsets.  The face-open path keeps
    # this compact table as an accepted optional sbix table.
    short_strike = struct.pack(">HH", STRIKE_PPEM, STRIKE_PPI)
    save_sbix_payload(
        "sbix-glyph-range-short.ttf",
        sbix_header_payload(1, 1, 1) + struct.pack(">I", 12) + short_strike,
    )

    first_data_offset = 4 + (num_glyphs + 1) * 4
    save_sbix_payload(
        "sbix-range-start-after-end.ttf",
        sbix_table_with_ranges(
            num_glyphs,
            1,
            {1: (first_data_offset + 8, first_data_offset)},
        ),
    )
    save_sbix_payload(
        "sbix-range-short-record.ttf",
        sbix_table_with_ranges(
            num_glyphs,
            1,
            {1: (first_data_offset, first_data_offset + 4)},
        ),
    )
    save_sbix_payload(
        "sbix-range-end-out-of-bounds.ttf",
        sbix_table_with_ranges(
            num_glyphs,
            1,
            {1: (first_data_offset, 0x1000)},
        ),
    )


def write_malformed_optional_fixtures() -> None:
    save_sbix_payload("sbix-short-table.ttf", b"\0" * 4)
    save_sbix_payload("sbix-version-zero.ttf", sbix_header_payload(0, 1, 0))
    save_sbix_payload("sbix-invalid-flags.ttf", sbix_header_payload(1, 0, 0))
    save_sbix_payload(
        "sbix-strike-count-overflow.ttf",
        sbix_header_payload(1, 1, 0x1_0000),
    )


def main() -> None:
    save_sbix_font("sbix-with-outlines.ttf", flags=1)
    save_sbix_font("sbix-overlay.ttf", flags=3)
    save_sbix_font("sbix-bitmap-only.ttf", flags=1, drop_outlines=True)
    write_error_fixtures()
    write_malformed_optional_fixtures()
    for name in sorted(OUT_DIR.glob("*.ttf")):
        data = name.read_bytes()
        import hashlib

        print(f"{name.name}: {len(data)} bytes sha256={hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
