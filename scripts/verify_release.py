#!/usr/bin/env python3
"""Verify synchronized versions and the contents of all publishable archives."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import posixpath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import atexit
from pathlib import Path, PurePosixPath
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = (
    ("fontdone", ROOT / "Cargo.toml"),
    ("fontdone-c-abi", ROOT / "fontdone-c-abi" / "Cargo.toml"),
    ("fontdone-wasm", ROOT / "fontdone-wasm" / "Cargo.toml"),
)
REQUIRED = {
    "fontdone": {
        "Cargo.toml",
        "README.md",
        "CHANGELOG.md",
        "LICENSE",
        "FTL.TXT",
        "NOTICE.md",
        "src/lib.rs",
        "examples/render_mask.rs",
    },
    "fontdone-c-abi": {
        "build.rs",
        "Cargo.toml",
        "README.md",
        "API_SUPPORT.md",
        "LICENSE",
        "FTL.TXT",
        "NOTICE.md",
        "fontdone2.pc",
        "include/fontdone_ffi.h",
        "include/freetype/freetype.h",
        "include/ft2build.h",
        "examples/render_glyph.c",
        "src/lib.rs",
    },
    "fontdone-wasm": {
        "Cargo.toml",
        "README.md",
        "abi.json",
        "fontdone_wasm.d.ts",
        "LICENSE",
        "FTL.TXT",
        "NOTICE.md",
        "examples/node.mjs",
        "examples/check_exports.mjs",
        "src/lib.rs",
    },
}
FORBIDDEN_PACKAGE_ROOTS = {
    ".git",
    ".github",
    "doc",
    "freetype",
    "scripts",
    "target",
    "tests",
}
# Match both destinations in linked images instead of silently skipping the
# outer package-documentation link.
MARKDOWN_LINK = re.compile(r"]\(([^)]+)\)")


def run(command: list[str], *, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, env=env, check=True)


def package_version(manifest: Path) -> str:
    text = manifest.read_text(encoding="utf-8")
    package = text.split("[package]", 1)
    if len(package) != 2:
        raise ValueError(f"{manifest}: missing [package]")
    match = re.search(r'(?m)^version\s*=\s*"([^"]+)"', package[1])
    if match is None:
        raise ValueError(f"{manifest}: missing package version")
    return match.group(1)


def verify_metadata() -> str:
    versions = {name: package_version(manifest) for name, manifest in PACKAGES}
    if len(set(versions.values())) != 1:
        raise ValueError(f"package version drift: {versions}")
    version = versions["fontdone"]
    root_manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    root_package = root_manifest.split("[package]", 1)[1]
    if not re.search(r'(?m)^name\s*=\s*"fontdone"$', root_package):
        raise ValueError("Cargo.toml: root package must be named fontdone")
    if not re.search(r'(?m)^publish\s*=\s*\["crates-io"\]$', root_package):
        raise ValueError("Cargo.toml: fontdone must publish to crates.io")
    if 'members = ["fontdone-c-abi", "fontdone-wasm"]' not in root_manifest:
        raise ValueError(
            "Cargo.toml: workspace must retain both synchronized facade members"
        )
    exact = f'version = "={version}"'
    for name, manifest in PACKAGES[1:]:
        if exact not in manifest.read_text(encoding="utf-8"):
            raise ValueError(f"{manifest}: {name} must require fontdone exactly at {version}")
    for name in ("fontdone-c-abi", "fontdone-wasm"):
        pattern = rf'{re.escape(name)}\s*=\s*\{{[^}}]*path\s*=\s*"{re.escape(name)}"'
        if re.search(pattern, root_manifest) is None:
            raise ValueError(f"Cargo.toml: path-only dev dependency {name} is missing")
    consumer_template = (
        ROOT / "tests" / "external" / "rust-consumer" / "Cargo.toml.in"
    ).read_text(encoding="utf-8")
    expected_consumer = (
        'fontdone = { version = "=@FONTDONE_VERSION@", '
        'path = "@FONTDONE_PATH@" }'
    )
    if expected_consumer not in consumer_template:
        raise ValueError(
            "external Rust consumer must exercise a versioned path dependency"
        )
    for path in (ROOT / "README.md", ROOT / "CHANGELOG.md"):
        if version not in path.read_text(encoding="utf-8"):
            raise ValueError(f"{path}: release version {version} is absent")
    print(f"release metadata: 3 packages synchronized at {version}")
    return version


def safe_extract(archive: Path, destination: Path) -> None:
    destination_resolved = destination.resolve()
    with tarfile.open(archive, "r:gz") as package:
        for member in package.getmembers():
            member_path = (destination / member.name).resolve()
            if (
                member_path != destination_resolved
                and destination_resolved not in member_path.parents
            ):
                raise ValueError(f"{archive}: unsafe member {member.name}")
        package.extractall(destination)


def inspect_archive(
    package_name: str, version: str, archive: Path, output: Path
) -> dict[str, object]:
    prefix = f"{package_name}-{version}"
    markdown: dict[str, str] = {}
    with tarfile.open(archive, "r:gz") as package:
        members = sorted(
            member.name
            for member in package.getmembers()
            if member.isfile()
        )
        for member in package.getmembers():
            if not member.isfile() or not member.name.lower().endswith(".md"):
                continue
            source = package.extractfile(member)
            if source is not None:
                markdown[member.name] = source.read().decode("utf-8")
    relative: list[str] = []
    for member in members:
        path = PurePosixPath(member)
        if not path.parts or path.parts[0] != prefix:
            raise ValueError(f"{archive}: unexpected archive root {member}")
        rel = PurePosixPath(*path.parts[1:]).as_posix()
        relative.append(rel)
        if len(path.parts) > 1 and path.parts[1] in FORBIDDEN_PACKAGE_ROOTS:
            raise ValueError(f"{archive}: forbidden published path {rel}")
        if rel.lower().endswith((".ttf", ".otf", ".woff", ".woff2", ".pfb", ".pfa")):
            raise ValueError(f"{archive}: font fixture leaked into package: {rel}")
    missing = sorted(REQUIRED[package_name] - set(relative))
    if missing:
        raise ValueError(f"{archive}: required files missing: {missing}")
    relative_set = set(relative)
    for member, text in sorted(markdown.items()):
        markdown_rel = PurePosixPath(member).relative_to(prefix).as_posix()
        parent = PurePosixPath(markdown_rel).parent.as_posix()
        for raw in MARKDOWN_LINK.findall(text):
            destination = raw.strip()
            if destination.startswith("<"):
                end = destination.find(">")
                destination = (
                    destination[1:end] if end >= 0 else destination[1:]
                )
            else:
                destination = destination.split(maxsplit=1)[0]
            if (
                not destination
                or destination.startswith(("#", "http://", "https://", "mailto:"))
            ):
                continue
            file_part = unquote(destination.split("#", 1)[0])
            resolved = posixpath.normpath(posixpath.join(parent, file_part))
            if resolved.startswith("../") or resolved not in relative_set:
                raise ValueError(
                    f"{archive}: {markdown_rel} has unpackaged local link "
                    f"{destination!r}"
                )
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    inventory = output / f"{package_name}-{version}.inventory.txt"
    inventory.write_text("\n".join(relative) + "\n", encoding="utf-8")
    return {
        "package": package_name,
        "version": version,
        "archive": str(archive.relative_to(ROOT)),
        "sha256": digest,
        "file_count": len(relative),
        "inventory": str(inventory.relative_to(ROOT)),
    }


def package_and_inspect(version: str) -> list[dict[str, object]]:
    output = ROOT / "target" / "release-evidence"
    output.mkdir(parents=True, exist_ok=True)
    extracted = Path(tempfile.mkdtemp(prefix="fontdone-release-archives-"))
    atexit.register(shutil.rmtree, extracted, ignore_errors=True)

    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(ROOT / "target")
    reports: list[dict[str, object]] = []
    for package_name, _manifest in PACKAGES:
        local_root_patch = []
        if package_name != "fontdone":
            local_root_patch = [
                "--config",
                f'patch.crates-io.fontdone.path="{ROOT}"',
            ]
        run(
            [
                "cargo",
                "package",
                "--package",
                package_name,
                "--locked",
                "--allow-dirty",
                "--no-verify",
                *local_root_patch,
            ],
            env=env,
        )
        archive = ROOT / "target" / "package" / f"{package_name}-{version}.crate"
        if not archive.is_file():
            raise ValueError(f"cargo did not create {archive}")
        reports.append(inspect_archive(package_name, version, archive, output))
        safe_extract(archive, extracted)

    # Compile the exact packaged source with registry dependencies redirected
    # to the other exact packaged sources. This is the local pre-publication
    # equivalent of registry verification for mutually version-pinned crates.
    patches = []
    for package_name, _manifest in PACKAGES:
        package_path = extracted / f"{package_name}-{version}"
        patches.extend(
            [
                "--config",
                f'patch.crates-io.{package_name}.path="{package_path}"',
            ]
        )
    for package_name, _manifest in PACKAGES:
        manifest = extracted / f"{package_name}-{version}" / "Cargo.toml"
        run(
            [
                "cargo",
                "check",
                "--manifest-path",
                str(manifest),
                "--all-features",
                "--lib",
                *patches,
            ],
            env=env,
        )

    report = {
        "schema_version": 1,
        "version": version,
        "publication_order": [name for name, _ in PACKAGES],
        "archives": reports,
    }
    (output / "package-report.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    checksums = "\n".join(
        f"{item['sha256']}  {Path(str(item['archive'])).name}" for item in reports
    )
    (output / "SHA256SUMS").write_text(checksums + "\n", encoding="utf-8")
    snapshot = json.loads(
        (ROOT / "doc" / "compatibility_snapshot.json").read_text(encoding="utf-8")
    )
    runtime = snapshot["runtime_parity"]
    public = snapshot["public_function_adoption"]
    contract = snapshot["c_contract"]
    notes = (
        f"# fontdone {version}\n\n"
        f"Pure-Rust runtime targeting FreeType {snapshot['freetype_version']}.\n\n"
        "## Measured compatibility\n\n"
        f"Evidence snapshot: {snapshot['snapshot_date']}.\n\n"
        f"- runnable exact comparisons: {runtime['passed']:,}/"
        f"{runtime['runnable']:,} passed\n"
        f"- failed: {runtime['failed']:,}\n"
        f"- explicitly pending: {runtime['pending']:,}\n"
        f"- functions with runtime route evidence: "
        f"{runtime['function_route_evidence']}/"
        f"{runtime['function_total']}\n"
        f"- pinned public functions: {public['total']} "
        f"({public['complete']} complete, "
        f"{public['implemented_mapping_incomplete']} implemented with incomplete "
        "mapping, "
        f"{public['partial']} partial, "
        f"{public['planned']} planned, "
        f"{public['intentionally_excluded']} excluded)\n"
        f"- C replacement contract: {contract['categories_complete']}/"
        f"{contract['categories_total']} categories complete "
        f"(runtime rows {contract['runtime_contract_rows_complete']:,}/"
        f"{contract['runtime_contract_rows_total']:,}, artifacts "
        f"{contract['binary_artifact_items_complete']}/"
        f"{contract['binary_artifact_items_total']}, platforms "
        f"{contract['platform_lanes_complete']}/"
        f"{contract['platform_lanes_total']})\n\n"
        "Published in dependency order: `fontdone`, `fontdone-c-abi`, "
        "`fontdone-wasm`. Route evidence is not a claim that every success path "
        "is complete; see the repository adoption map and C-contract scorecard.\n"
    )
    (output / "release-notes.md").write_text(notes, encoding="utf-8")
    print(f"release packages: {len(reports)} archives inspected and compiled")
    return reports


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--metadata-only",
        action="store_true",
        help="check synchronized manifests and release references only",
    )
    args = parser.parse_args()
    try:
        version = verify_metadata()
        if not args.metadata_only:
            package_and_inspect(version)
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"release verification failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
