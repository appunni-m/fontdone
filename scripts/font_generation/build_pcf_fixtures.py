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
PCF_COMPRESSED_METRICS = 0x00000100


def align4(data: bytes) -> bytes:
    return data + bytes((-len(data)) % 4)


def properties_table() -> bytes:
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
    records = bytearray()
    for name, value in properties:
        name_offset = string_offset(name)
        is_string = isinstance(value, str)
        raw_value = string_offset(value) if is_string else value
        records.extend(struct.pack("<iBi", name_offset, is_string, raw_value))

    records.extend(bytes((-len(properties)) % 4))
    return align4(
        struct.pack("<II", 0, len(properties))
        + records
        + struct.pack("<I", len(strings))
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


def encodings_table() -> bytes:
    return align4(
        struct.pack(
            "<IHHHHHH",
            0,
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
    data = build_pcf(
        [
            (PCF_PROPERTIES, 0, properties_table()),
            (PCF_ACCELERATORS, 0, accelerators_table()),
            (PCF_METRICS, PCF_COMPRESSED_METRICS, metrics_table()),
            (PCF_BITMAPS, 0, bitmaps_table()),
            (PCF_BDF_ENCODINGS, 0, encodings_table()),
        ]
    )
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    output = OUT_DIR / "properties-signed-only.pcf"
    if output.exists() or output.is_symlink():
        output.unlink()
    output.write_bytes(data)

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


if __name__ == "__main__":
    main()
