#!/usr/bin/env python3
"""Build small source-backed variable fonts for FTMM future parity rows."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables.DefaultTable import DefaultTable
from fontTools.varLib import instancer


ROOT = Path(__file__).resolve().parents[2]
BASE_FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "variable" / "compact-variable.ttf"
MVAR_FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "variation" / "mvar-vertical-metrics.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "variable"

GVAR_EMBEDDED_PEAK_TUPLE = 0x8000
GVAR_PRIVATE_POINT_NUMBERS = 0x2000


def save_font(path: Path, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    font.save(path, reorderTables=True)


def write_inter_wght() -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    # Keep a real variable font, but pin the width axis.  This leaves the
    # source-backed `wght` axis, fvar named instances, gvar, and HVAR data for
    # the single-axis FTMM coordinate rows.
    font = instancer.instantiateVariableFont(font, {"wdth": 100.0}, inplace=False)
    save_font(OUT_DIR / "inter-wght.ttf", font)


def write_compact_alias(name: str) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    save_font(OUT_DIR / name, font)


def write_mvar_alias(name: str) -> None:
    font = TTFont(MVAR_FONT, recalcTimestamp=False)
    save_font(OUT_DIR / name, font)


def raw_table(tag: str, data: bytes) -> DefaultTable:
    table = DefaultTable(tag)
    table.data = data
    return table


def put_u16(data: bytearray, offset: int, value: int) -> None:
    data[offset : offset + 2] = value.to_bytes(2, "big")


def put_u32(data: bytearray, offset: int, value: int) -> None:
    data[offset : offset + 4] = value.to_bytes(4, "big")


def put_i16(data: bytearray, value: int) -> None:
    data.extend(value.to_bytes(2, "big", signed=True))


def write_gvar_payload(
    name: str, payload: bytes, *, remove_hvar: bool = False
) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    font["gvar"] = raw_table("gvar", payload)
    if remove_hvar:
        del font["HVAR"]
    save_font(OUT_DIR / name, font)


def write_table_payload(base_path: Path, tag: str, name: str, payload: bytes) -> None:
    font = TTFont(base_path, recalcTimestamp=False)
    font[tag] = raw_table(tag, payload)
    save_font(OUT_DIR / name, font)


def empty_gvar_payload(
    *, long_offsets: bool, shared_tuples: list[list[int]], axis_count: int = 2
) -> bytes:
    """Build a parsed gvar table with no glyph records.

    The table is intentionally small, but its directory is complete.  This
    makes the face-open rows reach both offset widths and shared-tuple reads
    without making glyph output depend on a synthetic outline mutation.
    """

    glyph_count = 20
    offset_width = 4 if long_offsets else 2
    offsets_start = 20
    shared_offset = offsets_start + (glyph_count + 1) * offset_width
    shared_bytes = b"".join(
        value.to_bytes(2, "big", signed=True)
        for tuple_values in shared_tuples
        for value in tuple_values
    )
    data_offset = shared_offset + len(shared_bytes)
    payload = bytearray(data_offset)
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, len(shared_tuples))
    put_u32(payload, 8, shared_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 1 if long_offsets else 0)
    put_u32(payload, 16, data_offset)
    payload[shared_offset:data_offset] = shared_bytes
    for index in range(glyph_count + 1):
        offset = offsets_start + index * offset_width
        if long_offsets:
            put_u32(payload, offset, 0)
        else:
            put_u16(payload, offset, 0)
    return bytes(payload)


def single_tuple_gvar_payload(
    *, glyph_index: int, point_count: int, private_all_points: bool = False
) -> bytes:
    """Build one default-active tuple record for a selected base glyph.

    The compact variable font has 20 glyphs.  The zero peak makes the tuple
    active at the default instance, so the ordinary ``FT_Load_Glyph`` route
    reaches the packed point and delta readers without a coordinate setter.
    """

    glyph_count = 20
    axis_count = 2
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2

    tuple_index = GVAR_EMBEDDED_PEAK_TUPLE
    tuple_data = bytearray()
    if private_all_points:
        tuple_index |= GVAR_PRIVATE_POINT_NUMBERS
        tuple_data.append(0)

    if point_count > 64:
        raise ValueError("single-tuple fixture point count exceeds packed run width")
    zero_run = 0x80 | (point_count - 1)
    tuple_data.extend((zero_run, zero_run))
    if len(tuple_data) % 2:
        tuple_data.append(0)

    glyph_data = bytearray()
    glyph_data.extend((1).to_bytes(2, "big"))
    # The tuple data begins after the tuple-count/data-offset fields, the
    # variation-size/index fields, and both embedded two-byte peak values.
    glyph_data.extend((12).to_bytes(2, "big"))
    glyph_data.extend(len(tuple_data).to_bytes(2, "big"))
    glyph_data.extend(tuple_index.to_bytes(2, "big"))
    glyph_data.extend((0).to_bytes(2, "big"))
    glyph_data.extend((0).to_bytes(2, "big"))
    glyph_data.extend(tuple_data)
    if len(glyph_data) % 2:
        glyph_data.append(0)

    payload = bytearray(data_offset + len(glyph_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= glyph_index else len(glyph_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset:] = glyph_data
    return bytes(payload)


def partial_points_gvar_payload() -> bytes:
    """Build a default-active tuple that requires public IUP interpolation.

    Glyph 10 has two contours with 30 outline points.  Selecting endpoints
    and an interior point from each contour leaves gaps on both sides of a
    reference delta, so the ordinary glyph-load route reaches FreeType's
    partial-point interpolation before the output outline is compared.
    """

    glyph_count = 20
    axis_count = 2
    glyph_index = 10
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2

    points = [0, 7, 13, 14, 20, 29]
    point_data = bytearray((len(points), len(points) - 1))
    previous = 0
    for point in points:
        point_data.append(point - previous)
        previous = point

    def packed(values: list[int]) -> bytes:
        return bytes([len(values) - 1, *(value & 0xFF for value in values)])

    tuple_data = point_data + packed([4, 0, -2, 1, 0, 3]) + packed([0, 2, 0, -1, 1, 0])
    tuple_index = GVAR_EMBEDDED_PEAK_TUPLE | GVAR_PRIVATE_POINT_NUMBERS

    glyph_data = bytearray()
    glyph_data.extend((1).to_bytes(2, "big"))
    glyph_data.extend((12).to_bytes(2, "big"))
    glyph_data.extend(len(tuple_data).to_bytes(2, "big"))
    glyph_data.extend(tuple_index.to_bytes(2, "big"))
    glyph_data.extend((0).to_bytes(2, "big"))
    glyph_data.extend((0).to_bytes(2, "big"))
    glyph_data.extend(tuple_data)
    if len(glyph_data) % 2:
        glyph_data.append(0)

    payload = bytearray(data_offset + len(glyph_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= glyph_index else len(glyph_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset:] = glyph_data
    return bytes(payload)


def leading_and_single_reference_iup_gvar_payload() -> bytes:
    """Reach IUP's leading-gap and single-reference contour branches.

    Glyph 10's first contour has two references whose intervening points use
    the strict interior interpolation path and whose leading points require
    interpolation before the first reference.  Its second contour has one
    reference only, which reaches FreeType's copy-to-contour path.
    """

    glyph_count = 20
    axis_count = 2
    glyph_index = 10
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2

    points = [7, 13, 20]
    point_data = bytearray((len(points), len(points) - 1))
    previous = 0
    for point in points:
        point_data.append(point - previous)
        previous = point

    def packed(values: list[int]) -> bytes:
        return bytes([len(values) - 1, *(value & 0xFF for value in values)])

    tuple_data = point_data + packed([4, -2, 1]) + packed([0, 2, -1])
    tuple_index = GVAR_EMBEDDED_PEAK_TUPLE | GVAR_PRIVATE_POINT_NUMBERS

    glyph_data = bytearray()
    glyph_data.extend((1).to_bytes(2, "big"))
    glyph_data.extend((12).to_bytes(2, "big"))
    glyph_data.extend(len(tuple_data).to_bytes(2, "big"))
    glyph_data.extend(tuple_index.to_bytes(2, "big"))
    # Keep the width axis at its default and activate the wght axis at 500.
    # The compact variable font normalizes that design coordinate to 0.25,
    # then its avar map changes it to 0.325 (5325 in F2DOT14), so this tuple
    # is active at full scalar after avar normalization.
    glyph_data.extend((0).to_bytes(2, "big"))
    glyph_data.extend((5325).to_bytes(2, "big", signed=True))
    glyph_data.extend(tuple_data)
    if len(glyph_data) % 2:
        glyph_data.append(0)

    payload = bytearray(data_offset + len(glyph_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= glyph_index else len(glyph_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset:] = glyph_data
    return bytes(payload)


def scalar_region_gvar_payload() -> bytes:
    """Build non-intermediate and intermediate wght tuples for glyph 10."""

    glyph_count = 20
    axis_count = 2
    glyph_index = 10
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2

    headers = bytearray()
    headers.extend((2).to_bytes(2, "big"))
    headers.extend((28).to_bytes(2, "big"))

    # A positive non-intermediate peak exercises the proportional branch when
    # the wght coordinate is below the peak.
    headers.extend((2).to_bytes(2, "big"))
    headers.extend(GVAR_EMBEDDED_PEAK_TUPLE.to_bytes(2, "big"))
    put_i16(headers, 0)
    put_i16(headers, 0x2000)

    # The same peak with an open intermediate region exercises both sides of
    # the peak while wght remains between the explicit start and end values.
    headers.extend((2).to_bytes(2, "big"))
    headers.extend((GVAR_EMBEDDED_PEAK_TUPLE | 0x4000).to_bytes(2, "big"))
    put_i16(headers, 0)
    put_i16(headers, 0x2000)
    put_i16(headers, 0)
    put_i16(headers, 0)
    put_i16(headers, 0)
    put_i16(headers, 0x4000)
    assert len(headers) == 28

    glyph_data = headers + bytearray((0xA1, 0xA1, 0xA1, 0xA1))
    payload = bytearray(data_offset + len(glyph_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= glyph_index else len(glyph_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset:] = glyph_data
    return bytes(payload)


def malformed_packed_gvar_payload() -> bytes:
    """Build active tuple records whose packed payloads end prematurely.

    FreeType ignores each malformed tuple while continuing the glyph load.  A
    single glyph record covers private-point, point-run, X-delta, Y-delta, and
    tuple-frame truncation controls without changing the rendered outline.
    """

    glyph_count = 20
    axis_count = 2
    glyph_index = 10
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2

    headers = bytearray()
    headers.extend((5).to_bytes(2, "big"))
    headers.extend((44).to_bytes(2, "big"))
    for variation_size, tuple_index in (
        (1, GVAR_EMBEDDED_PEAK_TUPLE | GVAR_PRIVATE_POINT_NUMBERS),
        (1, GVAR_EMBEDDED_PEAK_TUPLE | GVAR_PRIVATE_POINT_NUMBERS),
        (1, GVAR_EMBEDDED_PEAK_TUPLE),
        (1, GVAR_EMBEDDED_PEAK_TUPLE),
        (5, GVAR_EMBEDDED_PEAK_TUPLE),
    ):
        headers.extend(variation_size.to_bytes(2, "big"))
        headers.extend(tuple_index.to_bytes(2, "big"))
        put_i16(headers, 0)
        put_i16(headers, 0)

    # Wide private point count without its second byte; byte point count
    # without its run; an incomplete implicit X stream; an X stream with no Y
    # stream; and a final tuple whose declared frame exceeds the glyph record.
    glyph_data = headers + bytearray((0x80, 0x01, 0x83, 0xA1))
    assert len(headers) == 44
    assert len(glyph_data) == 48

    payload = bytearray(data_offset + len(glyph_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= glyph_index else len(glyph_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset:] = glyph_data
    return bytes(payload)


def malformed_shared_points_gvar_payload() -> bytes:
    """Build an active tuple with a truncated shared point-number list."""

    glyph_count = 20
    axis_count = 2
    glyph_index = 10
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2

    glyph_data = bytearray()
    glyph_data.extend((0x8001).to_bytes(2, "big"))
    glyph_data.extend((12).to_bytes(2, "big"))
    glyph_data.extend((1).to_bytes(2, "big"))
    glyph_data.extend(GVAR_EMBEDDED_PEAK_TUPLE.to_bytes(2, "big"))
    put_i16(glyph_data, 0)
    put_i16(glyph_data, 0)
    # A wide shared point count is missing its second count byte.  The final
    # pad keeps the glyph record on the short-offset table's even boundary.
    glyph_data.extend((0x80, 0))

    payload = bytearray(data_offset + len(glyph_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= glyph_index else len(glyph_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset:] = glyph_data
    return bytes(payload)


def clamped_offsets_gvar_payload() -> bytes:
    """Build a valid record behind out-of-range and non-monotonic offsets."""

    payload = bytearray(
        single_tuple_gvar_payload(
            glyph_index=10, point_count=34, private_all_points=True
        )
    )
    offsets_start = 20
    # FreeType clamps the glyph-10 end offset to the table limit, then carries
    # that limit forward when the following offset moves backwards.
    put_u16(payload, offsets_start + 11 * 2, 0x7FFF)
    put_u16(payload, offsets_start + 12 * 2, 0)
    return bytes(payload)


def short_glyph_record_gvar_payload(glyph_data: bytes) -> bytes:
    """Build a gvar table with a selected glyph record shorter than its header.

    The matching FTMM route selects the font's default design tuple. Pinned
    FreeType therefore clears variation mode and does not enter this record at
    runtime; the fixtures preserve the malformed bytes for default-instance
    bypass coverage.
    """

    glyph_count = 20
    axis_count = 2
    glyph_index = 10
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2
    payload = bytearray(data_offset + len(glyph_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= glyph_index else len(glyph_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset:] = glyph_data
    return bytes(payload)


def packed_edge_gvar_payload(*, shared_points: bool, default_active: bool = False) -> bytes:
    """Build a valid two-axis gvar record for glyph 10.

    Glyph 10 has 30 outline points plus four phantom points in the compact
    variable base.  The first tuple uses all points (or a shared two-point
    list), the second uses byte point numbers plus byte/word deltas, and the
    third uses a wide point-number run.  The ordinary edge variant peaks at
    +1.0 on the first axis; the optional default-active variant uses zero peaks
    so the ordinary load-glyph route exercises packed decoding directly.
    """

    glyph_count = 20
    axis_count = 2
    offsets_start = 20
    data_offset = offsets_start + (glyph_count + 1) * 2
    tuple_count_flags = 3 | (0x8000 if shared_points else 0)

    headers = bytearray()
    headers.extend(tuple_count_flags.to_bytes(2, "big"))
    headers.extend((36).to_bytes(2, "big"))

    # Tuple 1: an embedded peak and intermediate region.  Its zero runs use
    # all 34 points when no shared point list is present, otherwise two shared
    # points.
    headers.extend((2).to_bytes(2, "big"))
    headers.extend((0xC000).to_bytes(2, "big"))
    put_i16(headers, 0 if default_active else 0x2000)
    put_i16(headers, 0)
    put_i16(headers, 0)
    put_i16(headers, 0)
    put_i16(headers, 0 if default_active else 0x4000)
    put_i16(headers, 0)

    # Tuple 2: private byte point numbers, byte X deltas, and word Y deltas.
    headers.extend((12).to_bytes(2, "big"))
    headers.extend((0xA000).to_bytes(2, "big"))
    put_i16(headers, 0 if default_active else 0x4000)
    put_i16(headers, 0)

    # Tuple 3: private word point numbers and zero-packed deltas.
    headers.extend((9).to_bytes(2, "big"))
    headers.extend((0xA000).to_bytes(2, "big"))
    put_i16(headers, 0 if default_active else 0x4000)
    put_i16(headers, 0)
    assert len(headers) == 36

    tuple_data = bytearray()
    if shared_points:
        # Two point numbers: 0 and 1, encoded as a byte run.
        tuple_data.extend((2, 0x01, 0, 1))
        tuple_data.extend((0x81, 0x81))
    else:
        # Two zero-packed streams covering glyph 10's 34 points.
        tuple_data.extend((0xA1, 0xA1))

    # Private points 0 and 1, followed by byte X and word Y deltas.
    tuple_data.extend((2, 0x01, 0, 1))
    tuple_data.extend((0x01, 1, 2))
    tuple_data.extend((0x41, 0, 0, 0, 3))

    # Wide count (two bytes), wide point-number deltas, then zero-packed X/Y.
    tuple_data.extend((0x80, 0x02, 0x81, 0, 0, 0, 1, 0x81, 0x81))
    if len(tuple_data) % 2:
        tuple_data.append(0)

    payload = bytearray(data_offset + len(headers) + len(tuple_data))
    put_u16(payload, 0, 1)
    put_u16(payload, 4, axis_count)
    put_u16(payload, 6, 0)
    put_u32(payload, 8, data_offset)
    put_u16(payload, 12, glyph_count)
    put_u16(payload, 14, 0)
    put_u32(payload, 16, data_offset)
    for index in range(glyph_count + 1):
        glyph_offset = 0 if index <= 10 else len(headers) + len(tuple_data)
        put_u16(payload, offsets_start + index * 2, glyph_offset // 2)
    payload[data_offset : data_offset + len(headers)] = headers
    payload[data_offset + len(headers) :] = tuple_data
    return bytes(payload)


def write_gvar_fixtures() -> None:
    # These are optional-table controls: Font::from_data keeps opening the
    # face while the malformed gvar parser result is discarded, matching the
    # pinned SFNT driver's face-open behavior.
    write_gvar_payload("gvar-short.ttf", b"\0" * 4)

    unsupported = bytearray(20)
    put_u16(unsupported, 0, 2)
    put_u16(unsupported, 12, 20)
    write_gvar_payload("gvar-version-2.ttf", bytes(unsupported))

    mismatch = bytearray(20)
    put_u16(mismatch, 0, 1)
    put_u16(mismatch, 12, 19)
    write_gvar_payload("gvar-glyph-count-mismatch.ttf", bytes(mismatch))

    truncated_offsets = bytearray(60)
    put_u16(truncated_offsets, 0, 1)
    put_u16(truncated_offsets, 4, 2)
    put_u16(truncated_offsets, 12, 20)
    put_u32(truncated_offsets, 16, 62)
    write_gvar_payload("gvar-offset-array-short.ttf", bytes(truncated_offsets))

    write_gvar_payload(
        "gvar-shared-tuple.ttf",
        empty_gvar_payload(long_offsets=False, shared_tuples=[[0x4000, 0]]),
    )
    write_gvar_payload(
        "gvar-long-offsets.ttf",
        empty_gvar_payload(long_offsets=True, shared_tuples=[]),
    )
    write_gvar_payload(
        "gvar-packed-edge.ttf", packed_edge_gvar_payload(shared_points=False)
    )
    write_gvar_payload(
        "gvar-shared-points.ttf", packed_edge_gvar_payload(shared_points=True)
    )
    write_gvar_payload(
        "gvar-packed-default.ttf",
        packed_edge_gvar_payload(shared_points=False, default_active=True),
    )
    write_gvar_payload(
        "gvar-shared-points-default.ttf",
        packed_edge_gvar_payload(shared_points=True, default_active=True),
    )
    write_gvar_payload(
        "gvar-all-points-default.ttf",
        single_tuple_gvar_payload(
            glyph_index=10, point_count=34, private_all_points=True
        ),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-interpolation-partial-points.ttf",
        partial_points_gvar_payload(),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-interpolation-leading-single-reference.ttf",
        leading_and_single_reference_iup_gvar_payload(),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-empty-outline-default.ttf",
        single_tuple_gvar_payload(glyph_index=17, point_count=4),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-axis-count-mismatch.ttf",
        empty_gvar_payload(long_offsets=False, shared_tuples=[], axis_count=3),
    )
    write_gvar_payload(
        "gvar-scalar-regions.ttf",
        scalar_region_gvar_payload(),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-malformed-packed.ttf",
        malformed_packed_gvar_payload(),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-malformed-shared-points.ttf",
        malformed_shared_points_gvar_payload(),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-offsets-clamped.ttf",
        clamped_offsets_gvar_payload(),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-glyph-data-short-runtime.ttf",
        short_glyph_record_gvar_payload(b"\0\0"),
        remove_hvar=True,
    )
    write_gvar_payload(
        "gvar-tuple-header-short-runtime.ttf",
        short_glyph_record_gvar_payload(b"\0\1\0\0"),
        remove_hvar=True,
    )


def write_avar_fixtures() -> None:
    base = TTFont(BASE_FONT, recalcTimestamp=False).getTableData("avar")

    # `avar` is optional at face construction.  These malformed controls keep
    # the variable face openable while reaching each parser rejection branch
    # through FT_New_Memory_Face, just as the pinned SFNT driver does.
    write_table_payload(BASE_FONT, "avar", "avar-short.ttf", b"\0" * 4)

    unsupported = bytearray(base)
    put_u32(unsupported, 0, 0x0002_0000)
    write_table_payload(
        BASE_FONT, "avar", "avar-version-2.ttf", bytes(unsupported)
    )

    axis_mismatch = bytearray(base)
    put_u16(axis_mismatch, 6, 1)
    write_table_payload(
        BASE_FONT, "avar", "avar-axis-count-mismatch.ttf", bytes(axis_mismatch)
    )

    truncated = bytearray(base)
    put_u16(truncated, 8, 0xFFFF)
    write_table_payload(
        BASE_FONT, "avar", "avar-map-truncated.ttf", bytes(truncated)
    )

def base_hvar_payload() -> bytes:
    return TTFont(BASE_FONT, recalcTimestamp=False).getTableData("HVAR")


def write_hvar_fixtures() -> None:
    base = base_hvar_payload()

    # Optional HVAR parsing is ignored by face construction when the table is
    # malformed, just as FreeType's SFNT driver ignores other optional-table
    # failures.  Keep these forms source-backed so parser guards execute via
    # FT_New_Memory_Face rather than private parser calls.
    write_table_payload(BASE_FONT, "HVAR", "hvar-short.ttf", b"\0" * 4)

    unsupported = bytearray(12)
    put_u16(unsupported, 0, 2)
    write_table_payload(BASE_FONT, "HVAR", "hvar-version-2.ttf", bytes(unsupported))

    no_map = bytearray(base)
    put_u32(no_map, 8, 0)
    write_table_payload(BASE_FONT, "HVAR", "hvar-no-advance-map.ttf", bytes(no_map))

    map_offset = int.from_bytes(base[8:12], "big")
    old_map = base[map_offset:]
    old_count = int.from_bytes(old_map[2:4], "big")
    old_entries = old_map[4:]
    assert old_map[0] == 0 and old_count == len(old_entries)

    # Format 1 uses the same one-byte entries as the compact base map but a
    # 32-bit entry count, which reaches DeltaSetIndexMap's alternate header.
    format_one_map = bytearray((1, old_map[1]))
    format_one_map.extend(old_count.to_bytes(4, "big"))
    format_one_map.extend(old_entries)
    format_one = bytearray(base[:map_offset]) + format_one_map
    write_table_payload(BASE_FONT, "HVAR", "hvar-map-format-one.ttf", bytes(format_one))

    # A four-byte map entry can carry FreeType's all-ones sentinel.  Keep the
    # other entries equivalent to the compact map and make glyph 10 sentinel
    # so the public glyph-load route observes the default zero delta.
    sentinel_map = bytearray((1, 0x33))
    sentinel_map.extend(old_count.to_bytes(4, "big"))
    for glyph_index, entry in enumerate(old_entries):
        if glyph_index == 10:
            map_data = 0xFFFF_FFFF
        else:
            outer = entry >> 3
            inner = entry & 0x07
            map_data = (outer << 4) | inner
        sentinel_map.extend(map_data.to_bytes(4, "big"))
    sentinel = bytearray(base[:map_offset]) + sentinel_map
    write_table_payload(BASE_FONT, "HVAR", "hvar-map-sentinel.ttf", bytes(sentinel))

    store_offset = int.from_bytes(base[4:8], "big")
    data_count = int.from_bytes(base[store_offset + 6 : store_offset + 8], "big")
    data_offsets = [
        int.from_bytes(
            base[store_offset + 8 + index * 4 : store_offset + 12 + index * 4],
            "big",
        )
        for index in range(data_count)
    ]
    region_list = store_offset + int.from_bytes(
        base[store_offset + 2 : store_offset + 6], "big"
    )
    region_count = int.from_bytes(base[region_list + 2 : region_list + 4], "big")
    first_start = store_offset + data_offsets[0]

    # These malformed optional tables are deliberately kept openable.  The
    # SFNT face route must ignore the failed HVAR parser just as FreeType does,
    # while the public input rows retain the exact face-record comparison.
    store_format = bytearray(base)
    put_u16(store_format, store_offset, 2)
    write_table_payload(
        BASE_FONT, "HVAR", "hvar-store-format-2.ttf", bytes(store_format)
    )

    store_without_data = bytearray(base)
    put_u16(store_without_data, store_offset + 6, 0)
    write_table_payload(
        BASE_FONT, "HVAR", "hvar-store-no-var-data.ttf", bytes(store_without_data)
    )

    store_axis_mismatch = bytearray(base)
    put_u16(store_axis_mismatch, region_list, 1)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-axis-count-mismatch.ttf",
        bytes(store_axis_mismatch),
    )

    # Keep the region list structurally valid while forcing invalid
    # start/peak/end triplets for each repair predicate.  FreeType repairs the
    # peak to zero instead of rejecting the optional HVAR table; the public
    # face-open route therefore reaches the same defensive normalization in
    # the Rust parser.
    store_invalid_region_axis = bytearray(base)
    # start > peak
    put_u16(store_invalid_region_axis, region_list + 4, 0x2000)
    put_u16(store_invalid_region_axis, region_list + 6, 0)
    put_u16(store_invalid_region_axis, region_list + 8, 0)
    # start < 0 && end > 0
    put_u16(store_invalid_region_axis, region_list + 10, 0xE000)
    put_u16(store_invalid_region_axis, region_list + 12, 0)
    put_u16(store_invalid_region_axis, region_list + 14, 0x2000)
    # peak > end
    put_u16(store_invalid_region_axis, region_list + 16, 0)
    put_u16(store_invalid_region_axis, region_list + 18, 0x2000)
    put_u16(store_invalid_region_axis, region_list + 20, 0)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-invalid-region-axis.ttf",
        bytes(store_invalid_region_axis),
    )

    store_region_limit = bytearray(base)
    put_u16(store_region_limit, region_list + 2, 0x8000)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-region-count-limit.ttf",
        bytes(store_region_limit),
    )

    store_delta_counts = bytearray(base)
    put_u16(store_delta_counts, first_start + 2, 2)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-invalid-delta-counts.ttf",
        bytes(store_delta_counts),
    )

    # Exercise the other half of the item-variation delta-count guard: the
    # region-index array is larger than the parsed region list while the word
    # delta count itself remains valid.
    store_region_index_count = bytearray(base)
    put_u16(store_region_index_count, first_start + 2, 0)
    put_u16(store_region_index_count, first_start + 4, region_count + 1)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-region-index-count-too-large.ttf",
        bytes(store_region_index_count),
    )

    store_region_index = bytearray(base)
    put_u16(store_region_index, first_start + 6, region_count)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-region-index-out-of-range.ttf",
        bytes(store_region_index),
    )

    store_truncated = bytearray(base)
    put_u16(store_truncated, first_start, 0xFFFF)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-delta-set-truncated.ttf",
        bytes(store_truncated),
    )

    map_unsupported = bytearray(base)
    map_unsupported[map_offset] = 2
    write_table_payload(
        BASE_FONT, "HVAR", "hvar-map-format-unsupported.ttf", bytes(map_unsupported)
    )

    map_invalid_entry = bytearray(base)
    map_invalid_entry[map_offset + 1] |= 0xC0
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-map-entry-format-invalid.ttf",
        bytes(map_invalid_entry),
    )

    map_truncated = bytearray(base)
    put_u16(map_truncated, map_offset + 2, 0xFFFF)
    write_table_payload(
        BASE_FONT, "HVAR", "hvar-map-entry-truncated.ttf", bytes(map_truncated)
    )

    map_outer_index = bytearray(base)
    map_outer_index[map_offset + 4] = 0x28
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-map-outer-index-out-of-range.ttf",
        bytes(map_outer_index),
    )

    map_inner_index = bytearray(base)
    map_inner_index[map_offset + 4] = 0x07
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-map-inner-index-out-of-range.ttf",
        bytes(map_inner_index),
    )

    map_empty = bytearray(base)
    put_u16(map_empty, map_offset + 2, 0)
    write_table_payload(BASE_FONT, "HVAR", "hvar-map-empty.ttf", bytes(map_empty))

    store_empty_regions = bytearray(base)
    put_u16(store_empty_regions, first_start + 2, 0)
    put_u16(store_empty_regions, first_start + 4, 0)
    write_table_payload(
        BASE_FONT,
        "HVAR",
        "hvar-store-empty-region-indexes.ttf",
        bytes(store_empty_regions),
    )

    # Convert the first one-region varData block from short to long words.
    # The signed values are preserved, so the public output remains the same
    # while ItemVariationStore's 32-bit and mixed-width delta readers are
    # exercised on an active public variation load.
    second_start = store_offset + data_offsets[1]
    first_block = base[first_start:second_start]
    item_count = int.from_bytes(first_block[0:2], "big")
    word_delta_count = int.from_bytes(first_block[2:4], "big")
    first_block_region_count = int.from_bytes(first_block[4:6], "big")
    assert item_count == 3 and word_delta_count == 1 and first_block_region_count == 1
    long_block = bytearray(first_block[:6])
    put_u16(long_block, 2, 0x8001)
    put_u16(long_block, 4, 2)
    long_block.extend(first_block[6:8])
    long_block.extend(b"\0\1")
    for index in range(item_count):
        short_delta = int.from_bytes(
            first_block[8 + index * 2 : 10 + index * 2], "big", signed=True
        )
        long_block.extend(short_delta.to_bytes(4, "big", signed=True))
        long_block.extend(b"\0\0")
    delta = len(long_block) - len(first_block)
    long_words = bytearray(base[:first_start]) + long_block + bytearray(base[second_start:])
    for index, relative in enumerate(data_offsets[1:], start=1):
        put_u32(long_words, store_offset + 8 + index * 4, relative + delta)
    put_u32(long_words, 8, map_offset + delta)
    write_table_payload(BASE_FONT, "HVAR", "hvar-long-words.ttf", bytes(long_words))


def base_mvar_payload() -> bytes:
    return TTFont(MVAR_FONT, recalcTimestamp=False).getTableData("MVAR")


def write_mvar_fixtures() -> None:
    base = base_mvar_payload()
    write_table_payload(MVAR_FONT, "MVAR", "mvar-short.ttf", b"\0" * 4)

    unsupported = bytearray(12)
    put_u16(unsupported, 0, 2)
    write_table_payload(MVAR_FONT, "MVAR", "mvar-version-2.ttf", bytes(unsupported))

    small_record = bytearray(12)
    put_u16(small_record, 0, 1)
    put_u16(small_record, 6, 4)
    write_table_payload(
        MVAR_FONT, "MVAR", "mvar-record-size-small.ttf", bytes(small_record)
    )

    # Make the record array overflow before the item store is reached.
    overflow = bytearray(base[:60])
    put_u16(overflow, 8, 7)
    write_table_payload(
        MVAR_FONT, "MVAR", "mvar-record-array-short.ttf", bytes(overflow)
    )

    # Keep the MVAR record structurally valid but point one supported tag at
    # an outer variation-data block that does not exist.  FreeType retains the
    # record and returns a zero delta when the item is queried.
    outer_index = bytearray(base)
    put_u16(outer_index, 16, 1)
    write_table_payload(
        MVAR_FONT,
        "MVAR",
        "mvar-record-outer-index-out-of-range.ttf",
        bytes(outer_index),
    )

    # Preserve the six supported records and append an unknown tag.  The
    # public vertical-header route ignores the unknown record, exercising the
    # same default match arm as FreeType's tag switch.
    unknown = bytearray(base)
    item_store_offset = int.from_bytes(base[10:12], "big")
    unknown_record = b"TEST\0\0\0\0"
    unknown = unknown[:item_store_offset] + unknown_record + unknown[item_store_offset:]
    put_u16(unknown, 8, 7)
    put_u16(unknown, 10, item_store_offset + len(unknown_record))
    write_table_payload(MVAR_FONT, "MVAR", "mvar-unknown-record.ttf", bytes(unknown))


def main() -> None:
    write_inter_wght()
    write_compact_alias("multi-axis-named-instances.ttf")
    write_compact_alias("named-instances-wght-wdth.ttf")
    write_compact_alias("named-instance-missing-psid.ttf")
    write_compact_alias("gvar-hvar-wght.ttf")
    write_mvar_alias("mvar-hvar-vvar.ttf")
    write_avar_fixtures()
    write_gvar_fixtures()
    write_hvar_fixtures()
    write_mvar_fixtures()


if __name__ == "__main__":
    main()
