#!/usr/bin/env python3
"""Publish or dry-run the exact npm archive through one command path."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess

from publish_release import version


def publish_archive(archive: Path, *, dry_run: bool) -> None:
    release_version = version()
    # npm-package-arg interprets a bare "directory/archive.tgz" as GitHub
    # shorthand before considering tarballs. Resolve local archives first.
    archive = archive.resolve(strict=True)
    if not archive.is_file() or archive.name != f"fontdone-{release_version}.tgz":
        raise ValueError("the synchronized fontdone npm archive is required")
    if not dry_run and (
        os.environ.get("GITHUB_ACTIONS") != "true"
        or os.environ.get("GITHUB_REPOSITORY") != "appunni-m/fontdone"
        or os.environ.get("GITHUB_REF") != f"refs/tags/v{release_version}"
        or not os.environ.get("ACTIONS_ID_TOKEN_REQUEST_URL")
    ):
        raise ValueError("npm publication requires the exact GitHub tag and OIDC job")
    command = [
        "npm", "publish", str(archive), "--access", "public", "--tag",
        "next" if "-" in release_version else "latest", "--provenance",
    ]
    if dry_run:
        command.extend([
            "--dry-run", "--ignore-scripts", "--offline", "--userconfig", os.devnull,
        ])
    print("+", " ".join(command), flush=True)
    subprocess.run(command, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--dry-run", action="store_true")
    mode.add_argument("--publish", action="store_true")
    args = parser.parse_args()
    publish_archive(args.archive, dry_run=args.dry_run)


if __name__ == "__main__":
    main()
