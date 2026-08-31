#!/usr/bin/env python3
"""Generate deterministic malformed BDF fixtures for constructor error parity."""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"


VALID_PREFIX = """STARTFONT 2.1
FONT PillowRsMalformedBDF
SIZE 8 75 75
FONTBOUNDINGBOX 8 8 0 -2
STARTPROPERTIES 2
FONT_ASCENT 6
FONT_DESCENT 2
ENDPROPERTIES
CHARS 1
"""

VALID_GLYPH = """STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 8 0
BBX 8 8 0 -2
BITMAP
00
18
24
42
7E
42
42
00
ENDCHAR
ENDFONT
"""


def write_fixture(relative: str, text: str) -> None:
    path = FIXTURE_ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("ascii"))


def pixel_size_property_fixture(pixel_size: str | None) -> str:
    properties = ["FONT_ASCENT 6", "FONT_DESCENT 2", "POINT_SIZE 120"]
    if pixel_size is not None:
        properties.append(f"PIXEL_SIZE {pixel_size}")
    return f'''STARTFONT 2.1
FONT FontdonePixelSizeEdge
SIZE 12 75 75
FONTBOUNDINGBOX 8 8 0 -2
STARTPROPERTIES {len(properties)}
{chr(10).join(properties)}
ENDPROPERTIES
CHARS 1
{VALID_GLYPH}'''


def malformed_numeric_property_fixture(
    index: int, property_name: str, raw_value: str | None
) -> str:
    properties = []
    if property_name != "FONT_ASCENT":
        properties.append("FONT_ASCENT 6")
    if property_name != "FONT_DESCENT":
        properties.append("FONT_DESCENT 2")
    properties.append(
        property_name if raw_value is None else f"{property_name} {raw_value}"
    )
    return f'''STARTFONT 2.1
FONT FontdoneBatch232BdfNumeric{index:02d}
SIZE 12 75 75
FONTBOUNDINGBOX 8 8 0 -2
STARTPROPERTIES {len(properties)}
{chr(10).join(properties)}
ENDPROPERTIES
CHARS 1
{VALID_GLYPH}'''


def malformed_size_fixture(index: int, size_line: str) -> str:
    return f'''STARTFONT 2.1
FONT FontdoneBatch235BdfSize{index:02d}
{size_line}
FONTBOUNDINGBOX 8 8 0 -2
STARTPROPERTIES 2
FONT_ASCENT 6
FONT_DESCENT 2
ENDPROPERTIES
CHARS 1
{VALID_GLYPH}'''


def bitmap_limit_fixture(
    family: str, size_line: str, bbx_line: str, bitmap_rows: str = ""
) -> str:
    return f'''STARTFONT 2.1
FONT {family}
{size_line}
FONTBOUNDINGBOX 8 8 0 -2
STARTPROPERTIES 3
FONT_ASCENT 6
FONT_DESCENT 2
PIXEL_SIZE 12
ENDPROPERTIES
CHARS 1
STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 65535 0
{bbx_line}
BITMAP
{bitmap_rows}
ENDCHAR
ENDFONT
'''


def malformed_bbx_fixture(family: str, bbx_line: str, bitmap_rows: str = "") -> str:
    return bitmap_limit_fixture(family, "SIZE 12 75 75", bbx_line, bitmap_rows)


def charset_registry_fixture(family: str, registry: str, encoding: str) -> str:
    return f'''STARTFONT 2.1
FONT {family}
SIZE 12 75 75
FONTBOUNDINGBOX 5 10 0 -2
STARTPROPERTIES 7
FONT_ASCENT 8
FONT_DESCENT 2
FOUNDRY "PillowRs"
POINT_SIZE 120
PIXEL_SIZE 12
CHARSET_REGISTRY "{registry}"
CHARSET_ENCODING "{encoding}"
ENDPROPERTIES
CHARS 1
STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 5 0
BBX 5 10 0 -2
BITMAP
70
88
88
F8
88
88
88
00
00
00
ENDCHAR
ENDFONT
'''


def main() -> None:
    malformed_size_variants = [
        "SIZE 12tail 75 75",
        "SIZE 12 75tail 75",
        "SIZE 12 75 96tail",
        "SIZE 12tail 75tail 96tail",
        "SIZE +12 75 75",
        "SIZE -12 75 75",
        "SIZE junk 75 75",
        "SIZE 0 75 75",
        "SIZE 12 0 0",
        "SIZE 12 75 0",
        "SIZE 12 0 75",
        "SIZE 12 75 99999",
        "SIZE 12 99999 75",
        "SIZE 12 99999 99999",
        "SIZE 12 75 75junk",
        "SIZE 2147483648 75 75",
        "SIZE 32768 75 75",
        "SIZE 32767 75 75",
        "SIZE 32767tail 75 75",
        "SIZE 999999999999999999999 75 75",
        "SIZE -999999999999999 75 75",
        "SIZE +999999999999999 75 75",
        "SIZE 12 +75 75",
        "SIZE 12 -75 75",
        "SIZE 12 75 +75",
        "SIZE 12 75 -75",
        "SIZE 12 75 075suffix",
        "SIZE 12 00000000000000000000075 75",
        "SIZE 12 75 00000000000000000000096",
        "SIZE 00012 00075 00096",
    ]
    for index, size_line in enumerate(malformed_size_variants, start=1):
        write_fixture(
            f"input/fonts/bdf/malformed-size/batch235-{index:02d}.bdf",
            malformed_size_fixture(index, size_line),
        )

    malformed_numeric_variants = [
        ("AVERAGE_WIDTH", None, "no-value"),
        ("AVG_CAPITAL_WIDTH", "junk", "junk"),
        ("AVG_LOWERCASE_WIDTH", "42tail", "prefix"),
        ("CAP_HEIGHT", "-17tail", "negative-prefix"),
        ("END_SPACE", "+9", "plus-sign"),
        ("FIGURE_WIDTH", "-0x1", "hex-prefix"),
        ("FONT_ASCENT", None, "no-value"),
        ("FONT_DESCENT", "12oops", "prefix"),
        ("ITALIC_ANGLE", "3.5", "real-prefix"),
        ("MAX_SPACE", "2147483647tail", "i32-prefix"),
        ("MIN_SPACE", "-2147483648tail", "i32-negative-prefix"),
        ("NORM_SPACE", "999999999999999999999tail", "saturated-prefix"),
        ("PIXEL_SIZE", "junk", "no-digit"),
        ("POINT_SIZE", "120oops", "prefix"),
        ("QUAD_WIDTH", "007suffix", "leading-zero-prefix"),
        ("RAW_ASCENT", "-7tail", "negative-prefix"),
        ("RAW_AVERAGE_WIDTH", None, "no-value"),
        ("RAW_CAP_HEIGHT", "5x", "prefix"),
        ("RAW_DESCENT", "+11", "plus-sign"),
        ("RAW_PIXEL_SIZE", "16.0", "real-prefix"),
        ("SMALL_CAP_SIZE", "4rest", "prefix"),
        ("STRIKEOUT_ASCENT", "-3tail", "negative-prefix"),
        ("SUBSCRIPT_X", "9abc", "prefix"),
        ("UNDERLINE_POSITION", None, "no-value"),
        ("DEFAULT_CHAR", None, "no-value"),
        ("DESTINATION", "42tail", "prefix"),
        ("RELATIVE_SETWIDTH", "+9", "plus-sign"),
        ("RELATIVE_WEIGHT", "-1", "minus-sign"),
        ("RESOLUTION_X", "75oops", "prefix"),
        ("RESOLUTION_Y", "96tail", "prefix"),
    ]
    for index, (property_name, raw_value, label) in enumerate(
        malformed_numeric_variants, start=1
    ):
        write_fixture(
            f"input/fonts/bdf/malformed-numeric/batch232-{index:02d}-{property_name.lower()}-{label}.bdf",
            malformed_numeric_property_fixture(index, property_name, raw_value),
        )
    write_fixture(
        "input/fonts/bdf/properties-missing-pixel-size.bdf",
        pixel_size_property_fixture(None),
    )
    write_fixture(
        "input/fonts/bdf/properties-zero-pixel-size.bdf",
        pixel_size_property_fixture("0"),
    )
    write_fixture(
        "input/fonts/bdf/properties-negative-pixel-size.bdf",
        pixel_size_property_fixture("-12"),
    )
    write_fixture(
        "input/fonts/bdf/properties-oversized-pixel-size.bdf",
        pixel_size_property_fixture("40000"),
    )
    write_fixture(
        "input/fonts/bdf/zero-glyphs-strike.bdf",
        """STARTFONT 2.1
FONT FontdoneZeroGlyphsStrike
SIZE 12 75 75
FONTBOUNDINGBOX 8 12 0 -2
STARTPROPERTIES 3
FAMILY_NAME "FontdoneZeroGlyphsStrike"
POINT_SIZE 120
PIXEL_SIZE 12
ENDPROPERTIES
CHARS 0
ENDFONT
""",
    )
    write_fixture(
        "input/fonts/bdf/properties-atoms-integers-cardinals.bdf",
        """STARTFONT 2.1
FONT PillowRsPropertiesAtomsIntegersCardinals
SIZE 12 75 75
FONTBOUNDINGBOX 5 10 0 -2
STARTPROPERTIES 6
FONT_ASCENT 8
FONT_DESCENT 2
FOUNDRY \"PillowRs\"
POINT_SIZE 120
PIXEL_SIZE 12
RESOLUTION_X 75
ENDPROPERTIES
CHARS 1
STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 5 0
BBX 5 10 0 -2
BITMAP
70
88
88
F8
88
88
88
00
00
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "input/fonts/bdf/properties-duplicate-and-empty.bdf",
        """STARTFONT 2.1
FONT PillowRsPropertiesDuplicateAndEmpty
SIZE 12 75 75
FONTBOUNDINGBOX 5 10 0 -2
STARTPROPERTIES 8
FONT_ASCENT 8
FONT_DESCENT 2
FOUNDRY "PillowRs"
POINT_SIZE 120
POINT_SIZE 144
PIXEL_SIZE 12
RESOLUTION_X 75
UNNAMED_PROPERTY_WITHOUT_VALUE
ENDPROPERTIES
CHARS 1
STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 5 0
BBX 5 10 0 -2
BITMAP
70
88
88
F8
88
88
88
00
00
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "input/fonts/bdf/charset-registry-iso8859.bdf",
        charset_registry_fixture(
            "PillowRsCharsetRegistryIso8859", "ISO8859", "1"
        ),
    )
    write_fixture(
        "input/fonts/bdf/charset-registry-iso8859-other.bdf",
        charset_registry_fixture(
            "PillowRsCharsetRegistryIso8859Other", "ISO8859", "2"
        ),
    )
    write_fixture(
        "input/fonts/bdf/charset-registry-iso646.bdf",
        charset_registry_fixture(
            "PillowRsCharsetRegistryIso646", "ISO646.1991", "IRV"
        ),
    )
    write_fixture(
        "input/fonts/no-encoding/bdf-or-pcf-encoding-none.bdf",
        """STARTFONT 2.1
FONT FontdoneEncodingNone
SIZE 12 75 75
FONTBOUNDINGBOX 5 10 0 -2
STARTPROPERTIES 3
FONT_ASCENT 8
CHARSET_REGISTRY "FONTDONE"
CHARSET_ENCODING "0"
ENDPROPERTIES
CHARS 1
STARTCHAR mapped
ENCODING 65
SWIDTH 500 0
DWIDTH 5 0
BBX 5 10 0 -2
BITMAP
00
00
00
00
00
00
00
00
00
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "input/generated/bdf/corrupted-header.bdf",
        "STARTFONT 2.1\nFONT PillowRsMalformedBDF\n",
    )
    write_fixture(
        "input/generated/bdf/corrupted-glyphs.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 8 0
BBX 8 8 0 -2
BITMAP
00
18
""",
    )
    write_fixture(
        "input/generated/bdf/bbx-too-big.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
SWIDTH 500 0
DWIDTH 8 0
BBX 70000 8 0 -2
BITMAP
00
ENDCHAR
ENDFONT
""",
    )
    bitmap_limit_variants = [
        (
            "zero-height",
            "FontdoneBatch259ZeroHeight",
            "SIZE 12 75 75",
            "BBX 1 0 0 0",
            "",
        ),
        (
            "width-saturates",
            "FontdoneBatch259WidthSaturates",
            "SIZE 12 75 75",
            "BBX 524288 1 0 0",
            "00",
        ),
        (
            "bpp2-overflow",
            "FontdoneBatch259Bpp2Overflow",
            "SIZE 12 75 75 2",
            "BBX 65535 4 0 0",
            "00",
        ),
        (
            "bpp4-overflow",
            "FontdoneBatch259Bpp4Overflow",
            "SIZE 12 75 75 4",
            "BBX 65535 2 0 0",
            "00",
        ),
        (
            "bpp8-overflow",
            "FontdoneBatch259Bpp8Overflow",
            "SIZE 12 75 75 8",
            "BBX 65535 2 0 0",
            "00",
        ),
    ]
    for suffix, family, size_line, bbx_line, bitmap_rows in bitmap_limit_variants:
        write_fixture(
            f"input/fonts/bdf/batch259-{suffix}.bdf",
            bitmap_limit_fixture(family, size_line, bbx_line, bitmap_rows),
        )
    malformed_bbx_variants = [
        (
            "width-no-digit",
            "FontdoneBatch273BdfBbxWidthNoDigit",
            "BBX junk 8 0 -2",
            "",
        ),
        (
            "height-no-digit",
            "FontdoneBatch273BdfBbxHeightNoDigit",
            "BBX 8 junk 0 -2",
            "",
        ),
        (
            "width-decimal-prefix",
            "FontdoneBatch273BdfBbxWidthPrefix",
            "BBX 42tail 8 0 -2",
            "\n".join(["00"] * 8),
        ),
        (
            "height-decimal-prefix",
            "FontdoneBatch273BdfBbxHeightPrefix",
            "BBX 8 42tail 0 -2",
            "\n".join(["00"] * 42),
        ),
        (
            "empty-fields",
            "FontdoneBatch273BdfBbxEmptyFields",
            "BBX ",
            "",
        ),
    ]
    for suffix, family, bbx_line, bitmap_rows in malformed_bbx_variants:
        write_fixture(
            f"input/fonts/bdf/batch273-{suffix}.bdf",
            malformed_bbx_fixture(family, bbx_line, bitmap_rows),
        )
    write_fixture(
        "input/fixtures/assets/bdf/missing_startfont_field.bdf",
        "FONT PillowRsMalformedBDF\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -2\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "input/fixtures/assets/bdf/missing_font_field.bdf",
        "STARTFONT 2.1\n\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -2\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "input/fixtures/assets/bdf/missing_size_field.bdf",
        "STARTFONT 2.1\nFONT PillowRsMalformedBDF\nFONTBOUNDINGBOX 8 8 0 -2\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "input/fixtures/assets/bdf/missing_fontboundingbox_field.bdf",
        "STARTFONT 2.1\nFONT PillowRsMalformedBDF\nSIZE 8 75 75\nCHARS 0\nENDFONT\n",
    )
    write_fixture(
        "input/fixtures/assets/bdf/missing_startchar_field.bdf",
        VALID_PREFIX + "ENCODING 65\nENDFONT\n",
    )
    write_fixture(
        "input/fixtures/assets/bdf/nested_startchar_before_endchar.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
STARTCHAR B
ENCODING 66
BBX 8 8 0 -2
BITMAP
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "input/fixtures/assets/bdf/missing_encoding_field.bdf",
        VALID_PREFIX
        + """STARTCHAR A
SWIDTH 500 0
DWIDTH 8 0
BBX 8 8 0 -2
BITMAP
00
ENDCHAR
ENDFONT
""",
    )
    write_fixture(
        "input/fixtures/assets/bdf/missing_bbx_field.bdf",
        VALID_PREFIX
        + """STARTCHAR A
ENCODING 65
BITMAP
00
ENDCHAR
ENDFONT
""",
    )


if __name__ == "__main__":
    main()
