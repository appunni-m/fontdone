#!/usr/bin/env python3
"""Build compact TrueType bytecode edge fixtures from the hinter matrix."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphComponent
from fontTools.ttLib.tables.ttProgram import Program


ROOT = Path(__file__).resolve().parents[2]
BASE_FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "glyf"


def save_font(name: str, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def empty_program() -> Program:
    return program_from_bytes(b"")


def program_from_bytes(bytecode: bytes) -> Program:
    program = Program()
    program.fromBytecode(bytecode)
    return program


def write_empty_fpgm() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = empty_program()
    save_font("hinter-empty-fpgm.ttf", font)


def write_empty_glyph_iup() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False, recalcBBoxes=False)
    # This body waives v40 compatibility, moves pp1, then invokes IUP[x].
    # C's empty-glyph shortcut ignores the entire body before simple-glyph
    # instruction parsing, so none of those phantom mutations may be observed.
    # FontTools normally compiles a zero-contour `Glyph` to a zero-length glyf
    # record, so preserve the valid raw header, instruction length, and program.
    font["glyf"]["empty"] = Glyph(
        bytes.fromhex(
            "00 00 00 00 00 00 00 00 00 00"
            " 00 09 b1 04 03 8e b1 00 40 48 31"
        )
    )
    save_font("hinter-empty-glyph-iup.ttf", font)


def write_invalid_contour_endpoints() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False, recalcBBoxes=False)
    # Two contours both end at point zero.  C `TT_Load_Simple_Glyph` rejects
    # the second endpoint before reading the otherwise complete point record.
    font["glyf"]["empty"] = Glyph(
        bytes.fromhex(
            "00 02 00 00 00 00 00 00 00 00"
            " 00 00 00 00"
            " 00 02 30 31"
            " 31"
        )
    )
    save_font("hinter-invalid-contour-endpoints.ttf", font)


def write_invalid_composite_attachment_points() -> None:
    for name, field, value in (
        ("hinter-invalid-composite-parent-point.ttf", "firstPt", 99),
        ("hinter-invalid-composite-component-point.ttf", "secondPt", 99),
    ):
        font = TTFont(BASE_FONT, recalcTimestamp=False, recalcBBoxes=False)
        # `attachPoint` has a base component with three points and a mark
        # component with three points.  Mutating only the attachment index keeps
        # the component glyph references valid so C reaches
        # `TT_Process_Composite_Component` and returns Invalid_Composite for the
        # out-of-range point (`src/truetype/ttgload.c:1059-1071`).
        setattr(font["glyf"]["attachPoint"].components[1], field, value)
        save_font(name, font)


def write_composite_depth_overflow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False, recalcBBoxes=False)
    previous = "base"
    # Keep the chain above the runtime recursion guard while remaining a valid
    # TrueType composite tree so the public load path reaches the pinned C
    # Invalid_Composite result.
    for index in range(102):
        name = f"depthOverflow{index}"
        glyph = Glyph()
        glyph.numberOfContours = -1
        glyph.xMin = 0
        glyph.yMin = 0
        glyph.xMax = 0
        glyph.yMax = 0
        component = GlyphComponent()
        component.glyphName = previous
        component.x = 0
        component.y = 0
        component.flags = 0
        glyph.components = [component]
        font["glyf"][name] = glyph
        font["hmtx"].metrics[name] = font["hmtx"].metrics["base"]
        previous = name
    font["maxp"].maxComponentDepth = 102
    save_font("hinter-composite-depth-overflow.ttf", font)


def write_prep_definitions() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    prep = font["prep"].program.getBytecode()
    # Existing prep sets INSTCTRL, then these no-output definitions exercise
    # range-0 FDEF and IDEF scanning without changing glyph points.
    prep += bytes.fromhex("b0 02 2c b0 01 21 2d")
    prep += bytes.fromhex("b0 84 89 b0 01 21 2d")
    font["prep"].program = program_from_bytes(prep)
    save_font("hinter-prep-definitions.ttf", font)


def write_prep_idef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    prep = font["prep"].program.getBytecode()
    prep += bytes.fromhex("b0 84 89 b0 01 21 2d")
    font["prep"].program = program_from_bytes(prep)
    save_font("hinter-prep-idef.ttf", font)


def write_prep_redefine_defs() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    prep = font["prep"].program.getBytecode()
    # FreeType allows definitions in prep.  Redefine existing FDEF 1 and IDEF
    # 0x8F so this stays within the font's maxp definition budgets.
    prep += bytes.fromhex("b0 01 2c b0 01 21 2d")
    prep += bytes.fromhex("b0 8f 89 b0 01 21 2d")
    font["prep"].program = program_from_bytes(prep)
    save_font("hinter-prep-redefine-defs.ttf", font)


def write_fpgm_loopcall() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b1 02 01 2a"))
    save_font("hinter-fpgm-loopcall.ttf", font)


def write_fpgm_loopcall_redefinition() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # The first LOOPCALL iteration jumps into PUSHB payload bytes that redefine
    # FDEF 1.  FreeType's call record points at that mutable definition record,
    # so the second iteration runs the new WCVTP body and writes CVT 0 to 1px.
    # The outer FDEF scanner treats the embedded definition as push data and
    # therefore accepts this deliberately broken but public-reachable program.
    font["fpgm"].program = program_from_bytes(
        bytes.fromhex("b0 01 2c b0 02 1c b7 b0 01 2c b1 00 40 44 2d 2d")
    )
    font["prep"].program = program_from_bytes(bytes.fromhex("b1 02 01 2a"))
    # MIAP point 0 to CVT 0 so the redefined second iteration changes geometry.
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b1 00 00 3e"))
    save_font("hinter-fpgm-loopcall-redefinition.ttf", font)


def write_called_fpgm_instctrl() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # A prep-initiated CALL switches curRange to fpgm, but C's Ins_INSTCTRL
    # validates iniRange and therefore accepts selector 1 under pedantic mode.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 01 2c b1 00 01 8e 2d"))
    font["prep"].program = program_from_bytes(bytes.fromhex("b0 01 2b"))
    save_font("hinter-called-fpgm-instctrl.ttf", font)


def write_direct_fpgm_instctrl() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Direct fpgm execution has iniRange 1.  C ignores selector 1 normally and
    # reports Invalid_Reference under pedantic hinting; this differs from the
    # prep-initiated CALL control above, whose iniRange remains 0.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b1 00 01 8e"))
    save_font("hinter-direct-fpgm-instctrl.ttf", font)


def write_fpgm_truncated_definition_pushes() -> None:
    for name, terminal_push in (
        ("hinter-fpgm-truncated-fdef-npushb.ttf", "40"),
        ("hinter-fpgm-truncated-fdef-npushw.ttf", "41"),
        ("hinter-fpgm-truncated-fdef-npushb-payload.ttf", "40 02 01"),
    ):
        font = TTFont(BASE_FONT, recalcTimestamp=False)
        # FreeType `SkipCode` reads the variable-push count while scanning the
        # FDEF body and returns Code_Overflow when the count or payload is
        # truncated.
        font["fpgm"].program = program_from_bytes(
            bytes.fromhex(f"b0 00 2c {terminal_push}")
        )
        save_font(name, font)


def write_unterminated_control_flow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # A false IF and a standalone ELSE both require the scanner to find EIF.
    # Pinned FreeType reaches codeSize and reports Code_Overflow in each case.
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b0 00 58"))
    font["glyf"]["mark"].program = program_from_bytes(bytes.fromhex("1b"))
    save_font("hinter-unterminated-control-flow.ttf", font)


def write_glyph_code_overflow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Keep setup programs nonempty but side-effect free so native bytecode is
    # active and FT_LOAD_PEDANTIC reaches the intended glyph-program failure.
    # Pinned Ins_NPUSHB, Ins_PUSHW, Ins_IF, and Ins_ELSE all report
    # Code_Overflow when their stream crosses codeSize.
    no_op = program_from_bytes(bytes.fromhex("b0 00 21"))
    font["fpgm"].program = no_op
    font["prep"].program = no_op
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b0 00 58"))
    font["glyf"]["mark"].program = program_from_bytes(bytes.fromhex("1b"))
    save_font("hinter-glyph-code-overflow.ttf", font)


def write_glyph_interpreter_errors() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Keep each public FT_Load_Glyph route independent from the base matrix's
    # deliberately invalid setup programs.  FT_LOAD_PEDANTIC then exposes the
    # exact TT_RunIns error instead of suppressing it after partial execution.
    no_op = program_from_bytes(bytes.fromhex("b0 00 21"))
    font["fpgm"].program = no_op
    font["prep"].program = no_op
    programs = {
        "base": "b0 00 1c",  # zero-offset JMPR => Bad_Argument
        "mark": "b1 01 00 62",  # DIV by zero => Divide_By_Zero
        "scanType0": "b0 00 4f",  # DEBUG => Debug_OpCode
        "scanType2": "2d",  # top-level ENDF => ENDF_In_Exec_Stream
        "idefCall": "60",  # ADD without operands => Too_Few_Arguments
        "untouchPoint": "b0 00 2c 2d",  # glyph FDEF => DEF_In_Glyf_Bytecode
        "superRoundMatrix": "28",  # RAW without IDEF => Invalid_Opcode
        # maxp declares 16 stack elements, so pinned FreeType allocates
        # 16 + max(8, 128) slots.  NPUSHB 255 crosses that exact limit.
        "pointMoveMatrix": "40 ff " + "00 " * 255,
    }
    for glyph_name, bytecode in programs.items():
        font["glyf"][glyph_name].program = program_from_bytes(bytes.fromhex(bytecode))
    save_font("hinter-glyph-interpreter-errors.ttf", font)


def write_invalid_twilight_scfs() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Select twilight zp2, then address point 65535.  C ignores invalid SCFS
    # points normally and reports Invalid_Reference under FT_LOAD_PEDANTIC.
    font["glyf"]["base"].program = program_from_bytes(
        bytes.fromhex("b0 00 15 b9 ff ff 00 00 48")
    )
    save_font("hinter-invalid-twilight-scfs.ttf", font)


def write_composite_compatibility_moves() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # The instructed composite runs with v40 compatibility enabled.  Vertical
    # and horizontal SHPIX prove both composite/freedom-vector outcomes; the
    # final vertical DELTAP1 proves the matching composite compatibility path.
    font["glyf"]["withInstructions"].program = program_from_bytes(
        bytes.fromhex(
            "04 b1 00 20 38 "
            "05 b1 00 20 38 "
            "04 b2 b8 00 01 5d"
        )
    )
    save_font("hinter-composite-compatibility-moves.ttf", font)


def write_fpgm_call_errors() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # This single font keeps CALL/LOOPCALL error coverage compact.  Its fpgm
    # defines FDEF 1 as a self-recursive body; function 0 and -1 remain invalid
    # references while glyph 24 reaches FreeType's call-stack overflow guard.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 01 2c b0 01 2b 2d"))
    font["glyf"][".notdef"].program = program_from_bytes(bytes.fromhex("b8 ff ff 2b"))
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b0 00 2b"))
    font["glyf"]["mark"].program = program_from_bytes(bytes.fromhex("b1 01 00 2a"))
    font["glyf"]["scanType0"].program = program_from_bytes(bytes.fromhex("b0 01 2b"))
    save_font("hinter-fpgm-call-errors.ttf", font)


def write_execution_too_long_loop() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Isolate the glyph loop from the control matrix's intentionally invalid
    # shared programs.  Under FT_LOAD_PEDANTIC they otherwise fail first with
    # Invalid_Reference, hiding C `TT_RunIns`'s negative-jump limit.
    no_op = program_from_bytes(bytes.fromhex("b0 00 21"))
    font["fpgm"].program = no_op
    font["prep"].program = no_op
    # The bytecode lands back at the PUSHW so the operand stack stays bounded.
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b8 ff fd 1c"))
    save_font("hinter-execution-too-long-loop.ttf", font)


def write_opcode_counter_limit() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # CLEAR has no operands and no control-flow side effects.  A straight-line
    # fpgm with one opcode beyond the pinned 1,000,000-opcode ceiling isolates
    # TT_RunIns's final counter guard from its much smaller backward-jump and
    # LOOPCALL heuristics.
    font["fpgm"].program = program_from_bytes(bytes([0x22]) * 1_000_001)
    font["prep"].program = program_from_bytes(bytes.fromhex("b0 00 21"))
    font["glyf"]["base"].program = empty_program()
    save_font("hinter-opcode-counter-limit.ttf", font)


def write_fpgm_function_overflow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Pinned `tt_load_maxp` reserves at least 64 FDEF slots.  Defining 65
    # distinct functions reaches Ins_FDEF's exact Too_Many_Function_Defs path.
    fpgm = bytearray()
    for function_id in range(65):
        fpgm.extend((0xB0, function_id, 0x2C, 0x2D))
    font["fpgm"].program = program_from_bytes(bytes(fpgm))
    font["prep"].program = program_from_bytes(bytes.fromhex("b0 00 21"))
    save_font("hinter-fpgm-function-overflow.ttf", font)


def write_too_many_hints() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False, recalcBBoxes=False)
    # One-contour simple glyph: valid header and endpoint followed by an
    # instruction length of four with no instruction payload.
    font["glyf"]["base"] = Glyph(
        bytes.fromhex(
            "00 01 00 00 00 00 00 00 00 00"
            " 00 00"
            " 00 04"
        )
    )
    save_font("hinter-too-many-hints.ttf", font)


def write_zero_units_per_em() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # SFNT face initialization validates head.unitsPerEm before a caller can
    # reach the auto-hinter.  This project-authored boundary fixture proves the
    # pinned public precedence: zero is Invalid_Table, not the otherwise real
    # internal afloader Corrupted_Font_Header branch.
    font["head"].unitsPerEm = 0
    save_font("hinter-zero-units-per-em.ttf", font)


def write_fpgm_fdef_index_overflow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # FDEF 256 is beyond the fixed TT_DefRecord array range.  FreeType rejects
    # it before scanning a function body, so no glyph points are needed.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b8 01 00 2c"))
    save_font("hinter-fpgm-fdef-index-overflow.ttf", font)


def write_idef_recursive_depth() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Redefine the existing ADJUST IDEF opcode with a body that calls itself.
    # FreeType bails out through its IDEF call-stack guard instead of looping.
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 8f 89 8f 2d"))
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("8f"))
    save_font("hinter-idef-recursive-depth.ttf", font)


def write_storage_cvt_reference_errors() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # maxp declares two storage and two CVT entries.  C's RS/WS/RCVT/WCVTP
    # handlers ignore index 9 in normal mode and return Invalid_Reference when
    # the same public glyph load enables FT_LOAD_PEDANTIC.
    font["glyf"][".notdef"].program = program_from_bytes(bytes.fromhex("b1 09 01 42"))
    font["glyf"]["base"].program = program_from_bytes(bytes.fromhex("b0 09 43"))
    font["glyf"]["mark"].program = program_from_bytes(bytes.fromhex("b1 09 20 44"))
    font["glyf"]["scanType0"].program = program_from_bytes(bytes.fromhex("b0 09 45"))
    # INSTCTRL selector 1 is valid only in prep.  A glyph-range use is ignored
    # normally and reports Invalid_Reference under FT_LOAD_PEDANTIC.
    font["glyf"]["scanType2"].program = program_from_bytes(bytes.fromhex("b1 01 01 8e"))
    font["glyf"]["idefCall"].program = program_from_bytes(bytes.fromhex("b1 09 20 70"))
    # Invalid selector 4 and invalid value 1 for selector 2 are both ignored
    # normally and report Invalid_Reference under FT_LOAD_PEDANTIC.
    font["glyf"]["untouchPoint"].program = program_from_bytes(bytes.fromhex("b1 00 04 8e"))
    font["glyf"]["superRoundMatrix"].program = program_from_bytes(bytes.fromhex("b1 01 02 8e"))
    # At 20 ppem, delta base 9 makes 0xB8 applicable.  CVT index 9 isolates
    # DELTAC's normal no-op / pedantic Invalid_Reference split.
    font["glyf"]["stackStateMatrix"].program = program_from_bytes(bytes.fromhex("b2 b8 09 01 73"))
    # Keep the pedantic SHP proof independent from the branch-edge program's
    # earlier invalid-reference probes.
    font["glyf"]["instructionControl"].program = program_from_bytes(bytes.fromhex("b0 09 32"))
    save_font("hinter-storage-cvt-reference-errors.ttf", font)


def write_fpgm_nested_fdef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    fpgm = font["fpgm"].program.getBytecode()
    # Redefine existing FDEF 1 so maxp budgets are already satisfied, then put
    # a nested FDEF in the body to exercise FreeType's Nested_DEFS error.
    fpgm += bytes.fromhex("b0 01 2c b0 00 2c b0 01 21 2d 2d")
    font["fpgm"].program = program_from_bytes(fpgm)
    save_font("hinter-fpgm-nested-fdef.ttf", font)


def write_fpgm_idef_opcode_overflow() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b8 01 00 89"))
    save_font("hinter-fpgm-idef-opcode-overflow.ttf", font)


def write_fpgm_nested_idef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 84 89 b0 85 89 2d"))
    save_font("hinter-fpgm-nested-idef.ttf", font)


def write_fpgm_unterminated_fdef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 00 2c"))
    save_font("hinter-fpgm-unterminated-fdef.ttf", font)


def write_fpgm_unterminated_idef() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["fpgm"].program = program_from_bytes(bytes.fromhex("b0 84 89 b0 01 21"))
    save_font("hinter-fpgm-unterminated-idef.ttf", font)


def write_opcode_stack_underflow_matrix() -> None:
    """Build one glyph per unvisited TrueType VM operand-pop entry point.

    These are real glyph-load inputs rather than unit probes.  Minimal fpgm/prep
    programs initialize the TrueType execution context while leaving the
    operand stack empty; each glyph's one-byte instruction is then the first
    glyph-program action. FT_LOAD_PEDANTIC makes the pinned C and Rust interpreters expose the
    same ``Too_Few_Arguments`` error and execute the corresponding dispatch
    arm.  Copying the project-authored ``base`` outline keeps every row a
    non-empty simple glyph, so the instruction stream is not skipped by the
    empty-glyph shortcut.
    """
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # A truly empty fpgm/prep makes pinned FreeType skip glyph instructions.
    # SVTCA[0] is stack-neutral and keeps the control programs valid while
    # forcing the glyph interpreter to run each underflow probe.
    font["fpgm"].program = program_from_bytes(bytes([0x00]))
    font["prep"].program = program_from_bytes(bytes([0x00]))
    base_glyph = deepcopy(font["glyf"]["base"])
    base_metrics = font["hmtx"].metrics["base"]

    # Keep the order stable: the generated glyph IDs are 60 onward, directly
    # after the maintained hinter-control-matrix glyphs.  Each opcode is the
    # first VM instruction and therefore intentionally runs with an empty
    # operand stack.  Adjacent opcode families need only one representative to
    # reach their shared match arm.
    opcodes = (
        0x20,  # DUP
        0x21,  # POP
        0x23,  # SWAP
        0x60,  # ADD
        0x61,  # SUB
        0x62,  # DIV
        0x63,  # MUL
        0x64,  # ABS
        0x65,  # NEG
        0x66,  # FLOOR
        0x67,  # CEILING
        0x42,  # WS
        0x43,  # RS
        0x44,  # WCVTP
        0x45,  # RCVT
        0x10,  # SRP0
        0x11,  # SRP1
        0x12,  # SRP2
        0x49,  # MD[0]
        0x46,  # GC[0]
        0x47,  # GC[1]
        0x48,  # SCFS
        0x2E,  # MDAP[0]
        0x3E,  # MIAP[0]
        0xC0,  # MDRP
        0xE0,  # MIRP
        0x3C,  # ALIGNRP
        0x32,  # SHP[0]
        0x34,  # SHC[0]
        0x36,  # SHZ[0]
        0x17,  # SLOOP
        0x2A,  # LOOPCALL
        0x2B,  # CALL
        0x38,  # SHPIX
        0x27,  # ALIGNPTS
        0x25,  # CINDEX
        0x26,  # MINDEX
        0x1A,  # SMD
        0x80,  # FLIPPT
        0x1C,  # JMPR
        0x78,  # JROT
        0x79,  # JROF
        0x08,  # SFVTL[0]
        0x0A,  # SPVFS
        0x0B,  # SFVFS
        0x06,  # SPVTL[0]
        0x0F,  # ISECT
        0x7E,  # SANGW
        0x76,  # SROUND
        0x77,  # S45ROUND
        0x70,  # WCVTF
        0x88,  # GETINFO
        0x29,  # UTP
        0x3A,  # MSIRP[0]
        0x5A,  # AND
        0x5E,  # SDB
        0x5F,  # SDS
        0x86,  # SDPVTL[0]
        0x5D,  # DELTAP1
        0x73,  # DELTAC1
        0x13,  # SZP0
        0x14,  # SZP1
        0x15,  # SZP2
        0x16,  # SZPS
        0x1D,  # SCVTCI
        0x1E,  # SSWCI
        0x1F,  # SSW
        0x56,  # ODD
        0x57,  # EVEN
        0x5C,  # NOT
        0x68,  # ROUND[0]
        0x6C,  # NROUND[0]
        0x8B,  # MAX
        0x8C,  # MIN
        0x8D,  # SCANTYPE
        0x89,  # IDEF
        0x7B,  # undefined custom instruction
    )
    glyph_order = font.getGlyphOrder()
    for index, opcode in enumerate(opcodes):
        name = f"opcodeUnderflow{index:02d}"
        glyph_order.append(name)
        glyph = deepcopy(base_glyph)
        glyph.program = program_from_bytes(bytes([opcode]))
        font["glyf"][name] = glyph
        font["hmtx"].metrics[name] = base_metrics
    font.setGlyphOrder(glyph_order)
    save_font("hinter-opcode-stack-underflow-matrix.ttf", font)


def main() -> None:
    write_empty_fpgm()
    write_empty_glyph_iup()
    write_invalid_contour_endpoints()
    write_invalid_composite_attachment_points()
    write_composite_depth_overflow()
    write_prep_definitions()
    write_prep_idef()
    write_prep_redefine_defs()
    write_fpgm_loopcall()
    write_fpgm_loopcall_redefinition()
    write_called_fpgm_instctrl()
    write_direct_fpgm_instctrl()
    write_fpgm_truncated_definition_pushes()
    write_unterminated_control_flow()
    write_glyph_code_overflow()
    write_glyph_interpreter_errors()
    write_invalid_twilight_scfs()
    write_composite_compatibility_moves()
    write_fpgm_call_errors()
    write_execution_too_long_loop()
    write_opcode_counter_limit()
    write_fpgm_function_overflow()
    write_too_many_hints()
    write_zero_units_per_em()
    write_fpgm_fdef_index_overflow()
    write_idef_recursive_depth()
    write_storage_cvt_reference_errors()
    write_fpgm_nested_fdef()
    write_fpgm_idef_opcode_overflow()
    write_fpgm_nested_idef()
    write_fpgm_unterminated_fdef()
    write_fpgm_unterminated_idef()
    write_opcode_stack_underflow_matrix()


if __name__ == "__main__":
    main()
