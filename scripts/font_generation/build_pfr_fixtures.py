#!/usr/bin/env python3
"""Build compact project-authored PFR metric-service fixtures."""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "pfr"


def u16(value: int) -> bytes:
    return value.to_bytes(2, "big", signed=False)


def i16(value: int) -> bytes:
    return value.to_bytes(2, "big", signed=True)


def u24(value: int) -> bytes:
    return value.to_bytes(3, "big", signed=False)


def i24(value: int) -> bytes:
    return (value & 0xFF_FFFF).to_bytes(3, "big", signed=False)


def build_basic_metrics_and_kerning(path: Path) -> None:
    """Write one proportional two-glyph PFR face with two kerning pairs."""
    header_size = 58
    logical_directory_offset = header_size
    logical_record_offset = logical_directory_offset + 7
    logical_record_size = 18
    physical_record_offset = logical_record_offset + logical_record_size

    # PFR physical-font flags: proportional advances plus extra-item table.
    physical_flags = 0x04 | 0x80
    kerning_item = (
        bytes([2])
        + i16(-128)
        + bytes([0])
        + bytes([65, 66, 96])
        + bytes([66, 65, 160])
    )
    physical_record = (
        u16(1)
        + u16(1000)
        + u16(500)
        + i16(0)
        + i16(-200)
        + i16(800)
        + i16(800)
        + bytes([physical_flags])
        + bytes([1, len(kerning_item), 4])
        + kerning_item
        + u24(0)
        + bytes([0])
        + bytes([0, 0])
        + u16(0)
        + u16(0)
        + u16(2)
        + bytes([65])
        + i16(500)
        + bytes([1])
        + u16(1)
        + bytes([66])
        + i16(600)
        + bytes([1])
        + u16(2)
    )
    physical_record_size = len(physical_record)
    gps_section_offset = physical_record_offset + physical_record_size

    header = (
        b"PFR0"
        + u16(4)
        + b"\r\n"
        + u16(header_size)
        + u16(7)
        + u16(logical_directory_offset)
        + u16(logical_record_size)
        + u24(logical_record_size)
        + u24(logical_record_offset)
        + u16(physical_record_size)
        + u24(physical_record_size)
        + u24(physical_record_offset)
        + u16(1)
        + u24(3)
        + u24(gps_section_offset)
        + bytes([0, 0, 0])
        + bytes([0, 0])
        + u24(0)
        + u24(0)
        + u24(0)
        + u16(1)
        + bytes([0, 0])
        + u16(2)
    )
    logical_directory = u16(1) + u16(logical_record_size) + u24(logical_record_offset)
    logical_record = (
        i24(0x01_0000)
        + i24(0)
        + i24(0)
        + i24(0x01_0000)
        + bytes([0])
        + u16(physical_record_size)
        + u24(physical_record_offset)
    )
    gps_section = b"\0\0\0"
    trailer = b"PFR1\r\n\0\0"

    assert len(header) == header_size
    assert len(logical_directory) == 7
    assert len(logical_record) == logical_record_size
    assert physical_record_size == 52
    data = header + logical_directory + logical_record + physical_record + gps_section + trailer
    assert len(data) == 146

    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    path.write_bytes(data)


def main() -> None:
    build_basic_metrics_and_kerning(OUT_DIR / "basic-metrics-and-kerning.pfr")


if __name__ == "__main__":
    main()
