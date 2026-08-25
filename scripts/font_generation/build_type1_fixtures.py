#!/usr/bin/env python3
"""Build compact Type 1 fixtures for non-SFNT public face routes."""

from __future__ import annotations

from pathlib import Path

from fontTools.misc.psCharStrings import T1CharString
from fontTools.t1Lib import StandardEncoding, T1Font, write


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "tests" / "fixtures"
OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1"
MM_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1-mm"
LEGACY_MM_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "mm"
INPUT_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1"
INPUT_AUX_OUT_DIR = FIXTURE_ROOT / "input" / "aux" / "type1"
INPUT_ENCODING_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1-encoding"
INPUT_MM_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "type1-mm"
CID_OUT_DIR = FIXTURE_ROOT / "input" / "fonts" / "cid"


def charstring(program: list[object]) -> T1CharString:
    return T1CharString(program=program)


def stable_generator_header(data: bytes) -> bytes:
    lines = data.split(b"\n")
    return b"\n".join(
        b"%t1Font: (fontdone fixture)" if line.startswith(b"%t1Font: ") else line
        for line in lines
    )


def build_cid_type1(path: Path, *, is_fixed_pitch: bool = False) -> None:
    """Write a minimal synthetic CID-keyed Type 1 resource."""
    fixed_pitch = "true" if is_fixed_pitch else "false"
    postscript = f"""%!PS-Adobe-3.0 Resource-CIDFont
%%DocumentNeededResources: ProcSet (CIDInit)
/CIDFontName /FontdoneCIDType1 def
/CIDFontVersion 1.0 def
/CIDFontType 0 def
/Registry (Adobe) def
/Ordering (Identity) def
/Supplement 0 def
/UIDBase 424242 def
/CIDMapOffset 0 def
/FDBytes 1 def
/GDBytes 1 def
/CIDCount 3 def
/FontBBox [0 0 500 700] def
/version (001.000) def
/Notice (Project-authored fontdone CID Type 1 fixture) def
/FullName (Fontdone CID Type 1 Regular) def
/FamilyName (Fontdone CID Type 1) def
/Weight (Regular) def
/ItalicAngle 0 def
/isFixedPitch {fixed_pitch} def
/UnderlinePosition -100 def
/UnderlineThickness 50 def
/FDArray 1 array def
%ADOBeginFontDict
dup 0 10 dict dup begin
/FontName /FontdoneCIDType1-Regular def
/FontType 1 def
/PaintType 0 def
/FontMatrix [0.001 0 0 0.001 0 0] def
/Private 4 dict dup begin
/lenIV -1 def
end def
end put
%ADOEndFontDict
(Hex) 8 StartData
"""
    # Four 2-byte CIDMap entries cover glyphs 0..2 plus the look-ahead
    # terminator.  Every entry selects FD 0 and an empty charstring at offset
    # 8; CID service calls need only validate this map and return GID == CID.
    binary_hex = b"0008000800080008>\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    path.write_bytes(postscript.encode("ascii") + binary_hex)


def build_simple_type1(
    path: Path,
    font_name: str,
    family_name: str,
    notice: str,
    *,
    weight: str = "Regular",
    is_fixed_pitch: bool = False,
    private_overrides: dict[str, object] | None = None,
    cleartext_replacements: list[tuple[bytes, bytes]] | None = None,
    charstrings: dict[str, T1CharString] | None = None,
) -> None:
    private_dict = {
        "BlueValues": [],
        "OtherBlues": [],
        "FamilyBlues": [],
        "FamilyOtherBlues": [],
        "BlueScale": 0.039625,
        "BlueShift": 7,
        "BlueFuzz": 1,
        "StdHW": [50],
        "StdVW": [80],
        "ForceBold": False,
        "LanguageGroup": 0,
        "password": 5839,
        "lenIV": 4,
        "RD": "-|",
        "ND": "|-",
        "NP": "|",
        "Subrs": [],
    }
    private_dict.update(private_overrides or {})
    font = T1Font.__new__(T1Font)
    font.encoding = "ascii"
    font.font = {
        "FontName": font_name,
        "FontInfo": {
            "version": "001.000",
            "Notice": notice,
            "FullName": family_name,
            "FamilyName": family_name,
            "Weight": weight,
            "ItalicAngle": 0,
            "isFixedPitch": is_fixed_pitch,
            "UnderlinePosition": -100,
            "UnderlineThickness": 50,
        },
        "Encoding": StandardEncoding,
        "PaintType": 0,
        "FontType": 1,
        "FontMatrix": [0.001, 0, 0, 0.001, 0, 0],
        "FontBBox": (0, 0, 500, 700),
        "Private": private_dict,
        "CharStrings": charstrings
        or {
            ".notdef": charstring([500, 0, "hsbw", "endchar"]),
            "A": charstring(
                [
                    500,
                    0,
                    "hsbw",
                    50,
                    0,
                    "rmoveto",
                    0,
                    700,
                    "rlineto",
                    400,
                    0,
                    "rlineto",
                    0,
                    -700,
                    "rlineto",
                    "closepath",
                    "endchar",
                ]
            ),
        },
    }
    data = stable_generator_header(font.getData())
    for before, after in cleartext_replacements or []:
        if before not in data:
            raise ValueError(f"missing Type 1 fixture token: {before!r}")
        data = data.replace(before, after, 1)
    path.parent.mkdir(parents=True, exist_ok=True)
    write(str(path), data, kind="PFB")


def build_unicode_charmap_fixture(path: Path) -> None:
    """Build Type 1 glyph names for the synthetic Unicode charmap service.

    The PostScript names service recognizes both ``uniXXXX`` and ``uXXXXXX``
    forms independently of the font's Encoding array.  Keep valid and
    malformed hexadecimal names together so public charmap probes exercise
    the same pure-Rust name decoder used by every frontend.
    """

    def square(advance: int) -> T1CharString:
        return charstring(
            [
                advance,
                0,
                "hsbw",
                50,
                0,
                "rmoveto",
                0,
                700,
                "rlineto",
                400,
                0,
                "rlineto",
                0,
                -700,
                "rlineto",
                "closepath",
                "endchar",
            ]
        )

    build_simple_type1(
        path,
        "Type1UnicodeNames",
        "Type 1 Unicode Names",
        "Generated for fontdone Type 1 Unicode charmap name parity",
        charstrings={
            ".notdef": charstring([500, 0, "hsbw", "endchar"]),
            "uni0041": square(500),
            "u0042": square(500),
            "uni00AF": square(500),
            "u1F600": square(500),
            "uni0041.alt": square(500),
            "uni00G1": square(500),
            "uni123": square(500),
            "uZZZZ": square(500),
            "1": square(500),
        },
    )


def build_agl_named_glyph_fixtures() -> None:
    """Build one valid Type 1 face for each named ASCII Adobe glyph case.

    Keeping each glyph in its own face makes the public parity rows isolate one
    `ps_unicode_value` mapping at a time instead of hiding all mappings behind
    a shared multi-glyph fixture.
    """

    named_glyphs = [
        ("space", 0x20),
        ("exclam", 0x21),
        ("quotedbl", 0x22),
        ("numbersign", 0x23),
        ("dollar", 0x24),
        ("percent", 0x25),
        ("ampersand", 0x26),
        ("quotesingle", 0x27),
        ("parenleft", 0x28),
        ("parenright", 0x29),
        ("asterisk", 0x2A),
        ("plus", 0x2B),
        ("comma", 0x2C),
        ("hyphen", 0x2D),
        ("period", 0x2E),
        ("slash", 0x2F),
        ("zero", 0x30),
        ("one", 0x31),
        ("two", 0x32),
        ("three", 0x33),
        ("four", 0x34),
        ("five", 0x35),
        ("six", 0x36),
        ("seven", 0x37),
        ("eight", 0x38),
        ("nine", 0x39),
        ("colon", 0x3A),
        ("semicolon", 0x3B),
        ("less", 0x3C),
        ("equal", 0x3D),
    ]
    empty_glyph = charstring([500, 0, "hsbw", "endchar"])
    for glyph_name, _codepoint in named_glyphs:
        build_simple_type1(
            OUT_DIR / f"agl-{glyph_name}.pfb",
            f"AGL{glyph_name.title()}",
            f"AGL named glyph {glyph_name}",
            "Generated for fontdone Type 1 Adobe glyph-name charmap parity",
            charstrings={
                ".notdef": charstring([500, 0, "hsbw", "endchar"]),
                glyph_name: empty_glyph,
            },
        )

    batch33_named_glyphs = [
        ("greater", "greater", 0x3E),
        ("question", "question", 0x3F),
        ("at", "at", 0x40),
        ("bracketleft", "bracketleft", 0x5B),
        ("backslash", "backslash", 0x5C),
        ("bracketright", "bracketright", 0x5D),
        ("asciicircum", "asciicircum", 0x5E),
        ("underscore", "underscore", 0x5F),
        ("grave", "grave", 0x60),
        ("braceleft", "braceleft", 0x7B),
        ("bar", "bar", 0x7C),
        ("braceright", "braceright", 0x7D),
        ("asciitilde", "asciitilde", 0x7E),
        ("Delta", "delta", 0x0394),
        ("Omega", "omega", 0x03A9),
        ("fraction", "fraction", 0x2215),
        ("macron", "macron", 0x00AF),
        ("mu", "mu", 0x03BC),
        ("periodcentered", "periodcentered", 0x00B7),
        ("nonbreakingspace", "nonbreakingspace", 0x00A0),
        ("Tcommaaccent", "tcommaaccent", 0x021A),
        ("tcommaaccent", "tcommaaccent-lower", 0x021B),
        ("A", "letter-a", 0x41),
        ("B", "letter-b", 0x42),
        ("C", "letter-c", 0x43),
        ("D", "letter-d", 0x44),
        ("E", "letter-e", 0x45),
        ("F", "letter-f", 0x46),
        ("G", "letter-g", 0x47),
        ("H", "letter-h", 0x48),
    ]
    for glyph_name, filename, _codepoint in batch33_named_glyphs:
        build_simple_type1(
            OUT_DIR / f"agl-{filename}.pfb",
            f"AGL{filename.replace('-', '').title()}",
            f"AGL named glyph {glyph_name}",
            "Generated for fontdone Type 1 Adobe glyph-name charmap parity",
            charstrings={
                ".notdef": charstring([500, 0, "hsbw", "endchar"]),
                glyph_name: empty_glyph,
            },
        )


def _pfb_segments(data: bytes) -> list[tuple[int, bytes]]:
    segments: list[tuple[int, bytes]] = []
    offset = 0
    while offset + 6 <= len(data) and data[offset] == 0x80:
        segment_type = data[offset + 1]
        length = int.from_bytes(data[offset + 2 : offset + 6], "little")
        start = offset + 6
        end = start + length
        if end > len(data):
            raise ValueError("truncated PFB segment")
        segments.append((segment_type, data[start:end]))
        offset = end
        if segment_type == 3:
            break
    return segments


def _type1_eexec_crypt(data: bytes, seed: int) -> bytes:
    r = seed
    output = bytearray()
    for plain_byte in data:
        cipher_byte = plain_byte ^ (r >> 8)
        r = ((cipher_byte + r) * 52845 + 22719) & 0xFFFF
        output.append(cipher_byte)
    return bytes(output)


def _type1_eexec_decrypt(data: bytes) -> bytes:
    r = 55665
    output = bytearray()
    for cipher_byte in data:
        plain_byte = cipher_byte ^ (r >> 8)
        r = ((cipher_byte + r) * 52845 + 22719) & 0xFFFF
        output.append(plain_byte)
    return bytes(output[4:])


def _write_pfb_segments(path: Path, segments: list[tuple[int, bytes]]) -> None:
    output = bytearray()
    for segment_type, payload in segments:
        output.extend(b"\x80")
        output.append(segment_type)
        output.extend(len(payload).to_bytes(4, "little"))
        output.extend(payload)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(output)


def build_private_parser_edge_fixture(
    path: Path,
    *,
    private_replacement: tuple[bytes, bytes] | None = None,
    private_text: bytes | None = None,
) -> None:
    """Derive a compact PFB while changing only its decrypted private program.

    These fixtures keep the generated clear-text dictionary and PFB framing
    stable, so each public parity row isolates one private-dictionary or
    CharStrings parser branch.
    """

    segments = _pfb_segments((OUT_DIR / "simple-type1.pfb").read_bytes())
    cleartext = segments[0][1]
    encrypted = next(payload for kind, payload in segments if kind == 2)
    private = private_text if private_text is not None else _type1_eexec_decrypt(encrypted)
    if private_replacement is not None:
        before, after = private_replacement
        if before not in private:
            raise ValueError(f"missing private Type 1 token: {before!r}")
        private = private.replace(before, after, 1)
    replaced = [(kind, payload) for kind, payload in segments]
    for index, (kind, _payload) in enumerate(replaced):
        if kind == 2:
            replaced[index] = (kind, _type1_eexec_crypt(b"\x00\x00\x00\x00" + private, 55665))
            break
    _write_pfb_segments(path, replaced)


def build_private_parser_trailing_space_fixture(path: Path) -> None:
    """Add whitespace after a Type 1 CharStrings procedure terminator.

    The parser's loop must consume the trailing ``None`` procedure token and
    then take its end-of-dictionary break when whitespace reaches ``\nend``.
    Keep the mutation in the decrypted private program so the fixture remains
    a valid PFB accepted by the pinned C loader.
    """

    segments = _pfb_segments((OUT_DIR / "simple-type1.pfb").read_bytes())
    encrypted = next(payload for kind, payload in segments if kind == 2)
    private = _type1_eexec_decrypt(encrypted)
    marker = b"/A 25 None "
    marker_start = private.index(marker)
    end_marker = private.index(b" None\nend", marker_start + len(marker))
    insert_at = end_marker + len(b" None")
    private = private[:insert_at] + b" " + private[insert_at:]
    replaced = [(kind, payload) for kind, payload in segments]
    for index, (kind, _payload) in enumerate(replaced):
        if kind == 2:
            replaced[index] = (kind, _type1_eexec_crypt(b"\x00\x00\x00\x00" + private, 55665))
            break
    _write_pfb_segments(path, replaced)
    path.write_bytes(path.read_bytes() + b"\x80\x03")


def invalidate_first_pfb_segment(path: Path) -> None:
    data = bytearray(path.read_bytes())
    if data[:2] != b"\x80\x01":
        raise ValueError("expected an ASCII PFB first segment")
    data[1] = 2
    path.write_bytes(data)


def build_adobe_mm_two_axis(path: Path) -> None:
    """Build a compact Adobe Type 1 Multiple Master descriptor fixture.

    FreeType's Type 1 MM parser reads these top-level dictionary keys in
    `src/type1/t1load.c`: `BlendAxisTypes`, `BlendDesignPositions`,
    `BlendDesignMap`, and `WeightVector`.  The glyph program is intentionally
    minimal; this fixture exists first to make Adobe MM descriptor, design
    coordinate, weight-vector, and named-instance reset API state reproducible
    through pinned C FreeType.
    """

    build_simple_type1(
        path,
        "AdobeMMTwoAxis",
        "Adobe MM Two Axis",
        "Generated for fontdone Type 1 Multiple Master API parity",
        cleartext_replacements=[
            (
                b"/FontBBox {0 0 500 700} def",
                b"/FontBBox {0 0 500 700} def\n"
                b"/BlendAxisTypes [/Weight /Width] def\n"
                b"/BlendDesignPositions [[400 100] [900 100] [400 200] [900 200]] def\n"
                b"/BlendDesignMap [[[400 0] [900 1]] [[100 0] [200 1]]] def\n"
                b"/WeightVector [0.25 0.25 0.25 0.25] def",
            )
        ],
    )


def build_adobe_mm_one_axis(path: Path) -> None:
    """Build a compact valid Adobe Type 1 Multiple Master one-axis face.

    Keep a non-linear design map so public design-coordinate setters exercise
    both interpolation intervals and the one-axis weight-vector unmapping
    route.  The descriptor uses the same valid Type 1 glyph program as the
    two-axis control, with all design coordinates in the inclusive 100..900
    range accepted by the pinned C loader.
    """

    build_simple_type1(
        path,
        "AdobeMMOneAxis",
        "Adobe MM One Axis",
        "Generated for fontdone Type 1 Multiple Master one-axis parity",
        cleartext_replacements=[
            (
                b"/FontBBox {0 0 500 700} def",
                b"/FontBBox {0 0 500 700} def\n"
                b"/BlendAxisTypes [/Weight] def\n"
                b"/BlendDesignPositions [[100] [900]] def\n"
                b"/BlendDesignMap [[[100 0] [400 0.4] [900 1]]] def\n"
                b"/WeightVector [0.5 0.5] def",
            )
        ],
    )


def build_mm_blend_fontinfo_private(path: Path) -> None:
    """Build the declared Type 1 MM fixture for private blend-table parity.

    The public rows under `t1tables.get_ps_font_private_mm_blend` need a
    Multiple Master face with populated Private-dictionary fields, not just the
    descriptor-only MM fixture used by `ftmm`.  Keep this source-backed so the
    eventual `FT_Get_PS_Font_Private`/`FT_Get_PS_Font_Value` route can compare
    pinned C and Rust against a reproducible same input.
    """

    build_simple_type1(
        path,
        "MMBlendPrivate",
        "MM Blend Private",
        "Generated for fontdone Type 1 MM private blend parity",
        private_overrides={
            "BlueValues": [-20, 0, 480, 500],
            "OtherBlues": [-250, -230],
            "FamilyBlues": [-15, 0, 470, 490],
            "FamilyOtherBlues": [-260, -240],
            "BlueScale": 0.047,
            "BlueShift": 9,
            "StdHW": [42],
            "StdVW": [83],
            "StemSnapH": [38, 42, 46],
            "StemSnapV": [78, 83, 91],
            "ForceBold": True,
        },
        cleartext_replacements=[
            (
                b"/FontBBox {0 0 500 700} def",
                b"/FontBBox {0 0 500 700} def\n"
                b"/BlendAxisTypes [/Weight /Width] def\n"
                b"/BlendDesignPositions [[400 100] [900 100] [400 200] [900 200]] def\n"
                b"/BlendDesignMap [[[400 0] [900 1]] [[100 0] [200 1]]] def\n"
                b"/WeightVector [0.25 0.25 0.25 0.25] def",
            )
        ],
    )


def build_mm_underline_blend_fixture(
    path: Path,
    font_name: str,
    family_name: str,
    underline_key: str,
    values: list[int],
) -> None:
    """Build Type 1 MM FontInfo underline-array fixtures.

    FreeType parses scalar FontInfo arrays in MM fonts into
    `blend->font_infos[1..]` (`src/type1/t1load.c:t1_load_keyword` via
    `ps_parser_load_field`).  The public `FT_Get_PS_Font_Info` record still
    exposes the base face FontInfo value; these fixtures pin that C behavior
    while proving the blend dictionary array is present in the source font.
    """

    array = " ".join(str(value) for value in values).encode()
    build_simple_type1(
        path,
        font_name,
        family_name,
        f"Generated for fontdone Type 1 MM {underline_key} blend parity",
        cleartext_replacements=[
            (
                b"/FontBBox {0 0 500 700} def",
                b"/FontBBox {0 0 500 700} def\n"
                b"/BlendAxisTypes [/Weight /Width] def\n"
                b"/BlendDesignPositions [[400 100] [900 100] [400 200] [900 200]] def\n"
                b"/BlendDesignMap [[[400 0] [900 1]] [[100 0] [200 1]]] def\n"
                b"/WeightVector [1 0 0 0] def\n"
                + f"/{underline_key} [".encode()
                + array
                + b"] def",
            )
        ],
    )


def build_non_mm_force_bold(path: Path) -> None:
    """Build the declared non-MM ForceBold control for Type 1 private parity."""

    build_simple_type1(
        path,
        "NonMMForceBold",
        "Non MM Force Bold",
        "Generated for fontdone Type 1 ForceBold private control parity",
        private_overrides={"ForceBold": True},
    )


def build_font_value_populated(path: Path) -> None:
    """Build the declared FT_Get_PS_Font_Value selector-matrix fixture."""

    build_simple_type1(
        path,
        "FontValuePopulated",
        "Font Value Populated",
        "Generated for fontdone Type 1 font-value selector parity",
        private_overrides={
            "BlueValues": [-20, 0, 480, 500],
            "StdHW": [42],
            "StdVW": [83],
        },
    )


def build_font_value_missing_optional_strings(path: Path) -> None:
    """Build a valid Type 1 face whose optional FontInfo strings are absent."""

    build_simple_type1(
        path,
        "FontValueMissingOptionalStrings",
        "Font Value Missing Optional Strings",
        "Generated for fontdone Type 1 missing optional FontInfo strings parity",
        cleartext_replacements=[
            (b"/FontInfo 12 dict dup begin", b"/FontInfo 7 dict dup begin"),
            (b"/version (001.000) def\n", b""),
            (
                b"/Notice (Generated for fontdone Type 1 missing optional FontInfo strings parity) def\n",
                b"",
            ),
            (b"/FullName (Font Value Missing Optional Strings) def\n", b""),
            (b"/FamilyName (Font Value Missing Optional Strings) def\n", b""),
            (b"/Weight (Regular) def\n", b""),
        ],
    )


def build_parser_opcode_coverage(path: Path) -> None:
    """Build a Type 1 glyph whose valid program reaches every supported path."""

    build_simple_type1(
        path,
        "Type1ParserOpcodes",
        "Type 1 Parser Opcodes",
        "Generated for fontdone Type 1 charstring parser coverage",
        charstrings={
            ".notdef": charstring([500, 0, "hsbw", "endchar"]),
            "A": charstring(
                [
                    0,
                    500,
                    "hsbw",
                    0,
                    "hstem",
                    0,
                    "vstem",
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    "hstem3",
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    "vstem3",
                    "dotsection",
                    0,
                    1,
                    "div",
                    "hstem",
                    0,
                    "vmoveto",
                    0,
                    0,
                    "hlineto",
                    0,
                    "vlineto",
                    0,
                    0,
                    "rlineto",
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    "rrcurveto",
                    "closepath",
                    0,
                    "hmoveto",
                    0,
                    "vlineto",
                    "closepath",
                    "endchar",
                ]
            ),
        },
    )


def build_parser_edge_programs() -> None:
    """Build small Type 1 programs for uncovered glyph-parser branches."""

    programs = [
        (
            "parser-vmoveto.pfb",
            "Type1ParserVMoveTo",
            [
                500,
                0,
                "hsbw",
                100,
                "vmoveto",
                100,
                0,
                "rlineto",
                100,
                "hlineto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-vmoveto-underflow.pfb",
            "Type1ParserVMoveToUnderflow",
            [
                500,
                0,
                "hsbw",
                "vmoveto",
                "endchar",
            ],
        ),
        (
            "parser-hmoveto.pfb",
            "Type1ParserHMoveTo",
            [
                500,
                0,
                "hsbw",
                100,
                "hmoveto",
                100,
                "vlineto",
                100,
                "hlineto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-number-boundaries.pfb",
            "Type1ParserNumberBoundaries",
            [
                500,
                0,
                "hsbw",
                -107,
                108,
                "rmoveto",
                107,
                1131,
                "rlineto",
                -108,
                -1131,
                "rlineto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-vhcurveto.pfb",
            "Type1ParserVhCurve",
            [
                500,
                0,
                "hsbw",
                0,
                0,
                "rmoveto",
                20,
                20,
                20,
                20,
                "vhcurveto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-hvcurveto.pfb",
            "Type1ParserHvCurve",
            [
                500,
                0,
                "hsbw",
                0,
                0,
                "rmoveto",
                20,
                20,
                20,
                20,
                "hvcurveto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-callothersubr.pfb",
            "Type1ParserCallOtherSubr",
            [
                500,
                0,
                "hsbw",
                0,
                0,
                "rmoveto",
                1,
                0,
                "callothersubr",
                "endchar",
            ],
        ),
        (
            "parser-setcurrentpoint.pfb",
            "Type1ParserSetCurrentPoint",
            [
                500,
                0,
                "hsbw",
                0,
                0,
                "rmoveto",
                10,
                10,
                "setcurrentpoint",
                "endchar",
            ],
        ),
        (
            "parser-setcurrentpoint-after-line.pfb",
            "Type1ParserSetCurrentPointAfterLine",
            [
                500,
                0,
                "hsbw",
                50,
                0,
                "rmoveto",
                0,
                300,
                "rlineto",
                50,
                300,
                "setcurrentpoint",
                300,
                0,
                "rlineto",
                0,
                -300,
                "rlineto",
                -300,
                0,
                "rlineto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-sbw.pfb",
            "Type1ParserSbw",
            [
                500,
                0,
                "hsbw",
                20,
                30,
                "sbw",
                "endchar",
            ],
        ),
        (
            "parser-truncated-escape.pfb",
            "Type1ParserTruncatedEscape",
            T1CharString(bytecode=bytes([12])),
        ),
        (
            "parser-sbw-success.pfb",
            "Type1ParserSbwSuccess",
            [
                500,
                0,
                "hsbw",
                20,
                30,
                500,
                0,
                "sbw",
                "endchar",
            ],
        ),
        (
            "parser-callothersubr-underflow.pfb",
            "Type1ParserCallOtherSubrUnderflow",
            [
                500,
                0,
                "hsbw",
                "callothersubr",
                "endchar",
            ],
        ),
        (
            "parser-callothersubr-invalid-count.pfb",
            "Type1ParserCallOtherSubrInvalidCount",
            [
                500,
                0,
                "hsbw",
                -1,
                0,
                "callothersubr",
                "endchar",
            ],
        ),
        (
            "parser-callothersubr-invalid-procedure.pfb",
            "Type1ParserCallOtherSubrInvalidProcedure",
            [
                500,
                0,
                "hsbw",
                0,
                0,
                "callothersubr",
                "endchar",
            ],
        ),
        (
            "parser-callothersubr-success.pfb",
            "Type1ParserCallOtherSubrSuccess",
            [
                500,
                0,
                "hsbw",
                0,
                1,
                "callothersubr",
                "endchar",
            ],
        ),
        (
            "parser-setcurrentpoint-underflow.pfb",
            "Type1ParserSetCurrentPointUnderflow",
            [
                500,
                0,
                "hsbw",
                "setcurrentpoint",
                "endchar",
            ],
        ),
        (
            "parser-hsbw-underflow.pfb",
            "Type1ParserHsbwUnderflow",
            ["hsbw", "endchar"],
        ),
        (
            "parser-rmoveto-underflow.pfb",
            "Type1ParserRMoveToUnderflow",
            [
                500,
                0,
                "hsbw",
                "rmoveto",
                "endchar",
            ],
        ),
        (
            "parser-vhcurveto-last-delta.pfb",
            "Type1ParserVhCurveLastDelta",
            [
                500,
                0,
                "hsbw",
                0,
                0,
                "rmoveto",
                20,
                20,
                20,
                20,
                20,
                "vhcurveto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-hvcurveto-last-delta.pfb",
            "Type1ParserHvCurveLastDelta",
            [
                500,
                0,
                "hsbw",
                0,
                0,
                "rmoveto",
                20,
                20,
                20,
                20,
                20,
                "hvcurveto",
                "closepath",
                "endchar",
            ],
        ),
        (
            "parser-unsupported-escape.pfb",
            "Type1ParserUnsupportedEscape",
            T1CharString(bytecode=bytes([12, 99])),
        ),
        (
            "parser-unsupported-op.pfb",
            "Type1ParserUnsupportedOp",
            T1CharString(bytecode=bytes([27])),
        ),
    ]
    for filename, font_name, program in programs:
        glyph_program = program if isinstance(program, T1CharString) else charstring(program)
        build_simple_type1(
            INPUT_OUT_DIR / filename,
            font_name,
            font_name.replace("Type1", "Type 1 "),
            "Generated for fontdone Type 1 glyph parser edge coverage",
            charstrings={
                ".notdef": charstring([500, 0, "hsbw", "endchar"]),
                "A": glyph_program,
            },
        )


def build_parser_noop_operands(path: Path, operator: str) -> None:
    """Build one valid Type 1 glyph whose movement operator consumes no operands."""

    build_simple_type1(
        path,
        f"Type1ParserNoop{operator.title()}",
        f"Type 1 Parser No-op {operator}",
        "Generated for fontdone Type 1 no-op operand parity",
        charstrings={
            ".notdef": charstring([500, 0, "hsbw", "endchar"]),
            "A": charstring([500, 0, "hsbw", 0, 0, "rmoveto", operator, "endchar"]),
        },
    )


def build_encoding_fixture(path: Path, font_name: str, family_name: str, encoding: bytes) -> None:
    """Build a Type 1 fixture with a specific clear-text Encoding object."""

    build_simple_type1(
        path,
        font_name,
        family_name,
        f"Generated for fontdone {family_name} encoding parity",
        cleartext_replacements=[(b"/Encoding StandardEncoding def", encoding)],
    )


def build_type1_encoding_fixtures() -> None:
    custom_array = (
        b"/Encoding 256 array\n"
        b"0 1 255 {1 index exch /.notdef put} for\n"
        b"dup 65 /A put\n"
        b"readonly def"
    )
    variants = [
        (
            INPUT_ENCODING_OUT_DIR / "custom-array.pfb",
            "EncodingCustomArray",
            "Encoding Custom Array",
            custom_array,
        ),
        (
            INPUT_ENCODING_OUT_DIR / "standard.pfb",
            "EncodingStandard",
            "Encoding Standard",
            b"/Encoding StandardEncoding def",
        ),
        (
            INPUT_ENCODING_OUT_DIR / "isolatin1.pfb",
            "EncodingISOLatin1",
            "Encoding ISO Latin 1",
            b"/Encoding ISOLatin1Encoding def",
        ),
        (
            INPUT_ENCODING_OUT_DIR / "expert.pfb",
            "EncodingExpert",
            "Encoding Expert",
            b"/Encoding ExpertEncoding def",
        ),
        (
            INPUT_ENCODING_OUT_DIR / "no-recognized-encoding.pfb",
            "EncodingNone",
            "Encoding None",
            b"/Encoding UnknownEncoding def",
        ),
        (
            OUT_DIR / "custom-encoding-array.pfb",
            "EncodingCustomArray",
            "Encoding Custom Array",
            custom_array,
        ),
        (
            OUT_DIR / "standard-encoding.pfb",
            "EncodingStandard",
            "Encoding Standard",
            b"/Encoding StandardEncoding def",
        ),
        (
            OUT_DIR / "isolatin1-encoding.pfb",
            "EncodingISOLatin1",
            "Encoding ISO Latin 1",
            b"/Encoding ISOLatin1Encoding def",
        ),
        (
            OUT_DIR / "expert-encoding.pfb",
            "EncodingExpert",
            "Encoding Expert",
            b"/Encoding ExpertEncoding def",
        ),
    ]
    for path, font_name, family_name, encoding in variants:
        build_encoding_fixture(path, font_name, family_name, encoding)


def build_attach_afm_fixture(path: Path, *, decimal_track_values: bool = False) -> None:
    """Build matching AFM data for the generated attach AFM Type 1 face.

    FreeType 2.14.3 parses Type 1 auxiliary AFM data through
    `src/type1/t1afm.c:T1_Read_Metrics` and `src/psaux/afmparse.c`.
    TrackKern records have degree, minimum point size, minimum kern, maximum
    point size, and maximum kern fields.  Keep the values intentionally simple
    and non-zero so later `FT_Attach_File`, `FT_Attach_Stream`, and
    `FT_Get_Track_Kerning` rows can prove observable attachment behavior
    instead of only proving that a file opened.  The track-kerning fixture can
    request equivalent decimal spellings to exercise the AFM fixed-point
    parser while preserving those values.
    """

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "StartFontMetrics 4.1",
                "Comment Generated for fontdone Type 1 attach/track parity",
                "FontName AttachAfmBase",
                "FullName Attach AFM Base",
                "FamilyName Attach AFM Base",
                "Weight Regular",
                "ItalicAngle 0",
                "IsFixedPitch false",
                "FontBBox 0 0 500 700",
                "UnderlinePosition -100",
                "UnderlineThickness 50",
                "StartCharMetrics 2",
                "C -1 ; WX 500 ; N .notdef ; B 0 0 0 0 ;",
                "C 65 ; WX 500 ; N A ; B 50 0 450 700 ;",
                "EndCharMetrics",
                "StartKernData",
                "StartTrackKern 3",
                *(
                    [
                        "TrackKern -1 .0 -30.0 72.0 -90.0",
                        "TrackKern 0 +8.0 +0.0 72.0 0.0",
                        "TrackKern 1 8.0 20.0 72.0 80.0",
                    ]
                    if decimal_track_values
                    else [
                        "TrackKern -1 8 -30 72 -90",
                        "TrackKern 0 8 0 72 0",
                        "TrackKern 1 8 20 72 80",
                    ]
                ),
                "EndTrackKern",
                "StartKernPairs 1",
                "KPX A A -25",
                "EndKernPairs",
                "EndKernData",
                "EndFontMetrics",
                "",
            ]
        ),
        encoding="ascii",
    )


def main() -> None:
    build_simple_type1(
        OUT_DIR / "simple-type1.pfb",
        "MinimalNonSfnt",
        "Minimal NonSFNT",
        "Generated for fontdone non-SFNT coverage",
    )
    build_unicode_charmap_fixture(OUT_DIR / "unicode-names-type1.pfb")
    build_agl_named_glyph_fixtures()
    build_simple_type1(
        OUT_DIR / "private-parser-edge-fields.pfb",
        "PrivateParserEdgeFields",
        "Private Parser Edge Fields",
        "Generated for fontdone Type 1 private parser parity",
        private_overrides={
            "BlueValues": [-20, 0, 480, 500],
            "OtherBlues": [-250, -230],
            "FamilyBlues": [-15, 0, 470, 490],
            "FamilyOtherBlues": [-260, -240],
            "BlueScale": 0.047,
            "BlueShift": 9,
            "BlueFuzz": 3,
            "UniqueID": 424242,
            "StdHW": [],
            "StdVW": [],
            "StemSnapV": [78, 83, 91],
            "StemSnapH": [38, 42, 46],
        },
    )
    build_simple_type1(
        OUT_DIR / "private-parser-negative-leniv.pfb",
        "PrivateParserNegativeLenIV",
        "Private Parser Negative LenIV",
        "Generated for fontdone Type 1 negative lenIV parity",
        private_overrides={"lenIV": -1},
    )
    build_simple_type1(
        OUT_DIR / "private-parser-invalid-encoding.pfb",
        "PrivateParserInvalidEncoding",
        "Private Parser Invalid Encoding",
        "Generated for fontdone Type 1 encoding parser parity",
        cleartext_replacements=[
            (
                b"/Encoding StandardEncoding def",
                b"/Encoding 256 array\n"
                b"0 1 255 {1 index exch /.notdef put} for\n"
                b"dup nope /Bad put\n"
                b"dup 999 /OutOfRange put\n"
                b"dup 65 BadName put\n"
                b"dup 66 /B put\n"
                b"readonly def",
            )
        ],
    )
    build_private_parser_edge_fixture(
        OUT_DIR / "private-parser-no-charstrings.pfb",
        private_replacement=(
            b"dup /CharStrings\n2 dict dup begin\n/.notdef 9 None",
            b"dup /NoCharStrings 0 dict def\n/.notdef 9 None",
        ),
    )
    build_private_parser_edge_fixture(
        OUT_DIR / "private-parser-no-charstrings-begin.pfb",
        private_replacement=(
            b"dup /CharStrings\n2 dict dup begin",
            b"dup /CharStrings\n2 dict dup",
        ),
    )
    build_private_parser_edge_fixture(
        OUT_DIR / "private-parser-invalid-length.pfb",
        private_replacement=(b"/A 25 None", b"/A nope None"),
    )
    build_private_parser_edge_fixture(
        OUT_DIR / "private-parser-truncated-charstring.pfb",
        private_replacement=(b"/A 25 None", b"/A 9999 None"),
    )
    build_private_parser_trailing_space_fixture(
        OUT_DIR / "private-parser-trailing-space.pfb"
    )
    build_private_parser_edge_fixture(
        OUT_DIR / "private-parser-missing-eexec-fields.pfb",
        private_text=b"dup /Private 1 dict dup begin\n/lenIV 4 def\nend\n",
    )
    build_simple_type1(
        OUT_DIR / "metadata-bold-invalid-bool.pfb",
        "MetadataProbe",
        "Metadata Probe",
        "Generated for fontdone Type 1 metadata coverage",
        weight="Bold",
        cleartext_replacements=[(b"/isFixedPitch false def", b"/isFixedPitch maybe def")],
    )
    build_simple_type1(
        OUT_DIR / "fixed-pitch-type1.pfb",
        "FixedPitchTypeOne",
        "Fixed Pitch Type One",
        "Generated for fontdone Type 1 fixed-pitch face-flag coverage",
        is_fixed_pitch=True,
    )
    build_simple_type1(
        OUT_DIR / "bbox-array-type1.pfb",
        "BBoxArrayTypeOne",
        "BBox Array Type One",
        "Generated for fontdone Type 1 array bbox coverage",
        cleartext_replacements=[
            (b"/FontBBox {0 0 500 700} def", b"/FontBBox [0 0 500 700] def")
        ],
    )
    invalid_segment_path = OUT_DIR / "invalid-first-segment-type1.pfb"
    build_simple_type1(
        invalid_segment_path,
        "InvalidSegmentTypeOne",
        "Invalid Segment Type One",
        "Generated for fontdone Type 1 PFB segment coverage",
    )
    invalidate_first_pfb_segment(invalid_segment_path)
    build_simple_type1(
        INPUT_OUT_DIR / "attach-afm-base.pfb",
        "AttachAfmBase",
        "Attach AFM Base",
        "Generated for fontdone Type 1 attach/patent coverage",
    )
    build_attach_afm_fixture(INPUT_AUX_OUT_DIR / "attach-afm-base.afm")
    build_simple_type1(
        INPUT_OUT_DIR / "track-kern-base.pfb",
        "AttachAfmBase",
        "Attach AFM Base",
        "Generated for fontdone Type 1 track-kerning coverage",
    )
    build_attach_afm_fixture(
        INPUT_AUX_OUT_DIR / "track-kern-base.afm", decimal_track_values=True
    )
    build_font_value_populated(INPUT_OUT_DIR / "font-value-populated.pfb")
    build_font_value_missing_optional_strings(
        INPUT_OUT_DIR / "font-value-missing-optional-strings.pfb"
    )
    build_parser_opcode_coverage(INPUT_OUT_DIR / "parser-opcodes.pfb")
    build_parser_edge_programs()
    for operator in ("rlineto", "hlineto", "vlineto", "rrcurveto", "vhcurveto", "hvcurveto"):
        build_parser_noop_operands(
            INPUT_OUT_DIR / f"parser-noop-{operator}.pfb",
            operator,
        )
    build_adobe_mm_two_axis(MM_OUT_DIR / "adobe-mm-two-axis.pfb")
    build_adobe_mm_one_axis(MM_OUT_DIR / "adobe-mm-one-axis.pfb")
    build_adobe_mm_two_axis(LEGACY_MM_OUT_DIR / "adobe-multiple-master.pfb")
    build_mm_blend_fontinfo_private(OUT_DIR / "mm-blend-fontinfo-private.pfb")
    build_mm_underline_blend_fixture(
        INPUT_MM_OUT_DIR / "underline-position.pfb",
        "MMUnderlinePosition",
        "MM Underline Position",
        "UnderlinePosition",
        [-111, -222, -333, -444],
    )
    build_mm_underline_blend_fixture(
        INPUT_MM_OUT_DIR / "underline-thickness.pfb",
        "MMUnderlineThickness",
        "MM Underline Thickness",
        "UnderlineThickness",
        [11, 22, 33, 44],
    )
    build_non_mm_force_bold(OUT_DIR / "non-mm-force-bold.pfb")
    build_type1_encoding_fixtures()
    build_cid_type1(CID_OUT_DIR / "fontinfo-populated.cid")
    build_cid_type1(
        CID_OUT_DIR / "fontinfo-fixed-pitch.cid",
        is_fixed_pitch=True,
    )


if __name__ == "__main__":
    main()
