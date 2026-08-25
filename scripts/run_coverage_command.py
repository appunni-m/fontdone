#!/usr/bin/env python3
"""Run the approved all-lane coverage command with public case arguments.

Coverage MCP appends each value in its ``arguments`` array to this command.
The optional wrapper interface is:

    --migration-coverage-case-ids case-a,case-b

The selector may be repeated with comma-separated chunks when a caller has a
per-argument size limit:

    --migration-coverage-case-ids case-a,case-b \
    --migration-coverage-case-ids case-c,case-d

The normal managed interface uses the existing Make command with
``MIGRATION_COVERAGE_CASE_IDS=case-a,case-b``. This wrapper remains available
for callers that specifically need the flag spelling.
"""

from __future__ import annotations

import argparse
import os
import sys


CASE_SELECTOR_ENV = "FONTDONE_UNIFIED_CASE_IDS"


def parse_case_ids(value: str) -> str:
    case_ids = [case_id.strip() for case_id in value.split(",")]
    if not case_ids or any(not case_id for case_id in case_ids):
        raise argparse.ArgumentTypeError(
            "case IDs must be a non-empty comma-separated list"
        )
    if len(case_ids) != len(set(case_ids)):
        raise argparse.ArgumentTypeError("case IDs must not contain duplicates")
    return ",".join(case_ids)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run the approved fontdone all-lane coverage command."
    )
    parser.add_argument(
        "--migration-coverage-case-ids",
        type=parse_case_ids,
        action="append",
        metavar="CASE_ID,...",
        help="run only these exact public parity case IDs (repeatable)",
    )
    args = parser.parse_args()

    environment = os.environ.copy()
    environment.pop(CASE_SELECTOR_ENV, None)
    if args.migration_coverage_case_ids is not None:
        environment[CASE_SELECTOR_ENV] = parse_case_ids(
            ",".join(args.migration_coverage_case_ids)
        )

    os.execvpe("make", ["make", "test-coverage-all"], environment)
    return 0


if __name__ == "__main__":
    sys.exit(main())
