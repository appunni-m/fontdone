#!/usr/bin/env python3
"""Unit tests for the single-public-crate release helper."""

from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts import publish_release


CHECKSUM = "a" * 64


class PublishReleaseTests(unittest.TestCase):
    def test_sha256_reads_the_complete_archive(self) -> None:
        archive = io.BytesIO(b"fontdone archive")
        with patch.object(publish_release.Path, "open", return_value=archive):
            expected = hashlib.sha256(b"fontdone archive").hexdigest()
            self.assertEqual(publish_release.sha256(publish_release.ROOT / "ignored"), expected)

    def test_registry_checksum_returns_exact_checksum(self) -> None:
        response = io.BytesIO(
            json.dumps({"version": {"num": "2.14.3-alpha.3", "checksum": CHECKSUM}}).encode()
        )
        with patch("urllib.request.urlopen", return_value=response):
            self.assertEqual(
                publish_release.registry_checksum("fontdone", "2.14.3-alpha.3"),
                CHECKSUM,
            )

    def test_registry_checksum_distinguishes_missing_version(self) -> None:
        missing = urllib.error.HTTPError(
            "https://crates.io/api/v1/crates/fontdone/2.14.3-alpha.3",
            404,
            "missing",
            {},
            None,
        )
        with patch("urllib.request.urlopen", side_effect=missing):
            self.assertIsNone(
                publish_release.registry_checksum("fontdone", "2.14.3-alpha.3")
            )

    def test_registry_checksum_rejects_unexpected_metadata(self) -> None:
        response = io.BytesIO(
            json.dumps({"version": {"num": "2.14.3-alpha.2", "checksum": CHECKSUM}}).encode()
        )
        with patch("urllib.request.urlopen", return_value=response):
            with self.assertRaises(TimeoutError):
                publish_release.registry_checksum("fontdone", "2.14.3-alpha.3")

        response = io.BytesIO(
            json.dumps({"version": {"num": "2.14.3-alpha.3", "checksum": "short"}}).encode()
        )
        with patch("urllib.request.urlopen", return_value=response):
            with self.assertRaises(TimeoutError):
                publish_release.registry_checksum("fontdone", "2.14.3-alpha.3")


if __name__ == "__main__":
    unittest.main()
