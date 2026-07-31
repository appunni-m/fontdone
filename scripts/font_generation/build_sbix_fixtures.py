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


def sbix_table(num_glyphs: int, flags: int) -> bytes:
    """Build a one-strike 'sbix' table with a `png ` record for glyph 1.

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
    offsets_bytes = offsets_count * 4
    first_data_offset = len(strike_header) + offsets_bytes
    # Project-authored payload: a PNG signature is all the pinned C loader
    # inspects before returning Unimplemented_Feature (PNG support disabled).
    payload = b"\x89PNG\r\n\x1a\n" + b"\0" * 8
    record = struct.pack(">hh4s", 0, 0, b"png ") + payload
    end_offset = first_data_offset + len(record)

    offsets = [first_data_offset, first_data_offset]
    offsets.extend([end_offset] * (offsets_count - 2))
    strike = strike_header + struct.pack(f">{offsets_count}I", *offsets) + record
    table = (
        struct.pack(">HHI", 1, flags, 1)
        + struct.pack(">I", strike_offset)
        + strike
    )
    return table


def save_sbix_font(
    name: str,
    flags: int,
    *,
    drop_outlines: bool = False,
) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    num_glyphs = font["maxp"].numGlyphs
    font["sbix"] = raw_table("sbix", sbix_table(num_glyphs, flags))
    if drop_outlines:
        del font["glyf"]
        del font["loca"]
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def main() -> None:
    save_sbix_font("sbix-with-outlines.ttf", flags=1)
    save_sbix_font("sbix-overlay.ttf", flags=3)
    save_sbix_font("sbix-bitmap-only.ttf", flags=1, drop_outlines=True)
    for name in sorted(OUT_DIR.glob("*.ttf")):
        data = name.read_bytes()
        import hashlib

        print(f"{name.name}: {len(data)} bytes sha256={hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
