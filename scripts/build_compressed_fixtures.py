#!/usr/bin/env python3
"""Build deterministic gzip, bzip2, and Unix-compress parity payloads."""

from __future__ import annotations

import bz2
import gzip
import io
import json
from pathlib import Path
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


def main() -> None:
    build_gzip()
    build_bzip2()
    build_lzw()


if __name__ == "__main__":
    main()
