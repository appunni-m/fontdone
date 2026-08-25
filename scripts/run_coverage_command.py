#!/usr/bin/env python3
"""Run the approved all-lane coverage command with public case arguments.

Coverage MCP appends each value in its ``arguments`` array to this command.
The public interface is intentionally argument-based:

    --migration-coverage-case-ids case-a,case-b

The parity harness keeps the selector in its internal environment variable so
all Rust, C-ABI, and WASM lane processes inherit the same exact allowlist.
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
        metavar="CASE_ID,...",
        help="run only these exact public parity case IDs",
    )
    args = parser.parse_args()

    environment = os.environ.copy()
    environment.pop(CASE_SELECTOR_ENV, None)
    if args.migration_coverage_case_ids is not None:
        environment[CASE_SELECTOR_ENV] = args.migration_coverage_case_ids

    os.execvpe("make", ["make", "test-coverage-all"], environment)
    return 0


if __name__ == "__main__":
    sys.exit(main())
