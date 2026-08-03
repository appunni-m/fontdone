#!/usr/bin/env python3
"""Build compact, project-authored PCF fixtures."""

from __future__ import annotations

from pathlib import Path
import struct


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "pcf"

PCF_FILE_VERSION = 0x70636601
PCF_PROPERTIES = 1 << 0
PCF_ACCELERATORS = 1 << 1
PCF_METRICS = 1 << 2
PCF_BITMAPS = 1 << 3
PCF_BDF_ENCODINGS = 1 << 5
PCF_SWIDTHS = 1 << 6
PCF_COMPRESSED_METRICS = 0x00000100
PCF_BYTE_MASK = 1 << 2


def align4(data: bytes) -> bytes:
    return data + bytes((-len(data)) % 4)


def properties_table(*, msb: bool = False) -> bytes:
    strings = bytearray()
    offsets: dict[str, int] = {}

    def string_offset(value: str) -> int:
        if value not in offsets:
            offsets[value] = len(strings)
            strings.extend(value.encode("ascii"))
            strings.append(0)
        return offsets[value]

    properties: list[tuple[str, str | int]] = [
        ("FAMILY_NAME", "Fontdone PCF"),
        # PCF exposes every numeric property as signed INTEGER, including
        # names that the BDF driver classifies as CARDINAL.
        ("POINT_SIZE", -120),
        ("PIXEL_SIZE", 10),
        ("RESOLUTION_X", 72),
        ("RESOLUTION_Y", 72),
        ("CHARSET_REGISTRY", "ISO10646"),
        ("CHARSET_ENCODING", "1"),
    ]
    endian = ">" if msb else "<"
    records = bytearray()
    for name, value in properties:
        name_offset = string_offset(name)
        is_string = isinstance(value, str)
        raw_value = string_offset(value) if is_string else value
        records.extend(struct.pack(f"{endian}iBi", name_offset, is_string, raw_value))

    records.extend(bytes((-len(properties)) % 4))
    # FreeType always reads the format word as little-endian, then uses its
    # byte-order bit for the property count, records, and string size.
    format_word = struct.pack("<I", PCF_BYTE_MASK if msb else 0)
    return align4(
        format_word
        + struct.pack(f"{endian}I", len(properties))
        + records
        + struct.pack(f"{endian}I", len(strings))
        + strings
    )


def metric_record() -> bytes:
    return struct.pack("<hhhhhh", 0, 8, 8, 8, 2, 0)


def accelerators_table() -> bytes:
    flags = bytes(
        [
            1,  # noOverlap
            1,  # constantMetrics
            1,  # terminalFont
            1,  # constantWidth
            1,  # inkInside
            0,  # inkMetrics
            0,  # left-to-right
            0,  # padding
        ]
    )
    header = flags + struct.pack("<iii", 8, 2, 0)
    metric = metric_record()
    return align4(struct.pack("<I", 0) + header + metric + metric)


def metrics_table() -> bytes:
    # Compressed metrics store each signed field biased by 0x80.
    metric = bytes([0x80, 0x88, 0x88, 0x88, 0x82])
    return align4(struct.pack("<IH", PCF_COMPRESSED_METRICS, 1) + metric)


def uncompressed_metrics_table() -> bytes:
    # Uncompressed metrics use a 32-bit glyph count followed by six signed
    # 16-bit fields.  Keep this alongside the compressed control so the
    # public PCF property route exercises both metric decoders.
    return align4(struct.pack("<II", 0, 1) + metric_record())


def bitmaps_table() -> bytes:
    bitmap = bytes(
        [
            0b00111100,
            0b01100110,
            0b11000011,
            0b11000011,
            0b11111111,
            0b11000011,
            0b11000011,
            0b11000011,
            0,
            0,
        ]
    )
    sizes = (len(bitmap), len(bitmap) * 2, len(bitmap) * 4, len(bitmap) * 8)
    return align4(
        struct.pack("<II", 0, 1)
        + struct.pack("<I", 0)
        + struct.pack("<IIII", *sizes)
        + bitmap
    )


def encodings_table(*, msb: bool = False) -> bytes:
    endian = ">" if msb else "<"
    format_word = struct.pack("<I", PCF_BYTE_MASK if msb else 0)
    return align4(
        format_word
        + struct.pack(
            f"{endian}HHHHHH",
            65,  # firstCol
            65,  # lastCol
            0,  # firstRow
            0,  # lastRow
            65,  # defaultChar
            0,  # glyph offset
        )
    )


def build_pcf(tables: list[tuple[int, int, bytes]]) -> bytes:
    toc_size = 8 + len(tables) * 16
    offset = toc_size
    toc = bytearray()
    body = bytearray()
    for table_type, table_format, table_data in tables:
        toc.extend(
            struct.pack(
                "<IIII",
                table_type,
                table_format,
                len(table_data),
                offset,
            )
        )
        body.extend(table_data)
        offset += len(table_data)
    return struct.pack("<II", PCF_FILE_VERSION, len(tables)) + toc + body


def main() -> None:
    tables = [
        (PCF_PROPERTIES, 0, properties_table()),
        (PCF_ACCELERATORS, 0, accelerators_table()),
        (PCF_METRICS, PCF_COMPRESSED_METRICS, metrics_table()),
        (PCF_BITMAPS, 0, bitmaps_table()),
        (PCF_BDF_ENCODINGS, 0, encodings_table()),
    ]
    data = build_pcf(tables)
    uncompressed_tables = [
        (PCF_PROPERTIES, 0, properties_table()),
        (PCF_ACCELERATORS, 0, accelerators_table()),
        (PCF_METRICS, 0, uncompressed_metrics_table()),
        (PCF_BITMAPS, 0, bitmaps_table()),
        (PCF_BDF_ENCODINGS, 0, encodings_table()),
    ]
    uncompressed_data = build_pcf(uncompressed_tables)
    invalid_version = struct.pack("<II", 0x12345678, 0)
    overlapping_tables = (
        struct.pack("<II", PCF_FILE_VERSION, 2)
        + struct.pack("<IIII", PCF_SWIDTHS, 0, 8, 40)
        + struct.pack("<IIII", PCF_PROPERTIES, 0, 8, 44)
        + bytes(12)
    )
    invalid_properties_tables = [
        (PCF_PROPERTIES, 0, properties_table(msb=True)),
        *tables[1:],
    ]
    invalid_properties_data = build_pcf(invalid_properties_tables)
    unsupported_properties_payload = bytearray(properties_table())
    struct.pack_into("<I", unsupported_properties_payload, 0, PCF_COMPRESSED_METRICS)
    unsupported_properties_tables = [
        (PCF_PROPERTIES, 0, bytes(unsupported_properties_payload)),
        *tables[1:],
    ]
    unsupported_properties_data = build_pcf(unsupported_properties_tables)
    msb_tables = [
        (PCF_PROPERTIES, PCF_BYTE_MASK, properties_table(msb=True)),
        *tables[1:4],
        (PCF_BDF_ENCODINGS, PCF_BYTE_MASK, encodings_table(msb=True)),
    ]
    msb_data = build_pcf(msb_tables)
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    output = OUT_DIR / "properties-signed-only.pcf"
    if output.exists() or output.is_symlink():
        output.unlink()
    output.write_bytes(data)

    uncompressed_output = OUT_DIR / "properties-uncompressed-metrics.pcf"
    if uncompressed_output.exists() or uncompressed_output.is_symlink():
        uncompressed_output.unlink()
    uncompressed_output.write_bytes(uncompressed_data)

    invalid_version_output = OUT_DIR / "invalid-version.pcf"
    if invalid_version_output.exists() or invalid_version_output.is_symlink():
        invalid_version_output.unlink()
    invalid_version_output.write_bytes(invalid_version)

    overlapping_output = OUT_DIR / "overlapping-tables.pcf"
    if overlapping_output.exists() or overlapping_output.is_symlink():
        overlapping_output.unlink()
    overlapping_output.write_bytes(overlapping_tables)

    invalid_properties_output = OUT_DIR / "invalid-properties-format.pcf"
    if invalid_properties_output.exists() or invalid_properties_output.is_symlink():
        invalid_properties_output.unlink()
    invalid_properties_output.write_bytes(invalid_properties_data)

    unsupported_properties_output = OUT_DIR / "unsupported-properties-format.pcf"
    if unsupported_properties_output.exists() or unsupported_properties_output.is_symlink():
        unsupported_properties_output.unlink()
    unsupported_properties_output.write_bytes(unsupported_properties_data)

    msb_output = OUT_DIR / "properties-msb.pcf"
    if msb_output.exists() or msb_output.is_symlink():
        msb_output.unlink()
    msb_output.write_bytes(msb_data)

    # Keep four bytes after the zero table count so this reaches the PCF
    # driver's Invalid_File_Format table-count check rather than the separate
    # eight-byte stream-operation boundary.
    zero_table_count = struct.pack("<II", PCF_FILE_VERSION, 0) + bytes(4)
    zero_table_output = OUT_DIR / "zero-table-count.pcf"
    if zero_table_output.exists() or zero_table_output.is_symlink():
        zero_table_output.unlink()
    zero_table_output.write_bytes(zero_table_count)

    # Keep only the TOC header so the pinned driver reaches its stream-size
    # Invalid_File_Format guard before attempting the first directory entry.
    truncated_directory = struct.pack("<II", PCF_FILE_VERSION, 1)
    truncated_output = OUT_DIR / "truncated-directory.pcf"
    if truncated_output.exists() or truncated_output.is_symlink():
        truncated_output.unlink()
    truncated_output.write_bytes(truncated_directory)

    # Keep a structurally valid one-entry TOC whose required properties table
    # is absent, reaching the pinned driver's missing-table error path.
    missing_properties = struct.pack(
        "<II",
        PCF_FILE_VERSION,
        1,
    ) + struct.pack("<IIII", PCF_SWIDTHS, 0, 0, 24)
    missing_properties_output = OUT_DIR / "missing-properties-table.pcf"
    if missing_properties_output.exists() or missing_properties_output.is_symlink():
        missing_properties_output.unlink()
    missing_properties_output.write_bytes(missing_properties)

    # Keep a structurally valid one-entry TOC whose table starts at the end of
    # the directory but extends past the stream, reaching the pinned driver's
    # table-range validation error before table-specific parsing.
    table_outside_stream = (
        struct.pack("<II", PCF_FILE_VERSION, 1)
        + struct.pack("<IIII", PCF_PROPERTIES, 0, 1, 24)
    )
    table_outside_output = OUT_DIR / "table-outside-stream.pcf"
    if table_outside_output.exists() or table_outside_output.is_symlink():
        table_outside_output.unlink()
    table_outside_output.write_bytes(table_outside_stream)

    # Keep a one-entry TOC whose table begins inside the directory itself,
    # reaching the first disjunct of the pinned table-range guard.
    table_before_directory = (
        struct.pack("<II", PCF_FILE_VERSION, 1)
        + struct.pack("<IIII", PCF_PROPERTIES, 0, 16, 8)
    )
    table_before_output = OUT_DIR / "table-before-directory.pcf"
    if table_before_output.exists() or table_before_output.is_symlink():
        table_before_output.unlink()
    table_before_output.write_bytes(table_before_directory)


if __name__ == "__main__":
    main()
