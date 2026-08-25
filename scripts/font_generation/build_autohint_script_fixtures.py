#!/usr/bin/env python3
"""Build compact autohint fonts that exercise script-selection coverage."""

from __future__ import annotations

import struct
import shutil
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables.DefaultTable import DefaultTable
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphCoordinates
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "autohint"

UNITS_PER_EM = 1000

SCRIPT_PROBES: list[tuple[str, int]] = [
    ("adlm", 0x1E90C),
    ("arab", 0x0627),
    ("armn", 0x0531),
    ("avst", 0x10B00),
    ("bamu", 0xA6A7),
    ("beng", 0x0987),
    ("buhd", 0x1750),
    ("cakm", 0x11103),
    ("cans", 0x15DC),
    ("cari", 0x102A7),
    ("cher", 0x13C6),
    ("copt", 0x2C8C),
    ("cprt", 0x1080D),
    ("cyrl", 0x0411),
    ("deva", 0x0908),
    ("dsrt", 0x10402),
    ("ethi", 0x1200),
    ("geor", 0x10D2),
    ("geok", 0x10B1),
    ("glag", 0x2C05),
    ("goth", 0x10332),
    ("grek", 0x0393),
    ("gujr", 0x0AA4),
    ("guru", 0x0A07),
    ("hebr", 0x05D1),
    ("kali", 0xA905),
    ("khmr", 0x1781),
    ("khms", 0x19E0),
    ("knda", 0x0C87),
    ("lao", 0x0EB2),
    ("latb", 0x2080),
    ("latp", 0x2070),
    ("latn", 0x006F),
    ("limb", 0x1900),
    ("lisu", 0xA4E1),
    ("mlym", 0x0D12),
    ("medf", 0x16E40),
    ("mong", 0x1833),
    ("mymr", 0x1001),
    ("nkoo", 0x07D0),
    ("olck", 0x1C5B),
    ("orkh", 0x10C17),
    ("orya", 0x0B13),
    ("osge", 0x104BE),
    ("osma", 0x10486),
    ("rohg", 0x10D03),
    ("saur", 0xA89C),
    ("shaw", 0x10455),
    ("sinh", 0x0D89),
    ("sund", 0x1B8B),
    ("sylo", 0xA807),
    ("taml", 0x0B89),
    ("tavt", 0xAA86),
    ("telu", 0x0C07),
    ("tfng", 0x2D54),
    ("thai", 0x0E1A),
    ("tibt", 0x0F40),
    ("vaii", 0xA5CD),
    ("hani", 0x4ED6),
]

# ASCII digits are global autohinter metrics probes, not script probes.  Keep
# them after the script glyphs so existing fixture glyph indices remain stable.
DIGIT_WIDTH_PROBES: list[tuple[str, int, int]] = [
    ("digit_zero_wide", 0x0030, 620),
    ("digit_one_narrow", 0x0031, 520),
]

STANDARD_CHARS: dict[str, int] = {
    "adlm": 0x1E90C,
    "arab": 0x0644,
    "armn": 0x057D,
    "avst": 0x10B1A,
    "bamu": 0xA6C1,
    "beng": 0x09E6,
    "buhd": 0x174B,
    "cakm": 0x11124,
    "cans": 0x144C,
    "cari": 0x102AB,
    "cher": 0x13A4,
    "copt": 0x2C9E,
    "cprt": 0x10805,
    "cyrl": 0x043E,
    "deva": 0x0920,
    "dsrt": 0x10404,
    "ethi": 0x12D0,
    "geok": 0x10B6,
    "geor": 0x10D8,
    "glag": 0x2C15,
    "goth": 0x10334,
    "grek": 0x03BF,
    "gujr": 0x0A9F,
    "guru": 0x0A20,
    "hani": 0x7530,
    "hebr": 0x05DD,
    "kali": 0xA90D,
    "khmr": 0x17E0,
    "khms": 0x19E1,
    "knda": 0x0CE6,
    "lao": 0x0ED0,
    "latb": 0x2092,
    "latn": 0x006F,
    "latp": 0x1D52,
    "limb": 0x006F,
    "lisu": 0xA4F3,
    "medf": 0x16E61,
    "mlym": 0x0D20,
    "mong": 0x1842,
    "mymr": 0x101D,
    "nkoo": 0x07CB,
    "olck": 0x1C5B,
    "orkh": 0x10C17,
    "orya": 0x006F,
    "osge": 0x104C2,
    "osma": 0x10486,
    "rohg": 0x10D30,
    "saur": 0xA89D,
    "shaw": 0x10474,
    "sinh": 0x0DA7,
    "sund": 0x1BB0,
    "sylo": 0x006F,
    "taml": 0x0BE6,
    "tavt": 0xAA92,
    "telu": 0x0C66,
    "tfng": 0x2D54,
    "thai": 0x0E32,
    "tibt": 0x006F,
    "vaii": 0xA613,
}

# Keep a few real blue-string characters mapped to each compact probe glyph.
# The aliases make metrics initialization walk script-specific blue strings
# without adding one glyph per Unicode character.
SCRIPT_BLUE_ALIASES: dict[str, tuple[int, ...]] = {
    "cyrl": (
        0x0411,
        0x0412,
        0x0415,
        0x041E,
        0x0421,
        0x042D,
        0x0435,
        0x0437,
        0x043E,
        0x0441,
        0x0443,
        0x0444,
        0x0445,
        0x0448,
    ),
    "grek": (
        0x0393,
        0x0398,
        0x03A9,
        0x03B1,
        0x03B2,
        0x03B3,
        0x03B4,
        0x03B5,
        0x03B8,
        0x03BF,
        0x03C1,
        0x03C3,
        0x03C4,
        0x03C6,
        0x03C7,
        0x03C8,
        0x03C9,
    ),
    "latn": (
        0x0043,
        0x0045,
        0x0048,
        0x004C,
        0x004F,
        0x0051,
        0x0053,
        0x0054,
        0x0055,
        0x005A,
        0x0062,
        0x0063,
        0x0064,
        0x0065,
        0x0066,
        0x0067,
        0x0068,
        0x0069,
        0x006A,
        0x006B,
        0x006E,
        0x006F,
        0x0070,
        0x0071,
        0x0072,
        0x0073,
        0x0075,
        0x0076,
        0x0078,
        0x0079,
        0x007A,
    ),
}


def empty_glyph():
    return TTGlyphPen(None).glyph()


def rectangle_glyph(left: int, bottom: int, right: int, top: int):
    pen = TTGlyphPen(None)
    pen.moveTo((left, bottom))
    pen.lineTo((left, top))
    pen.lineTo((right, top))
    pen.lineTo((right, bottom))
    pen.closePath()
    return pen.glyph()


def rectangles_glyph(rects: list[tuple[int, int, int, int]]):
    pen = TTGlyphPen(None)
    for left, bottom, right, top in rects:
        pen.moveTo((left, bottom))
        pen.lineTo((left, top))
        pen.lineTo((right, top))
        pen.lineTo((right, bottom))
        pen.closePath()
    return pen.glyph()


def extreme_rectangle_glyph():
    """Full signed-16-bit bounds with individually encodable glyf deltas."""
    pen = TTGlyphPen(None)
    pen.moveTo((-32768, -32768))
    pen.lineTo((-32768, -1))
    pen.lineTo((-32768, 32766))
    pen.lineTo((-32768, 32767))
    pen.lineTo((-1, 32767))
    pen.lineTo((32766, 32767))
    pen.lineTo((32767, 32767))
    pen.lineTo((32767, 0))
    pen.lineTo((32767, -32767))
    pen.lineTo((32767, -32768))
    pen.lineTo((0, -32768))
    pen.lineTo((-32767, -32768))
    pen.lineTo((-32768, -32768))
    pen.closePath()
    return pen.glyph()


def stacked_contour_glyph():
    """Three vertically separated contours for the double-top adjustment path."""
    return rectangles_glyph(
        [
            (100, 0, 500, 500),
            (180, 540, 420, 600),
            (210, 660, 390, 700),
        ]
    )


def top_tilde_glyph(extra_top: bool = False):
    contours = [
        (100, 0, 500, 500),
        # A compact tilde contour: the middle on-curve point is flanked by
        # off-curve controls at the same y so the Latin autohinter measures it
        # as a tilde wave, not as a plain accent rectangle.
        [
            (140, 620, True),
            (190, 580, False),
            (240, 580, True),
            (310, 580, False),
            (370, 620, True),
            (430, 540, True),
        ],
    ]
    if extra_top:
        contours.append((210, 660, 390, 700))
    return mixed_contour_glyph(contours)


def top_tilde_centering_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 616, True),
                (190, 574, False),
                (240, 574, True),
                (310, 574, False),
                (370, 616, True),
                (430, 536, True),
            ],
            (210, 664, 390, 704),
        ]
    )


def top_tilde_measure_zero_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 620, True),
                (190, 580, False),
                (240, 580, True),
                (310, 580, False),
                (370, 540, True),
                (430, 560, True),
            ],
        ]
    )


def top_tilde_flat_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 560, True),
                (430, 560, True),
            ],
        ]
    )


def top_tilde_flat_loop_glyph():
    return mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 560, True),
                (235, 560, True),
                (335, 560, True),
                (430, 560, True),
            ],
        ]
    )


def horizontal_flat_loop_glyph():
    return mixed_contour_glyph(
        [
            [
                (100, 500, True),
                (240, 500, True),
                (380, 500, True),
                (520, 500, True),
            ],
        ]
    )


def bottom_tilde_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 80, True),
                (190, 40, False),
                (240, 40, True),
                (310, 40, False),
                (370, 80, True),
                (430, 0, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tilde_measure_zero_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 80, True),
                (190, 40, False),
                (240, 40, True),
                (310, 40, False),
                (370, 0, True),
                (430, 20, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tilde_flat_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 60, True),
                (430, 60, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tilde_flat_loop_glyph():
    return mixed_contour_glyph(
        [
            [
                (140, 60, True),
                (235, 60, True),
                (335, 60, True),
                (430, 60, True),
            ],
            (100, 120, 500, 620),
        ]
    )


def bottom_tall_accent_glyph():
    return rectangles_glyph(
        [
            (100, 0, 500, 500),
            (190, -620, 410, -80),
        ]
    )


def top_and_bottom_accent_glyph():
    return mixed_contour_glyph(
        [
            [
                (190, -90, True),
                (410, -90, True),
                (410, -30, True),
                (190, -30, True),
            ],
            (100, 0, 500, 500),
            [
                (210, 550, True),
                (390, 550, True),
                (390, 610, True),
                (210, 610, True),
            ],
        ]
    )


def disjoint_top_accent_glyph():
    return rectangles_glyph(
        [
            (80, 0, 300, 500),
            (430, 548, 530, 588),
        ]
    )


def serif_m_symmetry_glyph():
    """Three serifed stems with 12 horizontal-dimension edges."""
    return rectangles_glyph(
        [
            (100, 0, 150, 500),
            (70, 0, 180, 120),
            (70, 380, 180, 500),
            (300, 0, 350, 500),
            (270, 0, 380, 120),
            (270, 380, 380, 500),
            (500, 0, 550, 500),
            (470, 0, 580, 120),
            (470, 380, 580, 500),
        ]
    )


def serif_overlap_break_glyph():
    """Serifed stem with an intermediate vertical edge sharing the serif range."""
    return rectangles_glyph(
        [
            (100, 0, 150, 500),
            (70, 0, 180, 120),
            (85, 30, 95, 90),
        ]
    )


def serif_pointer_order_glyph():
    """Rotated serif topology for a top-to-bottom vertical-dimension scan."""
    return rectangles_glyph(
        [
            (70, 100, 570, 150),
            (70, 70, 190, 180),
            (100, 85, 160, 95),
        ]
    )


def latin_wide_segment_filter_glyph():
    return mixed_contour_glyph(
        [
            [
                (100, 0, True),
                (140, 600, True),
                (260, 600, True),
                (220, 0, True),
            ],
        ]
    )


def hebrew_long_blue_replacement_glyph():
    """Short Hebrew extremum followed by a long, same-direction segment."""
    return mixed_contour_glyph(
        [
            [
                (110, 0, True),
                (100, 500, True),
                (120, 500, True),
                (-300, 480, True),
                (-300, 0, True),
            ],
        ]
    )


def hebrew_long_blue_degenerate_glyph():
    """All-vertical Hebrew contour rejected by the long-blue direction scan."""
    return mixed_contour_glyph(
        [
            [
                (100, 0, True),
                (100, 500, True),
                (100, 0, True),
                (100, 300, True),
            ],
        ]
    )


def hebrew_long_blue_scan_rejection_glyph():
    """Short Hebrew extremum whose next candidate has the opposite direction."""
    return mixed_contour_glyph(
        [
            [
                (90, 0, True),
                (100, 500, True),
                (120, 500, True),
                (-300, 480, True),
                (-300, 0, True),
            ],
        ]
    )


def hebrew_long_blue_offcurve_glyph():
    """Accepted long-blue replacement with off-curve points in its scan."""
    return mixed_contour_glyph(
        [
            [
                (110, 0, True),
                (100, 500, True),
                (120, 500, False),
                (-300, 480, True),
                (-300, 0, False),
            ],
        ]
    )


def hebrew_long_blue_inner_break_glyph():
    """Long-blue replacement with a short candidate and inner scan stop."""
    return mixed_contour_glyph(
        [
            [
                (110, 0, True),
                (100, 500, True),
                (110, 500, True),
                (120, 499, True),
                (90, 480, True),
                (-300, 478, True),
                (90, 0, True),
            ],
        ]
    )


def build_batch123_hebrew_long_blue_remaining() -> None:
    """Build valid Hebrew probes for the remaining long-blue scan branches."""
    top_blue_chars = (0x05D1, 0x05D3, 0x05D4, 0x05D7, 0x05DA, 0x05DB, 0x05DD, 0x05E1)
    contours = {
        "hebrew-long-blue-offcurve-next-rtl.ttf": [
            (110, 0, True),
            (100, 500, True),
            (120, 500, True),
            (-300, 480, False),
            (-300, 0, True),
        ],
        "hebrew-long-blue-offcurve-next-ltr.ttf": [
            (-110, 0, True),
            (-100, 500, True),
            (-120, 500, True),
            (300, 480, False),
            (300, 0, True),
        ],
        "hebrew-long-blue-short-same-rtl.ttf": [
            (110, 0, True),
            (100, 500, True),
            (120, 500, True),
            (100, 480, True),
            (80, 479, True),
            (-300, 478, True),
            (-300, 0, True),
        ],
        "hebrew-long-blue-short-same-ltr.ttf": [
            (-110, 0, True),
            (-100, 500, True),
            (-120, 500, True),
            (-100, 480, True),
            (-80, 479, True),
            (300, 478, True),
            (300, 0, True),
        ],
        "hebrew-long-blue-wrap-stop-rtl.ttf": [
            (110, 0, True),
            (100, 500, True),
            (120, 500, True),
            (100, 480, True),
            (-300, 480, True),
        ],
        "hebrew-long-blue-wrap-stop-ltr.ttf": [
            (-110, 0, True),
            (-100, 500, True),
            (-120, 500, True),
            (-100, 480, True),
            (300, 480, True),
        ],
    }
    for filename, contour in contours.items():
        glyph_order = [".notdef", "space", "hebrew_probe"]
        glyphs = {
            ".notdef": rectangle_glyph(80, -120, 520, 720),
            "space": empty_glyph(),
            "hebrew_probe": mixed_contour_glyph([contour]),
        }
        metrics = {
            ".notdef": (700, 80),
            "space": (300, 0),
            "hebrew_probe": (700, 100),
        }
        cmap = {0x20: "space", **{codepoint: "hebrew_probe" for codepoint in top_blue_chars}}

        font = FontBuilder(UNITS_PER_EM, isTTF=True)
        font.setupGlyphOrder(glyph_order)
        font.setupCharacterMap(cmap)
        font.setupGlyf(glyphs)
        font.setupHorizontalMetrics(metrics)
        font.setupHorizontalHeader(ascent=820, descent=-220)
        font.setupNameTable(
            {
                "familyName": "Autohint Hebrew Batch123",
                "styleName": "Regular",
                "uniqueFontIdentifier": f"Autohint Hebrew Batch123 {filename}",
                "fullName": f"Autohint Hebrew Batch123 {filename}",
                "psName": "AutohintHebrewBatch123-Regular",
                "version": "Version 1.0",
            }
        )
        font.setupOS2(
            sTypoAscender=820,
            sTypoDescender=-220,
            usWinAscent=820,
            usWinDescent=220,
        )
        font.setupPost()
        head = font.font["head"]
        head.created = 0
        head.modified = 0
        font.font.recalcTimestamp = False
        OUT_DIR.mkdir(parents=True, exist_ok=True)
        font.save(OUT_DIR / filename)


def build_batch190_hebrew_late_oncurve() -> None:
    """Build a valid Hebrew top-blue witness with a late on-curve point."""
    top_blue_chars = (0x05D1, 0x05D3, 0x05D4, 0x05D7, 0x05DA, 0x05DB, 0x05DD, 0x05E1)
    contour = [
        (110, 0, True),
        (100, 500, True),
        (120, 500, False),
        (-90, 490, False),
        (-180, 480, True),
        (-180, 0, True),
    ]
    glyph_order = [".notdef", "space", "hebrew_late_oncurve"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hebrew_late_oncurve": mixed_contour_glyph([contour]),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (300, 0),
        "hebrew_late_oncurve": (700, 100),
    }
    cmap = {0x20: "space", **{codepoint: "hebrew_late_oncurve" for codepoint in top_blue_chars}}

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Hebrew Late Oncurve",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Hebrew Late Oncurve Regular",
            "fullName": "Autohint Hebrew Late Oncurve Regular",
            "psName": "AutohintHebrewLateOncurve-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch190-hebrew-late-oncurve.ttf")


def build_batch191_hebrew_offcurve_apex() -> None:
    """Build a valid Hebrew top-blue witness with an off-curve apex."""
    top_blue_chars = (0x05D1, 0x05D3, 0x05D4, 0x05D7, 0x05DA, 0x05DB, 0x05DD, 0x05E1)
    contour = [
        (-60, 518, True),
        (0, 520, False),
        (60, 518, True),
        (100, 0, True),
        (-100, 0, True),
    ]
    glyph_order = [".notdef", "space", "hebrew_offcurve_apex"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hebrew_offcurve_apex": mixed_contour_glyph([contour]),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (300, 0),
        "hebrew_offcurve_apex": (700, 100),
    }
    cmap = {0x20: "space", **{codepoint: "hebrew_offcurve_apex" for codepoint in top_blue_chars}}

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Hebrew Offcurve Apex",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Hebrew Offcurve Apex Regular",
            "fullName": "Autohint Hebrew Offcurve Apex Regular",
            "psName": "AutohintHebrewOffcurveApex-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch191-hebrew-offcurve-apex.ttf")


def build_batch196_hebrew_near_top_span() -> None:
    """Build a Hebrew top-blue contour whose short walk spans nearly all points."""
    top_blue_chars = (0x05D1, 0x05D3, 0x05D4, 0x05D7, 0x05DA, 0x05DB, 0x05DD, 0x05E1)
    contour = [
        (0, 0, True),
        (-20, 500, True),
        (0, 520, True),
        (20, 500, True),
        (10, 518, True),
    ]
    glyph_order = [".notdef", "space", "hebrew_near_top_span"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hebrew_near_top_span": mixed_contour_glyph([contour]),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (300, 0),
        "hebrew_near_top_span": (700, 100),
    }
    cmap = {0x20: "space", **{codepoint: "hebrew_near_top_span" for codepoint in top_blue_chars}}

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Hebrew Near Top Span",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Hebrew Near Top Span Regular",
            "fullName": "Autohint Hebrew Near Top Span Regular",
            "psName": "AutohintHebrewNearTopSpan-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch196-hebrew-near-top-span.ttf")


def build_batch197_hebrew_near_top_span_mirror() -> None:
    """Build the horizontal mirror of the Batch196 merge witness."""
    top_blue_chars = (0x05D1, 0x05D3, 0x05D4, 0x05D7, 0x05DA, 0x05DB, 0x05DD, 0x05E1)
    contour = [
        (0, 0, True),
        (20, 500, True),
        (0, 520, True),
        (-20, 500, True),
        (-10, 518, True),
    ]
    glyph_order = [".notdef", "space", "hebrew_near_top_span_mirror"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hebrew_near_top_span_mirror": mixed_contour_glyph([contour]),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (300, 0),
        "hebrew_near_top_span_mirror": (700, 100),
    }
    cmap = {
        0x20: "space",
        **{codepoint: "hebrew_near_top_span_mirror" for codepoint in top_blue_chars},
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Hebrew Near Top Span Mirror",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Hebrew Near Top Span Mirror Regular",
            "fullName": "Autohint Hebrew Near Top Span Mirror Regular",
            "psName": "AutohintHebrewNearTopSpanMirror-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch197-hebrew-near-top-span-mirror.ttf")


def build_batch126_normal_scale_branches() -> None:
    """Build normal-scale Latin and Han probes for remaining hint branches."""
    glyph_order = [
        ".notdef",
        "space",
        "latin_standard",
        "hani_standard",
        "latin_top_measure",
        "latin_bottom_measure",
        "latin_below_top",
        "latin_mono_narrow",
        "latin_bound_near",
        "cjk_descending_link",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_standard": rectangle_glyph(100, 0, 500, 500),
        "hani_standard": rectangle_glyph(100, 0, 620, 560),
        # The internal on-curve point at (240, 620) is flanked by same-height
        # off-curve controls, while the surrounding on-curve points are lower.
        "latin_top_measure": mixed_contour_glyph(
            [
                (100, 0, 500, 500),
                [
                    (140, 500, True),
                    (190, 620, False),
                    (240, 620, True),
                    (310, 620, False),
                    (370, 500, True),
                    (430, 480, True),
                ],
            ]
        ),
        "latin_bottom_measure": mixed_contour_glyph(
            [
                [
                    (140, 0, True),
                    (190, -120, False),
                    (240, -120, True),
                    (310, -120, False),
                    (370, 0, True),
                    (430, 20, True),
                ],
                (100, 0, 500, 500),
            ]
        ),
        "latin_below_top": mixed_contour_glyph(
            [
                (100, 0, 500, 500),
                [
                    (150, 520, True),
                    (205, 570, False),
                    (260, 570, True),
                    (330, 570, False),
                    (390, 520, True),
                    (450, 505, True),
                ],
                (215, 650, 385, 710),
                (300, 535, 360, 565),
            ]
        ),
        # Three stems cross the narrow/medium/wide fitted-width regimes at
        # the five normal ppems without using extreme coordinates.
        "latin_mono_narrow": rectangles_glyph(
            [
                (80, 0, 120, 600),
                (200, 0, 264, 600),
                (340, 0, 460, 600),
            ]
        ),
        # Close reciprocal pairs make later edges land near the preceding
        # rounded edge while retaining six distinct valid contours.
        "latin_bound_near": rectangles_glyph(
            [
                (80, 0, 140, 600),
                (143, 0, 203, 600),
                (260, 0, 320, 600),
                (323, 0, 383, 600),
                (440, 0, 500, 600),
                (503, 0, 563, 600),
            ]
        ),
        # Emit horizontal bars from top to bottom; their repeated vertical
        # edges provide CJK descending linked segments.
        "cjk_descending_link": rectangles_glyph(
            [
                (80, 500, 620, 580),
                (80, 380, 620, 460),
                (80, 260, 620, 340),
                (80, 140, 620, 220),
            ]
        ),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (300, 0),
        "latin_standard": (700, 100),
        "hani_standard": (1000, 100),
        "latin_top_measure": (700, 100),
        "latin_bottom_measure": (700, 100),
        "latin_below_top": (700, 100),
        "latin_mono_narrow": (700, 80),
        "latin_bound_near": (700, 80),
        "cjk_descending_link": (1000, 80),
    }
    cmap = {
        0x20: "space",
        0x006F: "latin_standard",
        0x7530: "hani_standard",
        0x00F1: "latin_top_measure",
        0x1E1B: "latin_bottom_measure",
        0x1EAD: "latin_below_top",
        0x01D5: "latin_mono_narrow",
        0x01D7: "latin_bound_near",
        0x4ED6: "cjk_descending_link",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Normal Scale Branches",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Normal Scale Branches Regular",
            "fullName": "Autohint Normal Scale Branches Regular",
            "psName": "AutohintNormalScaleBranches-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()
    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch126-normal-scale-branches.ttf")


def build_batch127_cjk_edge_link_predicates() -> None:
    """Build valid CJK contours for edge-link predicate parity witnesses."""

    units_per_em = 1024

    def i_contour(base: int, reverse: bool = False) -> list[tuple[int, int, bool]]:
        # One I-shaped contour has three horizontal bands: an 80-unit bottom
        # serif, a 560-unit stem, and a 76-unit top serif.  Keep the ordinary
        # winding aligned with rectangles_glyph; selected probes reverse one
        # contour to exercise CJK direction and link cleanup independently.
        points = [
            (base, 0, True),
            (base + 160, 0, True),
            (base + 160, 80, True),
            (base + 120, 80, True),
            (base + 120, 640, True),
            (base + 160, 640, True),
            (base + 160, 716, True),
            (base, 716, True),
            (base, 640, True),
            (base + 40, 640, True),
            (base + 40, 80, True),
            (base, 80, True),
        ]
        if not reverse:
            points.reverse()
        return points

    def i_glyph(bases: tuple[int, int, int], reverse_index: int | None = None):
        return mixed_contour_glyph(
            [
                i_contour(base, index == reverse_index)
                for index, base in enumerate(bases)
            ]
        )

    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "serif_break_first",
        "serif_break_middle",
        "serif_break_third",
        "serif_spacing_skew",
        "long_short_competition",
        "duplicate_backlink_interp",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -160, 944, 840),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(160, 20, 864, 700),
        "serif_break_first": i_glyph((0, 320, 640), reverse_index=0),
        "serif_break_middle": i_glyph((0, 320, 640), reverse_index=1),
        "serif_break_third": i_glyph((0, 320, 640), reverse_index=2),
        "serif_spacing_skew": i_glyph((0, 320, 656)),
        "long_short_competition": rectangles_glyph(
            [
                (40, 20, 100, 620),
                (40, 260, 260, 380),
                (180, 20, 240, 620),
            ]
        ),
        "duplicate_backlink_interp": rectangles_glyph(
            [
                (40, 20, 100, 620),
                (40, 220, 300, 420),
                (180, 20, 240, 620),
                (180, 260, 260, 380),
            ]
        ),
    }
    metrics = {
        ".notdef": (1024, 80),
        "space": (320, 0),
        "hani_standard": (1024, 100),
        "serif_break_first": (1024, 64),
        "serif_break_middle": (1024, 64),
        "serif_break_third": (1024, 64),
        "serif_spacing_skew": (1024, 64),
        "long_short_competition": (1024, 64),
        "duplicate_backlink_interp": (1024, 64),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_standard",
        0x51B5: "serif_break_first",
        0x51B6: "serif_break_middle",
        0x51B7: "serif_break_third",
        0x51B8: "serif_spacing_skew",
        0x51B9: "long_short_competition",
        0x51BA: "duplicate_backlink_interp",
    }

    font = FontBuilder(units_per_em, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=840, descent=-240)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Edge Link Predicates",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Edge Link Predicates Regular",
            "fullName": "Autohint CJK Edge Link Predicates Regular",
            "psName": "AutohintCJKEdgeLinkPredicates-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=840,
        sTypoDescender=-240,
        usWinAscent=840,
        usWinDescent=240,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch127-cjk-edge-link-predicates.ttf")


def build_coverage_cjk_edge_order_links() -> None:
    """Build valid CJK contours for edge-order and link-reduction probes."""

    units_per_em = 1024
    order_names = [f"order_{index:02d}" for index in range(1, 6)]
    link_names = [f"link_{letter}" for letter in "abcde"]
    stem_bands = ((96, 320), (400, 624), (704, 928))
    order_y_values = (
        (640, 320, 0),
        (0, 640, 320),
        (320, 0, 640),
        (640, 0, 320),
        (320, 640, 0),
    )
    order_glyphs = {
        name: rectangles_glyph(
            [
                (left, bottom, right, bottom + 64)
                for (left, right), bottom in zip(stem_bands, y_values)
            ]
        )
        for name, y_values in zip(order_names, order_y_values)
    }

    link_rectangles = [
        [
            (40, 20, 100, 620),
            (180, 20, 240, 620),
            (40, 260, 260, 380),
        ],
        [
            (40, 20, 100, 620),
            (180, 20, 240, 620),
            (40, 220, 260, 300),
            (40, 340, 260, 420),
        ],
        [
            (40, 20, 100, 620),
            (180, 20, 240, 620),
            (320, 20, 380, 620),
            (40, 260, 260, 380),
            (180, 180, 400, 300),
        ],
        [
            (40, 20, 120, 620),
            (200, 20, 280, 620),
            (80, 240, 240, 400),
        ],
        [
            (40, 20, 100, 620),
            (180, 20, 240, 620),
            (320, 20, 380, 620),
            (40, 260, 260, 380),
            (180, 180, 400, 300),
        ],
    ]
    mirrored_link_rectangles = [
        (units_per_em - right, bottom, units_per_em - left, top)
        for left, bottom, right, top in reversed(link_rectangles[2])
    ]
    link_rectangles[-1] = mirrored_link_rectangles

    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        *order_names,
        *link_names,
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -160, 944, 840),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(96, 32, 928, 672),
        **{
            name: glyph
            for name, glyph in order_glyphs.items()
        },
        **{
            name: rectangles_glyph(rectangles)
            for name, rectangles in zip(link_names, link_rectangles)
        },
    }
    metrics = {
        ".notdef": (1024, 80),
        "space": (320, 0),
        "hani_standard": (1024, 128),
        **{name: (1024, 48) for name in order_names + link_names},
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_standard",
        **{
            0x4E10 + index: name
            for index, name in enumerate(order_names)
        },
        **{
            0x4E20 + index: name
            for index, name in enumerate(link_names)
        },
    }

    font = FontBuilder(units_per_em, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=840, descent=-240)
    font.setupNameTable(
        {
            "familyName": "Coverage CJK Edge Order Links",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Coverage CJK Edge Order Links Regular",
            "fullName": "Coverage CJK Edge Order Links Regular",
            "psName": "CoverageCJKEdgeOrderLinks-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=840,
        sTypoDescender=-240,
        usWinAscent=840,
        usWinDescent=240,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "coverage-cjk-edge-order-links.ttf")


def build_batch145_cjk_edge_interpolation_witnesses() -> None:
    """Build valid CJK contours for linked-edge interpolation witnesses."""

    units_per_em = 1024

    def contour(points: list[tuple[int, int]]) -> list[tuple[int, int, bool]]:
        return [(x, y, True) for x, y in points]

    backlink_contours = [
        [(80, 32), (80, 672), (176, 672), (176, 352), (288, 352), (288, 32)],
        [(96, 32), (96, 672), (200, 672), (200, 288), (336, 288), (336, 32)],
        [(64, 32), (64, 672), (152, 672), (152, 416), (272, 416), (272, 32)],
        [(112, 32), (112, 672), (224, 672), (224, 320), (368, 320), (368, 32)],
        [(48, 32), (48, 672), (160, 672), (160, 384), (304, 384), (304, 32)],
    ]
    equal_rectangles = [
        [(320, 352, 608, 672), (64, 32, 320, 304), (320, 32, 416, 304)],
        [(384, 336, 704, 656), (96, 48, 384, 288), (384, 48, 496, 288)],
        [(288, 320, 560, 640), (48, 16, 288, 272), (288, 16, 392, 272)],
        [(416, 368, 752, 704), (128, 40, 416, 320), (416, 40, 520, 320)],
        [(352, 344, 640, 680), (80, 24, 352, 296), (352, 24, 464, 296)],
    ]
    blue_contours = [
        [(64, 32), (320, 32), (320, 608), (960, 608), (960, 672), (64, 672)],
        [(80, 48), (384, 48), (384, 592), (944, 592), (944, 672), (80, 672)],
        [(48, 16), (288, 16), (288, 600), (928, 600), (928, 672), (48, 672)],
        [(96, 40), (416, 40), (416, 584), (912, 584), (912, 672), (96, 672)],
        [(72, 24), (352, 24), (352, 616), (952, 616), (952, 672), (72, 672)],
    ]

    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "backlink_a",
        "backlink_b",
        "backlink_c",
        "backlink_d",
        "backlink_e",
        "equal_a",
        "equal_b",
        "equal_c",
        "equal_d",
        "equal_e",
        "blue_a",
        "blue_b",
        "blue_c",
        "blue_d",
        "blue_e",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -160, 944, 840),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(128, 0, 896, 672),
        **{
            f"backlink_{letter}": mixed_contour_glyph([contour(points)])
            for letter, points in zip("abcde", backlink_contours)
        },
        **{
            f"equal_{letter}": rectangles_glyph(rectangles)
            for letter, rectangles in zip("abcde", equal_rectangles)
        },
        **{
            f"blue_{letter}": mixed_contour_glyph([contour(points)])
            for letter, points in zip("abcde", blue_contours)
        },
    }
    metrics = {
        ".notdef": (1024, 80),
        "space": (320, 0),
        "hani_standard": (1024, 128),
        **{name: (1024, 48) for name in glyph_order[3:]},
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_standard",
        0x519B: "hani_standard",
        **{
            0x51C0 + index: f"backlink_{letter}"
            for index, letter in enumerate("abcde")
        },
        **{
            0x51C5 + index: f"equal_{letter}"
            for index, letter in enumerate("abcde")
        },
        **{
            0x51CA + index: f"blue_{letter}"
            for index, letter in enumerate("abcde")
        },
    }

    font = FontBuilder(units_per_em, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=840, descent=-240)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Edge Interpolation Witnesses",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Edge Interpolation Witnesses Regular",
            "fullName": "Autohint CJK Edge Interpolation Witnesses Regular",
            "psName": "AutohintCJKEdgeInterpolationWitnesses-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=840,
        sTypoDescender=-240,
        usWinAscent=840,
        usWinDescent=240,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch145-cjk-edge-interpolation-witnesses.ttf")


def build_batch152_latin_adjustment_branches() -> None:
    """Build valid Latin glyphs with unique adjustment-database codepoints."""

    glyph_order = [
        ".notdef",
        "space",
        "capital_base",
        "small_base",
        "small_descender",
        "capital_bottom",
        "capital_top",
        "small_top",
        "small_bottom",
        "small_both",
        "capital_both",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -160, 560, 760),
        "space": empty_glyph(),
        # These base glyphs populate the standard Latin blue strings used by
        # af_latin_metrics_init_blues. The adjustment probes stay on unique
        # glyph indices so reverse_cmap_lookup cannot select an earlier
        # database codepoint that shares the same glyph.
        "capital_base": rectangle_glyph(100, 0, 500, 700),
        "small_base": ring_glyph(90, 0, 510, 520, 200, 120, 400, 400),
        "small_descender": ring_glyph(90, -180, 510, 520, 200, -80, 400, 320),
        "capital_bottom": ring_glyph(80, 0, 560, 700, 220, 160, 420, 540),
        "capital_top": rectangle_glyph(120, 0, 480, 700),
        "small_top": ring_glyph(90, 0, 510, 560, 200, 120, 400, 440),
        "small_bottom": ring_glyph(90, -140, 510, 520, 200, -20, 400, 360),
        "small_both": ring_glyph(90, -120, 510, 580, 200, 20, 400, 440),
        "capital_both": ring_glyph(80, 0, 560, 700, 220, 160, 420, 540),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (320, 0),
        "capital_base": (700, 100),
        "small_base": (620, 90),
        "small_descender": (620, 90),
        "capital_bottom": (700, 80),
        "capital_top": (700, 80),
        "small_top": (620, 90),
        "small_bottom": (620, 90),
        "small_both": (620, 90),
        "capital_both": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x48: "capital_base",
        0x45: "capital_base",
        0x5A: "capital_base",
        0x4C: "capital_base",
        0x4F: "capital_base",
        0x43: "capital_base",
        0x55: "capital_base",
        0x53: "capital_base",
        0x54: "capital_base",
        0x51: "capital_bottom",
        0x66: "small_base",
        0x6B: "small_base",
        0x64: "small_base",
        0x62: "small_base",
        0x68: "small_base",
        0x75: "small_base",
        0x76: "small_base",
        0x78: "small_base",
        0x7A: "small_base",
        0x6F: "small_base",
        0x65: "small_base",
        0x73: "small_base",
        0x63: "small_base",
        0x6E: "small_base",
        0x72: "small_base",
        0x70: "small_descender",
        0x71: "small_descender",
        0x67: "small_descender",
        0x6A: "small_descender",
        0x79: "small_descender",
        0x0187: "capital_top",  # Ƈ: AF_IGNORE_CAPITAL_TOP
        0x0188: "small_top",  # ƈ: AF_IGNORE_SMALL_TOP
        0x0105: "small_bottom",  # ą: AF_IGNORE_SMALL_BOTTOM
        0x00F8: "small_both",  # ø: AF_IGNORE_SMALL_TOP|BOTTOM
        0xA7C0: "capital_both",  # Ꟁ: AF_IGNORE_CAPITAL_TOP|BOTTOM
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Adjustment Branches",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Adjustment Branches Regular",
            "fullName": "Autohint Latin Adjustment Branches Regular",
            "psName": "AutohintLatinAdjustmentBranches-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch152-latin-adjustment-branches.ttf")


def build_batch153_latin_blue_empty_branches() -> None:
    """Build valid Latin faces whose blue-string outlines have no extrema."""

    glyph_order = [
        ".notdef",
        "space",
        "blue_single_a",
        "blue_single_b",
        "blue_single_c",
        "blue_single_d",
        "blue_single_e",
        "blue_single_f",
        "target_capital",
        "target_small",
        "target_descender",
        "target_tall",
        "target_multi",
        "target_flat",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -160, 560, 760),
        "space": empty_glyph(),
        # Each contour contains one point.  The raw glyph is valid and has
        # more than two points, but every contour is skipped by the pinned
        # Latin blue-string extremum scan.
        "blue_single_a": one_point_contour_glyph(
            [(100, 100), (240, 220), (380, 340)]
        ),
        "blue_single_b": one_point_contour_glyph(
            [(120, 140), (260, 260), (400, 380)]
        ),
        "blue_single_c": one_point_contour_glyph(
            [(140, 180), (280, 300), (420, 420)]
        ),
        "blue_single_d": one_point_contour_glyph(
            [(160, -120), (300, 40), (440, 200)]
        ),
        "blue_single_e": one_point_contour_glyph(
            [(180, -160), (320, 20), (460, 180)]
        ),
        "blue_single_f": one_point_contour_glyph(
            [(200, -200), (340, -40), (480, 120)]
        ),
        "target_capital": rectangle_glyph(100, 0, 500, 700),
        "target_small": ring_glyph(90, 0, 510, 520, 200, 120, 400, 400),
        "target_descender": ring_glyph(
            90, -180, 510, 520, 200, -80, 400, 320
        ),
        "target_tall": rectangle_glyph(120, -40, 480, 760),
        "target_multi": rectangles_glyph(
            [(100, 0, 180, 700), (420, 0, 500, 700), (180, 300, 420, 380)]
        ),
        "target_flat": rectangle_glyph(70, -80, 530, 680),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (320, 0),
        "blue_single_a": (620, 100),
        "blue_single_b": (620, 120),
        "blue_single_c": (620, 140),
        "blue_single_d": (620, 160),
        "blue_single_e": (620, 180),
        "blue_single_f": (620, 200),
        "target_capital": (700, 100),
        "target_small": (620, 90),
        "target_descender": (620, 90),
        "target_tall": (700, 120),
        "target_multi": (700, 100),
        "target_flat": (700, 70),
    }
    cmap = {
        0x20: "space",
        # The capital-top blue string resolves entirely to valid
        # single-point-contour outlines, while the capital-bottom and
        # lowercase strings retain ordinary support glyphs so the witness
        # isolates the no-extremum path without changing unrelated zones.
        0x48: "blue_single_a",
        0x45: "blue_single_b",
        0x5A: "blue_single_c",
        0x4F: "blue_single_d",
        0x43: "blue_single_e",
        0x51: "blue_single_f",
        0x53: "blue_single_a",
        0x54: "blue_single_b",
        0x4C: "target_capital",
        0x55: "target_capital",
        0x66: "target_small",
        0x69: "target_small",
        0x6A: "target_small",
        0x6B: "target_small",
        0x64: "target_small",
        0x62: "target_small",
        0x68: "target_small",
        0x75: "target_small",
        0x76: "target_small",
        0x78: "target_small",
        0x7A: "target_small",
        0x6F: "target_small",
        0x65: "target_small",
        0x73: "target_small",
        0x63: "target_small",
        0x6E: "target_small",
        0x72: "target_small",
        0x70: "target_descender",
        0x71: "target_descender",
        0x67: "target_descender",
        0x79: "target_descender",
        0x100: "target_capital",
        0x101: "target_small",
        0x102: "target_descender",
        0x103: "target_tall",
        0x104: "target_multi",
        0x105: "target_flat",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Blue Empty Branches",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Blue Empty Branches Regular",
            "fullName": "Autohint Latin Blue Empty Branches Regular",
            "psName": "AutohintLatinBlueEmptyBranches-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch153-latin-blue-empty-branches.ttf")


def build_batch159_latin_fallback_adjustment_branches() -> None:
    """Build valid Latin glyphs for public fallback and adjustment paths."""

    glyph_order = [
        ".notdef",
        "space",
        "fallback_base",
        "empty_target",
        "offcurve_extremum",
        "tilde_top2_two_contours",
        "bottom_tilde",
        "up2_three_contours",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -160, 560, 760),
        "space": empty_glyph(),
        # Do not map any of latn's standard-width candidates ('o', 'O', or
        # '0').  The public face initialization must therefore take the
        # constant-width fallback in metrics_init_widths.
        "fallback_base": rectangle_glyph(100, 0, 500, 600),
        "empty_target": empty_glyph(),
        # An off-curve top extremum preceded by an on-curve point exercises
        # the public blue-string walk's best_on_last fallback.
        "offcurve_extremum": mixed_contour_glyph(
            [
                [
                    (100, 0, True),
                    (100, 560, True),
                    (250, 650, False),
                    (400, 560, True),
                    (400, 0, True),
                ]
            ]
        ),
        # U+1E4C carries AF_ADJUST_UP2|AF_ADJUST_TILDE_TOP2.  Two contours
        # intentionally take find_second_highest_contour's valid public
        # fallback to contour zero.
        "tilde_top2_two_contours": mixed_contour_glyph(
            [
                (100, 0, 500, 500),
                [
                    (150, 560, True),
                    (250, 640, False),
                    (350, 560, True),
                ],
            ]
        ),
        # U+1E1B carries AF_ADJUST_DOWN|AF_ADJUST_TILDE_BOTTOM and gives the
        # bottom-tilde route a real overlapping accent contour.
        "bottom_tilde": mixed_contour_glyph(
            [
                (100, 120, 500, 620),
                [
                    (150, 40, True),
                    (250, -40, False),
                    (350, 40, True),
                ],
            ]
        ),
        # U+01D7 carries AF_ADJUST_UP2.  Three contours make the second-highest
        # adjustment and its separation scan reachable through FT_Load_Glyph.
        "up2_three_contours": mixed_contour_glyph(
            [
                (100, 0, 500, 500),
                (150, 540, 350, 620),
                (190, 680, 310, 740),
            ]
        ),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (320, 0),
        "fallback_base": (700, 100),
        "empty_target": (620, 80),
        "offcurve_extremum": (700, 100),
        "tilde_top2_two_contours": (700, 100),
        "bottom_tilde": (700, 100),
        "up2_three_contours": (700, 100),
    }
    cmap = {
        0x20: "space",
        # Latin blue-string witnesses deliberately omit 0x006F, 0x004F, and
        # 0x0030 so no standard-width candidate is present.
        0x0041: "fallback_base",
        0x0043: "fallback_base",
        0x0045: "fallback_base",
        0x0048: "offcurve_extremum",
        0x004C: "fallback_base",
        0x0055: "fallback_base",
        0x005A: "fallback_base",
        0x0063: "fallback_base",
        0x0065: "fallback_base",
        0x0068: "fallback_base",
        0x006E: "fallback_base",
        0x0072: "fallback_base",
        0x0073: "fallback_base",
        0x0075: "fallback_base",
        0x1E4C: "tilde_top2_two_contours",
        0x1E1B: "bottom_tilde",
        0x01D7: "up2_three_contours",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Fallback Adjustments",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Fallback Adjustments Regular",
            "fullName": "Autohint Latin Fallback Adjustments Regular",
            "psName": "AutohintLatinFallbackAdjustments-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch159-latin-fallback-adjustments.ttf")


def latin_vertical_cusp_glyph():
    return mixed_contour_glyph(
        [
            [
                (100, 400, True),
                (100, 500, True),
                (100, 0, False),
                (100, 490, True),
                (300, 490, True),
                (300, 400, True),
            ],
        ]
    )


def latin_segment_limit_glyph():
    """501 contours produce 1,002 segments in both hint dimensions."""
    rects = []
    for index in range(501):
        column = index % 25
        row = index // 25
        left = 10 + column * 38
        bottom = 10 + row * 38
        rects.append((left, bottom, left + 12, bottom + 12))
    return rectangles_glyph(rects)


def latin_blue_delta_round_glyph():
    return ring_glyph(90, 0, 510, 530, 190, 120, 410, 400)


def nonreciprocal_chain_glyph():
    # U+51A1: two major-direction segments share one opposite segment so CJK
    # link cleanup sees a non-reciprocal chain and assigns a serif fallback.
    pen = TTGlyphPen(None)
    pen.moveTo((20, 20))
    pen.lineTo((20, 220))
    pen.lineTo((60, 220))
    pen.lineTo((60, 460))
    pen.lineTo((80, 460))
    pen.lineTo((80, 20))
    pen.closePath()
    return pen.glyph()


def cjk_same_position_minor_edge_glyph():
    """Clockwise major and counter-clockwise minor edges at the same x."""
    pen = TTGlyphPen(None)

    # The larger clockwise contour fixes the glyph's TrueType orientation, so
    # its left edge is the horizontal axis' major-direction segment.
    pen.moveTo((40, 20))
    pen.lineTo((40, 220))
    pen.lineTo((320, 220))
    pen.lineTo((320, 20))
    pen.closePath()

    # Reverse the smaller contour.  Its closing left edge is minor-direction
    # and shares x=40 with the major edge above.
    pen.moveTo((40, 300))
    pen.lineTo((80, 300))
    pen.lineTo((80, 400))
    pen.lineTo((40, 400))
    pen.closePath()
    return pen.glyph()


def cjk_grouped_shorter_link_glyph():
    """Two grouped left segments whose second stem has the shorter link."""
    return rectangles_glyph(
        [
            (40, 20, 310, 220),
            (40, 280, 300, 480),
        ]
    )


def mixed_round_straight_edge_glyph():
    pen = TTGlyphPen(None)
    for bottom, right, top in ((20, 390, 180), (260, 410, 420)):
        pen.moveTo((100, bottom))
        pen.lineTo((100, top))
        pen.lineTo((right, top))
        pen.lineTo((right, bottom))
        pen.closePath()

    mid_x = 270
    mid_y = 260
    pen.moveTo((mid_x, 0))
    pen.qCurveTo((500, 0), (500, mid_y))
    pen.qCurveTo((500, 520), (mid_x, 520))
    pen.qCurveTo((40, 520), (40, mid_y))
    pen.qCurveTo((40, 0), (mid_x, 0))
    pen.closePath()
    pen.moveTo((250, 100))
    pen.qCurveTo((100, 100), (100, mid_y))
    pen.qCurveTo((100, 420), (250, 420))
    pen.qCurveTo((400, 420), (400, mid_y))
    pen.qCurveTo((400, 100), (250, 100))
    pen.closePath()
    return pen.glyph()


def ring_glyph(
    left: int,
    bottom: int,
    right: int,
    top: int,
    inset_left: int,
    inset_bottom: int,
    inset_right: int,
    inset_top: int,
):
    pen = TTGlyphPen(None)
    mid_x = (left + right) // 2
    mid_y = (bottom + top) // 2
    pen.moveTo((mid_x, bottom))
    pen.qCurveTo((right, bottom), (right, mid_y))
    pen.qCurveTo((right, top), (mid_x, top))
    pen.qCurveTo((left, top), (left, mid_y))
    pen.qCurveTo((left, bottom), (mid_x, bottom))
    pen.closePath()

    inset_mid_x = (inset_left + inset_right) // 2
    inset_mid_y = (inset_bottom + inset_top) // 2
    pen.moveTo((inset_mid_x, inset_bottom))
    pen.qCurveTo((inset_left, inset_bottom), (inset_left, inset_mid_y))
    pen.qCurveTo((inset_left, inset_top), (inset_mid_x, inset_top))
    pen.qCurveTo((inset_right, inset_top), (inset_right, inset_mid_y))
    pen.qCurveTo((inset_right, inset_bottom), (inset_mid_x, inset_bottom))
    pen.closePath()
    return pen.glyph()


def one_point_contour_glyph(points: list[tuple[int, int]]):
    glyph = Glyph()
    glyph.numberOfContours = len(points)
    glyph.coordinates = GlyphCoordinates(points)
    glyph.endPtsOfContours = list(range(len(points)))
    glyph.flags = bytearray([1] * len(points))
    program = Program()
    program.fromBytecode([])
    glyph.program = program
    glyph.xMin = min(x for x, _ in points)
    glyph.xMax = max(x for x, _ in points)
    glyph.yMin = min(y for _, y in points)
    glyph.yMax = max(y for _, y in points)
    return glyph


def low_upem_duplicate_point_glyph():
    """One contour whose first segment has zero length."""
    points = [(4, 0), (4, 0), (4, 48), (44, 48), (44, 0)]
    glyph = Glyph()
    glyph.numberOfContours = 1
    glyph.coordinates = GlyphCoordinates(points)
    glyph.endPtsOfContours = [len(points) - 1]
    glyph.flags = bytearray([1] * len(points))
    program = Program()
    program.fromBytecode([])
    glyph.program = program
    glyph.xMin = 4
    glyph.yMin = 0
    glyph.xMax = 44
    glyph.yMax = 48
    return glyph


def mixed_contour_glyph(contours: list[object]):
    coordinates: list[tuple[int, int]] = []
    end_pts: list[int] = []
    flags = bytearray()
    for contour in contours:
        if isinstance(contour, tuple):
            left, bottom, right, top = contour
            points = [
                (left, bottom, True),
                (left, top, True),
                (right, top, True),
                (right, bottom, True),
            ]
        else:
            points = contour
        for x, y, on_curve in points:
            coordinates.append((x, y))
            flags.append(1 if on_curve else 0)
        end_pts.append(len(coordinates) - 1)

    glyph = Glyph()
    glyph.numberOfContours = len(contours)
    glyph.coordinates = GlyphCoordinates(coordinates)
    glyph.endPtsOfContours = end_pts
    glyph.flags = flags
    program = Program()
    program.fromBytecode([])
    glyph.program = program
    glyph.xMin = min(x for x, _ in coordinates)
    glyph.xMax = max(x for x, _ in coordinates)
    glyph.yMin = min(y for _, y in coordinates)
    glyph.yMax = max(y for _, y in coordinates)
    return glyph


def table_offsets(path: Path) -> dict[str, tuple[int, int]]:
    data = path.read_bytes()
    num_tables = struct.unpack(">H", data[4:6])[0]
    tables: dict[str, tuple[int, int]] = {}
    for index in range(num_tables):
        entry = 12 + index * 16
        tag = data[entry : entry + 4].decode("ascii")
        offset = struct.unpack(">L", data[entry + 8 : entry + 12])[0]
        length = struct.unpack(">L", data[entry + 12 : entry + 16])[0]
        tables[tag] = (offset, length)
    return tables


def truncate_glyph_loca(path: Path, glyph_name: str, byte_len: int) -> None:
    font = TTFont(path, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    glyph_id = glyph_order.index(glyph_name)
    locations = list(font["loca"].locations)
    start = locations[glyph_id]
    locations[glyph_id + 1] = start + byte_len
    loca_format = font["head"].indexToLocFormat
    font.close()

    tables = table_offsets(path)
    loca_offset, _ = tables["loca"]
    data = bytearray(path.read_bytes())
    if loca_format == 0:
        if byte_len % 2 != 0:
            raise ValueError("short loca glyph lengths must be even")
        entry = loca_offset + (glyph_id + 1) * 2
        data[entry : entry + 2] = struct.pack(">H", locations[glyph_id + 1] // 2)
    else:
        entry = loca_offset + (glyph_id + 1) * 4
        data[entry : entry + 4] = struct.pack(">L", locations[glyph_id + 1])
    path.write_bytes(data)


def rewrite_loca_entry(path: Path, entry_index: int, offset: int) -> None:
    """Replace one raw loca entry while preserving the generated SFNT."""
    font = TTFont(path, recalcTimestamp=False)
    loca_format = font["head"].indexToLocFormat
    font.close()

    tables = table_offsets(path)
    loca_offset, _ = tables["loca"]
    data = bytearray(path.read_bytes())
    if loca_format == 0:
        if offset % 2 != 0 or offset // 2 > 0xFFFF:
            raise ValueError("short loca offsets must be representable as doubled uint16")
        entry = loca_offset + entry_index * 2
        data[entry : entry + 2] = struct.pack(">H", offset // 2)
    else:
        if offset > 0xFFFF_FFFF:
            raise ValueError("long loca offsets must be representable as uint32")
        entry = loca_offset + entry_index * 4
        data[entry : entry + 4] = struct.pack(">L", offset)
    path.write_bytes(data)


def convert_loca_to_long_format(path: Path) -> None:
    font = TTFont(path, recalcTimestamp=False)
    locations = list(font["loca"].locations)
    original_last = locations[-1]
    # FontTools chooses short format whenever all offsets fit.  Temporarily
    # force one long-format value so it serializes a genuine long `loca` table;
    # restore the real final offset after serialization below.
    locations[-1] = max(locations[-1], 0x20_000)
    font["loca"].locations = locations
    font["head"].indexToLocFormat = 1
    font.recalcTimestamp = False
    font.save(path)
    font.close()
    rewrite_loca_entry(path, len(locations) - 1, original_last)


def glyph_name(tag: str) -> str:
    return f"script_{tag}"


def build_script_coverage() -> None:
    glyph_order = [".notdef", "space"]
    glyph_order.extend(glyph_name(tag) for tag, _ in SCRIPT_PROBES)
    glyph_order.extend(name for name, _, _ in DIGIT_WIDTH_PROBES)
    # Append-only: public FT_Load_Glyph fixtures below this generator use
    # stable numeric glyph IDs.  New char-code probes must not shift them.
    glyph_order.append("latin_double_top")
    glyph_order.append("latin_tilde_top")
    glyph_order.append("latin_tilde_top2")
    glyph_order.append("latin_tilde_top_measure_zero")
    glyph_order.append("latin_tilde_top_flat")
    glyph_order.append("latin_tilde_top_flat_loop")
    glyph_order.append("latin_tilde_bottom")
    glyph_order.append("latin_tilde_bottom_measure_zero")
    glyph_order.append("latin_tilde_bottom_flat")
    glyph_order.append("latin_tilde_bottom_flat_loop")
    glyph_order.append("latin_bottom_tall_accent")
    glyph_order.append("latin_tilde_top2_topflag")
    glyph_order.append("latin_top_bottom_accent")
    glyph_order.append("latin_disjoint_top_accent")
    glyph_order.append("latin_serif_m_symmetry")
    glyph_order.append("latin_serif_overlap_break")
    glyph_order.append("latin_tilde_top2_centering")
    glyph_order.append("latin_vertical_cusp")
    glyph_order.append("latin_nonbase_tilde")
    glyph_order.append("latin_extreme_coordinate")
    glyph_order.append("latin_segment_limit")
    glyph_order.append("beng_serif_pointer_order")
    # Append-only: these Hebrew glyphs exercise the long-blue contour scan
    # without changing the numeric IDs used by existing fixture cases.
    glyph_order.append("hebrew_long_blue_replacement")
    glyph_order.append("hebrew_long_blue_degenerate")
    glyph_order.append("hebrew_long_blue_scan_rejection")
    glyph_order.append("hebrew_long_blue_offcurve")
    glyph_order.append("hebrew_long_blue_inner_break")

    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
    }
    cmap = {0x20: "space"}

    for index, (tag, codepoint) in enumerate(SCRIPT_PROBES):
        name = glyph_name(tag)
        width = 500 + (index % 5) * 20
        top = 480 + (index % 7) * 24
        left = 70 + (index % 3) * 10
        glyphs[name] = rectangle_glyph(left, 0, left + width, top)
        metrics[name] = (700, left)
        cmap[codepoint] = name
        standard = STANDARD_CHARS.get(tag)
        if standard is not None:
            cmap.setdefault(standard, name)
        for alias in SCRIPT_BLUE_ALIASES.get(tag, ()):
            cmap.setdefault(alias, name)

    for name, codepoint, advance in DIGIT_WIDTH_PROBES:
        glyphs[name] = rectangle_glyph(100, 0, 440, 560)
        metrics[name] = (advance, 100)
        cmap[codepoint] = name

    glyphs["latin_double_top"] = stacked_contour_glyph()
    metrics["latin_double_top"] = (700, 100)
    cmap[0x01D5] = "latin_double_top"
    glyphs["latin_tilde_top"] = top_tilde_glyph()
    metrics["latin_tilde_top"] = (700, 100)
    cmap[0x00F1] = "latin_tilde_top"
    glyphs["latin_tilde_top2"] = top_tilde_glyph(extra_top=True)
    metrics["latin_tilde_top2"] = (700, 100)
    cmap[0x1E4D] = "latin_tilde_top2"
    glyphs["latin_tilde_top_measure_zero"] = top_tilde_measure_zero_glyph()
    metrics["latin_tilde_top_measure_zero"] = (700, 100)
    cmap[0x00E3] = "latin_tilde_top_measure_zero"
    glyphs["latin_tilde_top_flat"] = top_tilde_flat_glyph()
    metrics["latin_tilde_top_flat"] = (700, 100)
    cmap[0x00D1] = "latin_tilde_top_flat"
    glyphs["latin_tilde_top_flat_loop"] = top_tilde_flat_loop_glyph()
    metrics["latin_tilde_top_flat_loop"] = (700, 100)
    cmap[0x00C3] = "latin_tilde_top_flat_loop"
    glyphs["latin_tilde_bottom"] = bottom_tilde_glyph()
    metrics["latin_tilde_bottom"] = (700, 100)
    cmap[0x1E1B] = "latin_tilde_bottom"
    glyphs["latin_tilde_bottom_measure_zero"] = bottom_tilde_measure_zero_glyph()
    metrics["latin_tilde_bottom_measure_zero"] = (700, 100)
    cmap[0x1E1A] = "latin_tilde_bottom_measure_zero"
    glyphs["latin_tilde_bottom_flat"] = bottom_tilde_flat_glyph()
    metrics["latin_tilde_bottom_flat"] = (700, 100)
    cmap[0x1E75] = "latin_tilde_bottom_flat"
    glyphs["latin_tilde_bottom_flat_loop"] = bottom_tilde_flat_loop_glyph()
    metrics["latin_tilde_bottom_flat_loop"] = (700, 100)
    cmap[0x1E74] = "latin_tilde_bottom_flat_loop"
    glyphs["latin_bottom_tall_accent"] = bottom_tall_accent_glyph()
    metrics["latin_bottom_tall_accent"] = (700, 100)
    cmap[0x0122] = "latin_bottom_tall_accent"
    glyphs["latin_tilde_top2_topflag"] = top_tilde_glyph(extra_top=True)
    metrics["latin_tilde_top2_topflag"] = (700, 100)
    cmap[0x1EAA] = "latin_tilde_top2_topflag"
    glyphs["latin_top_bottom_accent"] = top_and_bottom_accent_glyph()
    metrics["latin_top_bottom_accent"] = (700, 100)
    cmap[0x1EAD] = "latin_top_bottom_accent"
    glyphs["latin_disjoint_top_accent"] = disjoint_top_accent_glyph()
    metrics["latin_disjoint_top_accent"] = (700, 80)
    cmap[0x1E02] = "latin_disjoint_top_accent"
    glyphs["latin_serif_m_symmetry"] = serif_m_symmetry_glyph()
    metrics["latin_serif_m_symmetry"] = (700, 70)
    cmap[0x01D7] = "latin_serif_m_symmetry"
    glyphs["latin_serif_overlap_break"] = serif_overlap_break_glyph()
    metrics["latin_serif_overlap_break"] = (620, 70)
    cmap[0x0244] = "latin_serif_overlap_break"
    glyphs["beng_serif_pointer_order"] = serif_pointer_order_glyph()
    metrics["beng_serif_pointer_order"] = (620, 70)
    cmap[0x0988] = "beng_serif_pointer_order"
    glyphs["latin_tilde_top2_centering"] = top_tilde_centering_glyph()
    metrics["latin_tilde_top2_centering"] = (700, 100)
    cmap[0x1EB4] = "latin_tilde_top2_centering"
    glyphs["latin_vertical_cusp"] = latin_vertical_cusp_glyph()
    metrics["latin_vertical_cusp"] = (620, 100)
    cmap[0x0245] = "latin_vertical_cusp"
    glyphs["latin_nonbase_tilde"] = top_tilde_glyph()
    metrics["latin_nonbase_tilde"] = (0, 0)
    cmap[0x0303] = "latin_nonbase_tilde"
    glyphs["latin_extreme_coordinate"] = extreme_rectangle_glyph()
    metrics["latin_extreme_coordinate"] = (1000, -32768)
    cmap[0x0246] = "latin_extreme_coordinate"
    glyphs["latin_segment_limit"] = latin_segment_limit_glyph()
    metrics["latin_segment_limit"] = (1000, 10)
    cmap[0xE100] = "latin_segment_limit"
    glyphs["hebrew_long_blue_replacement"] = hebrew_long_blue_replacement_glyph()
    metrics["hebrew_long_blue_replacement"] = (700, 100)
    cmap[0x05D3] = "hebrew_long_blue_replacement"
    glyphs["hebrew_long_blue_degenerate"] = hebrew_long_blue_degenerate_glyph()
    metrics["hebrew_long_blue_degenerate"] = (700, 100)
    cmap[0x05D4] = "hebrew_long_blue_degenerate"
    glyphs["hebrew_long_blue_scan_rejection"] = hebrew_long_blue_scan_rejection_glyph()
    metrics["hebrew_long_blue_scan_rejection"] = (700, 100)
    cmap[0x05D7] = "hebrew_long_blue_scan_rejection"
    glyphs["hebrew_long_blue_offcurve"] = hebrew_long_blue_offcurve_glyph()
    metrics["hebrew_long_blue_offcurve"] = (700, 100)
    cmap[0x05DA] = "hebrew_long_blue_offcurve"
    glyphs["hebrew_long_blue_inner_break"] = hebrew_long_blue_inner_break_glyph()
    metrics["hebrew_long_blue_inner_break"] = (700, 100)
    cmap[0x05DB] = "hebrew_long_blue_inner_break"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Script Coverage",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Script Coverage Regular",
            "fullName": "Autohint Script Coverage Regular",
            "psName": "AutohintScriptCoverage-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "script-coverage.ttf")


def build_batch199_latin_vertical_cusp_merge() -> None:
    """Build a sibling face for the public Latin segment-merge witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch199-latin-vertical-cusp-merge.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_vertical_cusp") != 80:
        raise ValueError("script coverage glyph order no longer keeps Latin cusp at glyph 80")
    font["glyf"].glyphs["latin_vertical_cusp"] = mixed_contour_glyph(
        [
            [
                (100, 400, True),
                (100, 500, True),
                (100, 350, True),
                (100, 0, False),
                (100, 490, True),
                (300, 490, True),
                (300, 400, True),
            ],
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch200_latin_tilde_min_y() -> None:
    """Build a sibling face for the public top-tilde minimum witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch200-latin-tilde-min-y.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_top") != 64:
        raise ValueError("script coverage glyph order no longer keeps top tilde at glyph 64")
    font["glyf"].glyphs["latin_tilde_top"] = mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 620, True),
                (190, 540, False),
                (240, 540, True),
                (310, 540, False),
                (370, 620, True),
                (430, 560, True),
            ],
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch201_latin_tilde_prev_oncurve() -> None:
    """Build a sibling face for the public top-tilde control-flag witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch201-latin-tilde-prev-oncurve.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_top") != 64:
        raise ValueError("script coverage glyph order no longer keeps top tilde at glyph 64")
    font["glyf"].glyphs["latin_tilde_top"] = mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 620, True),
                (190, 580, True),
                (240, 580, True),
                (310, 580, False),
                (370, 620, True),
                (430, 540, True),
            ],
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch210_latin_tilde_next_oncurve() -> None:
    """Build a sibling face for the public top-tilde next-control witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch210-latin-tilde-next-oncurve.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_top") != 64:
        raise ValueError("script coverage glyph order no longer keeps top tilde at glyph 64")
    font["glyf"].glyphs["latin_tilde_top"] = mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 620, True),
                (190, 580, False),
                (240, 580, True),
                (310, 580, True),
                (370, 620, True),
                (430, 540, True),
            ],
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch211_latin_tilde_crossed_neighbors() -> None:
    """Build a sibling face for the public asymmetric top-tilde witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch211-latin-tilde-crossed-neighbors.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_top") != 64:
        raise ValueError("script coverage glyph order no longer keeps top tilde at glyph 64")
    font["glyf"].glyphs["latin_tilde_top"] = mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 540, True),
                (190, 580, False),
                (240, 580, True),
                (310, 580, False),
                (370, 620, True),
                (430, 560, True),
            ],
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch212_latin_thin_crossed_tilde() -> None:
    """Build a sibling face for the public thin crossed-tilde witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch212-latin-thin-crossed-tilde.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_top") != 64:
        raise ValueError("script coverage glyph order no longer keeps top tilde at glyph 64")
    font["glyf"].glyphs["latin_tilde_top"] = mixed_contour_glyph(
        [
            (100, 0, 500, 500),
            [
                (140, 540, True),
                (190, 545, False),
                (240, 545, True),
                (310, 545, False),
                (370, 550, True),
                (430, 543, True),
            ],
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch213_latin_bottom_tilde_prev_oncurve() -> None:
    """Build a sibling face for the public bottom-tilde predecessor witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch213-latin-bottom-tilde-prev-oncurve.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_bottom") != 69:
        raise ValueError("script coverage glyph order no longer keeps bottom tilde at glyph 69")
    font["glyf"].glyphs["latin_tilde_bottom"] = mixed_contour_glyph(
        [
            [
                (140, 0, True),
                (190, 40, True),
                (240, 40, True),
                (310, 40, False),
                (370, 80, True),
                (430, 0, True),
            ],
            (100, 120, 500, 620),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch214_latin_bottom_tilde_next_oncurve() -> None:
    """Build a sibling face for the public bottom-tilde successor witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch214-latin-bottom-tilde-next-oncurve.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_bottom") != 69:
        raise ValueError("script coverage glyph order no longer keeps bottom tilde at glyph 69")
    font["glyf"].glyphs["latin_tilde_bottom"] = mixed_contour_glyph(
        [
            [
                (140, 0, True),
                (190, 40, False),
                (240, 40, True),
                (310, 40, True),
                (370, 80, True),
                (430, 0, True),
            ],
            (100, 120, 500, 620),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch215_latin_bottom_tilde_crossed_neighbors() -> None:
    """Build a sibling face for the public bottom-tilde crossed-neighbor witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch215-latin-bottom-tilde-crossed-neighbors.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_bottom") != 69:
        raise ValueError("script coverage glyph order no longer keeps bottom tilde at glyph 69")
    font["glyf"].glyphs["latin_tilde_bottom"] = mixed_contour_glyph(
        [
            [
                (140, 0, True),
                (190, 40, False),
                (240, 40, True),
                (310, 40, False),
                (370, 80, True),
                (430, 20, True),
            ],
            (100, 120, 500, 620),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch216_latin_thin_bottom_crossed_tilde() -> None:
    """Build a sibling face for the public thin bottom-tilde threshold witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch216-latin-thin-bottom-crossed-tilde.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_bottom") != 69:
        raise ValueError("script coverage glyph order no longer keeps bottom tilde at glyph 69")
    font["glyf"].glyphs["latin_tilde_bottom"] = mixed_contour_glyph(
        [
            [
                (140, 0, True),
                (190, 5, False),
                (240, 5, True),
                (310, 5, False),
                (370, 10, True),
                (430, 3, True),
            ],
            (100, 120, 500, 620),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch202_latin_bottom_tilde_max_y() -> None:
    """Build a sibling face for the public bottom-tilde maximum witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch202-latin-bottom-tilde-max-y.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_tilde_bottom") != 69:
        raise ValueError("script coverage glyph order no longer keeps bottom tilde at glyph 69")
    font["glyf"].glyphs["latin_tilde_bottom"] = mixed_contour_glyph(
        [
            [
                (140, 0, True),
                (190, 80, False),
                (240, 80, True),
                (310, 80, False),
                (370, 0, True),
                (430, 40, True),
            ],
            (100, 120, 500, 620),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch203_latin_small_ignore_lowest_tie() -> None:
    """Build a sibling face for the public lowest-contour tie-break witness."""
    source = OUT_DIR / "latin-small-ignore.ttf"
    destination = OUT_DIR / "batch203-latin-small-ignore-lowest-tie.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_g_cedilla") != 7:
        raise ValueError("Latin small-ignore glyph order no longer keeps cedilla at glyph 7")
    font["glyf"].glyphs["latin_g_cedilla"] = mixed_contour_glyph(
        [
            (90, 0, 520, 560),
            (160, -100, 260, -20),
            (340, -100, 440, -60),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch217_latin_overlap_sentinel() -> None:
    """Build valid glyphs for the public overlap-helper sentinel branch."""
    source = OUT_DIR / "latin-small-ignore.ttf"
    destination = OUT_DIR / "batch217-latin-overlap-sentinel.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_k_cedilla_dimensionless") != 8:
        raise ValueError("Latin small-ignore glyph order no longer keeps the base face at glyph 8")
    if len(glyph_order) != 9:
        raise ValueError("Latin small-ignore base face unexpectedly changed glyph count")

    glyph_i = "latin_i_overlap_sentinel"
    glyph_u = "latin_u_diaeresis_overlap_sentinel"
    font.setGlyphOrder(glyph_order + [glyph_i, glyph_u])
    font["glyf"].glyphs[glyph_i] = mixed_contour_glyph(
        [
            [(120, 200, True), (480, 200, True)],
            (220, 560, 380, 640),
        ]
    )
    font["glyf"].glyphs[glyph_u] = mixed_contour_glyph(
        [
            [(120, 560, True), (200, 560, True)],
            (180, 600, 360, 680),
            (100, 500, 500, 720),
        ]
    )
    font["hmtx"].metrics[glyph_i] = (620, 90)
    font["hmtx"].metrics[glyph_u] = (620, 90)
    for cmap_table in font["cmap"].tables:
        if cmap_table.isUnicode():
            cmap_table.cmap[0x0069] = glyph_i
            cmap_table.cmap[0x01D5] = glyph_u
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch218_latin_bottom_distance_order() -> None:
    """Build a sibling face for the public ordered bottom-distance witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch218-latin-bottom-distance-order.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_bottom_tall_accent") != 73:
        raise ValueError("script coverage glyph order no longer keeps bottom accent at glyph 73")
    font["glyf"].glyphs["latin_bottom_tall_accent"] = mixed_contour_glyph(
        [
            (100, -60, 500, 500),
            (140, -20, 460, 420),
            (200, -80, 400, 0),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_batch204_latin_top_bottom_accent_overlap() -> None:
    """Build a sibling face for the public horizontal-overlap witness."""
    source = OUT_DIR / "script-coverage.ttf"
    destination = OUT_DIR / "batch204-latin-top-bottom-accent-overlap.ttf"
    font = TTFont(source, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    if glyph_order.index("latin_top_bottom_accent") != 75:
        raise ValueError("script coverage glyph order no longer keeps top-bottom accent at glyph 75")
    font["glyf"].glyphs["latin_top_bottom_accent"] = mixed_contour_glyph(
        [
            (190, -90, 410, -30),
            (100, 0, 500, 500),
            (300, 550, 700, 610),
        ]
    )
    font.recalcTimestamp = False
    font.save(destination)
    font.close()


def build_latin_x_height_rejection() -> None:
    """Build a Latin face whose x-height rescale reaches C's rejection path."""
    glyph_order = [
        ".notdef",
        "space",
        "capital",
        "small_f",
        "x_height",
        "descender",
        "probe",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(50, 0, 450, 700),
        "space": empty_glyph(),
        "capital": rectangle_glyph(80, 0, 520, 2000),
        "small_f": rectangle_glyph(90, 0, 500, 700),
        "x_height": rectangle_glyph(100, 0, 480, 100),
        "descender": rectangle_glyph(100, -200, 480, 100),
        "probe": rectangle_glyph(70, 0, 530, 800),
    }
    metrics = {name: (600, 70) for name in glyph_order}
    metrics["space"] = (300, 0)

    cmap = {0x20: "space", 0x01D8: "probe"}
    for codepoint in (0x54, 0x48, 0x45, 0x5A, 0x4F, 0x43, 0x51, 0x53, 0x4C, 0x55):
        cmap[codepoint] = "capital"
    for codepoint in (0x66, 0x69, 0x6A, 0x6B, 0x64, 0x62, 0x68):
        cmap[codepoint] = "small_f"
    for codepoint in (0x75, 0x76, 0x78, 0x7A, 0x6F, 0x65, 0x73, 0x63, 0x6E, 0x72):
        cmap[codepoint] = "x_height"
    for codepoint in (0x70, 0x71, 0x67, 0x79):
        cmap[codepoint] = "descender"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=2000, descent=-200)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin X Height Rejection",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin X Height Rejection Regular",
            "fullName": "Autohint Latin X Height Rejection Regular",
            "psName": "AutohintLatinXHeightRejection-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=2000,
        sTypoDescender=-200,
        usWinAscent=2000,
        usWinDescent=200,
    )
    font.setupPost()
    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-x-height-rejection.ttf")


def build_khmer_sub_top_overlap() -> None:
    """Build overlapping primary and sub-top Khmer blue zones."""
    glyph_order = [
        ".notdef",
        "space",
        "standard",
        "top_flat",
        "top_round",
        "sub_top",
        "bottom",
        "probe",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(60, 0, 500, 500),
        "space": empty_glyph(),
        "standard": rectangle_glyph(80, 0, 500, 500),
        "top_flat": rectangle_glyph(80, 0, 500, 490),
        "top_round": ring_glyph(80, 0, 500, 520, 160, 80, 420, 440),
        "sub_top": rectangle_glyph(100, 0, 480, 520),
        "bottom": rectangle_glyph(90, 0, 490, 450),
        "probe": rectangle_glyph(70, 0, 530, 530),
    }
    metrics = {name: (600, 70) for name in glyph_order}
    metrics["space"] = (300, 0)

    cmap = {0x20: "space", 0x17E0: "standard", 0x1782: "probe"}
    for codepoint in (0x1781, 0x1791, 0x1793):
        cmap[codepoint] = "top_flat"
    for codepoint in (0x17A7, 0x17A9, 0x17B6):
        cmap[codepoint] = "top_round"
    cmap[0x1780] = "sub_top"
    for codepoint in (0x1783, 0x1785, 0x178B, 0x1794, 0x1798, 0x1799, 0x17B2):
        cmap.setdefault(codepoint, "bottom")
    for codepoint in (0x178F, 0x179A, 0x17A2, 0x1784, 0x179B):
        cmap.setdefault(codepoint, "bottom")

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=600, descent=-100)
    font.setupNameTable(
        {
            "familyName": "Autohint Khmer Sub Top Overlap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Khmer Sub Top Overlap Regular",
            "fullName": "Autohint Khmer Sub Top Overlap Regular",
            "psName": "AutohintKhmerSubTopOverlap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=600,
        sTypoDescender=-100,
        usWinAscent=600,
        usWinDescent=100,
    )
    font.setupPost()
    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "khmer-sub-top-overlap.ttf")


def build_batch194_khmer_sub_top_lowered() -> None:
    """Build a valid Khmer face with a lowered, non-overlapping sub-top."""
    glyph_order = [
        ".notdef",
        "space",
        "standard",
        "top_flat",
        "top_round",
        "sub_top",
        "bottom",
        "probe",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(60, 0, 500, 500),
        "space": empty_glyph(),
        "standard": rectangle_glyph(80, 0, 500, 500),
        "top_flat": rectangle_glyph(80, 0, 500, 490),
        "top_round": ring_glyph(80, 0, 500, 520, 160, 80, 420, 440),
        "sub_top": rectangle_glyph(100, 0, 480, 250),
        "bottom": rectangle_glyph(90, 0, 490, 450),
        "probe": rectangle_glyph(70, 0, 530, 530),
    }
    metrics = {name: (600, 70) for name in glyph_order}
    metrics["space"] = (300, 0)

    cmap = {0x20: "space", 0x17E0: "standard", 0x1782: "probe"}
    for codepoint in (0x1781, 0x1791, 0x1793):
        cmap[codepoint] = "top_flat"
    for codepoint in (0x17A7, 0x17A9, 0x17B6):
        cmap[codepoint] = "top_round"
    cmap[0x1780] = "sub_top"
    for codepoint in (0x1783, 0x1785, 0x178B, 0x1794, 0x1798, 0x1799, 0x17B2):
        cmap.setdefault(codepoint, "bottom")
    for codepoint in (0x178F, 0x179A, 0x17A2, 0x1784, 0x179B):
        cmap.setdefault(codepoint, "bottom")

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=600, descent=-100)
    font.setupNameTable(
        {
            "familyName": "Autohint Khmer Sub Top Lowered",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Khmer Sub Top Lowered Regular",
            "fullName": "Autohint Khmer Sub Top Lowered Regular",
            "psName": "AutohintKhmerSubTopLowered-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=600,
        sTypoDescender=-100,
        usWinAscent=600,
        usWinDescent=100,
    )
    font.setupPost()
    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "batch194-khmer-sub-top-lowered.ttf")


def build_arabic_standard_fallback() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "arabic_target",
        "arabic_standard_ha",
        "arabic_join_sample",
        "arabic_neutral_stem",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "arabic_target": rectangle_glyph(100, 0, 180, 680),
        "arabic_standard_ha": ring_glyph(80, 0, 520, 500, 200, 120, 400, 380),
        # afblue.dat defines U+0640 TATWEEL as the Arabic neutral zone.
        # Its lower flat establishes y=300; the target below links that
        # neutral edge to the normal Arabic bottom zone at y=0, exercising
        # FreeType's linked-blue neutral dedup in aflatin.c:4276-4290.
        "arabic_join_sample": rectangle_glyph(80, 300, 520, 340),
        "arabic_neutral_stem": rectangle_glyph(100, 0, 500, 300),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "arabic_target": (420, 100),
        "arabic_standard_ha": (620, 80),
        "arabic_join_sample": (620, 80),
        "arabic_neutral_stem": (620, 100),
    }
    cmap = {
        0x20: "space",
        # U+0644 LAM, the first pinned standard character, is intentionally
        # absent.  U+062D HAH is the next candidate in afscript.h.
        0x062D: "arabic_standard_ha",
        0x0628: "arabic_neutral_stem",
        0x0640: "arabic_join_sample",
    }
    for codepoint in (0x0627, 0x0625, 0x0643, 0x0637, 0x0638, 0x062A, 0x062B):
        cmap[codepoint] = "arabic_target"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Arabic Standard Fallback",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Arabic Standard Fallback Regular",
            "fullName": "Autohint Arabic Standard Fallback Regular",
            "psName": "AutohintArabicStandardFallback-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "arabic-standard-fallback.ttf")


def build_arabic_neutral_first() -> None:
    """Put a neutral blue below a linked normal top blue."""
    glyph_order = [
        ".notdef",
        "space",
        "arabic_standard_ha",
        "arabic_bounds_sample",
        "arabic_join_sample",
        "arabic_neutral_first_stem",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -220, 520, 720),
        "space": empty_glyph(),
        "arabic_standard_ha": ring_glyph(80, 0, 520, 500, 200, 120, 400, 380),
        # Arabic top and bottom strings overlap.  One shared rectangle keeps
        # their normal zones at y=300 and y=-200 while U+0640 independently
        # establishes the neutral zone at y=0.
        "arabic_bounds_sample": rectangle_glyph(80, -200, 520, 300),
        "arabic_join_sample": rectangle_glyph(80, 0, 520, 40),
        "arabic_neutral_first_stem": rectangle_glyph(100, 0, 500, 300),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "arabic_standard_ha": (620, 80),
        "arabic_bounds_sample": (620, 80),
        "arabic_join_sample": (620, 80),
        "arabic_neutral_first_stem": (620, 100),
    }
    cmap = {
        0x20: "space",
        0x062D: "arabic_standard_ha",
        0x0628: "arabic_neutral_first_stem",
        0x0640: "arabic_join_sample",
    }
    for codepoint in (0x0627, 0x0625, 0x0644, 0x0643, 0x0637, 0x0638, 0x062A, 0x062B):
        cmap[codepoint] = "arabic_bounds_sample"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Arabic Neutral First",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Arabic Neutral First Regular",
            "fullName": "Autohint Arabic Neutral First Regular",
            "psName": "AutohintArabicNeutralFirst-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "arabic-neutral-first.ttf")


def build_arabic_neutral_round_skip() -> None:
    """Give the Arabic neutral blue string a round-only extremum."""
    glyph_order = [
        ".notdef",
        "space",
        "arabic_standard_ha",
        "arabic_bounds_sample",
        "arabic_join_round",
        "arabic_target",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -220, 520, 720),
        "space": empty_glyph(),
        "arabic_standard_ha": ring_glyph(80, 0, 520, 500, 200, 120, 400, 380),
        "arabic_bounds_sample": rectangle_glyph(80, -200, 520, 600),
        # U+0640 is the sole pinned Arabic neutral-blue character.  Curved
        # extrema make FreeType discard this zone instead of treating it as
        # a flat attachment line.
        "arabic_join_round": ring_glyph(80, 0, 520, 160, 180, 40, 420, 120),
        "arabic_target": rectangle_glyph(100, 0, 500, 300),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "arabic_standard_ha": (620, 80),
        "arabic_bounds_sample": (620, 80),
        "arabic_join_round": (620, 80),
        "arabic_target": (620, 100),
    }
    cmap = {
        0x20: "space",
        0x062D: "arabic_standard_ha",
        0x0628: "arabic_target",
        0x0640: "arabic_join_round",
    }
    for codepoint in (0x0627, 0x0625, 0x0644, 0x0643, 0x0637, 0x0638, 0x062A, 0x062B):
        cmap[codepoint] = "arabic_bounds_sample"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Arabic Neutral Round Skip",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Arabic Neutral Round Skip Regular",
            "fullName": "Autohint Arabic Neutral Round Skip Regular",
            "psName": "AutohintArabicNeutralRoundSkip-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "arabic-neutral-round-skip.ttf")


def build_cjk_empty_standard() -> None:
    glyph_order = [".notdef", "space", "hani_empty"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_empty": empty_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_empty": (700, 0),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_empty",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Empty Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Empty Standard Regular",
            "fullName": "Autohint CJK Empty Standard Regular",
            "psName": "AutohintCJKEmptyStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-empty-standard.ttf")




def build_latin_small_ignore() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_o",
        "latin_x",
        "latin_c",
        "latin_oslash",
        "latin_small_top",
        "latin_g_cedilla",
        "latin_k_cedilla_dimensionless",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o": ring_glyph(90, 0, 510, 520, 190, 120, 410, 400),
        "latin_x": rectangles_glyph(
            [
                (120, 0, 240, 520),
                (360, 0, 480, 520),
            ]
        ),
        "latin_c": rectangles_glyph(
            [
                (90, 0, 210, 520),
                (210, 0, 520, 90),
                (210, 430, 520, 520),
            ]
        ),
        # Keep U+00F8 on a unique glyph index so the adjustment database lookup
        # reaches AF_IGNORE_SMALL_TOP | AF_IGNORE_SMALL_BOTTOM for this row.
        "latin_oslash": ring_glyph(90, -40, 510, 560, 190, 100, 410, 420),
        "latin_small_top": ring_glyph(90, 0, 510, 560, 190, 120, 410, 430),
        "latin_g_cedilla": rectangles_glyph(
            [
                (90, 0, 520, 560),
                (220, -70, 360, -20),
            ]
        ),
        # U+0136 selects pinned FreeType's AF_ADJUST_DOWN route.  The third
        # contour is dimensionless so the bottom separation scan observes C's
        # FT_LONG_MAX/FT_LONG_MIN sentinel contour while the real lowest
        # cedilla contour remains the selected adjustment target.
        "latin_k_cedilla_dimensionless": mixed_contour_glyph(
            [
                (90, 0, 520, 560),
                (220, -70, 360, -20),
                [(300, 120, True), (340, 120, True)],
            ]
        ),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o": (620, 90),
        "latin_x": (620, 120),
        "latin_c": (620, 90),
        "latin_oslash": (620, 90),
        "latin_small_top": (620, 90),
        "latin_g_cedilla": (620, 90),
        "latin_k_cedilla_dimensionless": (620, 90),
    }
    cmap = {
        0x20: "space",
        0x0063: "latin_c",
        0x006F: "latin_o",
        0x0078: "latin_x",
        0x00F8: "latin_oslash",
        0x0188: "latin_small_top",
        0x0122: "latin_g_cedilla",
        0x0136: "latin_k_cedilla_dimensionless",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Small Ignore",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Small Ignore Regular",
            "fullName": "Autohint Latin Small Ignore Regular",
            "psName": "AutohintLatinSmallIgnore-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-small-ignore.ttf")


def build_latin_remaining_topology() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_capital_blue",
        "latin_wide_segment_filter",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_capital_blue": rectangle_glyph(100, 0, 500, 520),
        "latin_wide_segment_filter": latin_wide_segment_filter_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_capital_blue": (620, 100),
        "latin_wide_segment_filter": (620, 100),
    }
    cmap = {
        0x20: "space",
        0x0043: "latin_capital_blue",
        0x0045: "latin_capital_blue",
        0x0048: "latin_capital_blue",
        0x004C: "latin_capital_blue",
        0x004F: "latin_capital_blue",
        0x0051: "latin_capital_blue",
        0x0053: "latin_capital_blue",
        0x0054: "latin_capital_blue",
        0x0055: "latin_capital_blue",
        0x005A: "latin_capital_blue",
        0x0243: "latin_wide_segment_filter",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Remaining Topology",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Remaining Topology Regular",
            "fullName": "Autohint Latin Remaining Topology Regular",
            "psName": "AutohintLatinRemainingTopology-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-remaining-topology.ttf")


def build_latin_width_clusters() -> None:
    glyph_order = [".notdef", "space", "latin_o_width_clusters"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o_width_clusters": rectangles_glyph(
            [
                (60, 0, 100, 520),
                (180, 0, 260, 520),
                (340, 0, 470, 520),
            ]
        ),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o_width_clusters": (620, 60),
    }
    cmap = {
        0x20: "space",
        0x006F: "latin_o_width_clusters",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Width Clusters",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Width Clusters Regular",
            "fullName": "Autohint Latin Width Clusters Regular",
            "psName": "AutohintLatinWidthClusters-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-width-clusters.ttf")


def build_latin_many_widths() -> None:
    stems = []
    x = 40
    for i in range(18):
        width = 24 + i * 4
        stems.append((x, 0, x + width, 520))
        x += width + 34

    glyph_order = [".notdef", "space", "latin_o_many_widths"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o_many_widths": rectangles_glyph(stems),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o_many_widths": (x + 40, 40),
    }
    cmap = {
        0x20: "space",
        0x006F: "latin_o_many_widths",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Many Widths",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Many Widths Regular",
            "fullName": "Autohint Latin Many Widths Regular",
            "psName": "AutohintLatinManyWidths-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-many-widths.ttf")


def build_cjk_blue_edge_cases() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "blue_empty",
        "blue_two_points",
        "blue_degenerate",
        "top_flat",
        "bottom_fill",
        "bottom_flat",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 620, 560),
        "blue_empty": empty_glyph(),
        "blue_two_points": one_point_contour_glyph([(210, 40), (330, 180)]),
        "blue_degenerate": one_point_contour_glyph([(160, 40), (260, 120), (360, 200)]),
        "top_flat": rectangle_glyph(110, 20, 580, 220),
        "bottom_fill": rectangle_glyph(120, 0, 560, 360),
        "bottom_flat": rectangle_glyph(120, -80, 560, 360),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "blue_empty": (700, 0),
        "blue_two_points": (700, 210),
        "blue_degenerate": (700, 160),
        "top_flat": (700, 110),
        "bottom_fill": (700, 120),
        "bottom_flat": (700, 120),
    }
    cmap = {
        0x20: "space",
        0x4E2A: "bottom_fill",
        0x4E3B: "bottom_flat",
        0x4EBA: "blue_two_points",
        0x4ED6: "blue_empty",
        0x4EEC: "blue_degenerate",
        0x519B: "top_flat",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Blue Edge Cases",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Blue Edge Cases Regular",
            "fullName": "Autohint CJK Blue Edge Cases Regular",
            "psName": "AutohintCJKBlueEdgeCases-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-blue-edge-cases.ttf")


def build_latin_blue_edge_cases() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_o",
        "latin_A",
        "blue_empty",
        "blue_degenerate",
        "blue_flat_loop",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o": ring_glyph(90, 0, 510, 520, 190, 120, 410, 400),
        "latin_A": rectangles_glyph(
            [(100, 0, 180, 680), (420, 0, 500, 680), (180, 300, 420, 380)]
        ),
        "blue_empty": empty_glyph(),
        "blue_degenerate": one_point_contour_glyph([(180, 640)]),
        "blue_flat_loop": horizontal_flat_loop_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o": (620, 90),
        "latin_A": (700, 100),
        "blue_empty": (600, 0),
        "blue_degenerate": (600, 180),
        "blue_flat_loop": (620, 100),
    }
    cmap = {
        0x20: "space",
        0x41: "latin_A",
        0x6F: "latin_o",
        0x54: "blue_empty",
        0x48: "blue_degenerate",
        0x45: "blue_flat_loop",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Blue Edge Cases",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Blue Edge Cases Regular",
            "fullName": "Autohint Latin Blue Edge Cases Regular",
            "psName": "AutohintLatinBlueEdgeCases-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-blue-edge-cases.ttf")


def build_cjk_malformed_blue() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "bottom_fill_malformed",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 620, 560),
        "bottom_fill_malformed": rectangle_glyph(120, 0, 560, 360),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "bottom_fill_malformed": (700, 120),
    }
    cmap = {
        0x20: "space",
        0x4E2A: "bottom_fill_malformed",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Malformed Blue",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Malformed Blue Regular",
            "fullName": "Autohint CJK Malformed Blue Regular",
            "psName": "AutohintCJKMalformedBlue-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    path = OUT_DIR / "cjk-malformed-blue.ttf"
    font.save(path)
    truncate_glyph_loca(path, "bottom_fill_malformed", 2)


def build_cjk_tiny_stem() -> None:
    glyph_order = [".notdef", "space", "hani_tiny_stem"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_tiny_stem": rectangle_glyph(100, 0, 120, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_tiny_stem": (700, 100),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_tiny_stem",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Tiny Stem",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Tiny Stem Regular",
            "fullName": "Autohint CJK Tiny Stem Regular",
            "psName": "AutohintCJKTinyStem-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-tiny-stem.ttf")


def build_cjk_snap_below_standard() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_snap_below",
        "hani_snap_far_below",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_snap_below": rectangle_glyph(100, 0, 190, 560),
        "hani_snap_far_below": rectangle_glyph(100, 0, 140, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_snap_below": (700, 100),
        "hani_snap_far_below": (700, 100),
    }
    cmap = {
        0x20: "space",
        0x4E1E: "hani_snap_far_below",
        0x4ED6: "hani_snap_below",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Snap Below Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Snap Below Standard Regular",
            "fullName": "Autohint CJK Snap Below Standard Regular",
            "psName": "AutohintCJKSnapBelowStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-snap-below-standard.ttf")


def build_cjk_multi_width_snap() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_snap_width"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangles_glyph(
            [
                (80, 0, 140, 560),
                (260, 0, 390, 560),
            ]
        ),
        "hani_snap_width": rectangle_glyph(80, 0, 140, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 80),
        "hani_snap_width": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x4ED6: "hani_snap_width",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Multi Width Snap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Multi Width Snap Regular",
            "fullName": "Autohint CJK Multi Width Snap Regular",
            "psName": "AutohintCJKMultiWidthSnap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-multi-width-snap.ttf")


def build_cjk_quantized_widths() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_probe"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        # Keep the input order non-monotonic so the public route covers both
        # FreeType's insertion sort and its unusual cluster-boundary handling.
        "hani_standard": rectangles_glyph(
            [
                (40, 0, 70, 560),
                (120, 0, 128, 560),
                (180, 0, 199, 560),
            ]
        ),
        "hani_probe": rectangle_glyph(80, 0, 90, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 40),
        "hani_probe": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x4ED6: "hani_probe",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Quantized Widths",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Quantized Widths Regular",
            "fullName": "Autohint CJK Quantized Widths Regular",
            "psName": "AutohintCJKQuantizedWidths-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-quantized-widths.ttf")


def build_cjk_many_widths() -> None:
    glyph_order = [".notdef", "space", "hani_many_widths"]
    rects: list[tuple[int, int, int, int]] = []
    x = 40
    for width in range(8, 29):
        rects.append((x, 0, x + width, 560))
        x += width + 8

    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_many_widths": rectangles_glyph(rects),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_many_widths": (900, 40),
    }
    cmap = {
        0x20: "space",
        0x7530: "hani_many_widths",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Many Widths",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Many Widths Regular",
            "fullName": "Autohint CJK Many Widths Regular",
            "psName": "AutohintCJKManyWidths-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-many-widths.ttf")


def build_cjk_wide_stem_snap() -> None:
    glyph_order = [".notdef", "space", "hani_standard", "hani_wide_stem"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_wide_stem": rectangle_glyph(80, 0, 250, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_wide_stem": (700, 80),
    }
    cmap = {
        0x20: "space",
        0x4ED6: "hani_wide_stem",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Wide Stem Snap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Wide Stem Snap Regular",
            "fullName": "Autohint CJK Wide Stem Snap Regular",
            "psName": "AutohintCJKWideStemSnap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-wide-stem-snap.ttf")


def build_cjk_round_stem_light() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_round_ring",
        "hani_mixed_round_straight",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_round_ring": ring_glyph(80, 20, 520, 460, 180, 120, 420, 360),
        "hani_mixed_round_straight": mixed_round_straight_edge_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_round_ring": (700, 80),
        "hani_mixed_round_straight": (700, 40),
    }
    cmap = {
        0x20: "space",
        0x51A2: "hani_round_ring",
        0x51A3: "hani_mixed_round_straight",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Round Stem Light",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Round Stem Light Regular",
            "fullName": "Autohint CJK Round Stem Light Regular",
            "psName": "AutohintCJKRoundStemLight-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-round-stem-light.ttf")


def build_cjk_duplicate_edge() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_duplicate_edge",
        "hani_nonreciprocal_chain",
        "hani_leading_skip",
        "hani_serif_conflict",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_duplicate_edge": rectangles_glyph(
            [
                (40, 20, 80, 220),
                (40, 260, 320, 460),
            ]
        ),
        "hani_nonreciprocal_chain": nonreciprocal_chain_glyph(),
        "hani_leading_skip": rectangles_glyph(
            [
                (20, 20, 30, 22),
                (80, 20, 130, 460),
            ]
        ),
        "hani_serif_conflict": rectangles_glyph(
            [
                (80, 20, 130, 460),
                (190, 20, 230, 460),
                (60, 20, 130, 55),
            ]
        ),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_duplicate_edge": (700, 40),
        "hani_nonreciprocal_chain": (700, 20),
        "hani_leading_skip": (700, 20),
        "hani_serif_conflict": (700, 60),
    }
    cmap = {
        0x20: "space",
        0x519E: "hani_duplicate_edge",
        0x51A0: "hani_serif_conflict",
        0x51A1: "hani_nonreciprocal_chain",
        0x51A4: "hani_leading_skip",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Duplicate Edge",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Duplicate Edge Regular",
            "fullName": "Autohint CJK Duplicate Edge Regular",
            "psName": "AutohintCJKDuplicateEdge-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-duplicate-edge.ttf")


def build_cjk_remaining_branches() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "hani_standard",
        "hani_minor_same_position",
        "hani_grouped_shorter_link",
        "hani_symmetric_stems",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "hani_standard": rectangle_glyph(100, 0, 200, 560),
        "hani_minor_same_position": cjk_same_position_minor_edge_glyph(),
        "hani_grouped_shorter_link": cjk_grouped_shorter_link_glyph(),
        # Three equal, evenly spaced rectangles form the six vertical edges
        # recognized by `af_cjk_hint_edges` as a symmetric sans-serif m.
        "hani_symmetric_stems": rectangles_glyph(
            [
                (80, 20, 140, 620),
                (280, 20, 340, 620),
                (480, 20, 540, 620),
            ]
        ),
    }
    metrics = {
        ".notdef": (700, 80),
        "space": (300, 0),
        "hani_standard": (700, 100),
        "hani_minor_same_position": (700, 40),
        "hani_grouped_shorter_link": (700, 40),
        "hani_symmetric_stems": (700, 40),
    }
    cmap = {
        0x20: "space",
        0x51B0: "hani_minor_same_position",
        0x51B1: "hani_grouped_shorter_link",
        0x51B2: "hani_symmetric_stems",
        0x7530: "hani_standard",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint CJK Remaining Branches",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint CJK Remaining Branches Regular",
            "fullName": "Autohint CJK Remaining Branches Regular",
            "psName": "AutohintCJKRemainingBranches-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "cjk-remaining-branches.ttf")


def build_digit_notdef_cmap() -> None:
    glyph_order = [".notdef", "space", "latin_o"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_o": ring_glyph(90, 0, 510, 520, 190, 120, 410, 400),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_o": (620, 90),
    }
    cmap = {
        0x20: "space",
        # Exercise FreeType's digit-width scan case where a cmap-covered digit
        # still resolves to glyph 0.
        0x30: ".notdef",
        0x6F: "latin_o",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Digit Notdef Cmap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Digit Notdef Cmap Regular",
            "fullName": "Autohint Digit Notdef Cmap Regular",
            "psName": "AutohintDigitNotdefCmap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "digit-notdef-cmap.ttf")


def build_out_of_range_cmap_coverage() -> None:
    glyph_order = [".notdef", "space", "latin_O"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_O": ring_glyph(90, 0, 510, 680, 190, 120, 410, 560),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_O": (620, 90),
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap({0x20: "space", 0x4F: "latin_O"})
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Out Of Range Cmap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Out Of Range Cmap Regular",
            "fullName": "Autohint Out Of Range Cmap Regular",
            "psName": "AutohintOutOfRangeCmap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    # Keep U+004F valid so the public load succeeds. U+0304 maps explicitly to
    # glyph zero, and U+0D00 shares its glyph with Latin so the later Malayalam
    # non-base scan must skip it. The other format-12 mappings intentionally
    # exceed maxp.numGlyphs=3: pinned FreeType returns those cmap GIDs, then
    # afglobal.c skips them while assigning script, digit, and non-base coverage.
    mappings = [
        (0x0020, 1),
        (0x0030, 254),
        (0x004F, 2),
        (0x006F, 255),
        (0x0303, 253),
        (0x0304, 0),
        (0x0D00, 2),
    ]
    groups = b"".join(
        struct.pack(">III", codepoint, codepoint, glyph_id)
        for codepoint, glyph_id in mappings
    )
    subtable = struct.pack(">HHIII", 12, 0, 16 + len(groups), 0, len(mappings)) + groups
    cmap_data = struct.pack(">HHHHI", 0, 1, 3, 10, 12) + subtable
    cmap = DefaultTable("cmap")
    cmap.data = cmap_data
    font.font["cmap"] = cmap

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "out-of-range-cmap-coverage.ttf")


def build_latin_standard_fallbacks() -> None:
    fallback_cases = [
        (
            "latin-missing-standard.ttf",
            "Autohint Latin Missing Standard",
            {
                ".notdef": rectangle_glyph(80, -120, 520, 720),
                "space": empty_glyph(),
                "latin_A": rectangle_glyph(100, 0, 540, 680),
                "latin_aacute": top_tilde_glyph(),
            },
            {
                ".notdef": (600, 80),
                "space": (300, 0),
                "latin_A": (700, 100),
                "latin_aacute": (700, 100),
            },
            {
                0x20: "space",
                0x41: "latin_A",
                # Keep Latin hinting active with only a capital-top blue zone;
                # no lowercase or uppercase pair exists, so vertical accent
                # separation must use FreeType's scaled-EM height fallback.
                0x54: "latin_A",
                0xE1: "latin_aacute",
            },
            [".notdef", "space", "latin_A", "latin_aacute"],
        ),
        (
            "latin-empty-standard.ttf",
            "Autohint Latin Empty Standard",
            {
                ".notdef": rectangle_glyph(80, -120, 520, 720),
                "space": empty_glyph(),
                "latin_o_empty": empty_glyph(),
                "latin_A": rectangle_glyph(100, 0, 540, 680),
            },
            {
                ".notdef": (600, 80),
                "space": (300, 0),
                "latin_o_empty": (620, 0),
                "latin_A": (700, 100),
            },
            {
                0x20: "space",
                0x41: "latin_A",
                0x6F: "latin_o_empty",
            },
            [".notdef", "space", "latin_o_empty", "latin_A"],
        ),
    ]

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for filename, family, glyphs, metrics, cmap, glyph_order in fallback_cases:
        font = FontBuilder(UNITS_PER_EM, isTTF=True)
        font.setupGlyphOrder(glyph_order)
        font.setupCharacterMap(cmap)
        font.setupGlyf(glyphs)
        font.setupHorizontalMetrics(metrics)
        font.setupHorizontalHeader(ascent=820, descent=-220)
        font.setupNameTable(
            {
                "familyName": family,
                "styleName": "Regular",
                "uniqueFontIdentifier": f"{family} Regular",
                "fullName": f"{family} Regular",
                "psName": family.replace(" ", "") + "-Regular",
                "version": "Version 1.0",
            }
        )
        font.setupOS2(
            sTypoAscender=820,
            sTypoDescender=-220,
            usWinAscent=820,
            usWinDescent=220,
        )
        font.setupPost()

        head = font.font["head"]
        head.created = 0
        head.modified = 0
        font.font.recalcTimestamp = False
        font.save(OUT_DIR / filename)


def build_latin_malformed_standard() -> None:
    glyph_order = [".notdef", "space", "latin_A", "latin_o_malformed"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_A": rectangle_glyph(100, 0, 540, 680),
        "latin_o_malformed": rectangle_glyph(90, 0, 510, 520),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_A": (700, 100),
        "latin_o_malformed": (620, 90),
    }
    cmap = {
        0x20: "space",
        0x41: "latin_A",
        0x6F: "latin_o_malformed",
    }

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Malformed Standard",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Malformed Standard Regular",
            "fullName": "Autohint Latin Malformed Standard Regular",
            "psName": "AutohintLatinMalformedStandard-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    path = OUT_DIR / "latin-malformed-standard.ttf"
    font.save(path)
    truncate_glyph_loca(path, "latin_o_malformed", 2)


def build_latin_loca_boundary_variants() -> None:
    """Exercise the public loader's malformed loca boundary behavior."""
    base = OUT_DIR / "latin-malformed-standard.ttf"
    variants = (
        ("latin-loca-early-overflow.ttf", 2, 100),
        ("latin-loca-final-overflow.ttf", 4, 100),
        ("latin-loca-unordered.ttf", 3, 20),
    )
    for filename, entry_index, offset in variants:
        path = OUT_DIR / filename
        shutil.copyfile(base, path)
        rewrite_loca_entry(path, entry_index, offset)

    long_path = OUT_DIR / "latin-loca-long-format.ttf"
    shutil.copyfile(base, long_path)
    convert_loca_to_long_format(long_path)


def build_latin_blue_delta() -> None:
    glyph_order = [".notdef", "space", "latin_flat_cap", "latin_round_cap"]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_flat_cap": rectangle_glyph(100, 0, 540, 500),
        "latin_round_cap": latin_blue_delta_round_glyph(),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_flat_cap": (700, 100),
        "latin_round_cap": (700, 90),
    }
    cmap = {0x20: "space"}
    for codepoint in (0x54, 0x48, 0x45, 0x5A):
        cmap[codepoint] = "latin_flat_cap"
    for codepoint in (0x4F, 0x43, 0x51, 0x53):
        cmap[codepoint] = "latin_round_cap"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Blue Delta",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Blue Delta Regular",
            "fullName": "Autohint Latin Blue Delta Regular",
            "psName": "AutohintLatinBlueDelta-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-blue-delta.ttf")


def build_latin_blue_overlap() -> None:
    glyph_order = [
        ".notdef",
        "space",
        "latin_cap_flat",
        "latin_cap_round_high",
        "latin_small_high",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(80, -120, 520, 720),
        "space": empty_glyph(),
        "latin_cap_flat": rectangle_glyph(100, 0, 540, 500),
        "latin_cap_round_high": ring_glyph(90, 0, 550, 700, 190, 120, 450, 560),
        "latin_small_high": rectangle_glyph(110, 0, 520, 650),
    }
    metrics = {
        ".notdef": (600, 80),
        "space": (300, 0),
        "latin_cap_flat": (700, 100),
        "latin_cap_round_high": (700, 90),
        "latin_small_high": (620, 110),
    }
    cmap = {0x20: "space"}
    for codepoint in (0x54, 0x48, 0x45, 0x5A, 0x4C):
        cmap[codepoint] = "latin_cap_flat"
    for codepoint in (0x4F, 0x43, 0x51, 0x53):
        cmap[codepoint] = "latin_cap_round_high"
    for codepoint in (0x78, 0x7A, 0x6E, 0x72, 0x6F, 0x65, 0x73, 0x63):
        cmap[codepoint] = "latin_small_high"

    font = FontBuilder(UNITS_PER_EM, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=820, descent=-220)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Blue Overlap",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Blue Overlap Regular",
            "fullName": "Autohint Latin Blue Overlap Regular",
            "psName": "AutohintLatinBlueOverlap-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=820,
        sTypoDescender=-220,
        usWinAscent=820,
        usWinDescent=220,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-blue-overlap.ttf")


def build_latin_low_upem() -> None:
    units_per_em = 64
    glyph_order = [
        ".notdef",
        "space",
        "latin_o",
        "latin_small_flat",
        "latin_cap_duplicate",
    ]
    glyphs = {
        ".notdef": rectangle_glyph(4, -8, 36, 48),
        "space": empty_glyph(),
        "latin_o": ring_glyph(4, 0, 44, 36, 12, 8, 36, 28),
        "latin_small_flat": rectangle_glyph(6, 0, 40, 32),
        "latin_cap_duplicate": low_upem_duplicate_point_glyph(),
    }
    metrics = {
        ".notdef": (52, 4),
        "space": (24, 0),
        "latin_o": (52, 4),
        "latin_small_flat": (48, 6),
        "latin_cap_duplicate": (52, 4),
    }
    cmap = {
        0x20: "space",
        0x41: "latin_cap_duplicate",
        0x6F: "latin_o",
    }
    for codepoint in (0x54, 0x48, 0x45, 0x5A, 0x4C):
        cmap[codepoint] = "latin_cap_duplicate"
    for codepoint in (0x78, 0x7A, 0x6E, 0x72):
        cmap[codepoint] = "latin_small_flat"
    for codepoint in (0x65, 0x73, 0x63):
        cmap[codepoint] = "latin_o"

    font = FontBuilder(units_per_em, isTTF=True)
    font.setupGlyphOrder(glyph_order)
    font.setupCharacterMap(cmap)
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics(metrics)
    font.setupHorizontalHeader(ascent=52, descent=-12)
    font.setupNameTable(
        {
            "familyName": "Autohint Latin Low UPEM",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Autohint Latin Low UPEM Regular",
            "fullName": "Autohint Latin Low UPEM Regular",
            "psName": "AutohintLatinLowUPEM-Regular",
            "version": "Version 1.0",
        }
    )
    font.setupOS2(
        sTypoAscender=52,
        sTypoDescender=-12,
        usWinAscent=52,
        usWinDescent=12,
    )
    font.setupPost()

    head = font.font["head"]
    head.created = 0
    head.modified = 0
    font.font.recalcTimestamp = False

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    font.save(OUT_DIR / "latin-low-upem.ttf")


def main() -> None:
    build_script_coverage()
    build_latin_small_ignore()
    build_batch199_latin_vertical_cusp_merge()
    build_batch200_latin_tilde_min_y()
    build_batch201_latin_tilde_prev_oncurve()
    build_batch210_latin_tilde_next_oncurve()
    build_batch211_latin_tilde_crossed_neighbors()
    build_batch212_latin_thin_crossed_tilde()
    build_batch213_latin_bottom_tilde_prev_oncurve()
    build_batch214_latin_bottom_tilde_next_oncurve()
    build_batch215_latin_bottom_tilde_crossed_neighbors()
    build_batch216_latin_thin_bottom_crossed_tilde()
    build_batch202_latin_bottom_tilde_max_y()
    build_batch203_latin_small_ignore_lowest_tie()
    build_batch217_latin_overlap_sentinel()
    build_batch218_latin_bottom_distance_order()
    build_batch204_latin_top_bottom_accent_overlap()
    shutil.copyfile(
        OUT_DIR / "script-coverage.ttf",
        OUT_DIR / "mixed-script-map.ttf",
    )
    build_latin_x_height_rejection()
    build_khmer_sub_top_overlap()
    build_batch194_khmer_sub_top_lowered()
    build_arabic_standard_fallback()
    build_arabic_neutral_first()
    build_arabic_neutral_round_skip()
    build_cjk_empty_standard()
    build_latin_remaining_topology()
    build_latin_width_clusters()
    build_latin_many_widths()
    build_cjk_blue_edge_cases()
    build_latin_blue_edge_cases()
    build_cjk_malformed_blue()
    build_cjk_tiny_stem()
    build_cjk_snap_below_standard()
    build_cjk_multi_width_snap()
    build_cjk_quantized_widths()
    build_cjk_many_widths()
    build_cjk_wide_stem_snap()
    build_cjk_round_stem_light()
    build_cjk_duplicate_edge()
    build_cjk_remaining_branches()
    build_digit_notdef_cmap()
    build_out_of_range_cmap_coverage()
    build_latin_standard_fallbacks()
    build_latin_malformed_standard()
    build_latin_loca_boundary_variants()
    build_latin_blue_delta()
    build_latin_blue_overlap()
    build_latin_low_upem()
    build_batch123_hebrew_long_blue_remaining()
    build_batch126_normal_scale_branches()
    build_batch127_cjk_edge_link_predicates()
    build_coverage_cjk_edge_order_links()
    build_batch145_cjk_edge_interpolation_witnesses()
    build_batch152_latin_adjustment_branches()
    build_batch153_latin_blue_empty_branches()
    build_batch159_latin_fallback_adjustment_branches()
    build_batch190_hebrew_late_oncurve()
    build_batch191_hebrew_offcurve_apex()
    build_batch196_hebrew_near_top_span()
    build_batch197_hebrew_near_top_span_mirror()


if __name__ == "__main__":
    main()
