#!/usr/bin/env python3
"""Build and reconcile the ignored DuckDB queue for uncovered LLVM regions.

The queue is a derived planning artifact.  Coverage MCP remains the source of
truth for snapshots and runs; this script only materializes the exact
zero-hit source coordinates from an LLVM JSON report and records the reasoning
and disposition history needed to plan public parity inputs.

Examples (DuckDB is installed in the ignored target dependency directory)::

    PYTHONPATH=target/coverage-campaign-deps python3 \
      scripts/build_coverage_region_queue.py seed \
      --coverage-json target/coverage/unified-runtime-all-lanes.json \
      --db target/coverage/region_campaign.duckdb \
      --snapshot-id f435e2c3-5b75-43bc-b3fe-e52c9bc9d9c6 \
      --baseline-snapshot-id 0c3fbdf9-76de-4aff-a575-5dbb942f2495 \
      --batch-id batch330 --batch-size 100

    PYTHONPATH=target/coverage-campaign-deps python3 \
      scripts/build_coverage_region_queue.py reconcile \
      --coverage-json target/coverage/unified-runtime-all-lanes.json \
      --db target/coverage/region_campaign.duckdb \
      --snapshot-id <complete-verification-snapshot> \
      --run-id <coverage-mcp-run> --batch-id batch330

The ``reconcile`` command is deliberately explicit about whether the report
is a complete snapshot or a selected incremental subset.  Only a complete
snapshot can move a region to ``done``.  An incremental hit is recorded as
``hit_pending_full`` and a miss increments ``tries`` and remains searchable in
``queue_history``.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = "fontdone/coverage-region-queue@1"


def die(message: str) -> "NoReturn":
    raise SystemExit(message)


def duckdb_module():
    try:
        import duckdb  # type: ignore
    except ImportError as exc:  # pragma: no cover - environment guidance
        die(
            "DuckDB is required. Install it in an ignored environment, for "
            "example: mkdir -p target/coverage-campaign-deps && "
            "python3 -m pip install --target target/coverage-campaign-deps duckdb"
        )
    return duckdb


def now_text() -> str:
    return datetime.now(timezone.utc).isoformat()


def canonical_path(raw: str) -> str:
    path = Path(raw)
    try:
        return str(path.resolve().relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def absolute_path(raw: str) -> Path | None:
    path = Path(raw)
    if path.is_absolute():
        return path
    return REPO_ROOT / path


def report_data(report_path: Path) -> dict[str, Any]:
    try:
        report = json.loads(report_path.read_text())
        data = report["data"][0]
    except (OSError, json.JSONDecodeError, KeyError, IndexError, TypeError) as exc:
        die(f"invalid LLVM coverage JSON {report_path}: {exc}")
    if not isinstance(data, dict) or "files" not in data or "functions" not in data:
        die(f"coverage JSON has no usable files/functions data: {report_path}")
    return data


def source_line(path: str, line: int) -> str:
    file_path = absolute_path(path)
    if file_path is None:
        return ""
    try:
        lines = file_path.read_text(errors="replace").splitlines()
        if 1 <= line <= len(lines):
            return lines[line - 1].strip()
    except OSError:
        pass
    return ""


def source_window(path: str, line: int, radius: int = 2) -> str:
    file_path = absolute_path(path)
    if file_path is None:
        return ""
    try:
        lines = file_path.read_text(errors="replace").splitlines()
    except OSError:
        return ""
    start = max(1, line - radius)
    end = min(len(lines), line + radius)
    return "\\n".join(f"{number}: {lines[number - 1].strip()}" for number in range(start, end + 1))


def family_for(path: str) -> str:
    if path.startswith("fontdone-c-abi/"):
        return "c_abi"
    if path.startswith("fontdone-wasm/"):
        return "wasm"
    if path == "src/ffi/handles.rs":
        return "ffi_handles"
    if path in {
        "src/font.rs",
        "src/render.rs",
        "src/grays.rs",
        "src/autohint/latin.rs",
        "src/autohint/cjk.rs",
    }:
        return "core_render_autohint"
    return "tables_formats"


def difficulty_for(path: str, text: str) -> str:
    if "unreachable!" in text or "unreachable" in text:
        return "source-proof-first"
    if path.startswith("fontdone-wasm/") or path == "src/ffi/handles.rs":
        return "medium-safety-review"
    if any(token in text for token in ("parse", "read", "length", "offset", "format")):
        return "high-oracle-reduction"
    return "medium-public-input-search"


def reason_for(path: str, line: int, column: int, text: str, family: str, snapshot: str) -> str:
    span = f"{path}:{line}:{column}"
    if "unreachable!" in text or "unreachable" in text:
        return (
            f"Baseline snapshot {snapshot} leaves {span} at zero hits. The source "
            "looks defensive or logically unreachable, so first prove its domain "
            "with the pinned FreeType path; do not fabricate private state merely "
            "to obtain a hit."
        )
    axis = {
        "c_abi": "a public pointer, size, flag, stream, bitmap, or record field",
        "ffi_handles": "a public FFI record field or documented malformed table/array",
        "wasm": "a public WASM argument, allocator result, bitmap field, or lifecycle sequence",
        "core_render_autohint": "a public font-format, glyph, size, render flag, or hinting input",
        "tables_formats": "one isolated public SFNT/CFF/bitmap table boundary or malformed length",
    }[family]
    return (
        f"Baseline snapshot {snapshot} leaves {span} at zero hits in the {family} "
        f"surface (source: {text or 'source text unavailable'}). A candidate should "
        f"vary {axis} while keeping the entry point and comparison observable."
    )


def approach_for(path: str, family: str, text: str) -> str:
    if "unreachable!" in text or "unreachable" in text:
        return (
            "Read the enclosing invariant and pinned C implementation first. "
            "If every safe public route excludes the arm, record source-proof "
            "and refactor the denominator rather than adding an unsafe case."
        )
    return {
        "c_abi": (
            "Start from the nearest maintained public C-ABI fixture. Change one "
            "null/size/flag/stream/record axis, run the pinned C oracle, then run "
            "the exact case through all parity lanes and retain only exact output/error parity."
        ),
        "ffi_handles": (
            "Use the public FFI operation that owns this record. Prefer a valid "
            "object with one malformed public field; use null validation only when "
            "the ABI documents it, and never dereference a fabricated nonzero handle."
        ),
        "wasm": (
            "Reuse an existing public WASM lifecycle case and vary one allocator, "
            "bitmap, stream, or documented argument boundary. Validate the C oracle "
            "where applicable; treat impossible nonzero handles as source-proof items."
        ),
        "core_render_autohint": (
            "Construct or select the smallest public font/glyph/size/flag witness. "
            "Compare the first divergent stage against pinned FreeType, then add a "
            "parity fixture only if the same public input reaches this span."
        ),
        "tables_formats": (
            "Generate one minimal public font or bitmap with the target table field "
            "at its boundary. Probe the oracle before changing Rust; isolate malformed "
            "input from unrelated table errors and keep the resulting case input-only."
        ),
    }[family]


def region_inventory(data: dict[str, Any], snapshot_id: str) -> list[dict[str, Any]]:
    """Return unique zero-hit source coordinates in the report denominator.

    LLVM's function list can repeat an inline/source coordinate.  We aggregate
    those records by source coordinate and consider it covered if any instance
    has a positive count.  Filtering to ``files`` keeps Rust stdlib/instrumented
    helper coordinates outside the report denominator out of the queue.
    """
    denominator_paths = {str(file["filename"]) for file in data.get("files", [])}
    regions = coordinate_states(data)
    branch_keys: set[tuple[str, int, int, int, int]] = set()
    for file in data.get("files", []):
        raw_path = str(file.get("filename", ""))
        if raw_path not in denominator_paths:
            continue
        for branch in file.get("branches", []):
            if len(branch) >= 4:
                branch_keys.add((raw_path, *(int(value) for value in branch[:4])))
    for function in data.get("functions", []):
        names = str(function.get("name", ""))
        for raw_path in function.get("filenames", []):
            raw_path = str(raw_path)
            if raw_path not in denominator_paths:
                continue
            for region in function.get("regions", []):
                if len(region) < 5:
                    continue
                key = (raw_path, *(int(value) for value in region[:4]))
                item = regions[key]
                count = int(region[4])
                item["covered"] = item["covered"] or count > 0
                item["counts"].append(count)
                if names:
                    item["functions"].add(names)
    result: list[dict[str, Any]] = []
    for item in regions.values():
        if item["covered"]:
            continue
        path = item["path"]
        text = source_line(path, item["start_line"])
        family = family_for(path)
        coordinates = (
            f"{item['start_line']}:{item['start_column']}-"
            f"{item['end_line']}:{item['end_column']}"
        )
        region_id = "r-" + hashlib.sha256(
            f"{path}|{coordinates}".encode("utf-8")
        ).hexdigest()[:24]
        item.update(
            {
                "region_id": region_id,
                "family": family,
                "region_kind": "branch-associated" if (item["raw_path"], item["start_line"], item["start_column"], item["end_line"], item["end_column"]) in branch_keys else "source-region",
                "source_text": text,
                "source_context": source_window(path, item["start_line"]),
                "function_names": "; ".join(sorted(item["functions"])[:8]),
                "difficulty": difficulty_for(path, text),
                "reason": reason_for(path, item["start_line"], item["start_column"], text, family, snapshot_id),
                "approach": approach_for(path, family, text),
                "pinned_c_status": "unreviewed",
                "pinned_c_source": "",
                "case_id": None,
            }
        )
        result.append(item)
    return sorted(result, key=lambda item: (item["family"], item["path"], item["start_line"], item["start_column"], item["end_line"], item["end_column"]))


def coordinate_states(data: dict[str, Any]) -> dict[tuple[str, int, int, int, int], dict[str, Any]]:
    """Aggregate all denominator coordinates, including already-hit ones."""
    denominator_paths = {str(file["filename"]) for file in data.get("files", [])}
    regions: dict[tuple[str, int, int, int, int], dict[str, Any]] = {}
    for function in data.get("functions", []):
        names = str(function.get("name", ""))
        for raw_path in function.get("filenames", []):
            raw_path = str(raw_path)
            if raw_path not in denominator_paths:
                continue
            for region in function.get("regions", []):
                if len(region) < 5:
                    continue
                key = (raw_path, *(int(value) for value in region[:4]))
                item = regions.setdefault(
                    key,
                    {
                        "raw_path": raw_path,
                        "path": canonical_path(raw_path),
                        "start_line": key[1],
                        "start_column": key[2],
                        "end_line": key[3],
                        "end_column": key[4],
                        "covered": False,
                        "counts": [],
                        "functions": set(),
                    },
                )
                item["covered"] = item["covered"] or int(region[4]) > 0
                item["counts"].append(int(region[4]))
                if names:
                    item["functions"].add(names)
    return regions


def schema(connection: Any) -> None:
    connection.execute("CREATE SEQUENCE IF NOT EXISTS queue_history_seq START 1")
    connection.execute(
        """CREATE TABLE IF NOT EXISTS queue_metadata (
            key VARCHAR PRIMARY KEY,
            value VARCHAR NOT NULL
        )"""
    )
    connection.execute(
        """CREATE TABLE IF NOT EXISTS region_queue (
            region_id VARCHAR PRIMARY KEY,
            file_path VARCHAR NOT NULL,
            family VARCHAR NOT NULL,
            region_kind VARCHAR NOT NULL,
            function_names VARCHAR NOT NULL,
            start_line INTEGER NOT NULL,
            start_column INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            end_column INTEGER NOT NULL,
            source_text VARCHAR NOT NULL,
            source_context VARCHAR NOT NULL,
            difficulty VARCHAR NOT NULL,
            status VARCHAR NOT NULL,
            tries INTEGER NOT NULL DEFAULT 0,
            reason VARCHAR NOT NULL,
            approach VARCHAR NOT NULL,
            pinned_c_status VARCHAR NOT NULL,
            pinned_c_source VARCHAR NOT NULL,
            candidate_case_id VARCHAR,
            first_snapshot_id VARCHAR NOT NULL,
            last_run_id VARCHAR,
            last_snapshot_id VARCHAR,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )"""
    )
    connection.execute(
        """CREATE TABLE IF NOT EXISTS case_plan (
            case_id VARCHAR PRIMARY KEY,
            batch_id VARCHAR NOT NULL,
            ordinal INTEGER NOT NULL,
            agent VARCHAR NOT NULL,
            family VARCHAR NOT NULL,
            target_region_ids VARCHAR NOT NULL,
            target VARCHAR NOT NULL,
            public_entrypoint VARCHAR NOT NULL,
            route VARCHAR NOT NULL,
            input_spec VARCHAR NOT NULL,
            reachable_because VARCHAR NOT NULL,
            pinned_c VARCHAR NOT NULL,
            leverage VARCHAR NOT NULL,
            classification VARCHAR NOT NULL,
            status VARCHAR NOT NULL,
            parity_run_id VARCHAR,
            incremental_snapshot_id VARCHAR,
            verification_snapshot_id VARCHAR,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )"""
    )
    connection.execute(
        """CREATE TABLE IF NOT EXISTS case_region_plan (
            case_id VARCHAR NOT NULL,
            batch_id VARCHAR NOT NULL,
            region_id VARCHAR NOT NULL,
            relation VARCHAR NOT NULL,
            PRIMARY KEY (case_id, region_id)
        )"""
    )
    # Add planning evidence to existing campaign databases without reseeding
    # or losing their append-only execution history.
    for column in ("family_id", "source_context", "risks", "stop_condition"):
        connection.execute(f"ALTER TABLE case_plan ADD COLUMN IF NOT EXISTS {column} VARCHAR DEFAULT ''")
    connection.execute(
        """CREATE TABLE IF NOT EXISTS batch_plan (
            batch_id VARCHAR NOT NULL,
            ordinal INTEGER NOT NULL,
            region_id VARCHAR NOT NULL,
            candidate_case_id VARCHAR NOT NULL,
            agent VARCHAR NOT NULL,
            family VARCHAR NOT NULL,
            reason VARCHAR NOT NULL,
            approach VARCHAR NOT NULL,
            status VARCHAR NOT NULL,
            tries INTEGER NOT NULL DEFAULT 0,
            last_run_id VARCHAR,
            last_snapshot_id VARCHAR,
            updated_at TIMESTAMP NOT NULL,
            PRIMARY KEY (batch_id, ordinal)
        )"""
    )
    connection.execute(
        """CREATE TABLE IF NOT EXISTS queue_history (
            history_id BIGINT PRIMARY KEY DEFAULT nextval('queue_history_seq'),
            region_id VARCHAR NOT NULL,
            batch_id VARCHAR,
            try_no INTEGER NOT NULL,
            event VARCHAR NOT NULL,
            case_id VARCHAR,
            run_id VARCHAR,
            snapshot_id VARCHAR,
            details VARCHAR NOT NULL,
            observed_at TIMESTAMP NOT NULL
        )"""
    )
    connection.execute(
        "CREATE INDEX IF NOT EXISTS region_queue_status_idx ON region_queue(status, family)"
    )
    connection.execute(
        "CREATE INDEX IF NOT EXISTS region_queue_source_idx ON region_queue(file_path, start_line)"
    )
    connection.execute(
        "CREATE INDEX IF NOT EXISTS queue_history_region_idx ON queue_history(region_id, observed_at)"
    )
    connection.execute(
        "CREATE INDEX IF NOT EXISTS queue_history_event_idx ON queue_history(event, observed_at)"
    )


def metadata(connection: Any, key: str, value: str) -> None:
    connection.execute(
        "INSERT INTO queue_metadata(key, value) VALUES (?, ?) "
        "ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [key, value],
    )


def seed(args: argparse.Namespace) -> None:
    duckdb = duckdb_module()
    report_path = Path(args.coverage_json)
    data = report_data(report_path)
    items = region_inventory(data, args.snapshot_id)
    expected = int(data.get("totals", {}).get("regions", {}).get("notcovered", -1))
    if expected >= 0 and expected != len(items):
        die(f"exact uncovered-region check failed: report={expected}, derived={len(items)}")
    db_path = Path(args.db)
    db_path.parent.mkdir(parents=True, exist_ok=True)
    connection = duckdb.connect(str(db_path))
    try:
        schema(connection)
        timestamp = now_text()
        metadata(connection, "schema", SCHEMA_VERSION)
        metadata(connection, "repository", str(REPO_ROOT))
        metadata(connection, "source_report", str(report_path))
        metadata(connection, "source_snapshot_id", args.snapshot_id)
        metadata(connection, "baseline_snapshot_id", args.baseline_snapshot_id or "")
        metadata(connection, "uncovered_region_count", str(len(items)))
        for item in items:
            connection.execute(
                """INSERT OR IGNORE INTO region_queue(
                    region_id, file_path, family, region_kind, function_names,
                    start_line, start_column, end_line, end_column, source_text,
                    source_context, difficulty, status, tries, reason, approach,
                    pinned_c_status, pinned_c_source, candidate_case_id,
                    first_snapshot_id, last_run_id, last_snapshot_id, created_at,
                    updated_at
                ) VALUES (
                    ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, ?, ?, ?,
                    ?, ?, NULL, NULL, ?, ?
                )""",
                [
                    item["region_id"], item["path"], item["family"], item["region_kind"],
                    item["function_names"], item["start_line"], item["start_column"],
                    item["end_line"], item["end_column"], item["source_text"] or "",
                    item["source_context"] or "", item["difficulty"], item["reason"],
                    item["approach"], item["pinned_c_status"], item["pinned_c_source"],
                    item["case_id"], args.snapshot_id, timestamp, timestamp,
                ],
            )
            connection.execute(
                "INSERT INTO queue_history(region_id, batch_id, try_no, event, snapshot_id, details, observed_at) VALUES (?, ?, 0, 'seeded', ?, ?, ?)",
                [item["region_id"], None, args.snapshot_id, "zero-hit region seeded from exact LLVM coordinate", timestamp],
            )
        families = ["c_abi", "ffi_handles", "wasm", "core_render_autohint", "tables_formats"]
        per_family = max(1, args.batch_size // len(families))
        selected: list[dict[str, Any]] = []
        for family in families:
            selected.extend([item for item in items if item["family"] == family][:per_family])
        if len(selected) < args.batch_size:
            chosen = {item["region_id"] for item in selected}
            selected.extend(item for item in items if item["region_id"] not in chosen)
            selected = selected[:args.batch_size]
        for ordinal, item in enumerate(selected, start=1):
            candidate_case_id = f"coverage.{args.batch_id}.target-{item['region_id']}"
            agent = f"strategy-{((ordinal - 1) % 5) + 1}"
            connection.execute(
                """INSERT INTO batch_plan(batch_id, ordinal, region_id, candidate_case_id, agent, family, reason, approach, status, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'strategy_pending', ?) ON CONFLICT(batch_id, ordinal) DO NOTHING""",
                [args.batch_id, ordinal, item["region_id"], candidate_case_id, agent, item["family"], item["reason"], item["approach"], timestamp],
            )
            connection.execute(
                "UPDATE region_queue SET candidate_case_id=COALESCE(candidate_case_id, ?), updated_at=? WHERE region_id=?",
                [candidate_case_id, timestamp, item["region_id"]],
            )
        connection.commit()
        counts = connection.execute("SELECT status, count(*) FROM region_queue GROUP BY status ORDER BY status").fetchall()
        print(json.dumps({"db": str(db_path), "snapshot_id": args.snapshot_id, "uncovered_regions": len(items), "batch_id": args.batch_id, "batch_rows": len(selected), "status_counts": counts}, default=str))
    finally:
        connection.close()


def covered_coordinates(data: dict[str, Any]) -> set[str]:
    return {
        "r-" + hashlib.sha256(
            f"{item['path']}|{item['start_line']}:{item['start_column']}-"
            f"{item['end_line']}:{item['end_column']}".encode("utf-8")
        ).hexdigest()[:24]
        for item in coordinate_states(data).values()
        if item["covered"]
    }


def reconcile(args: argparse.Namespace) -> None:
    duckdb = duckdb_module()
    observed_hits = set()
    if args.run_status != "failed":
        if not args.coverage_json:
            die("--coverage-json is required for a passed verification run")
        observed_hits = covered_coordinates(report_data(Path(args.coverage_json)))
    connection = duckdb.connect(str(args.db))
    try:
        schema(connection)
        selected = getattr(args, "case_id", None)
        if args.verification_kind == "complete" and selected:
            die("complete verification cannot select case IDs")
        if selected:
            known = {row[0] for row in connection.execute(
                "SELECT case_id FROM case_plan WHERE batch_id=?", [args.batch_id]
            ).fetchall()}
            if set(selected) - known:
                die(f"unknown cases for {args.batch_id}: {sorted(set(selected) - known)}")
        connection.execute("CREATE TEMP TABLE selected_cases(case_id VARCHAR PRIMARY KEY)")
        if selected:
            connection.executemany("INSERT INTO selected_cases VALUES (?)", [(item,) for item in sorted(set(selected))])
        timestamp = now_text()
        metadata(connection, f"verification.{args.run_id or args.snapshot_id}.kind", args.verification_kind)
        metadata(connection, f"verification.{args.run_id or args.snapshot_id}.snapshot_id", args.snapshot_id)
        rows = connection.execute(
            """WITH mapped AS (
                SELECT crp.region_id, list(DISTINCT cp.case_id ORDER BY cp.case_id) AS case_ids
                FROM case_region_plan crp
                JOIN case_plan cp ON cp.case_id=crp.case_id
                WHERE crp.batch_id=? AND cp.status IN ('planned', 'candidate')
                  AND (? OR cp.case_id IN (SELECT case_id FROM selected_cases))
                GROUP BY crp.region_id
            ), fallback AS (
                SELECT bp.region_id, [bp.candidate_case_id] AS case_ids
                FROM batch_plan bp
                WHERE bp.batch_id=? AND ? AND NOT EXISTS (
                    SELECT 1 FROM case_region_plan crp WHERE crp.batch_id=?
                )
            )
            SELECT q.region_id, q.tries, coalesce(m.case_ids, f.case_ids) AS case_ids, q.status
            FROM region_queue q
            LEFT JOIN mapped m ON m.region_id=q.region_id
            LEFT JOIN fallback f ON f.region_id=q.region_id
            WHERE m.region_id IS NOT NULL OR f.region_id IS NOT NULL
            ORDER BY q.family, q.start_line, q.start_column""",
            [args.batch_id, not selected, args.batch_id, not selected, args.batch_id],
        ).fetchall()
        for region_id, tries, case_ids, previous_status in rows:
            if args.run_status == "failed":
                status = "failed"
                next_tries = int(tries) + 1
                event = "failed"
                details = "Coverage MCP run failed before a verification snapshot was ingested"
            elif region_id in observed_hits:
                status = "done" if args.verification_kind == "complete" else "hit_pending_full"
                next_tries = int(tries)
                event = "coverage_crossed_incremental" if status != "done" else "coverage_crossed"
                details = "zero-hit source coordinate has a positive count in the supplied report"
            else:
                status = "pending"
                next_tries = int(tries) + 1
                event = "pending"
                details = "selected batch produced no positive count for this coordinate; retain for next strategy pass"
            if args.verification_kind == "incremental" and previous_status in {"done", "hit_pending_full"}:
                # A selected slice cannot invalidate an earlier witnessed hit.
                # Keep the failed/missed attempt in history without downgrading it.
                status = previous_status
            connection.execute(
                "UPDATE batch_plan SET status=?, tries=?, last_run_id=?, last_snapshot_id=?, updated_at=? WHERE batch_id=? AND region_id=?",
                [status, next_tries, args.run_id, args.snapshot_id, timestamp, args.batch_id, region_id],
            )
            connection.execute(
                "UPDATE region_queue SET status=?, tries=?, last_run_id=?, last_snapshot_id=?, updated_at=? WHERE region_id=?",
                [status, next_tries, args.run_id, args.snapshot_id, timestamp, region_id],
            )
            for case_id in case_ids:
                connection.execute(
                    "INSERT INTO queue_history(region_id, batch_id, try_no, event, case_id, run_id, snapshot_id, details, observed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    [region_id, args.batch_id, next_tries, event, case_id, args.run_id, args.snapshot_id,
                     details + "; attribution=batch; selected_cases=" + json.dumps(selected), timestamp],
                )
        connection.commit()
        summary = connection.execute(
            "SELECT status, count(*) FROM batch_plan WHERE batch_id=? GROUP BY status ORDER BY status",
            [args.batch_id],
        ).fetchall()
        print(json.dumps({"batch_id": args.batch_id, "verification_kind": args.verification_kind, "observed_hits": len(observed_hits), "status_counts": summary}, default=str))
    finally:
        connection.close()


def packet_value(case: dict[str, Any], *names: str, default: Any = "") -> Any:
    for name in names:
        if name in case and case[name] is not None:
            return case[name]
    return default


def target_ranges(packet: dict[str, Any], case: dict[str, Any]) -> list[tuple[str, int, int]]:
    raw_target = packet_value(case, "target", default="")
    packet_target_file = str(packet_value(packet, "target_file", default=""))
    if not packet_target_file and isinstance(packet.get("target"), str):
        packet_target_file = str(packet["target"])
    if isinstance(raw_target, dict):
        packet_target_file = str(raw_target.get("file", packet_target_file))
        target = f"{raw_target.get('function', '')}:{raw_target.get('lines', '')}"
    else:
        target = str(raw_target)
    target_file = packet_target_file
    # Most packets name a path before the line range.  WASM packets sometimes
    # name only a function, so use the packet-level target_file in that case.
    pattern = re.compile(r"((?:fontdone-c-abi|fontdone-wasm|src)/[^: ,;]+):(\d+)(?:-(\d+))?")
    ranges: list[tuple[str, int, int]] = []
    for match in pattern.finditer(target):
        path, start, end = match.group(1), int(match.group(2)), match.group(3)
        ranges.append((path, start, int(end) if end else start))
    line_matches = list(re.finditer(r"(?::|\b)(\d+)(?:-(\d+))?", target))
    if not ranges and line_matches and target_file:
        for match in line_matches:
            start, end = int(match.group(1)), match.group(2)
            ranges.append((target_file, start, int(end) if end else start))
    return ranges


def import_packets(args: argparse.Namespace) -> None:
    duckdb = duckdb_module()
    try:
        payload = json.loads(Path(args.packets).read_text() if args.packets else sys.stdin.read())
    except (OSError, json.JSONDecodeError) as exc:
        die(f"cannot read strategy packet JSON: {exc}")
    packets = payload if isinstance(payload, list) else [payload]
    connection = duckdb.connect(str(args.db))
    try:
        schema(connection)
        connection.execute("BEGIN TRANSACTION")
        queue_rows = connection.execute(
            "SELECT region_id, file_path, start_line, end_line FROM region_queue"
        ).fetchall()
        timestamp = now_text()
        imported = 0
        mapped = 0
        excluded = 0
        for packet_index, packet in enumerate(packets, start=1):
            if not isinstance(packet, dict):
                continue
            packet_target_file = str(packet.get("target_file", ""))
            if not packet_target_file and isinstance(packet.get("target"), str):
                packet_target_file = str(packet["target"])
            packet_agent = str(packet.get("worker", packet.get("agent", args.agent_prefix)))
            cases = packet.get("cases", [])
            if not isinstance(cases, list):
                continue
            for ordinal, case in enumerate(cases, start=1):
                if not isinstance(case, dict):
                    continue
                case_id = str(packet_value(case, "case_id", default="")).strip()
                if not case_id:
                    continue
                raw_target = packet_value(case, "target", default="")
                target = json.dumps(raw_target, sort_keys=True) if isinstance(raw_target, dict) else str(raw_target)
                entrypoint = str(packet_value(case, "public_entrypoint", "entrypoint", default=""))
                route = str(packet_value(case, "route", "fixture_route", "extend_route", default=""))
                input_spec = packet_value(case, "input", "input_dimensions", "dimensions", default="")
                reachable = str(packet_value(case, "reachable_because", "why_reachable", "reachability", default=""))
                pinned = str(packet_value(case, "pinned_c", "c_behavior", "c_reference", default=""))
                leverage = str(packet_value(case, "leverage", "expected_coverage_leverage", default=""))
                classification = str(packet_value(case, "classification", default="reachable"))
                execute = packet_value(case, "execute", default=True)
                status = "planned" if execute is not False and classification not in {"unreachable", "dead"} else "excluded"
                ranges = target_ranges(packet, case)
                matched: list[str] = []
                for path, start, end in ranges:
                    normalized = canonical_path(path)
                    for region_id, file_path, region_start, region_end in queue_rows:
                        if file_path != normalized:
                            continue
                        if region_start <= end and region_end >= start:
                            matched.append(region_id)
                matched = sorted(set(matched))
                explicit_ids = case.get("target_region_ids")
                if explicit_ids is not None:
                    if not isinstance(explicit_ids, list) or not all(isinstance(item, str) for item in explicit_ids):
                        die(f"{case_id}: target_region_ids must be a list of region IDs")
                    unknown = set(explicit_ids) - {row[0] for row in queue_rows}
                    if unknown:
                        die(f"{case_id}: unknown target region IDs: {sorted(unknown)}")
                    if ranges and not set(explicit_ids).issubset(matched):
                        die(f"{case_id}: target region IDs do not match the supplied source ranges")
                    matched = sorted(set(explicit_ids))
                if not matched and status == "planned":
                    status = "no_gap_or_unmapped"
                if status == "excluded":
                    excluded += 1
                imported += 1
                connection.execute(
                    """INSERT INTO case_plan(
                        case_id,batch_id,ordinal,agent,family,target_region_ids,target,
                        public_entrypoint,route,input_spec,reachable_because,pinned_c,
                        leverage,classification,status,created_at,updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(case_id) DO UPDATE SET
                        batch_id=excluded.batch_id, ordinal=excluded.ordinal,
                        agent=excluded.agent, family=excluded.family,
                        target_region_ids=excluded.target_region_ids, target=excluded.target,
                        public_entrypoint=excluded.public_entrypoint, route=excluded.route,
                        input_spec=excluded.input_spec, reachable_because=excluded.reachable_because,
                        pinned_c=excluded.pinned_c, leverage=excluded.leverage,
                        classification=excluded.classification, status=excluded.status,
                        updated_at=excluded.updated_at""",
                    [
                        case_id, args.batch_id, ordinal, packet_agent,
                        family_for(packet_target_file or (ranges[0][0] if ranges else "")),
                        ",".join(matched), target, entrypoint, route,
                        json.dumps(input_spec, sort_keys=True) if not isinstance(input_spec, str) else input_spec,
                        reachable, pinned, leverage, classification, status, timestamp, timestamp,
                    ],
                )
                connection.execute("DELETE FROM case_region_plan WHERE case_id=?", [case_id])
                connection.execute(
                    "UPDATE case_plan SET family_id=?,source_context=?,risks=?,stop_condition=? WHERE case_id=?",
                    [
                        str(case.get("family_id", case.get("family", ""))),
                        json.dumps(case.get("source_context", case.get("source_ranges", "")), sort_keys=True),
                        json.dumps(case.get("risks", case.get("risk", [])), sort_keys=True),
                        str(case.get("stop_condition", "")),
                        case_id,
                    ],
                )
                for region_id in matched:
                    mapped += 1
                    connection.execute(
                        "INSERT OR IGNORE INTO case_region_plan(case_id,batch_id,region_id,relation) VALUES (?, ?, ?, 'target')",
                        [case_id, args.batch_id, region_id],
                    )
                    connection.execute(
                        "UPDATE region_queue SET candidate_case_id=?, pinned_c_status=?, pinned_c_source=?, reason=?, updated_at=? WHERE region_id=?",
                        [case_id, "reviewed", pinned, reachable, timestamp, region_id],
                    )
                    connection.execute(
                        "INSERT INTO queue_history(region_id,batch_id,try_no,event,case_id,details,observed_at) VALUES (?, ?, 0, 'case_planned', ?, ?, ?)",
                        [region_id, args.batch_id, case_id, f"strategy packet {packet_index} mapped target; classification={classification}", timestamp],
                    )
                    connection.execute(
                        "UPDATE batch_plan SET candidate_case_id=?, agent=?, reason=?, status=?, updated_at=? WHERE batch_id=? AND region_id=?",
                        [case_id, packet_agent, reachable, status, timestamp, args.batch_id, region_id],
                    )
        metadata(connection, f"batch.{args.batch_id}.case_count", str(imported))
        metadata(connection, f"batch.{args.batch_id}.mapped_region_links", str(mapped))
        metadata(connection, f"batch.{args.batch_id}.excluded_case_count", str(excluded))
        connection.commit()
        print(json.dumps({"batch_id": args.batch_id, "case_count": imported, "mapped_region_links": mapped, "excluded_case_count": excluded}, default=str))
    finally:
        connection.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    seed_parser = subparsers.add_parser("seed", help="seed exact uncovered regions and the first strategy batch")
    seed_parser.add_argument("--coverage-json", required=True)
    seed_parser.add_argument("--db", required=True)
    seed_parser.add_argument("--snapshot-id", required=True)
    seed_parser.add_argument("--baseline-snapshot-id", default="")
    seed_parser.add_argument("--batch-id", required=True)
    seed_parser.add_argument("--batch-size", type=int, default=100)
    seed_parser.set_defaults(handler=seed)
    reconcile_parser = subparsers.add_parser("reconcile", help="record full/incremental verification for a planned batch")
    reconcile_parser.add_argument("--coverage-json")
    reconcile_parser.add_argument("--db", required=True)
    reconcile_parser.add_argument("--snapshot-id", required=True)
    reconcile_parser.add_argument("--run-id", default="")
    reconcile_parser.add_argument("--batch-id", required=True)
    reconcile_parser.add_argument("--verification-kind", choices=("incremental", "complete"), required=True)
    reconcile_parser.add_argument("--run-status", choices=("passed", "failed"), default="passed")
    reconcile_parser.add_argument("--case-id", action="append", help="exact executed case ID for incremental family/slice reconciliation (repeatable)")
    reconcile_parser.set_defaults(handler=reconcile)
    import_parser = subparsers.add_parser("import-packets", help="import read-only strategy packets into the case plan")
    import_parser.add_argument("--db", required=True)
    import_parser.add_argument("--batch-id", required=True)
    import_parser.add_argument("--packets", help="JSON file; omit to read packets from stdin")
    import_parser.add_argument("--agent-prefix", default="strategy")
    import_parser.set_defaults(handler=import_packets)
    args = parser.parse_args()
    args.handler(args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
