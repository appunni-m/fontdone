#!/usr/bin/env python3
"""Publish the single public Cargo crate with registry verification."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# The C ABI and raw-WASM packages remain workspace build targets.  Their public
# distribution is the native C SDK archive and the JavaScript npm package; only
# the safe Rust API is published to crates.io.
PUBLISHED_PACKAGES = ("fontdone",)


def version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'(?m)^version\s*=\s*"([^"]+)"', text)
    if match is None:
        raise ValueError("root package version is missing")
    return match.group(1)


def wait_for_registry(package: str, expected_version: str, timeout: int) -> None:
    url = f"https://crates.io/api/v1/crates/{package}/{expected_version}"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            request = urllib.request.Request(
                url, headers={"User-Agent": "fontdone-release-verifier/1"}
            )
            with urllib.request.urlopen(request, timeout=15) as response:
                payload = json.load(response)
            if payload.get("version", {}).get("num") == expected_version:
                print(f"registry: {package} {expected_version} is available")
                return
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError):
            pass
        time.sleep(10)
    raise TimeoutError(
        f"{package} {expected_version} did not appear on crates.io within {timeout}s"
    )


def registry_checksum(package: str, expected_version: str) -> str | None:
    """Return the crates.io checksum for an exact version, if visible."""

    url = f"https://crates.io/api/v1/crates/{package}/{expected_version}"
    try:
        request = urllib.request.Request(
            url, headers={"User-Agent": "fontdone-release-verifier/1"}
        )
        with urllib.request.urlopen(request, timeout=15) as response:
            payload = json.load(response)
        version = payload.get("version", {})
        if version.get("num") != expected_version:
            raise TimeoutError(
                f"crates.io returned unexpected metadata for {package} {expected_version}"
            )
        checksum = version.get("checksum")
        if not isinstance(checksum, str) or len(checksum) != 64:
            raise TimeoutError(
                f"crates.io metadata for {package} {expected_version} has no checksum"
            )
        return checksum
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise TimeoutError(
            f"crates.io lookup for {package} {expected_version} failed: HTTP {error.code}"
        ) from error
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as error:
        raise TimeoutError(
            f"crates.io lookup for {package} {expected_version} did not complete"
        ) from error


def sha256(path: Path) -> str:
    """Return the lowercase SHA-256 digest of a release archive."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(128 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def run(command: list[str]) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def require_github_oidc(release_version: str) -> None:
    """Keep the completed local bootstrap path closed for future releases."""
    if (
        os.environ.get("GITHUB_ACTIONS") != "true"
        or os.environ.get("GITHUB_REPOSITORY") != "appunni-m/fontdone"
        or os.environ.get("GITHUB_REF") != f"refs/tags/v{release_version}"
        or not os.environ.get("ACTIONS_ID_TOKEN_REQUEST_URL")
        or not os.environ.get("CARGO_REGISTRY_TOKEN")
    ):
        raise ValueError("publication requires the exact GitHub tag and OIDC publish job")


def verify_archive(archive: Path, release_version: str) -> str:
    """Bind the clean publish checkout to the archive verified by tag CI."""
    local = ROOT / "target" / "package" / f"fontdone-{release_version}.crate"
    if not archive.is_file() or not local.is_file():
        raise ValueError("both the CI archive and locally verified Cargo archive are required")
    expected = sha256(archive)
    if sha256(local) != expected:
        raise ValueError("Cargo archive differs from the exact tag CI artifact")
    return expected


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--dry-run", action="store_true")
    mode.add_argument("--publish", action="store_true")
    mode.add_argument(
        "--publish-if-missing",
        action="store_true",
        help="skip immutable versions already visible on crates.io",
    )
    parser.add_argument("--registry-timeout", type=int, default=600)
    parser.add_argument("--verified-archive", type=Path)
    args = parser.parse_args()
    release_version = version()
    try:
        if args.publish or args.publish_if_missing:
            require_github_oidc(release_version)
            if args.verified_archive is None:
                raise ValueError("publication requires --verified-archive from successful tag CI")
            expected_checksum = verify_archive(args.verified_archive, release_version)
            status = subprocess.run(
                ["git", "status", "--porcelain=v1", "--untracked-files=all"],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
            )
            if status.stdout:
                raise ValueError("publishing requires a clean tracked and untracked worktree")
        else:
            run(["python3", "scripts/verify_release.py"])
            expected_checksum = None
        for index, package in enumerate(PUBLISHED_PACKAGES):
            command = [
                "cargo",
                "publish",
                "--package",
                package,
                "--locked",
            ]
            if args.dry_run:
                command.extend(["--dry-run", "--allow-dirty"])
            else:
                # A read-only job and the pre-authentication step compiled the
                # checksum-identical archive. Do not rebuild with a live token.
                command.append("--no-verify")
            already_visible = False
            if args.publish_if_missing:
                archive = ROOT / "target" / "package" / f"{package}-{release_version}.crate"
                if not archive.is_file():
                    raise ValueError(f"verified Cargo archive is missing: {archive}")
                checksum = registry_checksum(package, release_version)
                if checksum is not None:
                    local_checksum = sha256(archive)
                    if local_checksum != checksum:
                        raise ValueError(
                            f"crates.io artifact mismatch for {package} {release_version}: "
                            f"local={local_checksum} registry={checksum}"
                        )
                    already_visible = True
            if already_visible:
                print(
                    f"registry: {package} {release_version} is already visible; "
                    "keeping the immutable artifact"
                )
            else:
                run(command)
            if expected_checksum is not None:
                wait_for_registry(package, release_version, args.registry_timeout)
                if registry_checksum(package, release_version) != expected_checksum:
                    raise ValueError("published crate checksum differs from the verified CI archive")
            if (
                (args.publish or args.publish_if_missing)
                and index + 1 < len(PUBLISHED_PACKAGES)
            ):
                wait_for_registry(package, release_version, args.registry_timeout)
    except (OSError, ValueError, TimeoutError, subprocess.CalledProcessError) as exc:
        print(f"release stopped: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
