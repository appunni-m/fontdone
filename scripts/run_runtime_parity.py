#!/usr/bin/env python3
"""Run full runtime parity and write durable, source-bound summary evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "target" / "parity-evidence"
REPORT = OUTPUT_DIR / "runtime_parity.json"
LOG = OUTPUT_DIR / "test-parity.log"
COMMITTED_EVIDENCE = ROOT / "doc" / "runtime_parity_evidence.json"
SNAPSHOT = ROOT / "doc" / "compatibility_snapshot.json"
README = ROOT / "README.md"

RUNTIME_CASES = re.compile(r"runtime_cases: runnable=(\d+) pending=(\d+)")
RUNTIME_PARITY = re.compile(
    r"runtime_parity: passed=(\d+) failed=(\d+) total=(\d+) "
    r"covered_manifest_cases=(\d+)"
)
EXPLICIT_INPUTS = re.compile(
    r"explicit_inputs: logical_cases=(\d+) concrete_cases=(\d+) "
    r"additional_explicit_cases=(\d+) implicit_cases=(\d+)"
)

SOURCE_PREFIXES = (
    "src/",
    "fontdone-c-abi/src/",
    "fontdone-c-abi/include/",
    "fontdone-wasm/src/",
    "tests/",
    "scripts/",
)
SOURCE_FILES = {
    "Cargo.lock",
    "Cargo.toml",
    "Makefile",
    "rust-toolchain.toml",
    "fontdone-c-abi/Cargo.toml",
    "fontdone-c-abi/build.rs",
    "fontdone-wasm/Cargo.toml",
    "fontdone-wasm/abi.json",
}
NON_PARITY_SCRIPTS = {
    "scripts/audit_repository_files.py",
    "scripts/bench_freetype.py",
    "scripts/bench_ft_ops.c",
    "scripts/check_documentation.py",
    "scripts/check_rustdoc_contracts.py",
    "scripts/build_coverage_region_queue.py",
    "scripts/publish_release.py",
    "scripts/test_rust_consumer.py",
    "scripts/verify_release.py",
}
NON_PARITY_SUFFIXES = {".md", ".txt"}


def git_paths() -> list[str]:
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
    return sorted(
        {
            raw.decode("utf-8")
            for raw in result.stdout.split(b"\0")
            if raw and (ROOT / raw.decode("utf-8")).is_file()
        }
    )


def parity_source_paths() -> list[str]:
    paths: list[str] = []
    for path in git_paths():
        if path in SOURCE_FILES:
            paths.append(path)
            continue
        if not path.startswith(SOURCE_PREFIXES):
            continue
        if path in NON_PARITY_SCRIPTS:
            continue
        if Path(path).suffix.lower() in NON_PARITY_SUFFIXES:
            continue
        paths.append(path)
    return paths


def parity_source_digest() -> tuple[str, int]:
    digest = hashlib.sha256()
    paths = parity_source_paths()
    for path in paths:
        data = (ROOT / path).read_bytes()
        encoded = path.encode("utf-8")
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
        digest.update(len(data).to_bytes(8, "big"))
        digest.update(data)
    return digest.hexdigest(), len(paths)


def command_output(command: list[str]) -> str:
    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def git_worktree_dirty() -> bool:
    result = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    return bool(result.stdout)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def last_match(pattern: re.Pattern[str], text: str, label: str) -> re.Match[str]:
    matches = list(pattern.finditer(text))
    if not matches:
        raise ValueError(f"full parity output lacks {label}")
    return matches[-1]


def route_counts() -> tuple[int, int]:
    route_path = ROOT / "target" / "api-abi-audit" / "route_audit.json"
    route = json.loads(route_path.read_text(encoding="utf-8"))
    coverage = route["function_coverage"]
    totals = {
        lane: (
            coverage[lane]["covered"],
            coverage[lane]["total"],
        )
        for lane in ("rust", "c_abi", "wasm")
    }
    if len(set(totals.values())) != 1:
        raise ValueError(f"function route totals disagree by lane: {totals}")
    return totals["rust"]


def selected_case_ids() -> set[str] | None:
    value = os.environ.get("FONTDONE_UNIFIED_CASE_IDS", "").strip()
    if not value:
        return None

    case_id_values: list[str] = []
    current: list[str] = []
    escaped = False
    for character in value:
        if escaped:
            if character not in (",", "\\"):
                current.append("\\")
            current.append(character)
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == ",":
            case_id_values.append("".join(current).strip())
            current = []
        else:
            current.append(character)
    if escaped:
        raise ValueError("FONTDONE_UNIFIED_CASE_IDS must not end with an escape")
    case_id_values.append("".join(current).strip())

    case_ids = set(case_id_values)
    if not case_ids or "" in case_ids:
        raise ValueError("FONTDONE_UNIFIED_CASE_IDS contains an empty case ID")
    if len(case_ids) != len(case_id_values):
        raise ValueError("FONTDONE_UNIFIED_CASE_IDS contains duplicate case IDs")
    return case_ids


def build_report(log_text: str, exit_code: int) -> dict[str, object]:
    runtime_cases = last_match(RUNTIME_CASES, log_text, "runtime_cases summary")
    runtime = last_match(RUNTIME_PARITY, log_text, "runtime_parity summary")
    explicit = last_match(EXPLICIT_INPUTS, log_text, "explicit_inputs summary")
    runnable, pending = map(int, runtime_cases.groups())
    passed, failed, total, covered = map(int, runtime.groups())
    logical, concrete, additional, implicit = map(int, explicit.groups())
    if runnable != total:
        raise ValueError(f"runnable {runnable} does not equal runtime total {total}")
    case_ids = selected_case_ids()
    if case_ids is None:
        if concrete != runnable + pending:
            raise ValueError(
                f"concrete input cases {concrete} do not equal runnable + pending "
                f"({runnable + pending})"
            )
    elif len(case_ids) != runnable + pending:
        raise ValueError(
            f"selected case IDs {len(case_ids)} do not equal runnable + pending "
            f"({runnable + pending})"
        )
    if additional != concrete - logical or implicit != 0:
        raise ValueError("explicit-input accounting is internally inconsistent")
    route_covered, route_total = route_counts()
    source_digest, source_path_count = parity_source_digest()
    public_input_files = len(
        list((ROOT / "tests" / "fixtures" / "inputs" / "public-api").rglob("*.json"))
    )
    public_input_subjects = len(
        re.findall(
            r"^  - id: ",
            (ROOT / "tests" / "manifest.yaml").read_text(),
            re.MULTILINE,
        )
    )
    oracle = ROOT / "target" / "unified-fixtures" / "gen_unified_oracle"
    report: dict[str, object] = {
        "schema_version": 1,
        "evidence_kind": "full_runtime_parity",
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "command": "make test-parity",
        "source": {
            "git_head": os.environ.get("GITHUB_SHA")
            or command_output(["git", "rev-parse", "HEAD"]),
            "git_worktree_dirty": git_worktree_dirty(),
            "parity_tree_sha256": source_digest,
            "parity_path_count": source_path_count,
        },
        "toolchain": {
            "rustc": command_output(["rustc", "--version"]),
            "cargo": command_output(["cargo", "--version"]),
        },
        "run": {
            "github_run_id": os.environ.get("GITHUB_RUN_ID"),
            "github_run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
            "github_repository": os.environ.get("GITHUB_REPOSITORY"),
        },
        "artifacts": {
            "log": str(LOG.relative_to(ROOT)),
            "log_sha256": hashlib.sha256(log_text.encode("utf-8")).hexdigest(),
            "oracle": str(oracle.relative_to(ROOT)),
            "oracle_sha256": sha256_file(oracle),
        },
        "runtime_parity": {
            "runnable": runnable,
            "passed": passed,
            "failed": failed,
            "pending": pending,
            "covered_manifest": covered,
            "validated_input_subjects": public_input_subjects,
            "validated_input_files": public_input_files,
            "logical_input_cases": logical,
            "concrete_input_cases": concrete,
            "function_route_evidence": route_covered,
            "function_total": route_total,
        },
        "selection": {
            "kind": "case_ids" if case_ids is not None else "full",
            "case_count": len(case_ids) if case_ids is not None else concrete,
        },
        "test_exit_code": exit_code,
        "complete": (
            case_ids is None
            and exit_code == 0
            and failed == 0
            and passed == runnable
        ),
    }
    return report


def run_parity() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "test",
        "--test",
        "unified_fixture_parity",
        "--locked",
        "unified_fixture_parity",
        "--",
        "--nocapture",
    ]
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    lines: list[str] = []
    assert process.stdout is not None
    for line in process.stdout:
        sys.stdout.write(line)
        sys.stdout.flush()
        lines.append(line)
    exit_code = process.wait()
    log_text = "".join(lines)
    LOG.write_text(log_text, encoding="utf-8")
    try:
        report = build_report(log_text, exit_code)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f"runtime parity evidence failed: {exc}", file=sys.stderr)
        return exit_code or 1
    REPORT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    runtime = report["runtime_parity"]
    assert isinstance(runtime, dict)
    print(
        "runtime parity evidence: "
        f"{runtime['passed']}/{runtime['runnable']} passed, "
        f"{runtime['pending']} pending, report={REPORT.relative_to(ROOT)}"
    )
    return exit_code


def replace_readme_counts(runtime: dict[str, int], recorded_on: str) -> None:
    text = README.read_text(encoding="utf-8")
    text = re.sub(
        r"The last committed full parity snapshot was recorded on \*\*[^*]+\*\*:",
        f"The last committed full parity snapshot was recorded on **{recorded_on}**:",
        text,
    )
    rows = {
        "Runnable exact-comparison cases": runtime["runnable"],
        "Passed cases": runtime["passed"],
        "Failed cases": runtime["failed"],
        "Explicitly pending cases": runtime["pending"],
        "Covered manifest cases": runtime["covered_manifest"],
        "Validated public API subjects": runtime["validated_input_subjects"],
        "Validated public API input files": runtime["validated_input_files"],
        "Logical declared cases": runtime["logical_input_cases"],
        "Concrete expanded cases": runtime["concrete_input_cases"],
    }
    for label, value in rows.items():
        text, replacements = re.subn(
            rf"(\|\s*{re.escape(label)}\s*\|\s*)[\d,]+(\s*\|)",
            rf"\g<1>{value:,}\g<2>",
            text,
        )
        if replacements != 1:
            raise ValueError(f"README runtime row {label!r} was not replaced exactly once")
    route = (
        f"{runtime['function_route_evidence']:,} / "
        f"{runtime['function_total']:,}"
    )
    text, replacements = re.subn(
        r"(\|\s*Functions with at least one C/Rust/C-ABI/WASM runtime route\s*\|\s*)"
        r"[\d,]+\s*/\s*[\d,]+(\s*\|)",
        rf"\g<1>{route}\g<2>",
        text,
    )
    if replacements != 1:
        raise ValueError("README function-route row was not replaced exactly once")
    README.write_text(text, encoding="utf-8")


def record_snapshot() -> int:
    if not REPORT.is_file():
        print(f"missing {REPORT.relative_to(ROOT)}; run make test-parity", file=sys.stderr)
        return 1
    report = json.loads(REPORT.read_text(encoding="utf-8"))
    if not report.get("complete"):
        print("refusing to record incomplete runtime parity evidence", file=sys.stderr)
        return 1
    current_digest, current_count = parity_source_digest()
    source = report["source"]
    if (
        source["parity_tree_sha256"] != current_digest
        or source["parity_path_count"] != current_count
    ):
        print("parity-relevant source changed after the full run", file=sys.stderr)
        return 1
    evidence_text = json.dumps(report, indent=2) + "\n"
    COMMITTED_EVIDENCE.write_text(evidence_text, encoding="utf-8")
    snapshot = json.loads(SNAPSHOT.read_text(encoding="utf-8"))
    runtime = report["runtime_parity"]
    snapshot["snapshot_date"] = report["recorded_at_utc"][:10]
    snapshot["runtime_parity"] = dict(runtime)
    snapshot["runtime_evidence"] = {
        "path": str(COMMITTED_EVIDENCE.relative_to(ROOT)),
        "sha256": hashlib.sha256(evidence_text.encode("utf-8")).hexdigest(),
        "parity_tree_sha256": source["parity_tree_sha256"],
        "log_sha256": report["artifacts"]["log_sha256"],
        "command": report["command"],
    }
    SNAPSHOT.write_text(json.dumps(snapshot, indent=2) + "\n", encoding="utf-8")
    replace_readme_counts(snapshot["runtime_parity"], snapshot["snapshot_date"])
    print(
        "recorded runtime parity snapshot: "
        f"{runtime['passed']}/{runtime['runnable']} passed, "
        f"evidence={COMMITTED_EVIDENCE.relative_to(ROOT)}"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--record",
        action="store_true",
        help="copy a passing current report into the committed snapshot",
    )
    args = parser.parse_args()
    if args.record:
        return record_snapshot()
    return run_parity()


if __name__ == "__main__":
    raise SystemExit(main())
