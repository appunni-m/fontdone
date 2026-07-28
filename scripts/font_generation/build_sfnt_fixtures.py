#!/usr/bin/env python3
"""Build compact SFNT table fixtures for public table APIs."""

from __future__ import annotations

from pathlib import Path
import struct

from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.DefaultTable import DefaultTable


ROOT = Path(__file__).resolve().parents[2]
BASE_FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "sfnt"
OPENTYPE_OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "opentype"
LEGACY_SYNTHETIC_SFNT_OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "synthetic" / "sfnt"
GENERATED_OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "generated" / "sfnt"
MALFORMED_TTC_OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "malformed" / "ttc"


def save_font(name: str, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def save_generated_font(name: str, font: TTFont) -> None:
    GENERATED_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = GENERATED_OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def save_synthetic_sfnt(name: str, data: bytes) -> None:
    LEGACY_SYNTHETIC_SFNT_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = LEGACY_SYNTHETIC_SFNT_OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    out.write_bytes(data)


def save_malformed_ttc(name: str, data: bytes) -> None:
    MALFORMED_TTC_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = MALFORMED_TTC_OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    out.write_bytes(data)


def save_opentype_font(name: str, font: TTFont) -> None:
    OPENTYPE_OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OPENTYPE_OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def base_font() -> TTFont:
    return TTFont(BASE_FONT, recalcTimestamp=False)


def raw_table(tag: str, data: bytes) -> DefaultTable:
    table = DefaultTable(tag)
    table.data = data
    return table


def pclt_table(version: int) -> DefaultTable:
    typeface = b"Compact SFNT".ljust(16, b"\0")
    complement = b"COVRAGE1"
    filename = b"CSFNT1"
    data = struct.pack(
        ">LLHHHHHH16s8s6sbbBB",
        version,
        42,
        640,
        450,
        1,
        2,
        700,
        0x04E4,
        typeface,
        complement,
        filename,
        -3,
        5,
        2,
        0,
    )
    return raw_table("PCLT", data)


def add_vertical_metrics(font: TTFont) -> None:
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


def write_basic() -> None:
    font = base_font()
    save_font("basic-ttf.ttf", font)


def write_basic_alias(name: str) -> None:
    font = base_font()
    save_font(name, font)


def write_pclt_present() -> None:
    font = base_font()
    font["PCLT"] = pclt_table(0x00010000)
    save_font("pclt-present.ttf", font)


def write_pclt_short() -> None:
    font = base_font()
    font["PCLT"] = raw_table("PCLT", b"\0" * 12)
    save_font("pclt-short.ttf", font)


def write_pclt_version_zero() -> None:
    font = base_font()
    font["PCLT"] = pclt_table(0)
    save_font("pclt-version-zero.ttf", font)


def write_vertical_present() -> None:
    font = base_font()
    add_vertical_metrics(font)
    save_font("vhea-vmtx-present.ttf", font)


def write_no_os2() -> None:
    font = base_font()
    del font["OS/2"]
    save_font("no-os2.ttf", font)


def write_missing_hmtx() -> None:
    font = base_font()
    # FreeType 2.14.3 sfnt/sfobjs.c reports FT_Err_Hmtx_Table_Missing when
    # opening a TrueType SFNT with `hhea` present but no `hmtx` metrics table.
    del font["hmtx"]
    save_generated_font("missing-hmtx.ttf", font)


def write_malformed_maxp() -> None:
    # These files back `tttables.TT_MaxProfile.malformed_table_error_source`.
    # The previous fixture paths were DejaVuSans symlinks, which made the row a
    # green placeholder.  Keep both as complete SFNT wrappers with deliberately
    # malformed `maxp` payloads so pinned C FreeType and Rust observe the same
    # malformed table input before route promotion is considered.
    truncated = base_font()
    truncated["maxp"] = raw_table("maxp", b"\x00\x01\x00\x00")
    save_font("truncated-maxp.ttf", truncated)

    invalid = base_font()
    invalid["maxp"] = raw_table("maxp", b"\x00\x01\x00\x00\x00\x02\x00\x01")
    save_font("invalid-maxp.ttf", invalid)


def write_recognized_broken_sfnt() -> None:
    # FreeType's SFNT driver recognizes the 0x00010000 scaler type before it
    # validates the table directory.  A directory declaring zero tables reaches
    # the pinned C public `FT_Err_Invalid_Stream_Operation` constructor path.
    save_synthetic_sfnt(
        "recognized-broken-sfnt.ttf",
        b"\x00\x01\x00\x00" + b"\x00\x00" + b"\x00\x00" + b"\x00\x00" + b"\x00\x00",
    )


def valid_opentype_layout_font() -> TTFont:
    font = base_font()
    # This project-authored feature source creates compact, deterministic
    # GDEF, GPOS, and GSUB tables.  BASE, JSTF, and MATH are deliberately
    # absent so the validator fixture also proves selected-absent null output.
    addOpenTypeFeaturesFromString(
        font,
        """
        @BaseGlyphs = [\\base attachPoint];
        @MarkGlyphs = [\\mark];
        table GDEF {
          GlyphClassDef @BaseGlyphs, , @MarkGlyphs, ;
        } GDEF;
        feature kern {
          pos \\base \\mark -20;
        } kern;
        feature liga {
          sub \\base \\mark by attachPoint;
        } liga;
        """,
    )
    return font


def write_valid_opentype_layout() -> None:
    font = valid_opentype_layout_font()
    save_opentype_font("valid-all-layout.otf", font)

    # Keep one selected-table fixture per public validation flag.  The GDEF,
    # GPOS, and GSUB payloads come from the project-authored feature source
    # above; the compact BASE, JSTF, and MATH payloads are valid empty tables
    # authored directly from their public OpenType record layouts.
    for name, selected_tag in (
        ("valid-gdef.otf", "GDEF"),
        ("valid-gpos.otf", "GPOS"),
        ("valid-gsub.otf", "GSUB"),
    ):
        selected = valid_opentype_layout_font()
        for tag in ("GDEF", "GPOS", "GSUB"):
            if tag != selected_tag and tag in selected:
                del selected[tag]
        save_opentype_font(name, selected)

    base = base_font()
    base["BASE"] = raw_table("BASE", b"\x00\x01\x00\x00\x00\x00\x00\x00")
    save_opentype_font("valid-base.otf", base)

    jstf = base_font()
    jstf["JSTF"] = raw_table("JSTF", b"\x00\x01\x00\x00\x00\x00")
    save_opentype_font("valid-jstf.otf", jstf)

    math_constants = bytes(2 * (56 + 51))
    math_glyph_info = bytes(8)
    math_variants = bytes(10)
    constants_offset = 10
    glyph_info_offset = constants_offset + len(math_constants)
    variants_offset = glyph_info_offset + len(math_glyph_info)
    math_data = (
        b"\x00\x01\x00\x00"
        + constants_offset.to_bytes(2, "big")
        + glyph_info_offset.to_bytes(2, "big")
        + variants_offset.to_bytes(2, "big")
        + math_constants
        + math_glyph_info
        + math_variants
    )
    math = base_font()
    math["MATH"] = raw_table("MATH", math_data)
    save_opentype_font("valid-math.otf", math)


def write_malformed_opentype_layouts() -> None:
    # Each malformed fixture preserves a valid SFNT and changes only the
    # selected OpenType layout table.  A one-byte table cannot contain the
    # mandatory version and offset header, so the pinned validator reaches its
    # ordinary bounds-check failure instead of an SFNT-open failure.
    for name, tag in (
        ("malformed-gdef.otf", "GDEF"),
        ("malformed-gpos.otf", "GPOS"),
        ("malformed-gsub.otf", "GSUB"),
        ("malformed-jstf.otf", "JSTF"),
        ("malformed-math.otf", "MATH"),
        ("malformed-selected-layout.otf", "GDEF"),
    ):
        font = valid_opentype_layout_font()
        font[tag] = raw_table(tag, b"\0")
        save_opentype_font(name, font)

    # MATH is validated after the public BASE/GDEF/GPOS/GSUB/JSTF tables.
    # Keeping the generated GDEF/GPOS/GSUB tables valid and failing MATH proves
    # that all earlier face-memory allocations are reclaimed on a late error.
    partial = valid_opentype_layout_font()
    partial["MATH"] = raw_table("MATH", b"\0")
    save_opentype_font("partial-malformed-layout.otf", partial)


def write_ttc_count_overflow() -> None:
    # FreeType 2.14.3 sfnt/sfobjs.c rejects this TTC header as
    # FT_Err_Array_Too_Large because the declared face-count makes the offset
    # array larger than the stream before any face directory is read.
    save_malformed_ttc(
        "count-overflows-offset-array.ttc",
        b"ttcf" + (0x0001_0000).to_bytes(4, "big") + (0x4000_0000).to_bytes(4, "big"),
    )


def main() -> None:
    write_basic()
    write_basic_alias("pclt-missing.ttf")
    write_basic_alias("no-vhea.ttf")
    write_pclt_present()
    write_pclt_short()
    write_pclt_version_zero()
    write_vertical_present()
    write_no_os2()
    write_missing_hmtx()
    write_malformed_maxp()
    write_recognized_broken_sfnt()
    write_valid_opentype_layout()
    write_malformed_opentype_layouts()
    write_ttc_count_overflow()


if __name__ == "__main__":
    main()
