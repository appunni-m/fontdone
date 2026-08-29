#!/usr/bin/env python3
"""Build compact CFF/OpenType fixtures for public metadata paths."""

from __future__ import annotations

from array import array
from copy import copy
from pathlib import Path
from tempfile import TemporaryDirectory

from fontTools.fontBuilder import FontBuilder
from fontTools.cffLib import TopDict
from fontTools.misc.psCharStrings import T2CharString
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "cff"
INPUT_OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "cff"
CFF2_OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "cff2"
CID_OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "cid"
CID_SOURCE = CID_OUT_DIR / "ot-cff-cid-keyed.otf"
UNITS_PER_EM = 1000
FIXED_HEAD_TIME = 0
GLYPH_ORDER = [".notdef", "A"]
METRICS = {".notdef": (600, 0), "A": (600, 40)}
CUBIC_GLYPH_ORDER = [
    ".notdef",
    "A",
    "cubic_c2_x_flatness",
    "cubic_c2_y_flatness",
    "vertical_lines",
    "relative_lines",
    "vh_curve",
    "hv_curve_no_last_delta",
    "vh_curve_no_last_delta",
    "hmoveto_default_width",
    "vmoveto_default_width",
    "rmoveto_default_width",
    "endchar_default_width",
    "hvcurveto_initial_width",
    "fixed_hmoveto",
    "rlineto_initial_width",
    "rrcurveto_initial_width",
    "hlineto_missing_args",
    "hmoveto_missing_args",
    "vmoveto_missing_args",
    "rmoveto_missing_args",
    "hvcurveto_missing_args",
    "hvcurveto_trailing_args",
    "type2_escape_unsupported",
    "type2_op_unsupported",
    "type2_shortint_overflow",
    "rlineto_missing_args",
    "rrcurveto_missing_args",
    "type2_positive_overflow",
    "type2_negative_overflow",
    "type2_shortint_hmoveto",
    "type2_no_endchar_eof",
    "rlineto_secondary_malformed",
    "rrcurveto_secondary_malformed",
    "tiny_cubic_y_span",
    "flat_cubic_y_span",
    "moveto_endchar_empty_contour",
    "repeated_moveto_empty_contours",
    "explicit_close_point",
    "same_x_open_contour",
    # Append-only: public fixture rows use stable numeric glyph IDs.
    "cubic_close_to_start",
    "type2_stack_overflow",
    "type2_argument_underflow",
    "type2_escaped_add_success",
    "type2_escape_unknown",
    "hvcurveto_single_operand",
    "hvcurveto_last_delta",
    "vhcurveto_last_delta",
]
NAMES = {
    "familyName": "Hybrid OTTO Coverage",
    "styleName": "Regular",
    "uniqueFontIdentifier": "Hybrid OTTO Coverage Regular",
    "fullName": "Hybrid OTTO Coverage Regular",
    "psName": "HybridOTTOCoverage-Regular",
}


def t2_charstring(rectangle: bool = False, cubic: str | None = None):
    pen = T2CharStringPen(600, None)
    if cubic == "arched":
        pen.moveTo((128, 0))
        pen.curveTo((240, 900), (720, 900), (832, 0))
        pen.moveTo((128, 0))
        pen.curveTo((300, 1120), (660, 1120), (832, 0))
    elif cubic == "c2_x":
        # Exercises the third `split_sdf_cubic` flatness term via public SDF.
        pen.moveTo((0, 0))
        pen.curveTo((100, 33), (250, 66), (300, 100))
    elif cubic == "c2_y":
        # Exercises the fourth `split_sdf_cubic` flatness term via public SDF.
        pen.moveTo((0, 0))
        pen.curveTo((100, 0), (200, 80), (300, 0))
    elif cubic == "tiny_y":
        # At 24 ppem this remains below one scanline after scaling, which
        # reaches FreeType black rasterizer's Bezier_Up early span rejection.
        pen.moveTo((0, 0))
        pen.curveTo((100, 4), (200, 8), (300, 12))
    elif cubic == "close_to_start":
        # CFF's builder removes the explicit on-curve endpoint because it
        # duplicates the contour start.  The resulting [on, cubic, cubic]
        # outline makes the SDF walker close the cubic directly to v_start.
        pen.moveTo((100, 100))
        pen.curveTo((100, 500), (500, 500), (100, 100))
    elif rectangle:
        pen.moveTo((80, 0))
        pen.lineTo((520, 0))
        pen.lineTo((520, 700))
        pen.lineTo((80, 700))
        pen.closePath()
    return pen.getCharString()


def t2_program_charstring(program: list[object]) -> T2CharString:
    return T2CharString(program=program, private=None, globalSubrs=[])


def glyf_glyph(rectangle: bool = False):
    pen = TTGlyphPen(GLYPH_ORDER)
    if rectangle:
        pen.moveTo((80, 0))
        pen.lineTo((520, 0))
        pen.lineTo((520, 700))
        pen.lineTo((80, 700))
        pen.closePath()
    return pen.glyph()


def build_cff(path: Path) -> None:
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap({0x41: "A"})
    builder.setupHorizontalMetrics(METRICS)
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(NAMES)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupCFF(
        NAMES["psName"],
        {
            # Keep the CFF FontInfo fixture populated across both string and
            # scalar Top DICT fields.  The public PS_FontInfo route compares
            # these values against the pinned C oracle, so this remains a
            # parity input rather than coverage-only scaffolding.
            "version": "Fontdone CFF Version",
            "Notice": "Fontdone synthetic CFF notice",
            "FullName": NAMES["fullName"],
            "FamilyName": NAMES["familyName"],
            "Weight": NAMES["styleName"],
            "isFixedPitch": 1,
            "ItalicAngle": -12,
            "UnderlinePosition": -200,
            "UnderlineThickness": 80,
        },
        {".notdef": t2_charstring(), "A": t2_charstring(rectangle=True)},
        {},
    )
    builder.setupMaxp()
    # FontBuilder initializes `head` times from the wall clock. Pin both fields
    # before serialization so identical generator inputs produce identical
    # bytes; `fontinfo-populated.otf` exposed this as the first divergent table.
    builder.font["head"].created = FIXED_HEAD_TIME
    builder.font["head"].modified = FIXED_HEAD_TIME
    builder.font.recalcTimestamp = False
    builder.save(path)


def build_cff2(path: Path) -> None:
    """Build a compact, static CFF2 face for CFF2 service-branch coverage."""
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap({0x41: "A"})
    builder.setupHorizontalMetrics(METRICS)
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(
        {
            "familyName": "Fontdone CFF2",
            "styleName": "Regular",
            "uniqueFontIdentifier": "Fontdone CFF2 Regular",
            "fullName": "Fontdone CFF2 Regular",
            "psName": "FontdoneCFF2-Regular",
        }
    )
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupCFF2(
        {".notdef": t2_charstring(), "A": t2_charstring(rectangle=True)}
    )
    builder.setupMaxp()
    builder.font["head"].created = FIXED_HEAD_TIME
    builder.font["head"].modified = FIXED_HEAD_TIME
    builder.font.recalcTimestamp = False
    path.parent.mkdir(parents=True, exist_ok=True)
    builder.save(path)


def build_cff_random(
    path: Path, private_dict: dict[str, object] | None = None
) -> None:
    """Build a valid CFF Type 2 face whose glyph consumes ``random``.

    FreeType's Adobe CFF interpreter keeps the random state on the opened
    subfont, so loading this glyph twice produces two different outlines.  The
    parity case fixes the CFF driver's random-seed property to zero before the
    face is opened.  A non-empty private dictionary sanitizes a zero
    ``initialRandomSeed`` to the pinned 987654321 default; the default call
    keeps the generated fixture's ``Private=(0, offset)`` entry so that
    pinned FreeType exits before applying those defaults and leaves its exact
    initial random state at zero.
    """
    names = {
        "familyName": "Pure CFF Random Coverage",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF Random Coverage Regular",
        "fullName": "Pure CFF Random Coverage Regular",
        "psName": "PureCFFRandomCoverage-Regular",
    }
    glyph_order = [".notdef", "random"]
    metrics = {".notdef": (600, 0), "random": (600, 0)}
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(glyph_order)
    builder.setupCharacterMap({0x41: "random"})
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    random_charstring = t2_program_charstring(
        [
            0,
            0,
            "rmoveto",
            1000,
            "random",
            "mul",
            0,
            "rlineto",
            0,
            700,
            "rlineto",
            -1000,
            0,
            "rlineto",
            "endchar",
        ]
    )
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        {".notdef": t2_charstring(), "random": random_charstring},
        private_dict or {},
    )
    builder.setupMaxp()
    recalc_font_bbox = TopDict.recalcFontBBox
    try:
        # fontTools' bounds walker does not evaluate the stateful random
        # operator.  Keep the generated table structurally valid and preserve
        # the explicit bounds used by the maintained fixture.
        TopDict.recalcFontBBox = lambda self: None
        builder.font.recalcBBoxes = False
        builder.font["head"].created = FIXED_HEAD_TIME
        builder.font["head"].modified = FIXED_HEAD_TIME
        builder.font.recalcTimestamp = False
        builder.font["CFF "].cff.topDictIndex[0].FontBBox = [0, 0, 1000, 700]
        builder.save(path)
    finally:
        TopDict.recalcFontBBox = recalc_font_bbox


def build_cubic_cff(
    path: Path,
    with_vertical_metrics: bool = False,
    include_append_only_glyphs: bool = True,
) -> None:
    names = {
        "familyName": "Pure CFF Cubic Coverage",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF Cubic Coverage Regular",
        "fullName": "Pure CFF Cubic Coverage Regular",
        "psName": "PureCFFCubicCoverage-Regular",
    }
    metrics = {
        ".notdef": (900, 0),
        "A": (900, 128),
        "cubic_c2_x_flatness": (420, 0),
        "cubic_c2_y_flatness": (420, 0),
        "vertical_lines": (420, 0),
        "relative_lines": (420, 0),
        "vh_curve": (420, 0),
        "hv_curve_no_last_delta": (420, 0),
        "vh_curve_no_last_delta": (420, 0),
        "hmoveto_default_width": (420, 0),
        "vmoveto_default_width": (420, 0),
        "rmoveto_default_width": (420, 0),
        "endchar_default_width": (420, 0),
        "hvcurveto_initial_width": (420, 0),
        "fixed_hmoveto": (420, 0),
        "rlineto_initial_width": (420, 0),
        "rrcurveto_initial_width": (420, 0),
        "hlineto_missing_args": (420, 0),
        "hmoveto_missing_args": (420, 0),
        "vmoveto_missing_args": (420, 0),
        "rmoveto_missing_args": (420, 0),
        "hvcurveto_missing_args": (420, 0),
        "hvcurveto_trailing_args": (420, 0),
        "type2_escape_unsupported": (420, 0),
        "type2_op_unsupported": (420, 0),
        "type2_shortint_overflow": (420, 0),
        "rlineto_missing_args": (420, 0),
        "rrcurveto_missing_args": (420, 0),
        "type2_positive_overflow": (420, 0),
        "type2_negative_overflow": (420, 0),
        "type2_shortint_hmoveto": (420, 0),
        "type2_no_endchar_eof": (420, 0),
        "rlineto_secondary_malformed": (420, 0),
        "rrcurveto_secondary_malformed": (420, 0),
        "tiny_cubic_y_span": (420, 0),
        "flat_cubic_y_span": (420, 0),
        "moveto_endchar_empty_contour": (420, 0),
        "repeated_moveto_empty_contours": (420, 0),
        "explicit_close_point": (420, 0),
        "same_x_open_contour": (420, 0),
        "cubic_close_to_start": (620, 0),
        "type2_stack_overflow": (420, 0),
        "type2_argument_underflow": (420, 0),
        "type2_escaped_add_success": (420, 0),
        "type2_escape_unknown": (420, 0),
        "hvcurveto_single_operand": (420, 0),
        "hvcurveto_last_delta": (420, 0),
        "vhcurveto_last_delta": (420, 0),
    }
    glyph_order = (
        CUBIC_GLYPH_ORDER
        if include_append_only_glyphs
        else CUBIC_GLYPH_ORDER[:-2]
    )
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(glyph_order)
    character_map = {
            0x41: "A",
            0x42: "cubic_c2_x_flatness",
            0x43: "cubic_c2_y_flatness",
            0x44: "vertical_lines",
            0x45: "relative_lines",
            0x46: "vh_curve",
            0x47: "hv_curve_no_last_delta",
            0x48: "vh_curve_no_last_delta",
            0x49: "hmoveto_default_width",
            0x4A: "vmoveto_default_width",
            0x4B: "rmoveto_default_width",
            0x4C: "endchar_default_width",
            0x4D: "hvcurveto_initial_width",
            0x4E: "fixed_hmoveto",
            0x4F: "rlineto_initial_width",
            0x50: "rrcurveto_initial_width",
            0x51: "hlineto_missing_args",
            0x52: "hmoveto_missing_args",
            0x53: "vmoveto_missing_args",
            0x54: "rmoveto_missing_args",
            0x55: "hvcurveto_missing_args",
            0x56: "hvcurveto_trailing_args",
            0x57: "type2_escape_unsupported",
            0x58: "type2_op_unsupported",
            0x59: "type2_shortint_overflow",
            0x5A: "rlineto_missing_args",
            0x5B: "rrcurveto_missing_args",
            0x5C: "type2_positive_overflow",
            0x5D: "type2_negative_overflow",
            0x5E: "type2_shortint_hmoveto",
            0x5F: "type2_no_endchar_eof",
            0x60: "rlineto_secondary_malformed",
            0x61: "rrcurveto_secondary_malformed",
            0x62: "tiny_cubic_y_span",
            0x63: "flat_cubic_y_span",
            0x64: "moveto_endchar_empty_contour",
            0x65: "repeated_moveto_empty_contours",
            0x66: "explicit_close_point",
            0x67: "same_x_open_contour",
            0x68: "cubic_close_to_start",
            0x69: "type2_stack_overflow",
            0x6A: "type2_argument_underflow",
            0x6B: "type2_escaped_add_success",
            0x6C: "type2_escape_unknown",
            0x6D: "hvcurveto_single_operand",
            0x6E: "hvcurveto_last_delta",
            0x6F: "vhcurveto_last_delta",
    }
    if not include_append_only_glyphs:
        character_map.pop(0x6E)
        character_map.pop(0x6F)
    builder.setupCharacterMap(character_map)
    metrics = {name: metrics[name] for name in glyph_order}
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=1200, descent=-200)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=1200,
        sTypoDescender=-200,
        usWinAscent=1200,
        usWinDescent=200,
    )
    builder.setupPost()
    charstrings = {
            ".notdef": t2_charstring(),
            "A": t2_charstring(cubic="arched"),
            "cubic_c2_x_flatness": t2_charstring(cubic="c2_x"),
            "cubic_c2_y_flatness": t2_charstring(cubic="c2_y"),
            "cubic_close_to_start": t2_charstring(cubic="close_to_start"),
            "vertical_lines": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    200,
                    "vlineto",
                    100,
                    "hlineto",
                    -200,
                    "vlineto",
                    "endchar",
                ]
            ),
            "relative_lines": t2_program_charstring(
                [
                    600,
                    0,
                    60,
                    "rmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "vh_curve": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    80,
                    "vhcurveto",
                    "endchar",
                ]
            ),
            "hv_curve_no_last_delta": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "vh_curve_no_last_delta": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    "vhcurveto",
                    "endchar",
                ]
            ),
            "hmoveto_default_width": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    100,
                    100,
                    100,
                    -100,
                    -100,
                    -100,
                    "rlineto",
                    "endchar",
                ]
            ),
            "vmoveto_default_width": t2_program_charstring(
                [
                    100,
                    "vmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rmoveto_default_width": t2_program_charstring(
                [
                    0,
                    60,
                    "rmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "endchar_default_width": t2_program_charstring(
                [
                    "endchar",
                ]
            ),
            "hvcurveto_initial_width": t2_program_charstring(
                [
                    600,
                    100,
                    50,
                    60,
                    70,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "fixed_hmoveto": t2_program_charstring(
                [
                    600,
                    1.5,
                    "hmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rlineto_initial_width": t2_program_charstring(
                [
                    600,
                    80,
                    0,
                    0,
                    100,
                    -80,
                    0,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rrcurveto_initial_width": t2_program_charstring(
                [
                    600,
                    60,
                    0,
                    60,
                    100,
                    120,
                    0,
                    "rrcurveto",
                    "endchar",
                ]
            ),
            "hlineto_missing_args": t2_program_charstring(
                [
                    "hlineto",
                    "endchar",
                ]
            ),
            "hmoveto_missing_args": t2_program_charstring(
                [
                    "hmoveto",
                    "endchar",
                ]
            ),
            "vmoveto_missing_args": t2_program_charstring(
                [
                    "vmoveto",
                    "endchar",
                ]
            ),
            "rmoveto_missing_args": t2_program_charstring(
                [
                    "rmoveto",
                    "endchar",
                ]
            ),
            "hvcurveto_missing_args": t2_program_charstring(
                [
                    10,
                    20,
                    30,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "hvcurveto_trailing_args": t2_program_charstring(
                [
                    10,
                    20,
                    30,
                    40,
                    50,
                    60,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "type2_escape_unsupported": T2CharString(
                bytecode=bytes([12, 0, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_op_unsupported": T2CharString(
                bytecode=bytes([10, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_shortint_overflow": T2CharString(
                bytecode=bytes([28]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "rlineto_missing_args": t2_program_charstring(
                [
                    "rlineto",
                    "endchar",
                ]
            ),
            "rrcurveto_missing_args": t2_program_charstring(
                [
                    "rrcurveto",
                    "endchar",
                ]
            ),
            "type2_positive_overflow": T2CharString(
                bytecode=bytes([247]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_negative_overflow": T2CharString(
                bytecode=bytes([251]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_shortint_hmoveto": T2CharString(
                bytecode=bytes([28, 0, 0, 22, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_no_endchar_eof": T2CharString(
                bytecode=bytes([139, 22]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "rlineto_secondary_malformed": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    10,
                    "rlineto",
                    "endchar",
                ]
            ),
            "rrcurveto_secondary_malformed": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    10,
                    "rrcurveto",
                    "endchar",
                ]
            ),
            "tiny_cubic_y_span": t2_charstring(cubic="tiny_y"),
            "flat_cubic_y_span": t2_program_charstring(
                [
                    0,
                    0,
                    "rmoveto",
                    100,
                    0,
                    100,
                    0,
                    100,
                    0,
                    "rrcurveto",
                    0,
                    120,
                    -300,
                    0,
                    0,
                    -120,
                    "rlineto",
                    "endchar",
                ]
            ),
            "moveto_endchar_empty_contour": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    "endchar",
                ]
            ),
            "repeated_moveto_empty_contours": t2_program_charstring(
                [
                    0,
                    "hmoveto",
                    100,
                    "hmoveto",
                    "endchar",
                ]
            ),
            "explicit_close_point": t2_program_charstring(
                [
                    0,
                    0,
                    "rmoveto",
                    100,
                    0,
                    0,
                    100,
                    -100,
                    0,
                    0,
                    -100,
                    "rlineto",
                    "endchar",
                ]
            ),
            "same_x_open_contour": t2_program_charstring(
                [
                    0,
                    0,
                    "rmoveto",
                    0,
                    100,
                    "rlineto",
                    "endchar",
                ]
            ),
            "type2_stack_overflow": T2CharString(
                bytecode=bytes([139] * 49 + [14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_argument_underflow": T2CharString(
                bytecode=bytes([12, 10, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_escaped_add_success": T2CharString(
                # 0 + 1 leaves one width operand for endchar, matching the
                # pinned Adobe interpreter's successful escaped arithmetic
                # route.
                bytecode=bytes([139, 140, 12, 10, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "type2_escape_unknown": T2CharString(
                # Escape 99 is outside the pinned Type 2 escaped-op set.
                # FreeType logs and ignores unknown escaped operators, then
                # accepts the following endchar.
                bytecode=bytes([12, 99, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "hvcurveto_single_operand": T2CharString(
                # A one-operand alternating curve reaches the pinned
                # interpreter's short-argument boundary before endchar.
                bytecode=bytes([139, 31, 14]),
                program=None,
                private=None,
                globalSubrs=[],
            ),
            "hvcurveto_last_delta": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    80,
                    "hvcurveto",
                    "endchar",
                ]
            ),
            "vhcurveto_last_delta": t2_program_charstring(
                [
                    600,
                    100,
                    "vmoveto",
                    100,
                    50,
                    60,
                    70,
                    80,
                    "vhcurveto",
                    "endchar",
                ]
            ),
        }
    if not include_append_only_glyphs:
        charstrings.pop("hvcurveto_last_delta")
        charstrings.pop("vhcurveto_last_delta")
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        charstrings,
        {},
    )
    builder.setupMaxp()
    if with_vertical_metrics:
        add_vertical_metrics(builder.font)
    recalc_font_bbox = TopDict.recalcFontBBox
    try:
        # fontTools' bounds walker treats these `rlineto` and `rrcurveto`
        # programs as malformed because they intentionally begin with an odd
        # operand count and no moveto.  FreeType reaches them through real
        # public glyph loads and rejects them, so keep the raw charstrings and
        # preserve the explicit compact fixture bbox.
        TopDict.recalcFontBBox = lambda self: None
        builder.font.recalcBBoxes = False
        builder.font["head"].created = FIXED_HEAD_TIME
        builder.font["head"].modified = FIXED_HEAD_TIME
        builder.font["CFF "].cff.topDictIndex[0].FontBBox = [0, 0, 900, 1200]
        builder.save(path)
    finally:
        TopDict.recalcFontBBox = recalc_font_bbox


def build_matching_glyf(path: Path) -> None:
    builder = FontBuilder(UNITS_PER_EM, isTTF=True)
    builder.setupGlyphOrder(GLYPH_ORDER)
    builder.setupCharacterMap({0x41: "A"})
    builder.setupGlyf({".notdef": glyf_glyph(), "A": glyf_glyph(rectangle=True)})
    builder.setupHorizontalMetrics(METRICS)
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(NAMES)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupMaxp()
    builder.save(path)


def write_hybrid_otto_face_info() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "hybrid-otto-face-info.otf"
    with TemporaryDirectory() as tmp:
        cff_path = Path(tmp) / "cff.otf"
        glyf_path = Path(tmp) / "glyf.ttf"
        build_cff(cff_path)
        build_matching_glyf(glyf_path)

        cff_font = TTFont(cff_path, recalcTimestamp=False)
        glyf_font = TTFont(glyf_path, recalcTimestamp=False)
        # The current Rust parser accepts OTTO SFNT wrappers but still reads
        # TrueType outline tables for metadata-only public paths.  Keeping a
        # valid CFF table lets the native FreeType oracle open the same face.
        cff_font["glyf"] = glyf_font["glyf"]
        cff_font["loca"] = glyf_font["loca"]
        cff_font["maxp"] = glyf_font["maxp"]
        cff_font["head"].created = FIXED_HEAD_TIME
        cff_font["head"].modified = FIXED_HEAD_TIME
        cff_font.sfntVersion = "OTTO"
        cff_font.recalcTimestamp = False
        if out.exists() or out.is_symlink():
            out.unlink()
        cff_font.save(out, reorderTables=True)


def write_pure_cff_cubic() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-cubic.otf"
    if out.exists() or out.is_symlink():
        out.unlink()
    build_cubic_cff(out, include_append_only_glyphs=False)


def write_pure_cff_random() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-random.otf"
    if out.exists() or out.is_symlink():
        out.unlink()
    build_cff_random(out)


def write_pure_cff_random_private() -> None:
    """Build source-reviewed CFF Private-dictionary boundary controls."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    seeded = OUT_DIR / "pure-cff-random-private-seed-positive.otf"
    build_cff_random(seeded, private_dict={"initialRandomSeed": 123})
    default_seed = OUT_DIR / "pure-cff-random-private-default-seed.otf"
    build_cff_random(default_seed, private_dict={"BlueShift": 8})

    base = OUT_DIR / "pure-cff-random.otf"
    no_private_payload = bytearray(sfnt_table_payload(base, b"CFF "))
    patch_cff_private_top_dict(no_private_payload, operator_replacement=19)
    replace_sfnt_table(
        base,
        OUT_DIR / "pure-cff-random-no-private.otf",
        b"CFF ",
        bytes(no_private_payload),
    )

    offset_zero_payload = bytearray(sfnt_table_payload(base, b"CFF "))
    patch_cff_private_top_dict(offset_zero_payload, size=1, offset=0)
    replace_sfnt_table(
        base,
        OUT_DIR / "pure-cff-random-private-offset-zero.otf",
        b"CFF ",
        bytes(offset_zero_payload),
    )

    missing_seed_payload = bytearray(sfnt_table_payload(default_seed, b"CFF "))
    patch_cff_private_initial_random_seed_missing(missing_seed_payload)
    replace_sfnt_table(
        default_seed,
        OUT_DIR / "pure-cff-random-private-seed-missing-operand.otf",
        b"CFF ",
        bytes(missing_seed_payload),
    )


def write_pure_cff_random_private_parser_controls() -> None:
    """Build CFF Private parser controls reviewed against pinned cffparse.c."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    # `StdHW` is a valid one-byte Private DICT operator (cfftoken.h:90), so
    # this fixture reaches the non-escaped operator arm without relying on a
    # malformed dictionary.
    one_byte = OUT_DIR / "pure-cff-random-private-one-byte-op.otf"
    build_cff_random(one_byte, private_dict={"StdHW": 50})

    base = OUT_DIR / "pure-cff-random-private-default-seed.otf"
    controls = {
        "pure-cff-random-private-reserved-byte-22.otf": b"\x16",
        "pure-cff-random-private-reserved-byte-255.otf": b"\xff",
        # cff_parser_run classifies 27 as a number, then 0x16 as an unknown
        # operator.  This deliberately preserves that permissive C behavior.
        "pure-cff-random-private-reserved-number-27.otf": b"\x1b\x16",
        # A one-byte Private range ending after escape 12 reaches Syntax_Error
        # in pinned FreeType and becomes public Invalid_Argument.
        "pure-cff-random-private-truncated-escape.otf": b"\x0c",
    }
    for filename, private_payload in controls.items():
        payload = bytearray(sfnt_table_payload(base, b"CFF "))
        patch_cff_private_top_dict(payload, size=len(private_payload))
        patch_cff_private_payload(payload, private_payload)
        replace_sfnt_table(base, OUT_DIR / filename, b"CFF ", bytes(payload))


def write_pure_cff_random_private_edge_controls() -> None:
    """Build additional CFF Private edges accepted by the pinned parser."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    negative_seed = OUT_DIR / "pure-cff-random-private-seed-negative.otf"
    build_cff_random(negative_seed, private_dict={"initialRandomSeed": -123})
    minimum_seed = OUT_DIR / "pure-cff-random-private-seed-minimum.otf"
    build_cff_random(minimum_seed, private_dict={"initialRandomSeed": -2147483648})

    base = OUT_DIR / "pure-cff-random-private-default-seed.otf"
    controls = {
        "pure-cff-random-private-legacy-byte-31.otf": b"\x1f",
        # FreeType's DICT scanner exits successfully when a real number runs
        # to the end of the Private range without a 0xf terminator.
        "pure-cff-random-private-unterminated-real.otf": b"\x1e",
    }
    for filename, private_payload in controls.items():
        payload = bytearray(sfnt_table_payload(base, b"CFF "))
        patch_cff_private_top_dict(payload, size=len(private_payload))
        patch_cff_private_payload(payload, private_payload)
        replace_sfnt_table(base, OUT_DIR / filename, b"CFF ", bytes(payload))


def build_cff_random_global_subr_error(path: Path) -> None:
    """Build a valid CFF face whose second random subroutine call errors.

    The first random value selects global subroutine 107, while the next one
    selects 108.  With exactly 108 global subroutines, FreeType accepts the
    first glyph load and rejects the second without changing the public input.
    The public parity batch varies the seed and reaches the post-error reload
    branch without malformed input or an invalid glyph index.
    """
    names = {
        "familyName": "Pure CFF Random Global Subr Error",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF Random Global Subr Error Regular",
        "fullName": "Pure CFF Random Global Subr Error Regular",
        "psName": "PureCFFRandomGlobalSubrError-Regular",
    }
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder([".notdef", "error"])
    builder.setupCharacterMap({0x41: "error"})
    builder.setupHorizontalMetrics({".notdef": (600, 0), "error": (600, 0)})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    error_charstring = t2_program_charstring(
        ["random", 1, "eq", "callgsubr", "endchar"]
    )
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        {".notdef": t2_charstring(), "error": error_charstring},
        {},
    )
    cff = builder.font["CFF "].cff
    for _ in range(108):
        cff.GlobalSubrs.append(
            T2CharString(program=["return"], private=None, globalSubrs=cff.GlobalSubrs)
        )
    builder.setupMaxp()
    recalc_font_bbox = TopDict.recalcFontBBox
    try:
        TopDict.recalcFontBBox = lambda self: None
        builder.font.recalcBBoxes = False
        builder.font["head"].created = FIXED_HEAD_TIME
        builder.font["head"].modified = FIXED_HEAD_TIME
        builder.font.recalcTimestamp = False
        builder.font["CFF "].cff.topDictIndex[0].FontBBox = [0, 0, 0, 0]
        builder.save(path)
    finally:
        TopDict.recalcFontBBox = recalc_font_bbox


def write_pure_cff_random_global_subr_error() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-random-global-subr-error.otf"
    if out.exists() or out.is_symlink():
        out.unlink()
    build_cff_random_global_subr_error(out)


def write_pure_cff_below_baseline_no_vmtx() -> None:
    """Build a valid CFF face whose glyph bbox lies entirely below baseline."""
    out = OUT_DIR / "pure-cff-below-baseline-no-vmtx.otf"
    names = {
        "familyName": "Pure CFF Below Baseline",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF Below Baseline Regular",
        "fullName": "Pure CFF Below Baseline Regular",
        "psName": "PureCFFBelowBaseline-Regular",
    }
    glyph_order = [".notdef", "p"]
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(glyph_order)
    builder.setupCharacterMap({0x0070: "p"})
    builder.setupHorizontalMetrics({".notdef": (600, 0), "p": (600, 0)})
    builder.setupHorizontalHeader(ascent=800, descent=-400)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-400,
        usWinAscent=800,
        usWinDescent=400,
    )
    builder.setupPost()
    pen = T2CharStringPen(600, None)
    pen.moveTo((80, -360))
    pen.lineTo((520, -360))
    pen.lineTo((520, -120))
    pen.lineTo((80, -120))
    pen.closePath()
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        {".notdef": t2_charstring(), "p": pen.getCharString()},
        {},
    )
    builder.setupMaxp()
    builder.font["head"].created = FIXED_HEAD_TIME
    builder.font["head"].modified = FIXED_HEAD_TIME
    builder.font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    builder.save(out)


def write_pure_cff_baseline_touch_no_vmtx() -> None:
    """Build a valid CFF face whose glyph top edge touches the baseline."""
    out = OUT_DIR / "pure-cff-baseline-touch-no-vmtx.otf"
    names = {
        "familyName": "Pure CFF Baseline Touch",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF Baseline Touch Regular",
        "fullName": "Pure CFF Baseline Touch Regular",
        "psName": "PureCFFBaselineTouch-Regular",
    }
    glyph_order = [".notdef", "p"]
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(glyph_order)
    builder.setupCharacterMap({0x0070: "p"})
    builder.setupHorizontalMetrics({".notdef": (600, 0), "p": (600, 0)})
    builder.setupHorizontalHeader(ascent=800, descent=-400)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-400,
        usWinAscent=800,
        usWinDescent=400,
    )
    builder.setupPost()
    pen = T2CharStringPen(600, None)
    pen.moveTo((80, -360))
    pen.lineTo((520, -360))
    pen.lineTo((520, 0))
    pen.lineTo((80, 0))
    pen.closePath()
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        {".notdef": t2_charstring(), "p": pen.getCharString()},
        {},
    )
    builder.setupMaxp()
    builder.font["head"].created = FIXED_HEAD_TIME
    builder.font["head"].modified = FIXED_HEAD_TIME
    builder.font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    builder.save(out)


def write_pure_cff_cubic_last_delta() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-cubic-last-delta.otf"
    if out.exists() or out.is_symlink():
        out.unlink()
    build_cubic_cff(out)


def write_pure_cff_bbox_extrema() -> None:
    """Build valid cubic contours that exercise public exact-bbox extrema."""
    out = OUT_DIR / "pure-cff-bbox-extrema.otf"
    glyph_names = [
        ".notdef",
        "bbox_y_max",
        "bbox_y_min",
        "bbox_x_max",
        "bbox_x_min",
        "bbox_xy_max",
        "bbox_xy_min",
    ]
    contours = {
        "bbox_y_max": ((100, 100), (200, 700), (400, 700), (500, 100)),
        "bbox_y_min": ((100, 500), (200, -100), (400, -100), (500, 500)),
        "bbox_x_max": ((100, 100), (700, 200), (700, 400), (100, 500)),
        "bbox_x_min": ((500, 100), (-100, 200), (-100, 400), (500, 500)),
        "bbox_xy_max": ((100, 100), (700, 700), (700, 500), (300, 300)),
        "bbox_xy_min": ((700, 700), (100, 100), (100, 300), (500, 500)),
    }

    def cubic_charstring(points: tuple[tuple[int, int], ...]) -> T2CharString:
        pen = T2CharStringPen(900, None)
        pen.moveTo(points[0])
        pen.curveTo(*points[1:])
        pen.closePath()
        return pen.getCharString()

    names = {
        "familyName": "Pure CFF BBox Extrema",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF BBox Extrema Regular",
        "fullName": "Pure CFF BBox Extrema Regular",
        "psName": "PureCFFBBoxExtrema-Regular",
    }
    builder = FontBuilder(UNITS_PER_EM, isTTF=False)
    builder.setupGlyphOrder(glyph_names)
    builder.setupCharacterMap(
        {0x0100 + index: name for index, name in enumerate(glyph_names[1:])}
    )
    builder.setupHorizontalMetrics(
        {name: (900, 100 if name != ".notdef" else 0) for name in glyph_names}
    )
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        usWinAscent=800,
        usWinDescent=200,
    )
    builder.setupPost()
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        {
            ".notdef": t2_charstring(),
            **{name: cubic_charstring(points) for name, points in contours.items()},
        },
        {},
    )
    builder.setupMaxp()
    builder.font["head"].created = FIXED_HEAD_TIME
    builder.font["head"].modified = FIXED_HEAD_TIME
    builder.font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    builder.save(out)


def write_pure_cff_cubic_peak_shifts() -> None:
    """Build valid large-coordinate cubics for bbox peak shift regimes."""
    out = OUT_DIR / "pure-cff-cubic-peak-shifts.otf"
    glyph_names = [
        ".notdef",
        "peak_y_max",
        "peak_y_min",
        "peak_x_max",
        "peak_x_min",
        "peak_xy_max",
        "peak_xy_min",
    ]
    contours = {
        "peak_y_max": ((0, 0), (5000, 20000), (15000, 20000), (20000, 0)),
        "peak_y_min": ((0, 0), (5000, -20000), (15000, -20000), (20000, 0)),
        "peak_x_max": ((0, 0), (20000, 5000), (20000, 15000), (0, 20000)),
        "peak_x_min": ((0, 0), (-20000, 5000), (-20000, 15000), (0, 20000)),
        "peak_xy_max": ((0, 0), (20000, 16000), (12000, 20000), (0, 0)),
        "peak_xy_min": ((0, 0), (-20000, -16000), (-12000, -20000), (0, 0)),
    }

    def cubic_charstring(points: tuple[tuple[int, int], ...]) -> T2CharString:
        pen = T2CharStringPen(16, None)
        pen.moveTo(points[0])
        pen.curveTo(*points[1:])
        pen.closePath()
        return pen.getCharString()

    names = {
        "familyName": "Pure CFF Cubic Peak Shifts",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Pure CFF Cubic Peak Shifts Regular",
        "fullName": "Pure CFF Cubic Peak Shifts Regular",
        "psName": "PureCFFCubicPeakShifts-Regular",
    }
    units_per_em = 16
    builder = FontBuilder(units_per_em, isTTF=False)
    builder.setupGlyphOrder(glyph_names)
    builder.setupCharacterMap(
        {0x0100 + index: name for index, name in enumerate(glyph_names[1:])}
    )
    builder.setupHorizontalMetrics(
        {name: (16, 0) for name in glyph_names}
    )
    builder.setupHorizontalHeader(ascent=13, descent=-3)
    builder.setupNameTable(names)
    builder.setupOS2(
        sTypoAscender=13,
        sTypoDescender=-3,
        usWinAscent=13,
        usWinDescent=3,
    )
    builder.setupPost()
    builder.setupCFF(
        names["psName"],
        {
            "FullName": names["fullName"],
            "FamilyName": names["familyName"],
            "Weight": names["styleName"],
        },
        {
            ".notdef": t2_charstring(),
            **{name: cubic_charstring(points) for name, points in contours.items()},
        },
        {},
    )
    builder.setupMaxp()
    builder.font["head"].created = FIXED_HEAD_TIME
    builder.font["head"].modified = FIXED_HEAD_TIME
    builder.font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    builder.save(out)




def write_cid_cff_format2() -> None:
    """Derive a CID face whose contiguous charset must use format 2."""
    out = CID_OUT_DIR / "ot-cff-cid-keyed-format2.otf"
    font = TTFont(CID_SOURCE, recalcTimestamp=False)
    glyph_order = list(font.getGlyphOrder())
    if len(glyph_order) != 257 or glyph_order[-1] != "cid00256":
        raise ValueError("the pinned CID source must contain glyphs cid00001..cid00256")

    # A format-1 charset stores nLeft in one byte.  Adding one real CID makes
    # the existing contiguous 1..257 range require format 2's 16-bit nLeft.
    extra_glyph = "cid00257"
    hmtx = font["hmtx"]
    top_dict = font["CFF "].cff.topDictIndex[0]
    font.setGlyphOrder(glyph_order + [extra_glyph])
    hmtx.metrics[extra_glyph] = hmtx.metrics[glyph_order[-1]]
    font["maxp"].numGlyphs = len(glyph_order) + 1
    top_dict.charset = glyph_order + [extra_glyph]
    top_dict.CIDCount += 1
    top_dict.CharStrings.charStrings[extra_glyph] = copy(
        top_dict.CharStrings.charStrings[glyph_order[-1]]
    )
    top_dict.FDSelect.gidArray.append(top_dict.FDSelect.gidArray[-1])
    font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def write_cid_cff_charset_variants() -> None:
    """Derive CID faces whose charsets use CFF formats 0 and 1."""
    variants = {
        # Alternating CIDs make a format-0 table smaller than a range table.
        "format0": [2 * index + 1 for index in range(256)],
        # Two contiguous ranges fit the one-byte nLeft field used by format 1.
        "format1": list(range(1, 129)) + list(range(300, 428)),
    }
    for label, cid_values in variants.items():
        out = CID_OUT_DIR / f"ot-cff-cid-keyed-{label}.otf"
        font = TTFont(CID_SOURCE, recalcTimestamp=False)
        glyph_order = list(font.getGlyphOrder())
        if len(glyph_order) != len(cid_values) + 1:
            raise ValueError("the pinned CID source has an unexpected glyph count")

        top_dict = font["CFF "].cff.topDictIndex[0]
        old_charstrings = dict(top_dict.CharStrings.charStrings)
        old_metrics = dict(font["hmtx"].metrics)
        new_order = [".notdef"] + [f"cid{cid:05d}" for cid in cid_values]
        top_dict.CharStrings.charStrings.clear()
        top_dict.CharStrings.charStrings[".notdef"] = old_charstrings[".notdef"]
        for old_name, new_name in zip(glyph_order[1:], new_order[1:]):
            top_dict.CharStrings.charStrings[new_name] = old_charstrings[old_name]
        font["hmtx"].metrics.clear()
        font["hmtx"].metrics[".notdef"] = old_metrics[".notdef"]
        for old_name, new_name in zip(glyph_order[1:], new_order[1:]):
            font["hmtx"].metrics[new_name] = old_metrics[old_name]
        font.setGlyphOrder(new_order)
        top_dict.charset = new_order
        top_dict.CIDCount = max(cid_values) + 1
        font.recalcTimestamp = False
        if out.exists() or out.is_symlink():
            out.unlink()
        font.save(out, reorderTables=True)


def write_cid_cff_single_glyph() -> None:
    """Derive a CID face containing only the required `.notdef` glyph."""
    out = CID_OUT_DIR / "ot-cff-cid-keyed-single-glyph.otf"
    font = TTFont(CID_SOURCE, recalcTimestamp=False)
    top_dict = font["CFF "].cff.topDictIndex[0]
    charstrings = top_dict.CharStrings.charStrings
    metrics = font["hmtx"].metrics
    font.setGlyphOrder([".notdef"])
    font["maxp"].numGlyphs = 1
    top_dict.CharStrings.charStrings = {".notdef": charstrings[".notdef"]}
    top_dict.charset = [".notdef"]
    top_dict.CIDCount = 1
    top_dict.FDSelect.gidArray = top_dict.FDSelect.gidArray[:1]
    font["hmtx"].metrics = {".notdef": metrics[".notdef"]}
    font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def patch_cff_charset_top_dict(
    data: bytearray,
    *,
    charset_offset: int | None = None,
    operator_replacement: int | None = None,
) -> None:
    """Patch the CFF Top DICT charset operand without resizing the table."""
    header_size = data[2]
    _, cursor = cff_index_ranges(data, header_size)
    top_dict_ranges, _ = cff_index_ranges(data, cursor)
    if len(top_dict_ranges) != 1:
        raise ValueError("expected one CFF Top DICT")
    start, end = top_dict_ranges[0]
    top_dict = bytearray(data[start:end])
    operands: list[tuple[int, int]] = []
    pos = 0
    while pos < len(top_dict):
        byte = top_dict[pos]
        if byte <= 21:
            if byte == 12:
                operator = 0x0C00 | top_dict[pos + 1]
                operator_length = 2
            else:
                operator = byte
                operator_length = 1
            if operator == 15:
                if charset_offset is not None:
                    if len(operands) != 1:
                        raise ValueError("CFF charset does not have one operand")
                    operand_start, operand_end = operands[0]
                    encoded = encode_cff_dict_integer(charset_offset)
                    operand_width = operand_end - operand_start
                    if charset_offset == 0 and operand_width == 3:
                        # Keep the source's two-byte integer width so all
                        # following Top DICT offsets remain stable.
                        encoded = b"\x1c\x00\x00"
                    elif charset_offset == 0 and operand_width == 5:
                        encoded = b"\x1d\x00\x00\x00\x00"
                    if len(encoded) != operand_width:
                        raise ValueError(
                            "CFF charset replacement changed operand width"
                        )
                    top_dict[operand_start:operand_end] = encoded
                if operator_replacement is not None:
                    if operator_replacement > 21:
                        raise ValueError("CFF charset replacement must be one byte")
                    top_dict[pos] = operator_replacement
                data[start:end] = top_dict
                return
            operands.clear()
            pos += operator_length
            continue
        length = cff_dict_number_length(top_dict, pos)
        operands.append((pos, pos + length))
        pos += length
    raise ValueError("CFF Top DICT has no charset operator")


def write_malformed_cid_cff_faces() -> None:
    """Derive CID CFF faces for charset boundary and zero-glyph paths."""
    CID_OUT_DIR.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory() as tmp:
        base = Path(tmp) / "base.otf"
        font = TTFont(CID_SOURCE, recalcTimestamp=False)
        font.recalcTimestamp = False
        font.save(base, reorderTables=True)
        serialized = TTFont(base, recalcTimestamp=False).getTableData("CFF ")
        top_dict = font["CFF "].cff.topDictIndex[0].rawDict
        source_charset_offset = top_dict["charset"]
        charstrings_offset = top_dict["CharStrings"]

        base_data = base.read_bytes()
        num_tables = int.from_bytes(base_data[4:6], "big")
        maxp_payload: bytearray | None = None
        for index in range(num_tables):
            record = 12 + index * 16
            if base_data[record : record + 4] == b"maxp":
                offset = int.from_bytes(base_data[record + 8 : record + 12], "big")
                length = int.from_bytes(base_data[record + 12 : record + 16], "big")
                maxp_payload = bytearray(base_data[offset : offset + length])
                break
        if maxp_payload is None or len(maxp_payload) < 6:
            raise ValueError("CID source has no complete maxp table")
        maxp_payload[4:6] = b"\0\0"
        zero_charstrings = bytearray(serialized)
        zero_charstrings[charstrings_offset : charstrings_offset + 2] = b"\0\0"
        zero_charstrings_path = Path(tmp) / "zero-charstrings.otf"
        replace_sfnt_table(
            base,
            zero_charstrings_path,
            b"CFF ",
            bytes(zero_charstrings),
        )
        replace_sfnt_table(
            zero_charstrings_path,
            CID_OUT_DIR / "ot-cff-cid-keyed-zero-glyph.otf",
            b"maxp",
            bytes(maxp_payload),
        )

        missing_charset = bytearray(serialized)
        patch_cff_charset_top_dict(missing_charset, operator_replacement=14)
        replace_sfnt_table(
            base,
            CID_OUT_DIR / "ot-cff-cid-keyed-missing-charset.otf",
            b"CFF ",
            bytes(missing_charset),
        )

        predefined_charset = bytearray(serialized)
        patch_cff_charset_top_dict(predefined_charset, charset_offset=0)
        replace_sfnt_table(
            base,
            CID_OUT_DIR / "ot-cff-cid-keyed-predefined-charset.otf",
            b"CFF ",
            bytes(predefined_charset),
        )

        unsupported_format = bytearray(serialized)
        unsupported_format[source_charset_offset] = 3
        replace_sfnt_table(
            base,
            CID_OUT_DIR / "ot-cff-cid-keyed-unsupported-charset-format.otf",
            b"CFF ",
            bytes(unsupported_format),
        )

        truncated_range = bytearray(serialized)
        # Place the CFF table at the end of the SFNT and align its payload so
        # the C reader's stream limit falls immediately after the malformed
        # format-1 prefix. The Rust parser is intentionally bounded by the
        # CFF table length, while FreeType reads this public stream boundary.
        truncated_range.extend(b"\0" * ((4 - len(truncated_range) % 4) % 4))
        truncated_offset = len(truncated_range) - 3
        patch_cff_charset_top_dict(truncated_range, charset_offset=truncated_offset)
        truncated_range[truncated_offset:] = b"\x01\x00\x01"
        relocate_sfnt_table_to_end(
            base,
            CID_OUT_DIR / "ot-cff-cid-keyed-truncated-charset-range.otf",
            b"CFF ",
            bytes(truncated_range),
        )


def cff_index_ranges(data: bytes | bytearray, pos: int) -> tuple[list[tuple[int, int]], int]:
    """Return object byte ranges and the byte after one CFF INDEX."""
    count = int.from_bytes(data[pos : pos + 2], "big")
    if count == 0:
        return [], pos + 2
    off_size = data[pos + 2]
    if not 1 <= off_size <= 4:
        raise ValueError("invalid CFF INDEX offSize")
    offset_pos = pos + 3
    offsets = [
        int.from_bytes(data[offset_pos + index * off_size : offset_pos + (index + 1) * off_size], "big")
        for index in range(count + 1)
    ]
    object_base = offset_pos + (count + 1) * off_size
    ranges = [
        (object_base + offsets[index] - 1, object_base + offsets[index + 1] - 1)
        for index in range(count)
    ]
    return ranges, object_base + offsets[-1] - 1


def cff_dict_number_length(data: bytes | bytearray, pos: int) -> int:
    """Return the encoded length of one CFF DICT number."""
    byte = data[pos]
    if byte == 28:
        return 3
    if byte in {29, 255}:
        return 5
    if byte == 30:
        cursor = pos + 1
        while cursor < len(data):
            packed = data[cursor]
            cursor += 1
            if packed & 0x0F == 0x0F or packed >> 4 == 0x0F:
                return cursor - pos
        raise ValueError("unterminated CFF real number")
    if 32 <= byte <= 246:
        return 1
    if 247 <= byte <= 254:
        return 2
    raise ValueError(f"invalid CFF DICT number byte {byte}")


def encode_cff_dict_integer(value: int) -> bytes:
    """Encode an integer using the CFF DICT number representation."""
    if -107 <= value <= 107:
        return bytes([value + 139])
    if 108 <= value <= 1131:
        value -= 108
        return bytes([(value // 256) + 247, value % 256])
    if -1131 <= value <= -108:
        value = -value - 108
        return bytes([(value // 256) + 251, value % 256])
    if -32768 <= value <= 32767:
        return b"\x1c" + value.to_bytes(2, "big", signed=True)
    return b"\x1d" + value.to_bytes(4, "big", signed=True)


def decode_cff_dict_integer(data: bytes | bytearray, pos: int) -> tuple[int, int]:
    """Decode one integer DICT operand and return its value and end offset."""
    byte = data[pos]
    if 32 <= byte <= 246:
        return byte - 139, pos + 1
    if 247 <= byte <= 250:
        return (byte - 247) * 256 + data[pos + 1] + 108, pos + 2
    if 251 <= byte <= 254:
        return -(byte - 251) * 256 - data[pos + 1] - 108, pos + 2
    if byte == 28:
        return int.from_bytes(data[pos + 1 : pos + 3], "big", signed=True), pos + 3
    if byte == 29:
        return int.from_bytes(data[pos + 1 : pos + 5], "big", signed=True), pos + 5
    raise ValueError(f"expected integer CFF DICT operand at byte {pos}")


def cff_top_dict_range(data: bytes | bytearray) -> tuple[int, int]:
    """Return the sole CFF Top DICT object's byte range."""
    header_size = data[2]
    _, cursor = cff_index_ranges(data, header_size)
    top_dict_ranges, _ = cff_index_ranges(data, cursor)
    if len(top_dict_ranges) != 1:
        raise ValueError("expected one CFF Top DICT")
    return top_dict_ranges[0]


def cff_private_dict_range(data: bytes | bytearray) -> tuple[int, int]:
    """Find the raw Private DICT range named by the Top DICT."""
    start, end = cff_top_dict_range(data)
    top_dict = data[start:end]
    operands: list[tuple[int, int]] = []
    pos = 0
    while pos < len(top_dict):
        byte = top_dict[pos]
        if byte <= 21:
            if byte == 12:
                operator = 0x0C00 | top_dict[pos + 1]
                operator_length = 2
            else:
                operator = byte
                operator_length = 1
            if operator == 18:
                if len(operands) < 2:
                    raise ValueError("CFF Private operator has too few operands")
                size, _ = decode_cff_dict_integer(top_dict, operands[0][0])
                offset, _ = decode_cff_dict_integer(top_dict, operands[1][0])
                if size < 0 or offset < 0:
                    raise ValueError("CFF Private operands are negative")
                return offset, size
            operands.clear()
            pos += operator_length
            continue
        length = cff_dict_number_length(top_dict, pos)
        operands.append((pos, pos + length))
        pos += length
    raise ValueError("CFF Top DICT has no Private operator")


def _replace_cff_dict_operand(
    top_dict: bytearray, operand: tuple[int, int], value: int
) -> None:
    """Replace an integer operand, retaining its byte width if necessary."""
    start, end = operand
    encoded = encode_cff_dict_integer(value)
    width = end - start
    if len(encoded) > width:
        raise ValueError("CFF DICT replacement changed operand width")
    # A short zero encoding is intentionally padded with another zero number.
    # The CFF Private callback consumes the first two operands and ignores the
    # extra value, allowing a same-size `(size=1, offset=0)` boundary mutation.
    top_dict[start:end] = encoded + b"\x8b" * (width - len(encoded))


def patch_cff_private_top_dict(
    data: bytearray,
    *,
    size: int | None = None,
    offset: int | None = None,
    operator_replacement: int | None = None,
) -> None:
    """Patch a CFF Top DICT Private boundary without changing table size."""
    start, end = cff_top_dict_range(data)
    top_dict = bytearray(data[start:end])
    operands: list[tuple[int, int]] = []
    pos = 0
    while pos < len(top_dict):
        byte = top_dict[pos]
        if byte <= 21:
            if byte == 12:
                operator = 0x0C00 | top_dict[pos + 1]
                operator_length = 2
            else:
                operator = byte
                operator_length = 1
            if operator == 18:
                if len(operands) < 2:
                    raise ValueError("CFF Private operator has too few operands")
                if size is not None:
                    _replace_cff_dict_operand(top_dict, operands[0], size)
                if offset is not None:
                    _replace_cff_dict_operand(top_dict, operands[1], offset)
                if operator_replacement is not None:
                    if not 0 <= operator_replacement <= 21:
                        raise ValueError("CFF operator replacement must be one byte")
                    top_dict[pos] = operator_replacement
                data[start:end] = top_dict
                return
            operands.clear()
            pos += operator_length
            continue
        length = cff_dict_number_length(top_dict, pos)
        operands.append((pos, pos + length))
        pos += length
    raise ValueError("CFF Top DICT has no Private operator")


def patch_cff_private_initial_random_seed_missing(data: bytearray) -> None:
    """Replace a valid Private payload's first field with operand-less seed."""
    offset, size = cff_private_dict_range(data)
    if size < 2:
        raise ValueError("CFF Private payload is too short for escaped operator")
    # `12 19` is the Private DICT initialRandomSeed operator.  FreeType's
    # cff_parser_run reports Stack_Underflow when it sees it with no operand;
    # the remaining byte is deliberately left in place so the SFNT size and
    # all Top DICT offsets remain stable.
    data[offset : offset + 2] = b"\x0c\x13"


def patch_cff_private_payload(data: bytearray, private_payload: bytes) -> None:
    """Replace a Private DICT payload without changing its declared range."""
    offset, size = cff_private_dict_range(data)
    if len(private_payload) > size:
        raise ValueError("CFF Private payload is larger than its declared range")
    data[offset : offset + size] = private_payload + b"\0" * (
        size - len(private_payload)
    )


def patch_cff_ros_sids(
    data: bytearray,
    *,
    registry_sid: int | None = None,
    ordering_sid: int | None = None,
) -> None:
    """Patch ROS SID operands without changing the surrounding CFF layout."""
    header_size = data[2]
    _, cursor = cff_index_ranges(data, header_size)
    top_dict_ranges, _ = cff_index_ranges(data, cursor)
    if len(top_dict_ranges) != 1:
        raise ValueError("expected one CFF Top DICT")
    start, end = top_dict_ranges[0]
    top_dict = bytearray(data[start:end])
    operands: list[tuple[int, int]] = []
    pos = 0
    while pos < len(top_dict):
        byte = top_dict[pos]
        if byte <= 21:
            if byte == 12:
                operator = 0x0C00 | top_dict[pos + 1]
                operator_length = 2
            else:
                operator = byte
                operator_length = 1
            if operator == 0x0C1E:
                if len(operands) != 3:
                    raise ValueError("CFF ROS does not have three operands")
                replacements = (registry_sid, ordering_sid, None)
                for (operand_start, operand_end), replacement in zip(operands, replacements):
                    if replacement is None:
                        continue
                    encoded = encode_cff_dict_integer(replacement)
                    if len(encoded) != operand_end - operand_start:
                        raise ValueError("CFF ROS SID replacement changed operand width")
                    top_dict[operand_start:operand_end] = encoded
                data[start:end] = top_dict
                return
            operands.clear()
            pos += operator_length
            continue
        length = cff_dict_number_length(top_dict, pos)
        operands.append((pos, pos + length))
        pos += length
    raise ValueError("CFF Top DICT has no ROS operator")


def patch_cff_ros_absent_registry(data: bytearray) -> None:
    """Patch ROS to the CFF absent-registry sentinel without resizing CFF."""
    header_size = data[2]
    _, cursor = cff_index_ranges(data, header_size)
    top_dict_ranges, _ = cff_index_ranges(data, cursor)
    if len(top_dict_ranges) != 1:
        raise ValueError("expected one CFF Top DICT")
    start, end = top_dict_ranges[0]
    top_dict = bytearray(data[start:end])
    ros_registry: tuple[int, int] | None = None
    removable_cid_version: tuple[int, int] | None = None
    operands: list[tuple[int, int]] = []
    pos = 0
    while pos < len(top_dict):
        byte = top_dict[pos]
        if byte <= 21:
            if byte == 12:
                operator = 0x0C00 | top_dict[pos + 1]
                operator_length = 2
            else:
                operator = byte
                operator_length = 1
            if operator == 0x0C1E:
                if len(operands) != 3:
                    raise ValueError("CFF ROS does not have three operands")
                ros_registry = operands[0]
            elif operator == 0x0C1F:
                if len(operands) != 1:
                    raise ValueError("CFF CIDFontVersion does not have one operand")
                removable_cid_version = (operands[0][0], pos + operator_length)
            operands.clear()
            pos += operator_length
            continue
        length = cff_dict_number_length(top_dict, pos)
        operands.append((pos, pos + length))
        pos += length
    if ros_registry is None or removable_cid_version is None:
        raise ValueError("CFF Top DICT lacks ROS or CIDFontVersion")
    remove_start, remove_end = removable_cid_version
    if remove_start < ros_registry[1]:
        raise ValueError("CFF CIDFontVersion precedes ROS")
    top_dict[remove_start:remove_end] = b""
    registry_start, registry_end = ros_registry
    top_dict[registry_start:registry_end] = encode_cff_dict_integer(0xFFFF)
    if len(top_dict) != end - start:
        raise ValueError("CFF ROS sentinel patch changed Top DICT length")
    data[start:end] = top_dict


def write_cid_cff_unresolved_ordering() -> None:
    """Derive a CID face whose ROS ordering SID is absent from String INDEX."""
    out = CID_OUT_DIR / "ot-cff-cid-keyed-unresolved-ordering.otf"
    with TemporaryDirectory() as tmp:
        base = Path(tmp) / "base.otf"
        font = TTFont(CID_SOURCE, recalcTimestamp=False)
        font.recalcTimestamp = False
        font.save(base, reorderTables=True)
        serialized = TTFont(base, recalcTimestamp=False).getTableData("CFF ")
        cff = bytearray(serialized)
        # SID 800 is encoded in the same two bytes as the source's custom
        # ordering SID but is beyond this face's String INDEX.  Pinned
        # FreeType therefore returns a successful CID service result with a
        # null ordering pointer rather than rejecting the face.
        patch_cff_ros_sids(cff, ordering_sid=800)
        replace_sfnt_table(base, out, b"CFF ", bytes(cff))


def write_cid_cff_absent_registry() -> None:
    """Derive a CFF face with the absent-CID registry sentinel."""
    out = CID_OUT_DIR / "ot-cff-non-cid-sentinel-registry.otf"
    with TemporaryDirectory() as tmp:
        base = Path(tmp) / "base.otf"
        font = TTFont(CID_SOURCE, recalcTimestamp=False)
        font.recalcTimestamp = False
        font.save(base, reorderTables=True)
        serialized = TTFont(base, recalcTimestamp=False).getTableData("CFF ")
        cff = bytearray(serialized)
        # FreeType's CFF driver uses 0xFFFF as the explicit non-CID sentinel.
        # Preserve the otherwise valid CFF table and its offsets while
        # replacing the source's shorter registry operand.  CIDFontVersion is
        # optional and its default is valid, so its three-byte dictionary
        # entry makes room without shifting the rest of the CFF table.
        patch_cff_ros_absent_registry(cff)
        replace_sfnt_table(base, out, b"CFF ", bytes(cff))


def write_cid_cff_standard_ros() -> None:
    """Derive a CID face whose ROS uses standard CFF string SIDs 389/390."""
    out = CID_OUT_DIR / "ot-cff-cid-keyed-standard-ros.otf"
    font = TTFont(CID_SOURCE, recalcTimestamp=False)
    top_dict = font["CFF "].cff.topDictIndex[0]
    top_dict.ROS = ("Roman", "Semibold", 0)
    font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def write_cid_cff_standard_ros_weight_names() -> None:
    """Derive a CID face whose ROS uses standard SIDs 383/384."""
    out = CID_OUT_DIR / "ot-cff-cid-keyed-standard-ros-weight-names.otf"
    font = TTFont(CID_SOURCE, recalcTimestamp=False)
    top_dict = font["CFF "].cff.topDictIndex[0]
    top_dict.ROS = ("Black", "Bold", 0)
    font.recalcTimestamp = False
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def add_vertical_metrics(font: TTFont) -> None:
    glyph_order = font.getGlyphOrder()
    vmtx = newTable("vmtx")
    vmtx.metrics = {name: (880, 120) for name in glyph_order}
    font["vmtx"] = vmtx

    vhea = newTable("vhea")
    vhea.tableVersion = 0x00010000
    vhea.ascent = 760
    vhea.descent = -120
    vhea.lineGap = 0
    vhea.advanceHeightMax = 880
    vhea.minTopSideBearing = 120
    vhea.minBottomSideBearing = 0
    vhea.yMaxExtent = 880
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


def write_pure_cff_cubic_vmtx() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-cubic-vmtx.otf"
    if out.exists() or out.is_symlink():
        out.unlink()
    build_cubic_cff(
        out,
        with_vertical_metrics=True,
        include_append_only_glyphs=False,
    )


def empty_program_table(tag: str):
    table = newTable(tag)
    table.program = Program()
    table.program.fromBytecode([])
    return table


def write_pure_cff_empty_tt_programs() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "pure-cff-empty-tt-programs.otf"
    with TemporaryDirectory() as tmp:
        cff_path = Path(tmp) / "pure-cff-cubic.otf"
        build_cubic_cff(cff_path, include_append_only_glyphs=False)
        font = TTFont(cff_path, recalcTimestamp=False)
        # This deliberately odd OTTO face keeps CFF outlines while carrying
        # empty TrueType program tables.  It exercises the scaler's public CFF
        # metrics route without giving the TrueType VM any executable work.
        font["fpgm"] = empty_program_table("fpgm")
        font["prep"] = empty_program_table("prep")
        cvt = newTable("cvt ")
        cvt.values = array("h")
        font["cvt "] = cvt
        font["head"].created = FIXED_HEAD_TIME
        font["head"].modified = FIXED_HEAD_TIME
        font.recalcTimestamp = False
        if out.exists() or out.is_symlink():
            out.unlink()
        font.save(out, reorderTables=True)


def sfnt_checksum(data: bytes) -> int:
    padded = data + b"\0" * ((4 - len(data) % 4) % 4)
    return sum(int.from_bytes(padded[i : i + 4], "big") for i in range(0, len(padded), 4)) & 0xFFFFFFFF


def sfnt_table_payload(source: Path, tag: bytes) -> bytes:
    """Read one raw SFNT table without asking FontTools to recompile it."""
    data = source.read_bytes()
    num_tables = int.from_bytes(data[4:6], "big")
    for index in range(num_tables):
        record = 12 + index * 16
        if bytes(data[record : record + 4]) != tag:
            continue
        offset = int.from_bytes(data[record + 8 : record + 12], "big")
        length = int.from_bytes(data[record + 12 : record + 16], "big")
        return bytes(data[offset : offset + length])
    raise ValueError(f"missing table {tag!r} in {source}")


def replace_sfnt_table(source: Path, dest: Path, tag: bytes, payload: bytes) -> None:
    data = bytearray(source.read_bytes())
    num_tables = int.from_bytes(data[4:6], "big")
    for index in range(num_tables):
        record = 12 + index * 16
        if bytes(data[record : record + 4]) != tag:
            continue
        offset = int.from_bytes(data[record + 8 : record + 12], "big")
        old_length = int.from_bytes(data[record + 12 : record + 16], "big")
        if len(payload) > old_length:
            raise ValueError(f"{tag!r} replacement is larger than source table")
        data[offset : offset + len(payload)] = payload
        data[offset + len(payload) : offset + old_length] = b"\0" * (old_length - len(payload))
        data[record + 4 : record + 8] = sfnt_checksum(payload).to_bytes(4, "big")
        data[record + 12 : record + 16] = len(payload).to_bytes(4, "big")
        if dest.exists() or dest.is_symlink():
            dest.unlink()
        dest.write_bytes(data)
        return
    raise ValueError(f"missing table {tag!r} in {source}")


def relocate_sfnt_table_to_end(
    source: Path, dest: Path, tag: bytes, payload: bytes
) -> None:
    """Rewrite an SFNT so one replacement table is the final file payload."""
    data = bytearray(source.read_bytes())
    num_tables = int.from_bytes(data[4:6], "big")
    records: list[tuple[bytes, bytes]] = []
    for index in range(num_tables):
        record = 12 + index * 16
        table_tag = bytes(data[record : record + 4])
        offset = int.from_bytes(data[record + 8 : record + 12], "big")
        length = int.from_bytes(data[record + 12 : record + 16], "big")
        records.append((table_tag, bytes(data[offset : offset + length])))
    if not any(table_tag == tag for table_tag, _ in records):
        raise ValueError(f"missing table {tag!r} in {source}")

    payloads = dict(records)
    payloads[tag] = payload
    table_indexes = {table_tag: index for index, (table_tag, _) in enumerate(records)}
    order = [table_tag for table_tag, _ in records if table_tag != tag] + [tag]
    output = bytearray(data[:12] + b"\0" * (16 * num_tables))
    for table_tag in order:
        while len(output) % 4:
            output.append(0)
        offset = len(output)
        table_payload = payloads[table_tag]
        output.extend(table_payload)
        record = 12 + table_indexes[table_tag] * 16
        output[record : record + 4] = table_tag
        output[record + 4 : record + 8] = sfnt_checksum(table_payload).to_bytes(4, "big")
        output[record + 8 : record + 12] = offset.to_bytes(4, "big")
        output[record + 12 : record + 16] = len(table_payload).to_bytes(4, "big")
    while len(output) % 4:
        output.append(0)
    if dest.exists() or dest.is_symlink():
        dest.unlink()
    dest.write_bytes(output)


def cff_index(objects: list[bytes]) -> bytes:
    if not objects:
        return b"\0\0"
    offsets = [1]
    cursor = 1
    for item in objects:
        cursor += len(item)
        offsets.append(cursor)
    return (
        len(objects).to_bytes(2, "big")
        + b"\x01"
        + bytes(offsets)
        + b"".join(objects)
    )


def malformed_cff_payload(kind: str) -> bytes:
    header = b"\x01\x00\x04\x04"

    def minimal_payload(top_dict: bytes) -> bytes:
        return header + cff_index([]) + cff_index([top_dict]) + cff_index([]) + cff_index([])

    def with_charstrings(charstrings_index: bytes) -> bytes:
        # Keep the CharStrings operand self-consistent while varying only the
        # INDEX boundary under test.  The small synthetic top dictionary stays
        # within the one-byte DICT integer encoding.
        top_dict = b"\x8b\x11"
        for _ in range(3):
            prefix = (
                header
                + cff_index([])
                + cff_index([top_dict])
                + cff_index([])
                + cff_index([])
            )
            top_dict = encode_cff_dict_integer(len(prefix)) + b"\x11"
        return (
            header
            + cff_index([])
            + cff_index([top_dict])
            + cff_index([])
            + cff_index([])
            + charstrings_index
        )

    if kind == "empty_top_dict_index":
        return header + cff_index([]) + cff_index([]) + cff_index([]) + cff_index([])
    if kind == "strings_index_count_truncated":
        return header + cff_index([]) + cff_index([b"\x8b\x0f"]) + b"\0"
    if kind == "global_subr_index_count_truncated":
        return header + cff_index([]) + cff_index([b"\x8b\x0f"]) + cff_index([]) + b"\0"
    if kind == "charstrings_index_count_truncated":
        return with_charstrings(b"\0")
    if kind == "charstrings_index_offsize_missing":
        return with_charstrings(b"\0\x01")
    if kind == "charstrings_index_offset_truncated":
        return with_charstrings(b"\0\x01\x01\x01")
    if kind == "charstrings_index_offset_underflow":
        return with_charstrings(b"\0\x01\x01\0\0")
    if kind == "charstrings_index_offsets_out_of_order":
        return with_charstrings(b"\0\x01\x01\x02\x01")
    if kind == "charstrings_index_object_overflow":
        return with_charstrings(b"\0\x01\x01\x01\x03")
    if kind == "charstrings_index_empty":
        return with_charstrings(b"\0\0")
    if kind == "top_dict_shortint_valid":
        return minimal_payload(b"\x1c\x00\x01\x0c\x03")
    if kind == "top_dict_longint_valid":
        return minimal_payload(b"\x1d\x00\x00\x00\x01\x0c\x03")
    if kind == "top_dict_positive_valid":
        return minimal_payload(b"\xf7\xff\x0c\x03")
    if kind == "top_dict_negative_valid":
        return minimal_payload(b"\xfb\xff\x0c\x03")
    if kind == "top_dict_fixed_valid":
        return minimal_payload(b"\xff\x00\x01\x00\x00\x0c\x02")
    if kind == "top_dict_weight_operator":
        return minimal_payload(b"\x8b\x04")
    if kind == "top_dict_underline_thickness_operator":
        return minimal_payload(b"\x8b\x0c\x04")
    if kind == "top_dict_string_operators":
        return minimal_payload(b"\x8b\x00\x8b\x01\x8b\x02\x8b\x03\x8b\x04")
    if kind == "top_dict_unknown_operand":
        return minimal_payload(b"\x8b\x0c\x05")

    if kind == "short_header":
        return b"\x01\x00\x04"
    if kind == "invalid_name_index_offsize":
        return header + b"\x00\x01\x00"
    if kind == "name_index_offset_overflow":
        # INDEX count=1 and offSize=4 require two four-byte offsets.  This
        # supplies only the first offset so both FreeType and Rust fail
        # while reading the offset array, not while slicing object bytes.
        return header + b"\x00\x01\x04\x00\x00\x00\x01"
    if kind == "name_index_offsets_out_of_order":
        return header + b"\x00\x01\x01\x02\x01"
    if kind == "escaped_top_dict_op_overflow":
        return minimal_payload(b"\x0C")
    if kind == "escaped_top_dict_op_missing_charstrings":
        # A complete escaped Top DICT operator exercises the two-byte
        # operator path.  It is not CharStrings, so the face is rejected
        # later for the missing required CharStrings offset.
        return minimal_payload(b"\x0C\x00")
    if kind == "charstrings_operand_missing":
        # Top DICT operator 17 is CharStrings.  With no preceding operand,
        # pinned FreeType reports stack underflow while Rust reports the
        # same public face-open failure class.
        return minimal_payload(b"\x11")
    if kind == "charset_operand_missing":
        # Top DICT operator 15 is charset.  Its operand underflow is reported
        # while parsing the dictionary, before the required CharStrings check.
        return minimal_payload(b"\x0F")
    if kind == "ros_operands_missing":
        # ROS is an escaped operator with three required operands.  Keep the
        # dictionary otherwise minimal so the parser stops at that boundary.
        return minimal_payload(b"\x0C\x1E")
    if kind == "top_dict_longint_operand_missing_charstrings":
        # CFF DICT longint operand encoding (`cffparse.c:cff_parse_integer`) is
        # parsed by a normal numeric Top DICT operator (`UnderlinePosition`).
        # The public face-open failure remains the missing required
        # CharStrings offset, but this keeps the longint parser route
        # C-observable.
        return minimal_payload(b"\x1D\x00\x00\x00\x01\x0C\x03")
    if kind == "top_dict_real_operand_missing_charstrings":
        # CFF DICT real operands are legal Top DICT operands.  FreeType parses
        # the BCD real number and later rejects this minimal Top DICT for the
        # absent required CharStrings offset.
        return minimal_payload(b"\x1E\x1A\x5F\x0C\x03")
    if kind == "top_dict_real_exponent_operand_missing_charstrings":
        # Exercise the CFF real-number exponent-sign nibble while preserving
        # the same missing-CharStrings face-open failure boundary.
        return minimal_payload(b"\x1E\x1B\x5F\x0C\x03")
    if kind == "top_dict_real_negative_operand_missing_charstrings":
        # Exercise the CFF real-number negative-sign nibble while preserving
        # the same missing-CharStrings face-open failure boundary.
        return minimal_payload(b"\x1E\xE1\x5F\x0C\x03")
    if kind == "top_dict_real_reserved_nibble_missing_charstrings":
        # The 0xD BCD nibble is reserved.  FreeType ignores it while parsing
        # the legal real operand, then reaches the same missing-CharStrings
        # face-open boundary.
        return minimal_payload(b"\x1E\xD1\x5F\x0C\x03")
    if kind == "top_dict_real_negative_overflow_missing_charstrings":
        # The BCD payload spells -40000, driving cff_parse_fixed's negative
        # saturation path while preserving the missing-CharStrings boundary.
        return minimal_payload(b"\x1E\xE4\x00\x00\x0F\x0C\x03")
    if kind == "top_dict_positive_operand_missing_charstrings":
        return minimal_payload(b"\xF7\x00\x0C\x03")
    if kind == "top_dict_negative_operand_missing_charstrings":
        return minimal_payload(b"\xFB\x00\x0C\x03")
    if kind == "top_dict_positive_operand_overflow":
        # The positive two-byte CFF DICT number is truncated immediately
        # after its operator byte.  FreeType's cff_parse_integer path keeps
        # this at the public face-open error boundary.
        return minimal_payload(b"\xF7")
    if kind == "top_dict_negative_operand_overflow":
        # The negative two-byte CFF DICT number is truncated immediately
        # after its operator byte.
        return minimal_payload(b"\xFB")
    if kind == "top_dict_integer_clamps_missing_charstrings":
        # Exercise both signed 15-bit overflow clamps used by
        # cff_parse_fixed for integer operands, then send the values through
        # the public integer-valued UnderlinePosition field.
        return minimal_payload(
            b"\x1D\x00\x00\x9C\x40\x0C\x03"
            b"\x1D\xFF\xFF\x63\xC0\x0C\x03"
        )
    if kind == "top_dict_invalid_number":
        # Byte 31 is neither a valid CFF DICT operator nor a valid DICT number.
        # Pinned FreeType rejects it during Top DICT parsing.
        return minimal_payload(b"\x1F")
    raise ValueError(f"unknown malformed CFF fixture kind {kind}")

def write_malformed_cff_faces() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory() as tmp:
        base = Path(tmp) / "base.otf"
        # Keep malformed face-open controls independent of the append-only
        # glyph-load matrix so adding a glyph does not perturb unrelated
        # SFNT metadata in every derived error fixture.
        build_cubic_cff(base, include_append_only_glyphs=False)
        for kind in [
            "empty_top_dict_index",
            "strings_index_count_truncated",
            "global_subr_index_count_truncated",
            "charstrings_index_count_truncated",
            "charstrings_index_offsize_missing",
            "charstrings_index_offset_truncated",
            "charstrings_index_offset_underflow",
            "charstrings_index_offsets_out_of_order",
            "charstrings_index_object_overflow",
            "charstrings_index_empty",
            "top_dict_shortint_valid",
            "top_dict_longint_valid",
            "top_dict_positive_valid",
            "top_dict_negative_valid",
            "top_dict_fixed_valid",
            "top_dict_weight_operator",
            "top_dict_underline_thickness_operator",
            "top_dict_string_operators",
            "top_dict_unknown_operand",
            "short_header",
            "invalid_name_index_offsize",
            "name_index_offset_overflow",
            "name_index_offsets_out_of_order",
            "escaped_top_dict_op_overflow",
            "escaped_top_dict_op_missing_charstrings",
            "charstrings_operand_missing",
            "charset_operand_missing",
            "ros_operands_missing",
            "top_dict_longint_operand_missing_charstrings",
            "top_dict_real_operand_missing_charstrings",
            "top_dict_real_exponent_operand_missing_charstrings",
            "top_dict_real_negative_operand_missing_charstrings",
            "top_dict_real_reserved_nibble_missing_charstrings",
            "top_dict_real_negative_overflow_missing_charstrings",
            "top_dict_positive_operand_missing_charstrings",
            "top_dict_negative_operand_missing_charstrings",
            "top_dict_positive_operand_overflow",
            "top_dict_negative_operand_overflow",
            "top_dict_integer_clamps_missing_charstrings",
            "top_dict_invalid_number",
        ]:
            replace_sfnt_table(
                base,
                OUT_DIR / f"malformed-{kind.replace('_', '-')}.otf",
                b"CFF ",
                malformed_cff_payload(kind),
            )
def malformed_cff2_payload(kind: str) -> bytes:
    """Return a compact CFF2 table that fails at one parser boundary."""

    def with_charstrings(charstrings_index: bytes, global_index: bytes = b"\0\0\0\0") -> bytes:
        top_dict = b"\x94\x11"
        for _ in range(3):
            top_dict_end = 5 + len(top_dict)
            charstrings_offset = top_dict_end + len(global_index)
            top_dict = encode_cff_dict_integer(charstrings_offset) + b"\x11"
        header = b"\x02\x00\x05" + len(top_dict).to_bytes(2, "big")
        return header + top_dict + global_index + charstrings_index

    if kind == "charstrings_index_count_truncated":
        return with_charstrings(b"\0\0")
    if kind == "charstrings_index_offsize_missing":
        return with_charstrings(b"\0\0\0\x01")
    if kind == "charstrings_index_offset_truncated":
        return with_charstrings(b"\0\0\0\x01\x01\x01")
    if kind == "charstrings_index_offset_underflow":
        return with_charstrings(b"\0\0\0\x01\0\0")
    if kind == "charstrings_index_offsets_out_of_order":
        return with_charstrings(b"\0\0\0\x01\x02\x01")
    if kind == "charstrings_index_object_overflow":
        return with_charstrings(b"\0\0\0\x01\x01\x01\x03")
    if kind == "charstrings_index_empty":
        return with_charstrings(b"\0\0\0\0")
    if kind == "charstrings_index_one":
        return with_charstrings(b"\0\0\0\x01\x01\x01\x02\x0e")
    if kind == "global_index_one_object":
        return with_charstrings(
            b"\0\0\0\0",
            global_index=b"\0\0\0\x01\x01\x01\x02\0",
        )
    if kind == "short_header":
        return b"\x02\x00\x05\x00"
    if kind == "wrong_major_version":
        return b"\x01\x00\x05\x00\x00"
    if kind == "invalid_header_size":
        return b"\x02\x00\x04\x00\x00"
    if kind == "top_dict_truncated":
        return b"\x02\x00\x05\x00\x04"
    if kind == "missing_charstrings":
        return b"\x02\x00\x05\x00\x00"

    if kind == "top_dict_fixed_operand_missing_charstrings":
        # CFF2 DICTs accept the 16.16 fixed-number encoding (byte 255).  Keep
        # the Top DICT otherwise minimal so the face-open result remains the
        # required-CharStrings failure after the fixed operand is parsed.
        top_dict = b"\xff\x00\x00\x00\x00\x0c\x03"
        return b"\x02\x00\x05\x00\x07" + top_dict + b"\x00\x00\x00\x00"

    # CFF DICT integer 9 followed by CharStrings (operator 17).  The
    # resulting top-dict end is byte 7, so the payload after it is the CFF2
    # Global Subr INDEX probe.  Each malformed tail stops at a distinct
    # read_cff2_index validation boundary before any glyph is loaded.
    top_dict = b"\x94\x11"

    def with_global_index(tail: bytes) -> bytes:
        return b"\x02\x00\x05\x00\x02" + top_dict + tail

    if kind == "global_index_count_truncated":
        return with_global_index(b"")
    if kind == "global_index_invalid_offsize":
        return with_global_index(b"\x00\x00\x00\x01\x00")
    if kind == "global_index_offset_truncated":
        return with_global_index(b"\x00\x00\x00\x01\x01\x01")
    if kind == "global_index_offset_underflow":
        return with_global_index(b"\x00\x00\x00\x01\x01\x00\x00")
    if kind == "global_index_offsets_out_of_order":
        return with_global_index(b"\x00\x00\x00\x01\x01\x02\x01")
    if kind == "global_index_object_overflow":
        return with_global_index(b"\x00\x00\x00\x01\x01\x01\x03")
    raise ValueError(f"unknown malformed CFF2 fixture kind {kind}")


def write_malformed_cff2_faces() -> None:
    CFF2_OUT_DIR.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory() as tmp:
        base = Path(tmp) / "base-cff2.otf"
        build_cff2(base)
        for kind in [
            "charstrings_index_count_truncated",
            "charstrings_index_offsize_missing",
            "charstrings_index_offset_truncated",
            "charstrings_index_offset_underflow",
            "charstrings_index_offsets_out_of_order",
            "charstrings_index_object_overflow",
            "charstrings_index_empty",
            "charstrings_index_one",
            "global_index_one_object",
            "short_header",
            "wrong_major_version",
            "invalid_header_size",
            "top_dict_truncated",
            "missing_charstrings",
            "top_dict_fixed_operand_missing_charstrings",
            "global_index_count_truncated",
            "global_index_invalid_offsize",
            "global_index_offset_truncated",
            "global_index_offset_underflow",
            "global_index_offsets_out_of_order",
            "global_index_object_overflow",
        ]:
            replace_sfnt_table(
                base,
                CFF2_OUT_DIR / f"malformed-{kind.replace('_', '-')}.otf",
                b"CFF2",
                malformed_cff2_payload(kind),
            )


def main() -> None:
    write_hybrid_otto_face_info()
    INPUT_OUT_DIR.mkdir(parents=True, exist_ok=True)
    build_cff(INPUT_OUT_DIR / "fontinfo-populated.otf")
    build_cff2(CFF2_OUT_DIR / "fontinfo-invalid-argument.otf")
    write_pure_cff_cubic()
    write_pure_cff_random()
    write_pure_cff_random_private()
    write_pure_cff_random_private_parser_controls()
    write_pure_cff_random_private_edge_controls()
    write_pure_cff_random_global_subr_error()
    write_pure_cff_below_baseline_no_vmtx()
    write_pure_cff_baseline_touch_no_vmtx()
    write_pure_cff_cubic_last_delta()
    write_pure_cff_bbox_extrema()
    write_pure_cff_cubic_peak_shifts()
    write_cid_cff_format2()
    write_cid_cff_charset_variants()
    write_cid_cff_single_glyph()
    write_malformed_cid_cff_faces()
    write_cid_cff_unresolved_ordering()
    write_cid_cff_absent_registry()
    write_cid_cff_standard_ros()
    write_cid_cff_standard_ros_weight_names()
    write_pure_cff_cubic_vmtx()
    write_pure_cff_empty_tt_programs()
    write_malformed_cff_faces()
    write_malformed_cff2_faces()


if __name__ == "__main__":
    main()
