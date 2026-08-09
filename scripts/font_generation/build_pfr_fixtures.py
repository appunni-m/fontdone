#!/usr/bin/env python3
"""Build compact project-authored PFR metric-service fixtures."""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tests" / "fixtures" / "input" / "fonts" / "pfr"

PFR_PHY_VERTICAL = 0x01
PFR_PHY_2BYTE_CHARCODE = 0x02
PFR_PHY_PROPORTIONAL = 0x04
PFR_PHY_ASCII_CODE = 0x08
PFR_PHY_2BYTE_GPS_SIZE = 0x10
PFR_PHY_3BYTE_GPS_OFFSET = 0x20
PFR_PHY_EXTRA_ITEMS = 0x80

PFR_KERN_2BYTE_CHAR = 0x01
PFR_KERN_2BYTE_ADJ = 0x02

PFR_LOG_STROKE = 0x04
PFR_LOG_2BYTE_STROKE = 0x08
PFR_LOG_BOLD = 0x10
PFR_LOG_2BYTE_BOLD = 0x20
PFR_LOG_EXTRA_ITEMS = 0x40


def u16(value: int) -> bytes:
    return value.to_bytes(2, "big", signed=False)


def i16(value: int) -> bytes:
    return value.to_bytes(2, "big", signed=True)


def u24(value: int) -> bytes:
    return value.to_bytes(3, "big", signed=False)


def i24(value: int) -> bytes:
    return (value & 0xFF_FFFF).to_bytes(3, "big", signed=False)


def build_kerning_item(wide_characters: bool, wide_adjustment: bool) -> bytes:
    item = bytearray([1])
    item.extend(i16(-10))
    item.append(
        (PFR_KERN_2BYTE_CHAR if wide_characters else 0)
        | (PFR_KERN_2BYTE_ADJ if wide_adjustment else 0)
    )
    if wide_characters:
        item.extend(u16(65))
        item.extend(u16(66))
    else:
        item.extend([65, 66])
    item.extend(i16(-2) if wide_adjustment else bytes([2]))
    return bytes(item)


def build_physical(
    flags: int,
    wide_kerning: bool,
    *,
    character_count: int = 2,
    unknown_extra_item: bool = False,
) -> bytes:
    """Build a valid physical record covering the selected descriptor flags."""
    physical = bytearray(15)
    physical[2:4] = u16(1000)
    physical[4:6] = u16(2000)
    physical[6:8] = i16(-20)
    physical[8:10] = i16(-30)
    physical[10:12] = i16(800)
    physical[12:14] = i16(900)
    physical[14] = flags

    if not flags & PFR_PHY_PROPORTIONAL:
        physical.extend(i16(700))
    if flags & PFR_PHY_EXTRA_ITEMS:
        item = build_kerning_item(wide_kerning, wide_kerning)
        physical.append(2 if unknown_extra_item else 1)
        if unknown_extra_item:
            physical.extend([1, 9, 0xAA])
        physical.extend([len(item), 4])
        physical.extend(item)

    # No auxiliary records, no blue values, and zero vertical/horizontal
    # standard values.  The character descriptors follow immediately.
    physical.extend(u24(0))
    physical.append(0)
    physical.extend(bytes(6))
    physical.extend(u16(character_count))

    for code, advance, gps_size, gps_offset in (
        (65, 500, 1, 1),
        (66, 600, 2, 2),
    )[:character_count]:
        if flags & PFR_PHY_2BYTE_CHARCODE:
            physical.extend(u16(code))
        else:
            physical.append(code)
        if flags & PFR_PHY_PROPORTIONAL:
            physical.extend(i16(advance))
        if flags & PFR_PHY_ASCII_CODE:
            physical.append(code)
        physical.extend(
            u16(gps_size) if flags & PFR_PHY_2BYTE_GPS_SIZE else bytes([gps_size])
        )
        physical.extend(
            u24(gps_offset) if flags & PFR_PHY_3BYTE_GPS_OFFSET else u16(gps_offset)
        )

    return bytes(physical)


def build_pfr_stream(
    logical_flags: int,
    physical: bytes,
    high_size: bool,
    *,
    malformed_logical_extra: bool = False,
) -> bytes:
    """Build a PFR0/PFR1 stream around one generated logical font."""
    header_size = 58
    logical_directory_offset = header_size
    logical_offset = logical_directory_offset + 7

    logical = bytearray(13)
    logical[12] = logical_flags
    if logical_flags & PFR_LOG_STROKE:
        logical.append(1)
        if logical_flags & PFR_LOG_2BYTE_STROKE:
            logical.append(2)
        if logical_flags & 0x03 == 0:
            logical.extend(bytes(3))
    if logical_flags & PFR_LOG_BOLD:
        logical.append(1)
        if logical_flags & PFR_LOG_2BYTE_BOLD:
            logical.append(2)
    if logical_flags & PFR_LOG_EXTRA_ITEMS:
        if malformed_logical_extra:
            logical.extend([1, 0xFF, 9])
        else:
            logical.extend([1, 2, 9, 0xAA, 0xBB])

    physical_size_offset = len(logical)
    logical.extend(bytes(5))
    if high_size:
        logical.append((len(physical) >> 16) & 0xFF)
    physical_offset = logical_offset + len(logical)
    logical[physical_size_offset : physical_size_offset + 2] = u16(len(physical) & 0xFFFF)
    logical[physical_size_offset + 2 : physical_size_offset + 5] = u24(physical_offset)

    gps_section_offset = physical_offset + len(physical)
    header = (
        b"PFR0"
        + u16(4)
        + b"\r\n"
        + u16(header_size)
        + u16(7)
        + u16(logical_directory_offset)
        + u16(len(logical))
        + u24(len(logical))
        + u24(logical_offset)
        + u16(len(physical) & 0xFFFF)
        + u24(len(physical))
        + u24(physical_offset)
        + u16(1)
        + u24(3)
        + u24(gps_section_offset)
        + bytes([0, 0, 0])
        + bytes([(len(physical) >> 16) & 0xFF if high_size else 0, 0])
        + u24(0)
        + u24(0)
        + u24(0)
        + u16(1)
        + bytes([0, 0])
        + u16(2)
    )
    assert len(header) == header_size

    logical_directory = u16(1) + u16(len(logical)) + u24(logical_offset)
    gps_section = bytes(3)
    trailer = b"PFR1\r\n\0\0"
    data = header + logical_directory + bytes(logical) + physical + gps_section + trailer
    assert len(data) == gps_section_offset + len(gps_section) + len(trailer)
    return data


def build_extended_fixtures() -> None:
    """Write valid records for public PFR parser-flag parity coverage."""
    fixed = build_physical(0, False)
    (OUT_DIR / "fixed-advance.pfr").write_bytes(build_pfr_stream(0, fixed, False))

    all_physical_flags = (
        PFR_PHY_VERTICAL
        | PFR_PHY_2BYTE_CHARCODE
        | PFR_PHY_PROPORTIONAL
        | PFR_PHY_ASCII_CODE
        | PFR_PHY_2BYTE_GPS_SIZE
        | PFR_PHY_3BYTE_GPS_OFFSET
        | PFR_PHY_EXTRA_ITEMS
    )
    all_logical_flags = (
        PFR_LOG_STROKE
        | PFR_LOG_2BYTE_STROKE
        | PFR_LOG_BOLD
        | PFR_LOG_2BYTE_BOLD
        | PFR_LOG_EXTRA_ITEMS
    )
    all_flags = build_physical(all_physical_flags, True, unknown_extra_item=True)
    (OUT_DIR / "all-descriptor-flags.pfr").write_bytes(
        build_pfr_stream(all_logical_flags, all_flags, False)
    )

    # Exercise the false inner branches for the optional logical fields and a
    # non-miter line join while retaining the outer stroke/bold paths.
    logical_options = build_physical(0, False)
    (OUT_DIR / "logical-options.pfr").write_bytes(
        build_pfr_stream(PFR_LOG_STROKE | PFR_LOG_BOLD | 1, logical_options, False)
    )

    large_physical = build_physical(0, False) + bytes((1 << 16) - len(fixed))
    (OUT_DIR / "high-physical-size.pfr").write_bytes(
        build_pfr_stream(0, large_physical, True)
    )


def build_malformed_fixtures() -> None:
    """Write malformed PFR probes used by the public face-open matrix."""
    def header_probe(version: int, marker: bytes = b"\r\n", size: int = 58) -> bytes:
        header = bytearray(58)
        header[0:4] = b"PFR0"
        header[4:6] = u16(version)
        header[6:8] = marker
        header[8:10] = u16(size)
        return bytes(header)

    (OUT_DIR / "malformed-header-version.pfr").write_bytes(header_probe(5))
    (OUT_DIR / "malformed-header-marker.pfr").write_bytes(
        header_probe(4, marker=b"\0\0")
    )
    (OUT_DIR / "malformed-header-size.pfr").write_bytes(header_probe(4, size=57))

    fixed = build_physical(0, False)
    (OUT_DIR / "short-header.pfr").write_bytes(b"PFR0")
    (OUT_DIR / "truncated-physical-header.pfr").write_bytes(
        build_pfr_stream(0, bytes([0]), False)
    )

    zero_resolution = bytearray(fixed)
    zero_resolution[2:4] = u16(0)
    (OUT_DIR / "zero-resolution.pfr").write_bytes(
        build_pfr_stream(0, bytes(zero_resolution), False)
    )
    zero_metrics_resolution = bytearray(fixed)
    zero_metrics_resolution[4:6] = u16(0)
    (OUT_DIR / "zero-metrics-resolution.pfr").write_bytes(
        build_pfr_stream(0, bytes(zero_metrics_resolution), False)
    )
    (OUT_DIR / "zero-character-count.pfr").write_bytes(
        build_pfr_stream(0, build_physical(0, False, character_count=0), False)
    )

    truncated_logical = bytearray(build_pfr_stream(0, fixed, False))
    truncated_logical[60:62] = u16(17)
    (OUT_DIR / "truncated-logical-font.pfr").write_bytes(truncated_logical)

    (OUT_DIR / "truncated-character-descriptor.pfr").write_bytes(
        build_pfr_stream(0, fixed[:30], False)
    )
    (OUT_DIR / "malformed-logical-extra-item.pfr").write_bytes(
        build_pfr_stream(
            PFR_LOG_EXTRA_ITEMS,
            fixed,
            False,
            malformed_logical_extra=True,
        )
    )


def build_basic_metrics_and_kerning(path: Path) -> tuple[bytes, int]:
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
    return data, physical_record_offset


def main() -> None:
    basic_data, physical_record_offset = build_basic_metrics_and_kerning(
        OUT_DIR / "basic-metrics-and-kerning.pfr"
    )
    malformed_kerning = bytearray(basic_data)
    malformed_kerning[physical_record_offset + 16] = 3
    (OUT_DIR / "malformed-kerning-item.pfr").write_bytes(malformed_kerning)
    build_extended_fixtures()
    build_malformed_fixtures()


if __name__ == "__main__":
    main()
