#!/usr/bin/env python3
"""Generate the complete repository-file retention inventory."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import os
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "doc" / "FILE_RETENTION_INVENTORY.tsv"
SELF_CLEANING_PLANS = {
    "doc/ROADMAP.md",
}

ACTIVE_PLAN_DOCS = {
    "doc/ROADMAP.md",
}

FONT_INPUT_SUFFIXES = {
    ".afm",
    ".bdf",
    ".cid",
    ".fnt",
    ".fon",
    ".otf",
    ".otb",
    ".pcf",
    ".pfa",
    ".pfb",
    ".t42",
    ".ttc",
    ".ttf",
    ".ttx",
    ".woff",
    ".woff2",
}

FORMER_FIXTURE_ROOTS = (
    "tests/fixtures/assets/",
    "tests/fixtures/fixtures/",
    "tests/fixtures/font-sources/",
    "tests/fixtures/fonts/",
    "tests/fixtures/generated/",
    "tests/fixtures/malformed/",
)


def enforce_self_cleaning_plans() -> None:
    for path in SELF_CLEANING_PLANS:
        full_path = ROOT / path
        if not full_path.exists():
            continue
        text = full_path.read_text(encoding="utf-8")
        declared_match = re.search(r"^Open goals: \*\*(\d+)\*\*$", text, re.MULTILINE)
        states = re.findall(
            r"^\| G\d{2} \| (OPEN|BLOCKED|COMPLETE) \|",
            text,
            re.MULTILINE,
        )
        if declared_match is None or not states:
            raise SystemExit(f"{path}: missing open-goal count or evidence-ledger states")
        declared_open = int(declared_match.group(1))
        actual_open = sum(state != "COMPLETE" for state in states)
        if declared_open != actual_open:
            raise SystemExit(
                f"{path}: declares {declared_open} open goals but its ledger has "
                f"{actual_open}; update both values together"
            )
        if actual_open == 0:
            raise SystemExit(
                f"{path}: all goals are complete; move durable evidence, "
                "remove the plan from ACTIVE_PLAN_DOCS, and delete the plan"
            )


def enforce_fixture_layout(paths: list[str]) -> None:
    errors: list[str] = []
    for path in paths:
        full_path = ROOT / path
        if path.startswith(FORMER_FIXTURE_ROOTS):
            errors.append(f"{path}: legacy fixture root is forbidden")
        if path.startswith("tests/fixtures/") and full_path.is_symlink():
            errors.append(f"{path}: fixture aliases are forbidden; use the canonical input")
        if (
            path.startswith("tests/fixtures/")
            and Path(path).suffix.lower() in FONT_INPUT_SUFFIXES
            and not path.startswith("tests/fixtures/input/")
        ):
            errors.append(f"{path}: font input must live under tests/fixtures/input/")
    if errors:
        raise SystemExit("\n".join(errors))


def repository_paths() -> list[str]:
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    paths = set()
    for item in result.stdout.split(b"\0"):
        if not item:
            continue
        path = item.decode("utf-8")
        if path == OUTPUT.relative_to(ROOT).as_posix():
            continue
        full_path = ROOT / path
        # `git ls-files --cached` includes paths deleted in the prospective
        # worktree. The inventory describes the state about to be committed.
        if full_path.exists() or full_path.is_symlink():
            paths.add(path)
    paths.add(OUTPUT.relative_to(ROOT).as_posix())
    return sorted(paths)


def classify(path: str, kind: str) -> tuple[str, str, str]:
    if path == OUTPUT.relative_to(ROOT).as_posix():
        return (
            "generated-audit",
            "R11",
            "Generated exhaustive inventory; refresh with scripts/audit_repository_files.py.",
        )
    if kind == "symlink":
        return (
            "keep-alias",
            "R05",
            "Repository tooling alias whose path identity is an explicit contract.",
        )
    if path.startswith("src/"):
        if path in {
            "src/ffi/generated_constants.rs",
        }:
            return (
                "keep-generated-source",
                "R10",
                "Generated source required to build the runtime crate without the offline oracle.",
            )
        return ("keep-runtime", "R01", "Published pure-Rust runtime source.")
    if path.startswith(("fontdone-c-abi/", "fontdone-wasm/")):
        return (
            "keep-verification-facade",
            "R02",
            "Maintained ABI verification facade and workspace gate.",
        )
    if path.startswith("tests/fixtures/input/"):
        return (
            "keep-fixture-input",
            "R04",
            "Canonical maintained fixture input or its provenance/license record.",
        )
    if path.startswith("tests/fixtures/inputs/public-api/"):
        return (
            "keep-public-contract",
            "R03",
            "Executable public API parity input validated against tests/manifest.yaml.",
        )
    if path.startswith("tests/"):
        if path == "tests/support/generated_constant_lookup.rs":
            return (
                "keep-generated-source",
                "R10",
                "Generated lookup required by the public constant parity test.",
            )
        return (
            "keep-test-contract",
            "R03",
            "Maintained parity test, contract, focused fixture, source, or support file.",
        )
    if path.startswith("scripts/"):
        return (
            "keep-tooling",
            "R06",
            "Maintained oracle, audit, benchmark, or deterministic generation tooling.",
        )
    if path in ACTIVE_PLAN_DOCS:
        if path in SELF_CLEANING_PLANS:
            return (
                "keep-active-plan",
                "R08",
                "Temporary readiness plan; delete it when its open-goal count reaches zero.",
            )
        return (
            "keep-active-plan",
            "R08",
            "Incomplete implementation plan; condense and remove only after its open work closes.",
        )
    if path.startswith("doc/"):
        return (
            "keep-documentation",
            "R07",
            "Durable design, debugging, testing, benchmark, fixture, or compatibility context.",
        )
    if path.startswith(".github/"):
        return ("keep-ci", "R09", "Public continuous-integration contract.")
    if path.startswith("examples/"):
        return (
            "keep-developer-tool",
            "R06",
            "Maintained benchmark or executable diagnostic/example.",
        )
    if path in {
        "CODE_OF_CONDUCT.md",
        "CONTRIBUTING.md",
        "SECURITY.md",
    }:
        return ("keep-community", "R09", "Public community or security contract.")
    return (
        "keep-root-contract",
        "R02",
        "Root build, package, license, release, agent, or project contract.",
    )


def file_metadata(path: str) -> tuple[int, str, str, str]:
    full_path = ROOT / path
    if path == OUTPUT.relative_to(ROOT).as_posix():
        return 0, "generated", "-", "-"
    if full_path.is_symlink():
        target = os.readlink(full_path)
        return len(target.encode()), "symlink", "-", target
    data = full_path.read_bytes()
    return len(data), "file", hashlib.sha256(data).hexdigest(), "-"


def render_inventory() -> str:
    enforce_self_cleaning_plans()
    paths = repository_paths()
    enforce_fixture_layout(paths)
    output = io.StringIO(newline="")
    writer = csv.writer(output, delimiter="\t", lineterminator="\n")
    writer.writerow(
        [
            "path",
            "bytes",
            "kind",
            "sha256",
            "symlink_target",
            "retention_class",
            "reason_code",
            "reason",
        ]
    )
    for path in paths:
        size, kind, digest, target = file_metadata(path)
        retention_class, reason_code, reason = classify(path, kind)
        writer.writerow(
            [
                path,
                size,
                kind,
                digest,
                target,
                retention_class,
                reason_code,
                reason,
            ]
        )
    return output.getvalue()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when the tracked inventory differs from a fresh rendering",
    )
    args = parser.parse_args()
    rendered = render_inventory()
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_text(encoding="utf-8") != rendered:
            raise SystemExit(
                "doc/FILE_RETENTION_INVENTORY.tsv is stale; "
                "run make repository-inventory"
            )
        print("repository retention inventory: clean")
        return
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(rendered, encoding="utf-8")
    print(f"repository retention inventory: wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
