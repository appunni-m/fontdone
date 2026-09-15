#!/usr/bin/env python3
"""Unit tests for the single-public-crate release helper."""

from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts import publish_npm_release, publish_release


CHECKSUM = "a" * 64
VERSION = publish_release.version()


class PublishReleaseTests(unittest.TestCase):
    def test_npm_publication_rejects_local_and_wrong_tag_contexts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / f"fontdone-{VERSION}.tgz"
            archive.write_bytes(b"archive whose upload must not be attempted")
            environments = [
                {},
                {
                    "GITHUB_ACTIONS": "true",
                    "GITHUB_REPOSITORY": "appunni-m/fontdone",
                    "GITHUB_REF": "refs/tags/v0.0.0",
                    "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/oidc",
                },
            ]
            for environment in environments:
                with (
                    self.subTest(environment=environment),
                    patch.dict(os.environ, environment, clear=True),
                    patch.object(publish_npm_release.subprocess, "run") as run,
                ):
                    with self.assertRaisesRegex(ValueError, "GitHub tag and OIDC"):
                        publish_npm_release.publish_archive(archive, dry_run=False)
                    run.assert_not_called()

    def test_npm_rejects_a_missing_or_wrong_version_archive(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "fontdone-0.0.0.tgz"
            with patch.object(publish_npm_release.subprocess, "run") as run:
                with self.assertRaises(FileNotFoundError):
                    publish_npm_release.publish_archive(archive, dry_run=True)
                archive.touch()
                with self.assertRaisesRegex(ValueError, "synchronized"):
                    publish_npm_release.publish_archive(archive, dry_run=True)
                run.assert_not_called()

    def test_publication_rejects_local_and_wrong_tag_contexts(self) -> None:
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(ValueError):
                publish_release.require_github_oidc(VERSION)
        environment = {
            "GITHUB_ACTIONS": "true",
            "GITHUB_REPOSITORY": "appunni-m/fontdone",
            "GITHUB_REF": f"refs/tags/v{VERSION}",
            "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/oidc",
            "CARGO_REGISTRY_TOKEN": "test-placeholder",
        }
        with patch.dict(os.environ, environment, clear=True):
            publish_release.require_github_oidc(VERSION)
            with self.assertRaises(ValueError):
                publish_release.require_github_oidc("0.0.0")

    def test_archive_binding_rejects_missing_and_changed_packages(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            local = root / "target" / "package" / f"fontdone-{VERSION}.crate"
            local.parent.mkdir(parents=True)
            candidate = root / "verified.crate"
            with patch.object(publish_release, "ROOT", root):
                with self.assertRaises(ValueError):
                    publish_release.verify_archive(candidate, VERSION)
                candidate.write_bytes(b"verified source archive")
                local.write_bytes(candidate.read_bytes())
                self.assertEqual(
                    publish_release.verify_archive(candidate, VERSION),
                    hashlib.sha256(candidate.read_bytes()).hexdigest(),
                )
                local.write_bytes(b"different source archive")
                with self.assertRaises(ValueError):
                    publish_release.verify_archive(candidate, VERSION)

    def test_sha256_reads_the_complete_archive(self) -> None:
        archive = io.BytesIO(b"fontdone archive")
        with patch.object(publish_release.Path, "open", return_value=archive):
            expected = hashlib.sha256(b"fontdone archive").hexdigest()
            self.assertEqual(publish_release.sha256(publish_release.ROOT / "ignored"), expected)

    def test_registry_checksum_returns_exact_checksum(self) -> None:
        response = io.BytesIO(
            json.dumps({"version": {"num": VERSION, "checksum": CHECKSUM}}).encode()
        )
        with patch("urllib.request.urlopen", return_value=response):
            self.assertEqual(
                publish_release.registry_checksum("fontdone", VERSION),
                CHECKSUM,
            )

    def test_registry_checksum_distinguishes_missing_version(self) -> None:
        missing = urllib.error.HTTPError(
            f"https://crates.io/api/v1/crates/fontdone/{VERSION}",
            404,
            "missing",
            {},
            None,
        )
        with patch("urllib.request.urlopen", side_effect=missing):
            self.assertIsNone(
                publish_release.registry_checksum("fontdone", VERSION)
            )

    def test_registry_checksum_rejects_unexpected_metadata(self) -> None:
        response = io.BytesIO(
            json.dumps({"version": {"num": "2.14.3-alpha.2", "checksum": CHECKSUM}}).encode()
        )
        with patch("urllib.request.urlopen", return_value=response):
            with self.assertRaises(TimeoutError):
                publish_release.registry_checksum("fontdone", VERSION)

        response = io.BytesIO(
            json.dumps({"version": {"num": VERSION, "checksum": "short"}}).encode()
        )
        with patch("urllib.request.urlopen", return_value=response):
            with self.assertRaises(TimeoutError):
                publish_release.registry_checksum("fontdone", VERSION)


if __name__ == "__main__":
    unittest.main()
