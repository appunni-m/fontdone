#!/usr/bin/env python3
"""Publish the synchronized crates in dependency order with registry waits."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ("fontdone", "fontdone-c-abi", "fontdone-wasm")


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


def run(command: list[str]) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--dry-run", action="store_true")
    mode.add_argument("--publish", action="store_true")
    parser.add_argument("--registry-timeout", type=int, default=600)
    args = parser.parse_args()
    release_version = version()
    try:
        run(["python3", "scripts/verify_release.py"])
        if args.publish:
            status = subprocess.run(
                ["git", "diff", "--quiet", "HEAD", "--"],
                cwd=ROOT,
                check=False,
            )
            if status.returncode != 0:
                raise ValueError("publishing requires a clean tracked worktree")
        for index, package in enumerate(PACKAGES):
            command = [
                "cargo",
                "publish",
                "--package",
                package,
                "--locked",
            ]
            if args.dry_run:
                command.extend(["--dry-run", "--allow-dirty"])
            run(command)
            if args.publish and index + 1 < len(PACKAGES):
                wait_for_registry(package, release_version, args.registry_timeout)
    except (OSError, ValueError, TimeoutError, subprocess.CalledProcessError) as exc:
        print(f"release stopped: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
