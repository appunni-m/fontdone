#!/usr/bin/env python3
"""Assemble the native fontdone C SDK archive for a release.

The C ABI implementation is an internal Cargo workspace package.  C
consumers receive the compiled library, headers, pkg-config metadata, and
examples in this archive instead of a second crates.io package.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import os
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def package_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r"(?m)^version\s*=\s*\"([^\"]+)\"", text)
    if match is None:
        raise ValueError("Cargo.toml: root package version is missing")
    return match.group(1)


def host_target() -> str:
    command = os.environ.get("RUSTC", "rustc")
    result = subprocess.run(
        [command, "-vV"], cwd=ROOT, check=True, capture_output=True, text=True
    )
    for line in result.stdout.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    raise ValueError("rustc -vV did not report a host target")


def add_file(root: Path, source: Path, relative: str) -> None:
    destination = root / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def add_tree(root: Path, source: Path, relative: str) -> None:
    destination = root / relative
    shutil.copytree(source, destination, dirs_exist_ok=True)


def archive(source: Path, destination: Path) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("wb") as stream:
        with gzip.GzipFile(fileobj=stream, mode="wb", mtime=0) as gzip_stream:
            with tarfile.open(fileobj=gzip_stream, mode="w") as tar:
                for path in sorted(source.rglob("*")):
                    relative = path.relative_to(source.parent).as_posix()
                    info = tar.gettarinfo(str(path), arcname=relative)
                    info.mtime = 0
                    if path.is_file():
                        with path.open("rb") as file_stream:
                            tar.addfile(info, file_stream)
                    else:
                        tar.addfile(info)
    return hashlib.sha256(destination.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default=None, help="Rust target triple")
    parser.add_argument(
        "--target-dir",
        type=Path,
        default=Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")),
        help="Cargo target directory",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "c-sdk",
        help="archive output directory",
    )
    args = parser.parse_args()

    version = package_version()
    target = args.target or host_target()
    release_dir = args.target_dir / (target if args.target else "") / "release"
    if not release_dir.is_dir():
        raise SystemExit(f"missing Cargo release directory: {release_dir}")

    dynamic_names = {
        "darwin": ("libfontdone_c_abi.dylib",),
        "linux": ("libfontdone_c_abi.so",),
        "windows": ("fontdone_c_abi.dll", "fontdone_c_abi.lib"),
    }
    system = platform.system().lower()
    candidates = dynamic_names.get(system, ())
    libraries = [release_dir / "libfontdone_c_abi.a"]
    libraries.extend(release_dir / name for name in candidates)
    libraries = [path for path in libraries if path.is_file()]
    if not libraries:
        raise SystemExit(
            f"no built fontdone-c-abi libraries found in {release_dir}; "
            "run cargo build --release -p fontdone-c-abi first"
        )

    archive_root_name = f"fontdone-c-abi-{version}-{target}"
    with tempfile.TemporaryDirectory(prefix="fontdone-c-sdk-") as temporary:
        staging_parent = Path(temporary)
        staging = staging_parent / archive_root_name
        (staging / "lib").mkdir(parents=True)
        add_tree(staging, ROOT / "fontdone-c-abi" / "include", "include")
        add_file(staging, ROOT / "fontdone-c-abi" / "fontdone2.pc", "lib/pkgconfig/fontdone2.pc")
        add_file(staging, ROOT / "fontdone-c-abi" / "README.md", "README.md")
        add_file(staging, ROOT / "fontdone-c-abi" / "API_SUPPORT.md", "API_SUPPORT.md")
        add_file(staging, ROOT / "fontdone-c-abi" / "examples" / "render_glyph.c", "examples/render_glyph.c")
        for name in ("LICENSE", "FTL.TXT", "NOTICE.md"):
            add_file(staging, ROOT / name, name)
        for library in libraries:
            add_file(staging, library, f"lib/{library.name}")
        output = args.output_dir / f"{archive_root_name}.tar.gz"
        digest = archive(staging, output)

    checksum = output.with_suffix(output.suffix + ".sha256")
    checksum.write_text(f"{digest}  {output.name}\n", encoding="utf-8")
    print(f"C SDK archive: {output}")
    print(f"C SDK SHA-256: {digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
