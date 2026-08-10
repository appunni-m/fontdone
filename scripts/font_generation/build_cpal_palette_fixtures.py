#!/usr/bin/env python3
"""Generate deterministic CPAL/COLR fixtures for public color API parity."""

from __future__ import annotations

from pathlib import Path

from fontTools.colorLib.builder import buildCOLR
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables.C_O_L_R_ import LayerRecord
from fontTools.ttLib.tables.C_P_A_L_ import Color
from fontTools.ttLib.tables._f_v_a_r import Axis
from fontTools.ttLib.tables import otTables as ot
from fontTools.varLib import builder as var_builder


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
SOURCE_FONT = FIXTURE_ROOT / "input" / "fonts" / "DejaVuSans.ttf"
OUTPUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "color"
COLOR_OUTPUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "color"


PALETTES = [
    [
        Color(0x10, 0x20, 0x30, 0x40),
        Color(0x50, 0x60, 0x70, 0x80),
        Color(0x90, 0xA0, 0xB0, 0xC0),
    ],
    [
        Color(0x01, 0x02, 0x03, 0x04),
        Color(0x11, 0x12, 0x13, 0x14),
        Color(0x21, 0x22, 0x23, 0x24),
    ],
]

CPAL_FIXTURE_HEAD_MODIFIED = 3867487964


def build_cpal_font(path: Path) -> None:
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    # Preserve the already-tracked deterministic timestamp for these existing
    # CPAL fixtures.  Their CPAL data is stable; changing only `head.modified`
    # and checksum adjustment creates noisy binary fixture churn.
    font["head"].modified = CPAL_FIXTURE_HEAD_MODIFIED
    cpal = newTable("CPAL")
    cpal.version = 1
    cpal.numPaletteEntries = len(PALETTES[0])
    cpal.palettes = PALETTES
    # FreeType exposes these through FT_Palette_Data as FT_UShort arrays.
    cpal.paletteTypes = [0x0001, 0x0002]
    cpal.paletteLabels = [256, cpal.NO_NAME_ID]
    cpal.paletteEntryLabels = [257, 258, cpal.NO_NAME_ID]
    font["CPAL"] = cpal
    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_cpal_zero_entry_font(path: Path) -> None:
    """Build a valid CPAL face whose palettes contain no color entries."""
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    font["head"].modified = CPAL_FIXTURE_HEAD_MODIFIED
    cpal = newTable("CPAL")
    cpal.version = 1
    cpal.numPaletteEntries = 0
    cpal.palettes = [[], []]
    cpal.paletteTypes = [0x0001, 0x0002]
    cpal.paletteLabels = [256, cpal.NO_NAME_ID]
    cpal.paletteEntryLabels = []
    font["CPAL"] = cpal
    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_cpal_variant(path: Path, variant: str) -> None:
    """Build a deterministic CPAL parser control from the canonical v1 face.

    FreeType treats malformed CPAL as an optional-table load failure, so the
    surrounding SFNT remains openable.  The valid no-metadata variant also
    records the CPAL v1 representation where all three optional offsets are
    zero and therefore all three public metadata pointers remain NULL.
    """
    source = OUTPUT_DIR / "cpal-palettes-names-flags.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"CPAL")
    if table is None or table.length < 12:
        raise RuntimeError(f"canonical CPAL fixture has no usable table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    table_length = table.length
    num_palettes = int.from_bytes(data[table_offset + 4 : table_offset + 6], "big")
    extensions_offset = 12 + num_palettes * 2
    if extensions_offset + 12 > table_length:
        raise RuntimeError("canonical CPAL fixture has no complete v1 extension header")

    record_start = None
    num_tables = int.from_bytes(data[4:6], "big")
    for index in range(num_tables):
        candidate = 12 + index * 16
        if data[candidate : candidate + 4] == b"CPAL":
            record_start = candidate
            break
    if record_start is None:
        raise RuntimeError(f"canonical CPAL fixture has no directory record: {source}")

    if variant == "no_optional_metadata":
        data[table_offset + extensions_offset : table_offset + extensions_offset + 12] = b"\0" * 12
    elif variant == "truncated_indices":
        data[record_start + 12 : record_start + 16] = (12).to_bytes(4, "big")
    elif variant in {"truncated_types", "truncated_labels", "truncated_entry_labels"}:
        field_offset = {
            "truncated_types": 0,
            "truncated_labels": 4,
            "truncated_entry_labels": 8,
        }[variant]
        data[table_offset + extensions_offset + field_offset : table_offset + extensions_offset + field_offset + 4] = (
            table_length - 1
        ).to_bytes(4, "big")
    else:
        raise ValueError(f"unknown CPAL variant: {variant}")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v0_layers_font(path: Path) -> None:
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)

    colr = newTable("COLR")
    colr.version = 0
    layers = []
    for glyph_name, color_id in (("B", 0), ("C", 1), ("D", 2)):
        layer = LayerRecord()
        layer.name = glyph_name
        layer.colorID = color_id
        layers.append(layer)
    colr.ColorLayers = {"A": layers}
    font["COLR"] = colr

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 3
    cpal.palettes = [
        [
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0xFF),
            Color(0x70, 0x80, 0x90, 0xFF),
        ]
    ]
    font["CPAL"] = cpal

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def solid_paint(palette_index: int, alpha: float = 1.0) -> dict[str, object]:
    return {
        "Format": int(ot.PaintFormat.PaintSolid),
        "PaletteIndex": palette_index,
        "Alpha": alpha,
    }


def build_colr_v1_composite_font(path: Path) -> None:
    """Build a compact COLRv1 paint graph fixture.

    The fixture intentionally starts with the first batchable COLRv1 public
    paint surfaces: root PaintSolid, nested PaintGlyph, and every real
    PaintComposite mode.  Gradients, color lines, transforms, and ClipList rows
    remain separate batches so their pending route counts stay visible until
    same-input C/Rust/C-ABI/WASM comparisons exist.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: solid_paint(1),
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(2, 0.5),
            "Glyph": base_names[2],
        },
    }
    for offset, mode in enumerate(ot.CompositeMode):
        color_glyphs[base_names[3 + offset]] = {
            "Format": int(ot.PaintFormat.PaintComposite),
            "SourcePaint": solid_paint(1),
            "CompositeMode": int(mode),
            "BackdropPaint": solid_paint(2, 0.5),
        }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_layers_font(path: Path) -> None:
    """Build a compact COLRv1 PaintColrLayers fixture.

    FreeType 2.14.3 exposes PaintColrLayers through `FT_Get_Paint` as an
    initialized `FT_LayerIterator`, then consumes that iterator through
    `FT_Get_Paint_Layers`.  Keep this fixture focused on two- and three-layer
    records; FontTools canonicalizes a one-layer PaintColrLayers node to its
    child paint, so single-layer and malformed layer-list behavior should stay
    in separate future fixtures.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:40]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintColrLayers),
            "Layers": [
                solid_paint(1),
                solid_paint(2, 0.5),
            ],
        },
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintColrLayers),
            "Layers": [
                solid_paint(1),
                solid_paint(2, 0.5),
                {
                    "Format": int(ot.PaintFormat.PaintGlyph),
                    "Paint": solid_paint(3, 0.25),
                    "Glyph": base_names[3],
                },
            ],
        },
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_colr_glyph_font(path: Path) -> None:
    """Build a compact COLRv1 PaintColrGlyph recursive fixture."""
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:40]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintColrGlyph),
            "Glyph": base_names[1],
        },
        base_names[1]: solid_paint(2, 0.5),
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_transform_paints_font(path: Path) -> None:
    """Build compact COLRv1 transform-paint fixture variants.

    FreeType 2.14.3 normalizes several internal COLRv1 table formats to the
    public FT_PaintScale, FT_PaintRotate, and FT_PaintSkew records.  Keep root
    transform synthesis out of this fixture; that depends on active size and
    FT_Set_Transform state and remains a separate route.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:48]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    transform = {
        "xx": 1.5,
        "xy": -0.125,
        "dx": 5.0,
        "yx": 0.25,
        "yy": 0.75,
        "dy": -3.0,
    }

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintTransform),
            "Paint": solid_paint(1),
            "Transform": transform,
        },
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintTranslate),
            "Paint": solid_paint(2, 0.5),
            "dx": 17,
            "dy": -9,
        },
        base_names[2]: {
            "Format": int(ot.PaintFormat.PaintScale),
            "Paint": solid_paint(1),
            "scaleX": 0.75,
            "scaleY": -0.5,
        },
        base_names[3]: {
            "Format": int(ot.PaintFormat.PaintScaleAroundCenter),
            "Paint": solid_paint(2, 0.5),
            "scaleX": 1.25,
            "scaleY": 0.625,
            "centerX": 11,
            "centerY": -7,
        },
        base_names[4]: {
            "Format": int(ot.PaintFormat.PaintScaleUniform),
            "Paint": solid_paint(3, 0.25),
            "scale": 1.5,
        },
        base_names[5]: {
            "Format": int(ot.PaintFormat.PaintScaleUniformAroundCenter),
            "Paint": solid_paint(1),
            "scale": 0.5,
            "centerX": -13,
            "centerY": 19,
        },
        base_names[6]: {
            "Format": int(ot.PaintFormat.PaintRotate),
            "Paint": solid_paint(2, 0.5),
            "angle": 0.25,
        },
        base_names[7]: {
            "Format": int(ot.PaintFormat.PaintRotateAroundCenter),
            "Paint": solid_paint(3, 0.25),
            "angle": -0.125,
            "centerX": 23,
            "centerY": -29,
        },
        base_names[8]: {
            "Format": int(ot.PaintFormat.PaintSkew),
            "Paint": solid_paint(1),
            "xSkewAngle": 0.0625,
            "ySkewAngle": -0.1875,
        },
        base_names[9]: {
            "Format": int(ot.PaintFormat.PaintSkewAroundCenter),
            "Paint": solid_paint(2, 0.5),
            "xSkewAngle": -0.25,
            "ySkewAngle": 0.125,
            "centerX": -31,
            "centerY": 37,
        },
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_root_transform_font(path: Path) -> None:
    """Build a compact COLRv1 root-transform fixture.

    The font intentionally keeps the actual root paint simple.  The parity
    surface under test is FreeType's synthetic top-level PaintTransform that
    `FT_Get_Paint` inserts from active size and `FT_Set_Transform` state when
    `FT_Get_Color_Glyph_Paint` is called with `FT_COLOR_INCLUDE_ROOT_TRANSFORM`.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:38]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 3
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(1),
            "Glyph": base_names[1],
        },
        base_names[1]: solid_paint(2, 0.5),
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_all_paints_font(path: Path) -> None:
    """Build one maintained COLRv1 fixture with every supported paint form."""
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:60]

    cpal = newTable("CPAL")
    cpal.version = 1
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ],
        [
            Color(0x01, 0x02, 0x03, 0xFF),
            Color(0x11, 0x12, 0x13, 0xE0),
            Color(0x21, 0x22, 0x23, 0xC0),
            Color(0x31, 0x32, 0x33, 0xA0),
        ],
    ]
    cpal.paletteTypes = [0x0001, 0x0002]
    cpal.paletteLabels = [256, cpal.NO_NAME_ID]
    cpal.paletteEntryLabels = [257, 258, cpal.NO_NAME_ID, cpal.NO_NAME_ID]
    font["CPAL"] = cpal

    transform = {
        "xx": 1.5,
        "xy": -0.125,
        "dx": 5.0,
        "yx": 0.25,
        "yy": 0.75,
        "dy": -3.0,
    }

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintColrLayers),
            "Layers": [
                solid_paint(1),
                solid_paint(2, 0.5),
                {
                    "Format": int(ot.PaintFormat.PaintGlyph),
                    "Paint": solid_paint(3, 0.25),
                    "Glyph": base_names[15],
                },
            ],
        },
        base_names[1]: solid_paint(1),
        base_names[2]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(2, 0.5),
            "Glyph": base_names[15],
        },
        base_names[3]: {
            "Format": int(ot.PaintFormat.PaintColrGlyph),
            "Glyph": base_names[1],
        },
        base_names[4]: {
            "Format": int(ot.PaintFormat.PaintLinearGradient),
            "ColorLine": color_line(
                ot.ExtendMode.PAD,
                [(0.0, 1, 1.0), (0.5, 2, 0.5), (1.0, 3, 0.25)],
            ),
            "x0": -10,
            "y0": 0,
            "x1": 40,
            "y1": 0,
            "x2": 40,
            "y2": 20,
        },
        base_names[5]: {
            "Format": int(ot.PaintFormat.PaintRadialGradient),
            "ColorLine": color_line(
                ot.ExtendMode.REPEAT,
                [(0.25, 2, 0.75), (0.875, 3, 0.125)],
            ),
            "x0": 5,
            "y0": -7,
            "r0": 3,
            "x1": 33,
            "y1": 29,
            "r1": 41,
        },
        base_names[6]: {
            "Format": int(ot.PaintFormat.PaintSweepGradient),
            "ColorLine": color_line(ot.ExtendMode.REFLECT, [(0.75, 1, 0.625)]),
            "centerX": -13,
            "centerY": 17,
            "startAngle": -0.25,
            "endAngle": 0.5,
        },
        base_names[7]: {
            "Format": int(ot.PaintFormat.PaintTransform),
            "Paint": solid_paint(1),
            "Transform": transform,
        },
        base_names[8]: {
            "Format": int(ot.PaintFormat.PaintTranslate),
            "Paint": solid_paint(2, 0.5),
            "dx": 17,
            "dy": -9,
        },
        base_names[9]: {
            "Format": int(ot.PaintFormat.PaintScale),
            "Paint": solid_paint(1),
            "scaleX": 0.75,
            "scaleY": -0.5,
        },
        base_names[10]: {
            "Format": int(ot.PaintFormat.PaintRotateAroundCenter),
            "Paint": solid_paint(3, 0.25),
            "angle": -0.125,
            "centerX": 23,
            "centerY": -29,
        },
        base_names[11]: {
            "Format": int(ot.PaintFormat.PaintSkewAroundCenter),
            "Paint": solid_paint(2, 0.5),
            "xSkewAngle": -0.25,
            "ySkewAngle": 0.125,
            "centerX": -31,
            "centerY": 37,
        },
        base_names[12]: {
            "Format": int(ot.PaintFormat.PaintComposite),
            "SourcePaint": solid_paint(1),
            "CompositeMode": int(ot.CompositeMode.SRC_OVER),
            "BackdropPaint": solid_paint(2, 0.5),
        },
        base_names[13]: {
            "Format": int(ot.PaintFormat.PaintGlyph),
            "Paint": solid_paint(1),
            "Glyph": base_names[15],
        },
        base_names[14]: solid_paint(0xFFFF),
        base_names[15]: {
            "Format": int(ot.PaintFormat.PaintScaleAroundCenter),
            "Paint": solid_paint(1),
            "scaleX": 0.625,
            "scaleY": -0.75,
            "centerX": -11,
            "centerY": 19,
        },
        base_names[16]: {
            "Format": int(ot.PaintFormat.PaintScaleUniform),
            "Paint": solid_paint(2, 0.5),
            "scale": 0.5,
        },
        base_names[17]: {
            "Format": int(ot.PaintFormat.PaintScaleUniformAroundCenter),
            "Paint": solid_paint(3, 0.25),
            "scale": -0.375,
            "centerX": 17,
            "centerY": -23,
        },
        base_names[18]: {
            "Format": int(ot.PaintFormat.PaintRotate),
            "Paint": solid_paint(1),
            "angle": 0.25,
        },
        base_names[19]: {
            "Format": int(ot.PaintFormat.PaintSkew),
            "Paint": solid_paint(2, 0.5),
            "xSkewAngle": 0.125,
            "ySkewAngle": -0.25,
        },
        base_names[20]: {
            "Format": int(ot.PaintFormat.PaintRadialGradient),
            "ColorLine": color_line(ot.ExtendMode.PAD, [(0.0, 1, 1.0)]),
            "x0": -5,
            "y0": 8,
            # The OpenType fields are UFWORD.  FreeType 2.14.3 reads these
            # bytes with FT_NEXT_SHORT and maps the negative values to
            # FT_INT_MAX; retain the encoded edge value to exercise that
            # compatibility behavior in both the C oracle and Rust parser.
            "r0": 0xFFFF,
            "x1": 19,
            "y1": -12,
            "r1": 0xFFFF,
        },
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_malformed_paints_font(path: Path) -> None:
    """Build a deterministic COLRv1 control with an invalid base-list offset.

    The SFNT remains openable, but both pinned C and Rust reject its COLR v1
    root lookup before dereferencing paint records.  The mutation is applied
    after canonical serialization so no external font or nondeterministic
    table writer is involved.
    """
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 18:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")
    data = bytearray(source.read_bytes())
    table_offset = table.offset
    data[table_offset + 14 : table_offset + 18] = (0).to_bytes(4, "big")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_paint_formats_font(path: Path, formats: tuple[int, ...]) -> None:
    """Build a COLRv1 control with malformed root paint-format bytes.

    The canonical all-paints fixture supplies three adjacent base glyph
    records.  Replace only the first records' format byte so the face remains
    openable and the final valid control record remains available for the
    lazy ``FT_Get_Color_Glyph_Paint``/``FT_Get_Paint`` comparison.
    """
    if not formats or any(not 0 <= value <= 0xFF for value in formats):
        raise ValueError(f"paint formats must be non-empty bytes: {formats!r}")
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 18:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    table_end = table_offset + table.length
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    base_count = int.from_bytes(data[base_start : base_start + 4], "big")
    if len(formats) > base_count:
        raise ValueError(
            f"canonical COLRv1 fixture has only {base_count} base records, "
            f"cannot mutate {len(formats)}"
        )

    for record_index, format_byte in enumerate(formats):
        record_start = base_start + 4 + record_index * 6
        glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
        expected_glyph_id = 36 + record_index
        if glyph_id != expected_glyph_id:
            raise RuntimeError(
                f"unexpected canonical COLRv1 glyph at record {record_index}: "
                f"{glyph_id} != {expected_glyph_id}"
            )
        paint_offset = int.from_bytes(data[record_start + 2 : record_start + 6], "big")
        paint_position = base_start + paint_offset
        if not table_offset <= paint_position < table_end:
            raise RuntimeError(
                f"canonical COLRv1 paint offset leaves table: {paint_position:#x}"
            )
        data[paint_position] = format_byte

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_child_paints_font(path: Path) -> None:
    """Build a COLRv1 matrix with invalid child-paint offsets.

    Each mutated root remains addressable through ``FT_Get_Color_Glyph_Paint``
    while the lazy ``FT_Get_Paint`` reader rejects one wrapper's child offset.
    The final solid root is retained as a valid control for the full public
    two-step call sequence.
    """
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 18:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    table_end = table_offset + table.length
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    base_count = int.from_bytes(data[base_start : base_start + 4], "big")
    formats = (10, 12, 14, 16, 24, 28, 32, 32)
    if base_count < len(formats):
        raise ValueError(
            f"canonical COLRv1 fixture has only {base_count} base records, "
            f"cannot mutate {len(formats)}"
        )

    for record_index, paint_format in enumerate(formats):
        record_start = base_start + 4 + record_index * 6
        glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
        expected_glyph_id = 36 + record_index
        if glyph_id != expected_glyph_id:
            raise RuntimeError(
                f"unexpected canonical COLRv1 glyph at record {record_index}: "
                f"{glyph_id} != {expected_glyph_id}"
            )
        paint_offset = int.from_bytes(data[record_start + 2 : record_start + 6], "big")
        paint_position = base_start + paint_offset
        if not table_offset <= paint_position < table_end:
            raise RuntimeError(
                f"canonical COLRv1 paint offset leaves table: {paint_position:#x}"
            )
        data[paint_position] = paint_format
        if paint_format == 32 and record_index == len(formats) - 1:
            # Keep the source pointer in range so the second composite offset
            # is the failing field for this row.
            data[paint_position + 1 : paint_position + 4] = (1).to_bytes(3, "big")
            data[paint_position + 4] = 0
            data[paint_position + 5 : paint_position + 8] = (0).to_bytes(3, "big")
        else:
            child_offset = 0 if record_index % 2 == 0 else 0xFFFFFF
            data[paint_position + 1 : paint_position + 4] = child_offset.to_bytes(3, "big")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_payloads_font(path: Path) -> None:
    """Build a COLRv1 matrix whose paint payloads end at the table boundary.

    The base-glyph records point eight roots at the final bytes of the COLR
    table.  Their format bytes remain addressable through
    ``FT_Get_Color_Glyph_Paint``, while the pinned C reader and Rust parser
    reject the first unavailable payload field for PaintColrLayers, PaintSolid,
    and wrapper paint families.  Glyph 50 remains the valid solid control.
    """
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 18:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    table_end = table_offset + table.length
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    base_count = int.from_bytes(data[base_start : base_start + 4], "big")
    formats = (1, 2, 4, 6, 8, 5, 10, 32)
    if base_count < len(formats):
        raise ValueError(
            f"canonical COLRv1 fixture has only {base_count} base records, "
            f"cannot mutate {len(formats)}"
        )

    table_relative_positions = [table.length - 2 - index for index in range(len(formats))]
    for record_index, (paint_format, table_relative_position) in enumerate(
        zip(formats, table_relative_positions)
    ):
        record_start = base_start + 4 + record_index * 6
        glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
        expected_glyph_id = 36 + record_index
        if glyph_id != expected_glyph_id:
            raise RuntimeError(
                f"unexpected canonical COLRv1 glyph at record {record_index}: "
                f"{glyph_id} != {expected_glyph_id}"
            )
        paint_position = table_offset + table_relative_position
        if not table_offset <= paint_position < table_end:
            raise RuntimeError(
                f"canonical COLRv1 boundary paint offset leaves table: {paint_position:#x}"
            )
        data[record_start + 2 : record_start + 6] = (
            table_relative_position - base_offset
        ).to_bytes(4, "big")
        data[paint_position] = paint_format

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_colorline_font(path: Path) -> None:
    """Build a COLRv1 matrix with one invalid shared ColorLine record."""
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 18:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    table_end = table_offset + table.length
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    target_relative_position = table.length - 40
    target_position = table_offset + target_relative_position
    if target_position <= table_offset or target_position >= table_end:
        raise RuntimeError("canonical COLRv1 table has no safe invalid ColorLine target")
    data[target_position] = 0xFF  # Extend mode above REFLECT is invalid.

    formats = (4, 6, 8, 5)
    for record_index, paint_format in enumerate(formats, start=4):
        record_start = base_start + 4 + record_index * 6
        glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
        expected_glyph_id = 36 + record_index
        if glyph_id != expected_glyph_id:
            raise RuntimeError(
                f"unexpected canonical COLRv1 glyph at record {record_index}: "
                f"{glyph_id} != {expected_glyph_id}"
            )
        paint_offset = int.from_bytes(data[record_start + 2 : record_start + 6], "big")
        paint_position = base_start + paint_offset
        if not table_offset <= paint_position < table_end:
            raise RuntimeError(
                f"canonical COLRv1 paint offset leaves table: {paint_position:#x}"
            )
        child_offset = target_position - paint_position
        if not 0 < child_offset <= 0xFFFFFF:
            raise RuntimeError(
                f"invalid shared ColorLine offset {child_offset} for glyph {glyph_id}"
            )
        data[paint_position] = paint_format
        data[paint_position + 1 : paint_position + 4] = child_offset.to_bytes(3, "big")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_gradient_payloads_font(path: Path) -> None:
    """Build COLRv1 roots whose gradient payload ends after a valid ColorLine."""
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 24:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    table_end_relative = table.length
    colorline_relative_position = table_end_relative - 3
    data[
        table_offset + colorline_relative_position : table_offset + table_end_relative
    ] = b"\0\0\0"

    # Each root points at the shared three-byte ColorLine header.  The roots
    # are spaced so their Offset24 fields do not overlap the next format byte;
    # the final gradient field then reaches exactly past the COLR table.
    specs = (
        (0, 5, table_end_relative - 19),
        (1, 4, table_end_relative - 15),
        (2, 8, table_end_relative - 11),
        (3, 28, table_end_relative - 7),
    )
    for record_index, paint_format, paint_relative_position in specs:
        record_start = base_start + 4 + record_index * 6
        glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
        expected_glyph_id = 36 + record_index
        if glyph_id != expected_glyph_id:
            raise RuntimeError(
                f"unexpected canonical COLRv1 glyph at record {record_index}: "
                f"{glyph_id} != {expected_glyph_id}"
            )
        paint_position = table_offset + paint_relative_position
        child_offset = colorline_relative_position - paint_relative_position
        if not 0 < child_offset <= 0xFFFFFF:
            raise RuntimeError(
                f"invalid shared ColorLine offset {child_offset} for glyph {glyph_id}"
            )
        data[record_start + 2 : record_start + 6] = (
            paint_relative_position - base_offset
        ).to_bytes(4, "big")
        data[paint_position] = paint_format
        data[paint_position + 1 : paint_position + 4] = child_offset.to_bytes(3, "big")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_radial_payload_font(path: Path) -> None:
    """Build a radial root whose payload ends after a valid ColorLine."""
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 24:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    table_end_relative = table.length
    colorline_relative_position = table_end_relative - 3
    data[
        table_offset + colorline_relative_position : table_offset + table_end_relative
    ] = b"\0\0\0"

    record_start = base_start + 4
    glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
    if glyph_id != 36:
        raise RuntimeError(f"unexpected canonical COLRv1 first glyph: {glyph_id} != 36")

    # PaintRadialGradient has the same fixed payload extent as PaintLinearGradient
    # through r1 at offset +14.  Starting it 15 bytes from the table end leaves
    # the ColorLine header readable while the final two-byte field is truncated.
    paint_relative_position = table_end_relative - 15
    paint_position = table_offset + paint_relative_position
    child_offset = colorline_relative_position - paint_relative_position
    if not 0 < child_offset <= 0xFFFFFF:
        raise RuntimeError(f"invalid shared ColorLine offset {child_offset} for glyph 36")
    data[record_start + 2 : record_start + 6] = (
        paint_relative_position - base_offset
    ).to_bytes(4, "big")
    data[paint_position] = 6
    data[paint_position + 1 : paint_position + 4] = child_offset.to_bytes(3, "big")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_transform_payloads_font(path: Path) -> None:
    """Build scale and rotate roots whose fixed payloads end at the table boundary."""
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 24:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    specs = (
        (16, table.length - 6),
        (18, table.length - 8),
        (20, table.length - 4),
        (22, table.length - 9),
        (24, table.length - 3),
        (26, table.length - 7),
    )
    for record_index, (paint_format, paint_relative_position) in enumerate(specs):
        record_start = base_start + 4 + record_index * 6
        glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
        expected_glyph_id = 36 + record_index
        if glyph_id != expected_glyph_id:
            raise RuntimeError(
                f"unexpected canonical COLRv1 glyph at record {record_index}: "
                f"{glyph_id} != {expected_glyph_id}"
            )
        paint_position = table_offset + paint_relative_position
        if not table_offset <= paint_position < table_offset + table.length:
            raise RuntimeError(
                f"canonical COLRv1 transform paint leaves table: {paint_position:#x}"
            )
        data[record_start + 2 : record_start + 6] = (
            paint_relative_position - base_offset
        ).to_bytes(4, "big")
        data[paint_position] = paint_format

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_transform_boundary_font(
    path: Path,
    paint_format: int,
    trailing_bytes: int,
) -> None:
    """Build one transform root whose child is valid but a fixed field is truncated."""
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 24:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    record_start = base_start + 4
    glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
    if glyph_id != 36:
        raise RuntimeError(f"unexpected canonical COLRv1 first glyph: {glyph_id} != 36")

    paint_relative_position = table.length - trailing_bytes
    paint_position = table_offset + paint_relative_position
    if not table_offset <= paint_position < table_offset + table.length:
        raise RuntimeError(f"canonical COLRv1 transform paint leaves table: {paint_position:#x}")
    data[record_start + 2 : record_start + 6] = (
        paint_relative_position - base_offset
    ).to_bytes(4, "big")
    data[paint_position] = paint_format
    # Point the nested paint at the final byte, whose value is the unsupported
    # format 3. This keeps child resolution successful before the target fixed
    # field reaches the COLR table boundary.
    data[paint_position + 1 : paint_position + 4] = b"\0\0\3"

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_malformed_scale_initial_payload_font(path: Path) -> None:
    """Build a static scale root whose first scale field reaches the table end."""
    build_colr_v1_malformed_transform_boundary_font(path, 16, 4)


def build_colr_v1_malformed_rotate_centered_final_payload_font(path: Path) -> None:
    """Build a centered rotate root whose second center field reaches the table end."""
    build_colr_v1_malformed_transform_boundary_font(path, 26, 9)


def build_colr_v1_malformed_translate_dx_payload_font(path: Path) -> None:
    """Build a translate root whose dx field reaches the table end."""
    build_colr_v1_malformed_transform_boundary_font(path, 14, 5)


def build_colr_v1_malformed_translate_dy_payload_font(path: Path) -> None:
    """Build a translate root whose dy field reaches the table end after dx."""
    build_colr_v1_malformed_transform_boundary_font(path, 14, 6)


def build_colr_v1_malformed_skew_centered_final_payload_font(path: Path) -> None:
    """Build a centered skew root whose final center field reaches the table end."""
    build_colr_v1_malformed_transform_boundary_font(path, 30, 11)


def build_colr_v1_malformed_layer_list_font(path: Path) -> None:
    """Build a COLRv1 control with an out-of-range PaintColrLayers index.

    The root PaintColrLayers record remains addressable through
    ``FT_Get_Color_Glyph_Paint`` while its FirstLayerIndex cannot be resolved
    through LayerV1List.  Glyph 50 remains the valid solid control for the
    public two-step call sequence.
    """
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 18:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    table_end = table_offset + table.length
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    base_start = table_offset + base_offset
    base_count = int.from_bytes(data[base_start : base_start + 4], "big")
    if base_count == 0:
        raise RuntimeError("canonical COLRv1 fixture has no base glyph records")

    record_start = base_start + 4
    glyph_id = int.from_bytes(data[record_start : record_start + 2], "big")
    if glyph_id != 36:
        raise RuntimeError(f"unexpected canonical COLRv1 first glyph: {glyph_id} != 36")
    paint_offset = int.from_bytes(data[record_start + 2 : record_start + 6], "big")
    paint_position = base_start + paint_offset
    if not table_offset <= paint_position or paint_position + 6 > table_end:
        raise RuntimeError(f"canonical COLRv1 layer paint leaves table: {paint_position:#x}")
    if data[paint_position] != int(ot.PaintFormat.PaintColrLayers):
        raise RuntimeError(
            "canonical COLRv1 first root is not PaintColrLayers: "
            f"{data[paint_position]}"
        )

    data[paint_position + 1] = 1
    # The canonical LayerV1List has three entries.  Use a non-wrapping index
    # so FreeType's unsigned ``first_layer_index + num_layers`` check rejects
    # the record instead of accepting 0xFFFFFFFF through 32-bit overflow.
    data[paint_position + 2 : paint_position + 6] = (4).to_bytes(4, "big")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def build_colr_v1_recursive_paint_depth_font(path: Path, source_cycle: bool) -> None:
    """Build a COLRv1 paint graph that reaches the Rust parser depth guard.

    The layer-list entry at table-relative offset 16 points at a composite root
    and the entry at offset 26 points at a PaintColrLayers node.  Those nodes
    point back to each other, so face-open parsing reaches the same graph at
    depth 33 without allowing an unbounded recursive allocation.  The source
    control makes the composite child itself recursive; the backdrop control
    keeps the source a valid solid and makes only the backdrop recursive.
    """
    source = COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf"
    font = TTFont(source, recalcTimestamp=False)
    table = font.reader.tables.get(b"COLR")
    if table is None or table.length < 18:
        raise RuntimeError(f"canonical COLRv1 fixture has no usable COLR table: {source}")

    data = bytearray(source.read_bytes())
    table_offset = table.offset
    table_end = table_offset + table.length
    base_offset = int.from_bytes(data[table_offset + 14 : table_offset + 18], "big")
    layer_list_offset = int.from_bytes(data[table_offset + 18 : table_offset + 22], "big")
    composite_relative_position = layer_list_offset + 16
    layer_relative_position = layer_list_offset + 26
    solid_relative_position = layer_list_offset + 32
    composite_position = table_offset + composite_relative_position
    layer_position = table_offset + layer_relative_position
    solid_position = table_offset + solid_relative_position
    if solid_position + 5 > table_end:
        raise RuntimeError("canonical COLRv1 table has no room for recursive paint control")

    def write_u24(position: int, value: int) -> None:
        data[position : position + 3] = value.to_bytes(3, "big")

    def write_u32(position: int, value: int) -> None:
        data[position : position + 4] = value.to_bytes(4, "big")

    # The first layer-list entry targets the composite and the third targets
    # the layer node.  Both offsets are relative to LayerV1List.
    write_u32(table_offset + layer_list_offset + 4, 16)
    write_u32(table_offset + layer_list_offset + 12, 26)

    data[composite_position] = int(ot.PaintFormat.PaintComposite)
    source_target = layer_position if source_cycle else solid_position
    write_u24(composite_position + 1, source_target - composite_position)
    data[composite_position + 4] = 0  # FT_COLR_COMPOSITE_CLEAR
    write_u24(composite_position + 5, layer_position - composite_position)

    data[layer_position] = int(ot.PaintFormat.PaintColrLayers)
    data[layer_position + 1] = 1
    write_u32(layer_position + 2, 0)

    data[solid_position] = int(ot.PaintFormat.PaintSolid)
    data[solid_position + 1 : solid_position + 3] = (0).to_bytes(2, "big")
    data[solid_position + 3 : solid_position + 5] = (0x4000).to_bytes(2, "big", signed=False)

    base_start = table_offset + base_offset
    first_record = base_start + 4
    second_record = first_record + 6
    write_u32(first_record + 2, composite_relative_position - base_offset)
    write_u32(second_record + 2, layer_relative_position - base_offset)

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def color_line(extend: ot.ExtendMode, stops: list[tuple[float, int, float]]) -> dict[str, object]:
    return {
        "Extend": int(extend),
        "ColorStop": [
            {
                "StopOffset": stop_offset,
                "PaletteIndex": palette_index,
                "Alpha": alpha,
            }
            for stop_offset, palette_index, alpha in stops
        ],
    }


def add_color_variation_axes(font: TTFont) -> None:
    """Add the compact `wght` and `GRAD` axes used by COLR VarStore fixtures."""
    fvar = newTable("fvar")
    fvar.axes = []
    fvar.instances = []
    for tag, minimum, default, maximum, name_id, label in (
        ("wght", 100.0, 400.0, 900.0, 300, "Weight"),
        ("GRAD", 0.0, 0.0, 1.0, 301, "Gradient"),
    ):
        axis = Axis()
        axis.axisTag = tag
        axis.minValue = minimum
        axis.defaultValue = default
        axis.maxValue = maximum
        axis.flags = 0
        axis.axisNameID = name_id
        fvar.axes.append(axis)
        font["name"].setName(label, name_id, 3, 1, 0x0409)
        font["name"].setName(label, name_id, 1, 0, 0)
    font["fvar"] = fvar


def colr_v1_color_var_store(font: TTFont) -> ot.VarStore:
    """Build a deterministic COLR VarStore for VarColorStop/gradient deltas.

    The single region peaks at `wght=max, GRAD=max`.  FreeType applies COLR
    variation deltas through the COLR VarStore using the public VarIndexBase
    fields documented for VarColorStop and PaintVarLinearGradient in the
    OpenType COLR v1 format.
    """
    axis_tags = [axis.axisTag for axis in font["fvar"].axes]
    region_list = var_builder.buildVarRegionList(
        [{"wght": (0.0, 1.0, 1.0), "GRAD": (0.0, 1.0, 1.0)}],
        axis_tags,
    )
    deltas = [
        [4096],  # stop 0 offset: +0.25 in F2Dot14 units.
        [1024],  # stop 0 alpha: +0.0625 in F2Dot14 units.
        [-2048],  # stop 1 offset: -0.125 in F2Dot14 units.
        [-2048],  # stop 1 alpha: -0.125 in F2Dot14 units.
        [5],  # PaintVarLinearGradient x0.
        [0],  # PaintVarLinearGradient y0.
        [10],  # PaintVarLinearGradient x1.
        [0],  # PaintVarLinearGradient y1.
        [10],  # PaintVarLinearGradient x2.
        [5],  # PaintVarLinearGradient y2.
    ]
    var_data = var_builder.buildVarData([0], deltas, optimize=False)
    return var_builder.buildVarStore(region_list, [var_data])


def build_colr_v1_static_gradients_font(path: Path) -> None:
    """Build compact static COLRv1 gradient and ColorLine fixture.

    FreeType 2.14.3 exposes PaintLinearGradient, PaintRadialGradient, and
    PaintSweepGradient coordinates as 16.16 public values and initializes
    ColorLine iterators from static ColorStop records.  This fixture covers
    the static PAD/REPEAT/REFLECT routes only; variable ColorLine rows remain
    pending until VarColorStop deltas are implemented and compared.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:40]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: {
            "Format": int(ot.PaintFormat.PaintLinearGradient),
            "ColorLine": color_line(
                ot.ExtendMode.PAD,
                [
                    (0.0, 1, 1.0),
                    (0.5, 2, 0.5),
                    (1.0, 3, 0.25),
                ],
            ),
            "x0": -10,
            "y0": 0,
            "x1": 40,
            "y1": 0,
            "x2": 40,
            "y2": 20,
        },
        base_names[1]: {
            "Format": int(ot.PaintFormat.PaintRadialGradient),
            "ColorLine": color_line(
                ot.ExtendMode.REPEAT,
                [
                    (0.25, 2, 0.75),
                    (0.875, 3, 0.125),
                ],
            ),
            "x0": 5,
            "y0": -7,
            "r0": 3,
            "x1": 33,
            "y1": 29,
            "r1": 41,
        },
        base_names[2]: {
            "Format": int(ot.PaintFormat.PaintSweepGradient),
            "ColorLine": color_line(
                ot.ExtendMode.REFLECT,
                [
                    (0.75, 1, 0.625),
                ],
            ),
            "centerX": -13,
            "centerY": 17,
            "startAngle": -0.25,
            "endAngle": 0.5,
        },
    }

    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def build_colr_v1_variable_gradients_font(path: Path) -> None:
    """Build compact variable COLRv1 gradient and VarColorStop fixture."""
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    add_color_variation_axes(font)
    glyph_order = font.getGlyphOrder()
    base_name = glyph_order[36]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 4
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x10, 0x20, 0x30, 0xFF),
            Color(0x40, 0x50, 0x60, 0x80),
            Color(0x70, 0x80, 0x90, 0x40),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_name: {
            "Format": int(ot.PaintFormat.PaintVarLinearGradient),
            "ColorLine": {
                "Extend": int(ot.ExtendMode.PAD),
                "ColorStop": [
                    {
                        "StopOffset": 0.0,
                        "PaletteIndex": 1,
                        "Alpha": 0.5,
                        "VarIndexBase": 0,
                    },
                    {
                        "StopOffset": 1.0,
                        "PaletteIndex": 2,
                        "Alpha": 1.0,
                        "VarIndexBase": 2,
                    },
                ],
            },
            "x0": 0,
            "y0": 0,
            "x1": 40,
            "y1": 0,
            "x2": 40,
            "y2": 20,
            "VarIndexBase": 4,
        }
    }
    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        varStore=colr_v1_color_var_store(font),
        allowLayerReuse=False,
    )

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def clip_box(x_min: int, y_min: int, x_max: int, y_max: int, fmt: int = 1) -> ot.ClipBox:
    box = ot.ClipBox()
    box.Format = fmt
    box.xMin = x_min
    box.yMin = y_min
    box.xMax = x_max
    box.yMax = y_max
    if fmt == 2:
        box.VarIndexBase = 0
    return box


def colr_v1_clipbox_var_store(font: TTFont) -> ot.VarStore:
    """Build the four-delta VarStore used by the variable ClipBox record."""
    axis_tags = [axis.axisTag for axis in font["fvar"].axes]
    region_list = var_builder.buildVarRegionList(
        [{"wght": (0.0, 1.0, 1.0), "GRAD": (0.0, 1.0, 1.0)}],
        axis_tags,
    )
    var_data = var_builder.buildVarData(
        [0],
        [
            [4096],  # xMin: +0.25 in F2Dot14 units.
            [2048],  # yMin: +0.125 in F2Dot14 units.
            [-4096],  # xMax: -0.25 in F2Dot14 units.
            [-2048],  # yMax: -0.125 in F2Dot14 units.
        ],
        optimize=False,
    )
    return var_builder.buildVarStore(region_list, [var_data])


def build_colr_v1_clipbox_font(
    path: Path,
    include_clip_list: bool,
    variable_clip_box: bool = False,
) -> None:
    """Build deterministic COLRv1 ClipList fixtures for FT_Get_Color_Glyph_ClipBox.

    The success fixture includes a tested format 1 ClipBox plus a format 2
    record. Both records are serialized into the ClipList, and the maintained
    parity matrix routes one case through each layout so face-open parsing and
    public ClipBox output cover both static and variable records.
    """
    font = TTFont(SOURCE_FONT, recalcTimestamp=False)
    if variable_clip_box:
        add_color_variation_axes(font)
    glyph_order = font.getGlyphOrder()
    base_names = glyph_order[36:39]

    cpal = newTable("CPAL")
    cpal.version = 0
    cpal.numPaletteEntries = 2
    cpal.palettes = [
        [
            Color(0x00, 0x00, 0x00, 0xFF),
            Color(0x20, 0x40, 0x60, 0xFF),
        ]
    ]
    font["CPAL"] = cpal

    color_glyphs: dict[str, object] = {
        base_names[0]: solid_paint(1),
        base_names[1]: solid_paint(1),
    }
    font["COLR"] = buildCOLR(
        color_glyphs,
        version=1,
        glyphMap=font.getReverseGlyphMap(),
        varStore=colr_v1_clipbox_var_store(font) if variable_clip_box else None,
        allowLayerReuse=False,
    )

    if include_clip_list:
        glyph_map = font.getReverseGlyphMap()
        clip_list = ot.ClipList()
        clip_list.Format = 1
        clip_list.clips = {
            base_names[0]: clip_box(-120, -80, 340, 510),
            base_names[1]: clip_box(-64, -32, 256, 384, fmt=2),
        }
        font["COLR"].table.ClipList = clip_list

    path.parent.mkdir(parents=True, exist_ok=True)
    font.save(path, reorderTables=False)


def main() -> None:
    for name in (
        "cpal-palettes-names-flags.ttf",
        "cpal-palettes-light-dark.ttf",
    ):
        build_cpal_font(OUTPUT_DIR / name)
    build_cpal_zero_entry_font(OUTPUT_DIR / "cpal-zero-entries.ttf")
    build_cpal_variant(
        OUTPUT_DIR / "cpal-v1-no-optional-metadata.ttf", "no_optional_metadata"
    )
    build_cpal_variant(OUTPUT_DIR / "malformed" / "cpal-v1-truncated-indices.ttf", "truncated_indices")
    build_cpal_variant(OUTPUT_DIR / "malformed" / "cpal-v1-truncated-types.ttf", "truncated_types")
    build_cpal_variant(OUTPUT_DIR / "malformed" / "cpal-v1-truncated-labels.ttf", "truncated_labels")
    build_cpal_variant(
        OUTPUT_DIR / "malformed" / "cpal-v1-truncated-entry-labels.ttf",
        "truncated_entry_labels",
    )
    build_colr_v0_layers_font(COLOR_OUTPUT_DIR / "colr-v0-layers-cpal.ttf")
    build_colr_v0_layers_font(COLOR_OUTPUT_DIR / "colr-v0-layer-control.ttf")
    build_colr_v1_composite_font(COLOR_OUTPUT_DIR / "colr_v1_composite_modes.ttf")
    build_colr_v1_layers_font(COLOR_OUTPUT_DIR / "colr-v1-paint-colr-layers-cpal.ttf")
    build_colr_v1_colr_glyph_font(COLOR_OUTPUT_DIR / "colr-v1-colr-glyph-recursive.ttf")
    build_colr_v1_transform_paints_font(COLOR_OUTPUT_DIR / "colr-v1-transform-paints.ttf")
    build_colr_v1_root_transform_font(COLOR_OUTPUT_DIR / "colr-v1-root-transform.ttf")
    build_colr_v1_all_paints_font(COLOR_OUTPUT_DIR / "colr-v1-all-paints.ttf")
    build_colr_v1_malformed_paints_font(COLOR_OUTPUT_DIR / "malformed-colr-v1-paints.ttf")
    build_colr_v1_malformed_paint_formats_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-paint-format-unsupported.ttf",
        (33, 255),
    )
    build_colr_v1_malformed_paint_formats_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-paint-format-max-and-above.ttf",
        (33, 34),
    )
    build_colr_v1_malformed_child_paints_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-child-paints.ttf"
    )
    build_colr_v1_malformed_payloads_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-paint-payloads.ttf"
    )
    build_colr_v1_malformed_colorline_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-colorline-paints.ttf"
    )
    build_colr_v1_malformed_gradient_payloads_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-gradient-payloads.ttf"
    )
    build_colr_v1_malformed_radial_payload_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-radial-payload.ttf"
    )
    build_colr_v1_malformed_transform_payloads_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-transform-payloads.ttf"
    )
    build_colr_v1_malformed_scale_initial_payload_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-scale-initial-payload.ttf"
    )
    build_colr_v1_malformed_rotate_centered_final_payload_font(
        COLOR_OUTPUT_DIR
        / "malformed"
        / "colr-v1-malformed-rotate-centered-final-payload.ttf"
    )
    build_colr_v1_malformed_translate_dx_payload_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-translate-dx-payload.ttf"
    )
    build_colr_v1_malformed_translate_dy_payload_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-translate-dy-payload.ttf"
    )
    build_colr_v1_malformed_skew_centered_final_payload_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-skew-centered-final-payload.ttf"
    )
    build_colr_v1_malformed_layer_list_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-malformed-layer-list.ttf"
    )
    build_colr_v1_recursive_paint_depth_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-recursive-source-paints.ttf",
        source_cycle=True,
    )
    build_colr_v1_recursive_paint_depth_font(
        COLOR_OUTPUT_DIR / "malformed" / "colr-v1-recursive-backdrop-paints.ttf",
        source_cycle=False,
    )
    build_colr_v1_static_gradients_font(COLOR_OUTPUT_DIR / "colr-v1-static-gradients.ttf")
    build_colr_v1_variable_gradients_font(COLOR_OUTPUT_DIR / "colr-v1-variable-gradients.ttf")
    build_colr_v1_clipbox_font(COLOR_OUTPUT_DIR / "colr-v1-clipbox-format1-format2.ttf", True)
    build_colr_v1_clipbox_font(
        COLOR_OUTPUT_DIR / "colr-v1-clipbox-format2-varstore.ttf",
        True,
        variable_clip_box=True,
    )
    build_colr_v1_clipbox_font(COLOR_OUTPUT_DIR / "colr-v1-no-clipbox-control.ttf", False)


if __name__ == "__main__":
    main()
