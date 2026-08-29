#!/usr/bin/env python3
"""Run the approved all-lane coverage command with public case arguments.

Coverage MCP appends each value in its ``arguments`` array to this command.
The optional wrapper interface is:

    --migration-coverage-case-ids case-a,case-b

The selector may be repeated with comma-separated chunks when a caller has a
per-argument size limit:

    --migration-coverage-case-ids case-a,case-b \
    --migration-coverage-case-ids case-c,case-d

For a compact high-throughput window, select an inclusive 1-based range of
valid public parity cases in generated input order:

    --migration-coverage-case-range 8253:9252

Case IDs containing a comma use a backslash escape with the plural form, or
the exact singular form when the value is passed as one argument:

    --migration-coverage-case-ids 'case[0\,1],case-two'
    --migration-coverage-case-id 'case[0,1]'

The normal managed interface uses the existing Make command with
``MIGRATION_COVERAGE_CASE_IDS=case-a,case-b``. This wrapper remains available
for callers that specifically need the flag spelling.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path


CASE_SELECTOR_ENV = "FONTDONE_UNIFIED_CASE_IDS"
REPO_ROOT = Path(__file__).resolve().parents[1]
INPUT_CASES_PATH = REPO_ROOT / "target" / "unified-fixtures" / "input_cases.json"
ROUTE_AUDIT_PATH = REPO_ROOT / "target" / "api-abi-audit" / "route_audit.json"
PUBLIC_ROUTE_CATEGORIES = {"real-parity", "real-null-validation"}


def split_case_ids(value: str) -> list[str]:
    case_ids: list[str] = []
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
            case_ids.append("".join(current).strip())
            current = []
        else:
            current.append(character)
    if escaped:
        raise argparse.ArgumentTypeError("case IDs must not end with an escape")
    case_ids.append("".join(current).strip())
    if not case_ids or any(not case_id for case_id in case_ids):
        raise argparse.ArgumentTypeError(
            "case IDs must be a non-empty comma-separated list"
        )
    if len(case_ids) != len(set(case_ids)):
        raise argparse.ArgumentTypeError("case IDs must not contain duplicates")
    return case_ids


def parse_case_id(value: str) -> str:
    case_id = value.strip()
    if not case_id:
        raise argparse.ArgumentTypeError("case ID must not be empty")
    return case_id


def escape_case_id(case_id: str) -> str:
    return case_id.replace("\\", "\\\\").replace(",", "\\,")


def parse_case_range(value: str) -> tuple[int, int]:
    start_text, separator, end_text = value.partition(":")
    if not separator or ":" in end_text:
        raise argparse.ArgumentTypeError(
            "case range must use the inclusive START:END form"
        )
    try:
        start = int(start_text)
        end = int(end_text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(
            "case range bounds must be positive integers"
        ) from exc
    if start < 1 or end < start:
        raise argparse.ArgumentTypeError(
            "case range must have 1 <= START <= END"
        )
    return start, end


def public_parity_case_ids() -> list[str]:
    try:
        input_cases = json.loads(INPUT_CASES_PATH.read_text())
        route_audit = json.loads(ROUTE_AUDIT_PATH.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeError(
            "public case ranges require generated input_cases.json and "
            "route_audit.json; run make api-abi-runtime-check first"
        ) from exc

    eligible_ids = {
        row.get("runtime_id") or row["case_id"]
        for row in route_audit.get("rows", [])
        if row.get("category") in PUBLIC_ROUTE_CATEGORIES
    }
    return [
        case["case_id"]
        for case in input_cases.get("cases", [])
        if case.get("case_id") in eligible_ids
    ]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run the approved fontdone all-lane coverage command."
    )
    parser.add_argument(
        "--migration-coverage-case-ids",
        type=split_case_ids,
        action="append",
        metavar="CASE_ID,...",
        help="run exact public case IDs as a comma-separated list (repeatable; escape literal commas as \\,)",
    )
    parser.add_argument(
        "--migration-coverage-case-id",
        type=parse_case_id,
        action="append",
        metavar="CASE_ID",
        help="run one exact public parity case ID (repeatable; preserves literal commas)",
    )
    parser.add_argument(
        "--migration-coverage-case-range",
        type=parse_case_range,
        action="append",
        metavar="START:END",
        help=(
            "run the inclusive 1-based range of valid public parity cases "
            "in generated input order (repeatable)"
        ),
    )
    args = parser.parse_args()

    environment = os.environ.copy()
    environment.pop(CASE_SELECTOR_ENV, None)
    case_ids = [
        case_id
        for chunk in args.migration_coverage_case_ids or []
        for case_id in chunk
    ]
    case_ids.extend(args.migration_coverage_case_id or [])
    if args.migration_coverage_case_range:
        try:
            eligible_ids = public_parity_case_ids()
        except RuntimeError as exc:
            parser.error(str(exc))
        for start, end in args.migration_coverage_case_range:
            if end > len(eligible_ids):
                parser.error(
                    f"public case range {start}:{end} exceeds the "
                    f"{len(eligible_ids)} eligible public parity cases"
                )
            case_ids.extend(eligible_ids[start - 1 : end])
    if len(case_ids) != len(set(case_ids)):
        parser.error("case IDs must not contain duplicates")
    if case_ids:
        environment[CASE_SELECTOR_ENV] = ",".join(
            escape_case_id(case_id) for case_id in case_ids
        )

    os.execvpe("make", ["make", "test-coverage-all"], environment)
    return 0


if __name__ == "__main__":
    sys.exit(main())
