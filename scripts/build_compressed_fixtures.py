#!/usr/bin/env python3
"""Build deterministic gzip, bzip2, and Unix-compress parity payloads."""

from __future__ import annotations

import bz2
import gzip
import io
import json
from pathlib import Path
import struct
import zlib


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests" / "fixtures"
GZIP_OUT = FIXTURES / "compressed" / "gzip"
BZIP2_OUT = FIXTURES / "compressed" / "bzip2"
LZW_OUT = FIXTURES / "compressed" / "lzw"
RAW_PCF = FIXTURES / "input" / "fonts" / "bitmap" / "bitmap-only.pcf"


PAYLOADS = {
    "small_text": b"fontdone gzip parity fixture\nsmall text payload\n",
    "empty": b"",
}

STREAM_PAYLOADS = {
    "small_stream": b"fontdone gzip stream fixture\n" * 16,
    "large_stream": (b"fontdone gzip stream large fixture block\n" * 1200) + b"tail\n",
}

# Legal gzip header flag combinations used by the public FT_Stream_OpenGzip
# parity batch. The payloads deliberately include FNAME, FCOMMENT, FEXTRA,
# FHCRC, and FTEXT combinations while keeping every member well-formed.
BATCH98_OPTIONAL_HEADER_FLAGS = (
    0x08,
    0x10,
    0x18,
    0x04,
    0x0C,
    0x14,
    0x1C,
    0x0A,
    0x12,
    0x1A,
    0x06,
    0x0E,
    0x16,
    0x1E,
    0x09,
    0x11,
    0x19,
    0x05,
    0x0D,
    0x15,
    0x1D,
    0x0B,
    0x13,
    0x1B,
    0x07,
    0x0F,
    0x17,
    0x1F,
    0x02,
    0x03,
)


def write_if_changed(path: Path, data: bytes) -> None:
    if path.exists() and path.read_bytes() == data:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def deterministic_gzip(data: bytes) -> bytes:
    """Return a gzip stream with a platform-independent header."""
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode="wb", compresslevel=9, mtime=0) as stream:
        stream.write(data)
    return output.getvalue()


def deterministic_gzip_with_optional_header(
    data: bytes,
    flags: int,
    *,
    filename: str,
    comment: str,
    extra_payload: bytes,
) -> bytes:
    """Return a deterministic, valid gzip member with optional header fields."""
    header = bytearray(b"\x1f\x8b\x08")
    header.append(flags)
    header.extend(b"\x00\x00\x00\x00\x00\x03")
    if flags & 0x04:
        extra = b"FD" + struct.pack("<H", len(extra_payload)) + extra_payload
        header.extend(struct.pack("<H", len(extra)))
        header.extend(extra)
    if flags & 0x08:
        header.extend(filename.encode("ascii"))
        header.append(0)
    if flags & 0x10:
        header.extend(comment.encode("ascii"))
        header.append(0)
    if flags & 0x02:
        header.extend(struct.pack("<H", zlib.crc32(header) & 0xFFFF))

    compressor = zlib.compressobj(level=9, wbits=-15)
    body = compressor.compress(data) + compressor.flush()
    trailer = struct.pack("<II", zlib.crc32(data) & 0xFFFFFFFF, len(data) & 0xFFFFFFFF)
    return bytes(header) + body + trailer


def literal_unix_compress(data: bytes) -> bytes:
    """Encode bytes as valid 9-bit literal LZW codes in `.Z` bit order."""
    # 0x90 selects the traditional 16-bit maximum and block mode.  This small
    # payload never widens beyond 9-bit codes, so literals are sufficient.
    output = bytearray((0x1F, 0x9D, 0x90))
    accumulator = 0
    bits = 0
    for byte in data:
        accumulator |= byte << bits
        bits += 9
        while bits >= 8:
            output.append(accumulator & 0xFF)
            accumulator >>= 8
            bits -= 8
    if bits:
        output.append(accumulator & 0xFF)
    return bytes(output)


def unix_compress(
    data: bytes,
    *,
    max_bits: int = 16,
    block_mode: bool = True,
    clear_after: int | None = None,
) -> bytes:
    """Encode deterministic block-mode Unix-compress data.

    The encoder follows the pinned FreeType decoder's 9-bit start, dictionary
    growth, LSB-first bit order, and fixed-width input blocks.  A single
    optional CLEAR code makes the maintained reset fixture exercise block-mode
    dictionary reinitialization without relying on a platform `compress`
    implementation.
    """
    if not 9 <= max_bits <= 16:
        raise ValueError("Unix-compress max_bits must be between 9 and 16")
    if clear_after is not None and not block_mode:
        raise ValueError("CLEAR requires Unix-compress block mode")
    if not data:
        header = max_bits | (0x80 if block_mode else 0)
        return bytes((0x1F, 0x9D, header))

    clear_code = 256
    max_code = 1 << max_bits
    dictionary = {bytes((value,)): value for value in range(256)}
    next_code = 257
    width = 9
    decoder_free = 1 if block_mode else 0
    data_codes = 0
    records: list[tuple[int, int, bool]] = []
    emitted = 0
    pending_clear = clear_after
    phrase = bytes((data[0],))

    for value in data[1:]:
        candidate = phrase + bytes((value,))
        if candidate in dictionary:
            phrase = candidate
            continue

        records.append((dictionary[phrase], width, False))
        emitted += 1
        if data_codes > 0 and decoder_free < max_code - 256:
            decoder_free += 1
        data_codes += 1
        if decoder_free >= (1 << width) - 256 and width < max_bits:
            width += 1
        if next_code < max_code:
            dictionary[candidate] = next_code
            next_code += 1

        if pending_clear is not None and emitted >= pending_clear:
            records.append((clear_code, width, True))
            dictionary = {bytes((value,)): value for value in range(256)}
            next_code = 257
            width = 9
            decoder_free = 0
            data_codes = 0
            pending_clear = None

        phrase = bytes((value,))

    records.append((dictionary[phrase], width, False))

    output = bytearray((0x1F, 0x9D, max_bits | (0x80 if block_mode else 0)))
    block_width = 9
    block = 0
    block_bits = 0

    def flush_block(width_bits: int, value: int) -> None:
        for _ in range(width_bits):
            output.append(value & 0xFF)
            value >>= 8

    for code, code_width, reset_after in records:
        if code_width != block_width:
            if block_bits:
                flush_block(block_width, block)
                block = 0
                block_bits = 0
            block_width = code_width

        if block_bits + code_width > block_width * 8:
            flush_block(block_width, block)
            block = 0
            block_bits = 0

        block |= code << block_bits
        block_bits += code_width
        if block_bits == block_width * 8:
            flush_block(block_width, block)
            block = 0
            block_bits = 0

        if reset_after:
            if block_bits:
                flush_block(block_width, block)
            block = 0
            block_bits = 0
            block_width = 9

    if block_bits:
        flush_block((block_bits + 7) // 8, block)
    return bytes(output)


def unix_compress_codes(
    codes: list[int],
    *,
    header: int = 0x90,
    width: int = 9,
    minimum_data_bytes: int = 0,
) -> bytes:
    """Pack a small deterministic code sequence for malformed-stream probes."""
    output = bytearray((0x1F, 0x9D, header))
    accumulator = 0
    bits = 0
    for code in codes:
        if not 0 <= code < (1 << width):
            raise ValueError(f"code {code} does not fit in {width} bits")
        accumulator |= code << bits
        bits += width
        while bits >= 8:
            output.append(accumulator & 0xFF)
            accumulator >>= 8
            bits -= 8
    if bits:
        output.append(accumulator & 0xFF)
    while len(output) < 3 + minimum_data_bytes:
        output.append(0)
    return bytes(output)


def build_gzip() -> None:
    GZIP_OUT.mkdir(parents=True, exist_ok=True)
    manifest_payloads = []
    for payload_id, raw in PAYLOADS.items():
        stem = payload_id.replace("_", "-")
        raw_path = GZIP_OUT / f"{stem}.raw"
        gzip_path = GZIP_OUT / f"{stem}.gz"
        zlib_path = GZIP_OUT / f"{stem}.zlib"
        write_if_changed(raw_path, raw)
        write_if_changed(gzip_path, deterministic_gzip(raw))
        write_if_changed(zlib_path, zlib.compress(raw, level=9))
        manifest_payloads.append(
            {
                "id": payload_id,
                "raw": f"compressed/gzip/{stem}.raw",
                "gzip": f"compressed/gzip/{stem}.gz",
                "zlib_wrapped": f"compressed/gzip/{stem}.zlib",
            }
        )

    manifest = {
        "version": 1,
        "source": "scripts/build_compressed_fixtures.py",
        "payloads": manifest_payloads,
    }
    manifest_path = GZIP_OUT / "small-text-and-empty-payloads.json"
    encoded = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    write_if_changed(manifest_path, encoded)

    stream_manifest_payloads = []
    for payload_id, raw in STREAM_PAYLOADS.items():
        stem = payload_id.replace("_", "-")
        raw_path = GZIP_OUT / f"{stem}.raw"
        gzip_path = GZIP_OUT / f"{stem}.gz"
        write_if_changed(raw_path, raw)
        write_if_changed(gzip_path, deterministic_gzip(raw))
        stream_manifest_payloads.append(
            {
                "id": payload_id,
                "raw": f"compressed/gzip/{stem}.raw",
                "gzip": f"compressed/gzip/{stem}.gz",
            }
        )

    stream_manifest = {
        "version": 1,
        "source": "scripts/build_compressed_fixtures.py",
        "small_stream_threshold": 40960,
        "payloads": stream_manifest_payloads,
    }
    stream_manifest_path = GZIP_OUT / "small-and-large-streams.json"
    encoded = json.dumps(stream_manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    write_if_changed(stream_manifest_path, encoded)

    for index, flags in enumerate(BATCH98_OPTIONAL_HEADER_FLAGS, start=1):
        stem = f"batch98-optional-header-{index:02d}"
        payload = (
            f"fontdone batch98 optional gzip payload {index:02d}\n".encode("ascii")
            + bytes((index,)) * (index + 3)
        )
        raw_path = GZIP_OUT / f"{stem}.raw"
        gzip_path = GZIP_OUT / f"{stem}.gz"
        write_if_changed(raw_path, payload)
        write_if_changed(
            gzip_path,
            deterministic_gzip_with_optional_header(
                payload,
                flags,
                filename=f"batch98-{index:02d}.txt",
                comment=f"fontdone batch98 member {index:02d}",
                extra_payload=bytes((index, 0x42, 0x98)),
            ),
        )
        manifest = {
            "version": 1,
            "source": "scripts/build_compressed_fixtures.py",
            "payloads": [
                {
                    "id": f"batch98_optional_header_{index:02d}",
                    "raw": f"compressed/gzip/{stem}.raw",
                    "gzip": f"compressed/gzip/{stem}.gz",
                }
            ],
        }
        manifest_path = GZIP_OUT / f"{stem}.json"
        encoded = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
        write_if_changed(manifest_path, encoded)


def build_bzip2() -> None:
    raw = RAW_PCF.read_bytes()
    write_if_changed(BZIP2_OUT / "valid-pcf-header.raw", raw)
    write_if_changed(
        BZIP2_OUT / "valid-pcf-header.pcf.bz2",
        bz2.compress(raw, compresslevel=9),
    )
    write_if_changed(BZIP2_OUT / "not-bzip2.bin", b"fontdone-invalid-bzip2-header\n")
    write_if_changed(BZIP2_OUT / "truncated.bz2", b"BZh")
    manifest = {
        "version": 1,
        "source": "scripts/build_compressed_fixtures.py",
        "payloads": [
            {
                "id": "small_pcf",
                "raw": "compressed/bzip2/valid-pcf-header.raw",
                "bzip2": "compressed/bzip2/valid-pcf-header.pcf.bz2",
            }
        ],
    }
    encoded = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    write_if_changed(BZIP2_OUT / "small-streams.json", encoded)


def build_lzw() -> None:
    raw = RAW_PCF.read_bytes()
    write_if_changed(LZW_OUT / "small-valid-pcf.Z", literal_unix_compress(raw))
    write_if_changed(LZW_OUT / "invalid-header.bin", b"fontdone-invalid-lzw-header\n")

    dictionary_raw = (
        (bytes(range(256)) * 8)
        + bytes(((index * 73 + 19) & 0xFF) for index in range(4096))
        + (b"ABRACADABRA!" * 128)
    )
    clear_raw = (
        bytes(((index * 37 + 11) & 0xFF) for index in range(2048))
        + (b"clear-block-reset-fontdone\n" * 96)
    )
    write_if_changed(LZW_OUT / "dictionary-growth.raw", dictionary_raw)
    write_if_changed(
        LZW_OUT / "dictionary-growth.Z",
        unix_compress(dictionary_raw),
    )
    write_if_changed(LZW_OUT / "block-reset.raw", clear_raw)
    write_if_changed(
        LZW_OUT / "block-reset.Z",
        unix_compress(clear_raw, clear_after=64),
    )
    boundary_raw = bytes(range(256)) * 4
    write_if_changed(LZW_OUT / "dictionary-boundary.raw", boundary_raw)
    write_if_changed(
        LZW_OUT / "max-bits-9-boundary.Z",
        unix_compress(boundary_raw, max_bits=9),
    )
    write_if_changed(
        LZW_OUT / "max-bits-9-clear.Z",
        unix_compress(clear_raw[:320], max_bits=9, clear_after=64),
    )
    write_if_changed(LZW_OUT / "max-bits-9-clear.raw", clear_raw[:320])
    write_if_changed(
        LZW_OUT / "max-bits-10-transition.Z",
        unix_compress(boundary_raw, max_bits=10),
    )
    write_if_changed(
        LZW_OUT / "non-block-dictionary.Z",
        unix_compress(boundary_raw, block_mode=False),
    )
    non_block_clear_raw = bytes(8)
    write_if_changed(LZW_OUT / "non-block-clear.raw", non_block_clear_raw)
    write_if_changed(
        LZW_OUT / "non-block-clear.Z",
        unix_compress_codes([0, 256], header=0x10, minimum_data_bytes=9),
    )
    malformed_raw = b""
    write_if_changed(LZW_OUT / "malformed-empty.raw", malformed_raw)
    write_if_changed(LZW_OUT / "truncated-header.Z", bytes((0x1F, 0x9D)))
    write_if_changed(LZW_OUT / "truncated-code.Z", bytes((0x1F, 0x9D, 0x90)))
    write_if_changed(LZW_OUT / "invalid-max-bits-low.Z", bytes((0x1F, 0x9D, 0x88)))
    write_if_changed(LZW_OUT / "invalid-max-bits-high.Z", bytes((0x1F, 0x9D, 0x91)))
    write_if_changed(
        LZW_OUT / "first-code-non-literal.Z",
        unix_compress_codes([256], minimum_data_bytes=9),
    )
    invalid_reference_raw = bytes((0,))
    write_if_changed(LZW_OUT / "invalid-reference.raw", invalid_reference_raw)
    write_if_changed(
        LZW_OUT / "invalid-reference.Z",
        unix_compress_codes([0, 511], minimum_data_bytes=9),
    )
    manifest = {
        "version": 1,
        "source": "scripts/build_compressed_fixtures.py",
        "payloads": [
            {
                "id": "small_pcf",
                "raw": "input/fonts/bitmap/bitmap-only.pcf",
                "lzw": "compressed/lzw/small-valid-pcf.Z",
            }
        ],
    }
    encoded = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    write_if_changed(LZW_OUT / "small-streams.json", encoded)
    dictionary_manifest = {
        "version": 1,
        "source": "scripts/build_compressed_fixtures.py",
        "payloads": [
            {
                "id": "dictionary_growth",
                "raw": "compressed/lzw/dictionary-growth.raw",
                "lzw": "compressed/lzw/dictionary-growth.Z",
            },
            {
                "id": "block_reset",
                "raw": "compressed/lzw/block-reset.raw",
                "lzw": "compressed/lzw/block-reset.Z",
            },
            {
                "id": "max_bits_9_boundary",
                "raw": "compressed/lzw/dictionary-boundary.raw",
                "lzw": "compressed/lzw/max-bits-9-boundary.Z",
            },
            {
                "id": "max_bits_9_clear",
                "raw": "compressed/lzw/max-bits-9-clear.raw",
                "lzw": "compressed/lzw/max-bits-9-clear.Z",
            },
            {
                "id": "max_bits_10_transition",
                "raw": "compressed/lzw/dictionary-boundary.raw",
                "lzw": "compressed/lzw/max-bits-10-transition.Z",
            },
            {
                "id": "non_block_dictionary",
                "raw": "compressed/lzw/dictionary-boundary.raw",
                "lzw": "compressed/lzw/non-block-dictionary.Z",
            },
            {
                "id": "non_block_clear_code",
                "raw": "compressed/lzw/non-block-clear.raw",
                "lzw": "compressed/lzw/non-block-clear.Z",
            },
        ],
    }
    encoded = json.dumps(dictionary_manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    write_if_changed(LZW_OUT / "dictionary-streams.json", encoded)
    malformed_manifest = {
        "version": 1,
        "source": "scripts/build_compressed_fixtures.py",
        "payloads": [
            {
                "id": "truncated_header",
                "raw": "compressed/lzw/malformed-empty.raw",
                "lzw": "compressed/lzw/truncated-header.Z",
            },
            {
                "id": "truncated_code",
                "raw": "compressed/lzw/malformed-empty.raw",
                "lzw": "compressed/lzw/truncated-code.Z",
            },
            {
                "id": "invalid_max_bits_low",
                "raw": "compressed/lzw/malformed-empty.raw",
                "lzw": "compressed/lzw/invalid-max-bits-low.Z",
            },
            {
                "id": "invalid_max_bits_high",
                "raw": "compressed/lzw/malformed-empty.raw",
                "lzw": "compressed/lzw/invalid-max-bits-high.Z",
            },
            {
                "id": "first_code_non_literal",
                "raw": "compressed/lzw/malformed-empty.raw",
                "lzw": "compressed/lzw/first-code-non-literal.Z",
            },
            {
                "id": "invalid_reference",
                "raw": "compressed/lzw/invalid-reference.raw",
                "lzw": "compressed/lzw/invalid-reference.Z",
            },
        ],
    }
    encoded = json.dumps(malformed_manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    write_if_changed(LZW_OUT / "malformed-streams.json", encoded)


def main() -> None:
    build_gzip()
    build_bzip2()
    build_lzw()


if __name__ == "__main__":
    main()
