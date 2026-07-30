#!/usr/bin/env python3
"""Validate the maintained documentation map, links, commands, and snapshot."""

from __future__ import annotations

import csv
import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path
from urllib.parse import unquote

from run_runtime_parity import parity_source_digest

ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / "doc" / "README.md"
# Match each Markdown destination independently, including both the image and
# outer link in linked badges (`[![...](image)](destination)`).
MARKDOWN_LINK = re.compile(r"]\(([^)]+)\)")
MAKE_COMMAND = re.compile(r"(?<![\w-])make(?:\s+(?:-[A-Za-z]+\s+)*)\s+([A-Za-z0-9_.-]+)")
TARGET = re.compile(r"^([A-Za-z0-9_.-]+)\s*:", re.MULTILINE)
HISTORICAL_NOTICE = "> **Historical, non-authoritative:**"
PLAN_FIELDS = ("Owner:", "Open goals:", "Deletion condition:", "Evidence ledger")
LEGACY_MAC_HELPERS = {
    "FT_GetFilePath_From_Mac_ATS_Name",
    "FT_GetFile_From_Mac_ATS_Name",
    "FT_GetFile_From_Mac_Name",
    "FT_New_Face_From_FOND",
    "FT_New_Face_From_FSRef",
    "FT_New_Face_From_FSSpec",
}
PERFORMANCE_ARTIFACT_IDS = {
    "fontdone-rust-benchmark",
    "freetype-c-benchmark",
    "fontdone-c-abi-shared",
    "freetype-shared",
    "fontdone-wasm",
}
PERFORMANCE_THRESHOLD_KEYS = {
    "minimum_weighted_speedup_vs_c",
    "minimum_total_throughput_ratio_vs_c",
    "maximum_peak_rss_ratio_vs_c",
    "maximum_shared_library_size_ratio_vs_c",
    "maximum_wasm_bytes",
}
PERFORMANCE_ENVIRONMENT_KEYS = {
    "platform",
    "machine",
    "cpu_model",
    "runner_image",
    "rustc_version",
    "c_compiler_version",
}


def normalize_link(raw: str) -> str:
    raw = raw.strip()
    if raw.startswith("<"):
        end = raw.find(">")
        return raw[1:end] if end >= 0 else raw[1:]
    # Optional Markdown title follows the destination.
    return raw.split(maxsplit=1)[0]


def anchor_set(text: str) -> set[str]:
    anchors: set[str] = set()
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*$", text, re.MULTILINE):
        value = re.sub(r"[^\w\s-]", "", heading.lower(), flags=re.UNICODE)
        value = re.sub(r"[\s-]+", "-", value).strip("-")
        anchors.add(value)
    anchors.update(re.findall(r'<a\s+(?:name|id)="([^"]+)"', text, re.IGNORECASE))
    return anchors


def classified_documents() -> dict[str, str]:
    text = INDEX.read_text(encoding="utf-8")
    current: str | None = None
    result: dict[str, str] = {}
    heading_class = {
        "## Authoritative": "authoritative",
        "## Active plans": "active-plan",
        "## Historical": "historical",
        "## Generated": "generated",
    }
    for line in text.splitlines():
        row = re.match(
            r"^\|\s*(authoritative|active-plan|historical|generated)\s*\|",
            line,
        )
        if row is not None:
            lifecycle = row.group(1)
            for raw in MARKDOWN_LINK.findall(line):
                destination = normalize_link(raw).split("#", 1)[0]
                if not destination:
                    continue
                path = (INDEX.parent / unquote(destination)).resolve()
                try:
                    rel = path.relative_to(ROOT / "doc").as_posix()
                except ValueError:
                    continue
                if rel in result:
                    raise ValueError(f"doc/{rel} is classified more than once")
                result[rel] = lifecycle
            continue
        if line in heading_class:
            current = heading_class[line]
            continue
        if line.startswith("## "):
            current = None
        if current is None:
            continue
        for raw in MARKDOWN_LINK.findall(line):
            destination = normalize_link(raw).split("#", 1)[0]
            if not destination:
                continue
            path = (INDEX.parent / unquote(destination)).resolve()
            try:
                rel = path.relative_to(ROOT / "doc").as_posix()
            except ValueError:
                continue
            if rel in result:
                raise ValueError(f"doc/{rel} is classified more than once")
            result[rel] = current
    return result


def validate_index(errors: list[str]) -> dict[str, str]:
    try:
        classified = classified_documents()
    except ValueError as exc:
        errors.append(str(exc))
        return {}
    actual = {
        path.relative_to(ROOT / "doc").as_posix()
        for path in (ROOT / "doc").rglob("*")
        if path.is_file()
    }
    expected = set(classified)
    for missing in sorted(actual - expected):
        errors.append(f"doc/{missing}: not classified in doc/README.md")
    for stale in sorted(expected - actual):
        errors.append(f"doc/{stale}: classified but file does not exist")

    for rel, lifecycle in sorted(classified.items()):
        path = ROOT / "doc" / rel
        if not path.exists() or path.suffix.lower() != ".md":
            continue
        text = path.read_text(encoding="utf-8")
        if lifecycle == "historical" and HISTORICAL_NOTICE not in text[:700]:
            errors.append(f"doc/{rel}: historical file lacks visible notice")
        if lifecycle == "active-plan":
            for field in PLAN_FIELDS:
                if field not in text:
                    errors.append(f"doc/{rel}: active plan lacks {field}")
    return classified


def candidate_text_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z", "--", "*.md"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    paths: list[Path] = []
    for raw in result.stdout.split(b"\0"):
        if not raw:
            continue
        path = ROOT / raw.decode("utf-8")
        if path.exists():
            paths.append(path)
    return sorted(paths)


def validate_links(errors: list[str], files: list[Path]) -> None:
    for path in files:
        text = path.read_text(encoding="utf-8")
        for raw in MARKDOWN_LINK.findall(text):
            destination = normalize_link(raw)
            if (
                not destination
                or destination.startswith(("#", "http://", "https://", "mailto:"))
            ):
                continue
            file_part, _, fragment = destination.partition("#")
            target = (path.parent / unquote(file_part)).resolve()
            if not target.exists():
                errors.append(
                    f"{path.relative_to(ROOT)}: broken link {destination!r}"
                )
                continue
            if fragment and target.is_file() and target.suffix.lower() == ".md":
                target_text = target.read_text(encoding="utf-8")
                if unquote(fragment).lower() not in anchor_set(target_text):
                    errors.append(
                        f"{path.relative_to(ROOT)}: missing anchor {destination!r}"
                    )


def validate_make_commands(
    errors: list[str], files: list[Path], classified: dict[str, str]
) -> None:
    targets = set(TARGET.findall((ROOT / "Makefile").read_text(encoding="utf-8")))
    for path in files:
        try:
            rel_doc = path.relative_to(ROOT / "doc").as_posix()
        except ValueError:
            rel_doc = None
        if rel_doc and classified.get(rel_doc) == "historical":
            continue
        text = path.read_text(encoding="utf-8")
        for target in sorted(set(MAKE_COMMAND.findall(text))):
            if target not in targets:
                errors.append(
                    f"{path.relative_to(ROOT)}: references missing Make target {target!r}"
                )


def validate_performance_snapshot(
    snapshot: dict[str, object],
    readme: str,
    errors: list[str],
) -> None:
    performance = snapshot.get("performance")
    if not isinstance(performance, dict):
        errors.append(
            "doc/compatibility_snapshot.json: performance ledger is missing"
        )
        return
    matrix_path = ROOT / "tests" / "data" / "perf_operation_matrix.json"
    matrix_bytes = matrix_path.read_bytes()
    matrix_sha256 = hashlib.sha256(matrix_bytes).hexdigest()
    matrix = json.loads(matrix_bytes)
    policy = matrix.get("regression_policy", {})
    if set(policy.get("environment_identity", [])) != PERFORMANCE_ENVIRONMENT_KEYS:
        errors.append(
            "tests/data/perf_operation_matrix.json: performance environment "
            "identity fields are incomplete"
        )
    expected_fields = {
        "schema_version": 1,
        "command": "make bench BENCH_SAMPLES=10 BENCH_PROFILE=default",
        "record_command": "make record-performance-baseline",
        "matrix": "tests/data/perf_operation_matrix.json",
        "matrix_version": matrix.get("version"),
        "matrix_sha256": matrix_sha256,
        "required_workload_profile": policy.get("required_workload_profile"),
        "minimum_samples_per_run": policy.get("minimum_samples_per_run"),
        "minimum_clean_runs_per_environment": policy.get(
            "minimum_clean_runs_per_environment"
        ),
        "regression_status": policy.get("status"),
        "thresholds": policy.get("thresholds"),
    }
    for key, expected in expected_fields.items():
        if performance.get(key) != expected:
            errors.append(
                "doc/compatibility_snapshot.json: performance."
                f"{key} is {performance.get(key)!r}, expected {expected!r}"
            )

    minimum_samples = policy.get("minimum_samples_per_run")
    minimum_runs = policy.get("minimum_clean_runs_per_environment")
    profile = policy.get("required_workload_profile")
    matrix_row_ids = {row["id"] for row in matrix.get("rows", [])}
    runs = performance.get("clean_runs")
    if not isinstance(runs, list):
        errors.append(
            "doc/compatibility_snapshot.json: performance.clean_runs is not a list"
        )
        return
    report_hashes = set()
    current_runs = []
    environment_run_counts: dict[str, int] = {}
    for index, run in enumerate(runs):
        prefix = (
            "doc/compatibility_snapshot.json: "
            f"performance.clean_runs[{index}]"
        )
        if not isinstance(run, dict):
            errors.append(f"{prefix} is not an object")
            continue
        report_hash = str(run.get("report_sha256", ""))
        if re.fullmatch(r"[0-9a-f]{64}", report_hash) is None:
            errors.append(f"{prefix}.report_sha256 is invalid")
        elif report_hash in report_hashes:
            errors.append(f"{prefix}.report_sha256 is duplicated")
        report_hashes.add(report_hash)
        if re.fullmatch(r"[0-9a-f]{40}", str(run.get("source_commit", ""))) is None:
            errors.append(f"{prefix}.source_commit is invalid")
        if run.get("clean_source") is not True:
            errors.append(f"{prefix} was not measured from a clean source")
        if (
            not isinstance(run.get("sample_count"), int)
            or not isinstance(minimum_samples, int)
            or run["sample_count"] < minimum_samples
        ):
            errors.append(f"{prefix}.sample_count is below the policy minimum")
        environment_id = str(run.get("environment_id", ""))
        if re.fullmatch(r"[0-9a-f]{64}", environment_id) is None:
            errors.append(f"{prefix}.environment_id is invalid")
        environment = run.get("environment")
        if not isinstance(environment, dict):
            errors.append(f"{prefix}.environment is missing")
        else:
            if set(environment) != PERFORMANCE_ENVIRONMENT_KEYS:
                errors.append(f"{prefix}.environment fields are incomplete")
            computed_id = hashlib.sha256(
                json.dumps(
                    environment,
                    sort_keys=True,
                    separators=(",", ":"),
                ).encode()
            ).hexdigest()
            if environment_id != computed_id:
                errors.append(f"{prefix}.environment_id does not match its fields")
            for field, value in environment.items():
                if field != "runner_image" and value in (None, ""):
                    errors.append(f"{prefix}.environment.{field} is missing")

        for section, keys in (
            (
                "latency",
                (
                    "rust_ns_per_iter_mean",
                    "rust_ns_per_iter_median",
                    "rust_ns_per_iter_p90",
                    "rust_ns_per_iter_p99",
                    "c_ns_per_iter_mean",
                    "c_ns_per_iter_median",
                    "c_ns_per_iter_p90",
                    "c_ns_per_iter_p99",
                    "weighted_speedup_vs_c",
                ),
            ),
            (
                "throughput",
                (
                    "operation_count",
                    "rust_operations_per_second",
                    "c_operations_per_second",
                    "ratio_vs_c",
                ),
            ),
            (
                "peak_process_memory",
                (
                    "rust_peak_rss_bytes_median",
                    "c_peak_rss_bytes_median",
                    "rust_to_c_peak_rss_ratio",
                ),
            ),
        ):
            values = run.get(section)
            if not isinstance(values, dict):
                errors.append(f"{prefix}.{section} is missing")
                continue
            for key in keys:
                value = values.get(key)
                if (
                    isinstance(value, bool)
                    or not isinstance(value, (int, float))
                    or value <= 0
                ):
                    errors.append(f"{prefix}.{section}.{key} is not positive")

        binary_size = run.get("binary_size")
        if not isinstance(binary_size, dict):
            errors.append(f"{prefix}.binary_size is missing")
        else:
            artifacts = binary_size.get("artifacts")
            artifact_ids = (
                {artifact.get("id") for artifact in artifacts}
                if isinstance(artifacts, list)
                and all(isinstance(artifact, dict) for artifact in artifacts)
                else set()
            )
            if (
                artifact_ids != PERFORMANCE_ARTIFACT_IDS
                or len(artifacts or []) != len(PERFORMANCE_ARTIFACT_IDS)
            ):
                errors.append(f"{prefix}.binary_size artifact set is incomplete")
            else:
                for artifact in artifacts:
                    if (
                        not isinstance(artifact.get("bytes"), int)
                        or artifact["bytes"] <= 0
                        or re.fullmatch(
                            r"[0-9a-f]{64}",
                            str(artifact.get("sha256", "")),
                        )
                        is None
                    ):
                        errors.append(
                            f"{prefix}.binary_size artifact evidence is invalid"
                        )

        per_operation = run.get("per_operation")
        operation_ids = (
            {row.get("id") for row in per_operation}
            if isinstance(per_operation, list)
            and all(isinstance(row, dict) for row in per_operation)
            else set()
        )
        if operation_ids != matrix_row_ids or len(per_operation or []) != len(
            matrix_row_ids
        ):
            errors.append(f"{prefix}.per_operation does not match the matrix")
        regression = run.get("regression")
        observations = (
            regression.get("observations")
            if isinstance(regression, dict)
            else None
        )
        if not isinstance(observations, dict) or set(observations) != (
            PERFORMANCE_THRESHOLD_KEYS
        ):
            errors.append(f"{prefix}.regression observations are incomplete")

        if (
            run.get("matrix_sha256") == matrix_sha256
            and run.get("matrix_version") == matrix.get("version")
            and run.get("workload_profile") == profile
        ):
            current_runs.append(run)
            environment_run_counts[environment_id] = (
                environment_run_counts.get(environment_id, 0) + 1
            )

    if performance.get("current_matrix_clean_run_count") != len(current_runs):
        errors.append(
            "doc/compatibility_snapshot.json: performance current-run count "
            "does not match clean_runs"
        )
    if performance.get("environment_run_counts") != environment_run_counts:
        errors.append(
            "doc/compatibility_snapshot.json: performance environment counts "
            "do not match clean_runs"
        )
    ready = (
        isinstance(minimum_runs, int)
        and any(count >= minimum_runs for count in environment_run_counts.values())
    )
    if performance.get("ready_for_threshold_review") != ready:
        errors.append(
            "doc/compatibility_snapshot.json: performance threshold-review "
            "readiness is stale"
        )
    qualifying_count = max(environment_run_counts.values(), default=0)
    expected_readme = f"**{qualifying_count} / {minimum_runs} clean runs**"
    if expected_readme not in readme:
        errors.append(
            f"README.md: missing performance baseline count {expected_readme!r}"
        )
    roadmap = (ROOT / "doc" / "ROADMAP.md").read_text(encoding="utf-8")
    expected_roadmap = (
        f"**{qualifying_count} / {minimum_runs} qualifying clean runs**"
    )
    if expected_roadmap not in roadmap:
        errors.append(
            "doc/ROADMAP.md: missing performance baseline count "
            f"{expected_roadmap!r}"
        )


def validate_snapshot(errors: list[str]) -> None:
    snapshot = json.loads(
        (ROOT / "doc" / "compatibility_snapshot.json").read_text(encoding="utf-8")
    )
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    if snapshot.get("schema_version") != 3:
        errors.append("doc/compatibility_snapshot.json: expected schema_version 3")
        return
    if snapshot.get("snapshot_kind") != "committed_measured_evidence":
        errors.append(
            "doc/compatibility_snapshot.json: unexpected snapshot_kind"
        )
    if f"recorded on **{snapshot['snapshot_date']}**" not in readme:
        errors.append(
            "README.md: compatibility snapshot date does not match "
            "doc/compatibility_snapshot.json"
        )

    api = snapshot["public_function_adoption"]
    expected_rows = {
        "Complete": api["complete"],
        "Implemented, mapping incomplete": api["implemented_mapping_incomplete"],
        "Partial": api["partial"],
        "Planned": api["planned"],
        "Intentionally excluded": api["intentionally_excluded"],
        "**Total**": api["total"],
    }
    for label, count in expected_rows.items():
        pattern = rf"\|\s*{re.escape(label)}\s*\|\s*\**{count:,}\**\s*\|"
        if re.search(pattern, readme) is None:
            errors.append(f"README.md: snapshot row {label!r} is not {count:,}")

    interface_map = json.loads(
        (ROOT / "tests" / "data" / "interface_map.json").read_text(encoding="utf-8")
    )
    statuses: dict[str, str] = {}
    for group in interface_map["paths"]:
        for symbol, contract in group["symbols"].items():
            if symbol in LEGACY_MAC_HELPERS:
                continue
            status = contract["status"]
            previous = statuses.setdefault(symbol, status)
            if previous != status:
                errors.append(
                    "tests/data/interface_map.json: conflicting status for "
                    f"{symbol}: {previous!r} and {status!r}"
                )
    status_counts = Counter(statuses.values())
    source_keys = {
        "complete": "complete",
        "implemented_mapping_incomplete": "implemented",
        "partial": "partial",
        "planned": "planned",
        "intentionally_excluded": "out_of_scope",
    }
    for snapshot_key, source_key in source_keys.items():
        if api[snapshot_key] != status_counts[source_key]:
            errors.append(
                "doc/compatibility_snapshot.json: "
                f"public_function_adoption.{snapshot_key} is {api[snapshot_key]}, "
                f"interface map has {status_counts[source_key]}"
            )
    if api["total"] != len(statuses):
        errors.append(
            "doc/compatibility_snapshot.json: public function total is "
            f"{api['total']}, interface map has {len(statuses)}"
        )

    runtime = snapshot["runtime_parity"]
    evidence_contract = snapshot.get("runtime_evidence")
    if not isinstance(evidence_contract, dict):
        errors.append(
            "doc/compatibility_snapshot.json: runtime_evidence object is missing"
        )
        evidence_contract = {}
    evidence_path = ROOT / str(
        evidence_contract.get("path", "doc/runtime_parity_evidence.json")
    )
    if not evidence_path.is_file():
        errors.append(
            f"{evidence_path.relative_to(ROOT)}: committed runtime evidence is missing"
        )
    else:
        evidence_bytes = evidence_path.read_bytes()
        evidence_sha256 = hashlib.sha256(evidence_bytes).hexdigest()
        if evidence_contract.get("sha256") != evidence_sha256:
            errors.append(
                "doc/compatibility_snapshot.json: runtime evidence SHA-256 does "
                f"not match {evidence_path.relative_to(ROOT)}"
            )
        evidence = json.loads(evidence_bytes)
        if evidence.get("schema_version") != 1:
            errors.append(
                f"{evidence_path.relative_to(ROOT)}: expected schema_version 1"
            )
        if (
            evidence.get("test_exit_code") != 0
            or evidence.get("complete") is not True
        ):
            errors.append(
                f"{evidence_path.relative_to(ROOT)}: committed run is not complete"
            )
        if evidence.get("runtime_parity") != runtime:
            errors.append(
                "doc/compatibility_snapshot.json: runtime_parity does not exactly "
                f"match {evidence_path.relative_to(ROOT)}"
            )
        evidence_source = evidence.get("source", {})
        current_tree, current_path_count = parity_source_digest()
        if evidence_source.get("parity_tree_sha256") != current_tree:
            errors.append(
                f"{evidence_path.relative_to(ROOT)}: parity source digest is stale; "
                "run make record-parity-snapshot"
            )
        if evidence_source.get("parity_path_count") != current_path_count:
            errors.append(
                f"{evidence_path.relative_to(ROOT)}: parity source path count is "
                f"{evidence_source.get('parity_path_count')}, current count is "
                f"{current_path_count}"
            )
        for contract_key, evidence_value in (
            ("parity_tree_sha256", evidence_source.get("parity_tree_sha256")),
            ("log_sha256", evidence.get("artifacts", {}).get("log_sha256")),
            ("command", evidence.get("command")),
        ):
            if evidence_contract.get(contract_key) != evidence_value:
                errors.append(
                    "doc/compatibility_snapshot.json: runtime_evidence."
                    f"{contract_key} does not match committed evidence"
                )

        current_report_path = (
            ROOT / "target" / "parity-evidence" / "runtime_parity.json"
        )
        if current_report_path.is_file():
            current_report = json.loads(
                current_report_path.read_text(encoding="utf-8")
            )
            current_source = current_report.get("source", {})
            if current_source.get("parity_tree_sha256") == current_tree:
                if current_report.get("runtime_parity") != runtime:
                    errors.append(
                        "doc/compatibility_snapshot.json: runtime snapshot differs "
                        "from the current full parity report"
                    )
                if current_report.get("complete") is not True:
                    errors.append(
                        f"{current_report_path.relative_to(ROOT)}: current report "
                        "is not a complete passing run"
                    )

    for label, key in (
        ("Runnable exact-comparison cases", "runnable"),
        ("Passed cases", "passed"),
        ("Failed cases", "failed"),
        ("Explicitly pending cases", "pending"),
        ("Covered manifest cases", "covered_manifest"),
        ("Validated public API subjects", "validated_input_subjects"),
        ("Validated public API input files", "validated_input_files"),
        ("Logical declared cases", "logical_input_cases"),
        ("Concrete expanded cases", "concrete_input_cases"),
    ):
        count = runtime[key]
        if re.search(rf"\|\s*{re.escape(label)}\s*\|\s*{count:,}\s*\|", readme) is None:
            errors.append(f"README.md: runtime snapshot {label!r} is not {count:,}")
    if runtime["passed"] != runtime["runnable"] or runtime["failed"] != 0:
        errors.append(
            "doc/compatibility_snapshot.json: committed runtime evidence must "
            "represent a fully passing runnable matrix"
        )
    route_text = (
        f"{runtime['function_route_evidence']:,} / {runtime['function_total']:,}"
    )
    if route_text not in readme:
        errors.append(f"README.md: function route evidence is not {route_text}")

    audit_path = ROOT / "target" / "api-abi-audit" / "api_abi_audit.json"
    if audit_path.exists():
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
        for key, count in snapshot["public_inventory"].items():
            current = audit["counts"].get(key)
            if current != count:
                errors.append(
                    "doc/compatibility_snapshot.json: "
                    f"public_inventory.{key} is {count}, current audit has {current}"
                )

    contract = snapshot["c_contract"]
    contract_path = (
        ROOT / "target" / "api-abi-audit" / "c_abi_contract_status.json"
    )
    if contract_path.exists():
        scorecard = json.loads(contract_path.read_text(encoding="utf-8"))
        for snapshot_key, scorecard_key in (
            ("categories_complete", "categories_complete"),
            ("categories_total", "categories_total"),
            ("is_complete", "is_complete"),
        ):
            if contract[snapshot_key] != scorecard[scorecard_key]:
                errors.append(
                    "doc/compatibility_snapshot.json: "
                    f"c_contract.{snapshot_key} is {contract[snapshot_key]!r}, "
                    f"current scorecard has {scorecard[scorecard_key]!r}"
                )
        metrics = {
            metric["id"]: metric
            for category in scorecard["categories"]
            for metric in category["metrics"]
        }
        for metric_id, complete_key, total_key in (
            (
                "C01.7",
                "runtime_contract_rows_complete",
                "runtime_contract_rows_total",
            ),
            (
                "C11.3",
                "binary_artifact_items_complete",
                "binary_artifact_items_total",
            ),
            ("C12.3", "platform_lanes_complete", "platform_lanes_total"),
        ):
            metric = metrics.get(metric_id)
            if metric is None:
                errors.append(
                    f"{contract_path.relative_to(ROOT)}: missing metric {metric_id}"
                )
                continue
            if (
                contract[complete_key] != metric["complete"]
                or contract[total_key] != metric["total"]
            ):
                errors.append(
                    "doc/compatibility_snapshot.json: "
                    f"{metric_id} is {contract[complete_key]}/"
                    f"{contract[total_key]}, current scorecard has "
                    f"{metric['complete']}/{metric['total']}"
                )
    if (
        contract["runtime_contract_rows_pending"]
        != contract["runtime_contract_rows_total"]
        - contract["runtime_contract_rows_complete"]
    ):
        errors.append(
            "doc/compatibility_snapshot.json: C01.7 pending count does "
            "not equal total minus complete"
        )

    coverage = snapshot.get("coverage")
    if not isinstance(coverage, dict) or coverage.get("measured") is not True:
        errors.append(
            "doc/compatibility_snapshot.json: measured coverage snapshot is missing"
        )
    else:
        if coverage.get("command") != "make test-coverage-all":
            errors.append(
                "doc/compatibility_snapshot.json: coverage command must be "
                "make test-coverage-all"
            )
        if coverage.get("exit_code") != 0:
            errors.append(
                "doc/compatibility_snapshot.json: coverage run did not pass"
            )
        source_commit = str(coverage.get("source_commit", ""))
        if re.fullmatch(r"[0-9a-f]{40}", source_commit) is None:
            errors.append(
                "doc/compatibility_snapshot.json: coverage source_commit is "
                "not a full Git SHA"
            )
        coverage_runtime = coverage.get("runtime_parity", {})
        for key in ("runnable", "passed", "failed", "pending"):
            if coverage_runtime.get(key) != runtime[key]:
                errors.append(
                    "doc/compatibility_snapshot.json: coverage runtime_parity."
                    f"{key} does not match committed runtime evidence"
                )
        for label, key in (
            ("Lines", "lines"),
            ("Branches", "branches"),
            ("Functions", "functions"),
            ("Regions", "regions"),
        ):
            metric = coverage.get(key, {})
            covered = metric.get("covered")
            total = metric.get("total")
            percent = metric.get("percent")
            if (
                not isinstance(covered, int)
                or not isinstance(total, int)
                or not isinstance(percent, (int, float))
                or covered < 0
                or total <= 0
                or covered > total
            ):
                errors.append(
                    "doc/compatibility_snapshot.json: invalid coverage metric "
                    f"{key}"
                )
                continue
            computed = covered * 100.0 / total
            if abs(float(percent) - computed) > 1e-10:
                errors.append(
                    "doc/compatibility_snapshot.json: coverage percentage does "
                    f"not match {key} numerator and denominator"
                )
            expected = (
                rf"\|\s*{label}\s*\|\s*{covered:,}\s*/\s*{total:,}\s*\|\s*"
                rf"{computed:.2f}%\s*\|"
            )
            if re.search(expected, readme) is None:
                errors.append(
                    f"README.md: measured coverage row {label!r} is stale"
                )

    validate_performance_snapshot(snapshot, readme, errors)

    for expected in (
        f"**{contract['categories_complete']} / "
        f"{contract['categories_total']} categories complete**",
        f"{contract['binary_artifact_items_complete']} / "
        f"{contract['binary_artifact_items_total']}",
        f"{contract['platform_lanes_complete']} / "
        f"{contract['platform_lanes_total']}",
        f"{contract['runtime_contract_rows_complete']:,} / "
        f"{contract['runtime_contract_rows_total']:,}",
        f"{contract['runtime_contract_rows_pending']:,} pending",
    ):
        if expected not in readme:
            errors.append(f"README.md: missing C-contract snapshot {expected!r}")

    cargo_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    version = re.search(r'^version\s*=\s*"([^"]+)"', cargo_text, re.MULTILINE)
    if version is None or snapshot["package_version"] != version.group(1):
        errors.append(
            "doc/compatibility_snapshot.json: package_version does not match "
            "Cargo.toml"
        )


def validate_retention_summary(errors: list[str]) -> None:
    inventory = ROOT / "doc" / "FILE_RETENTION_INVENTORY.tsv"
    with inventory.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source, delimiter="\t"))
    counts = Counter(row["reason_code"] for row in rows)
    development = (ROOT / "doc" / "DEVELOPMENT.md").read_text(encoding="utf-8")
    for reason_code, count in sorted(counts.items()):
        pattern = rf"\|\s*{re.escape(reason_code)}\s*\|\s*{count:,}\s*\|"
        if re.search(pattern, development) is None:
            errors.append(
                "doc/DEVELOPMENT.md: retention summary row "
                f"{reason_code!r} is not {count:,}"
            )
    if (
        re.search(
            rf"\|\s*\*\*Total\*\*\s*\|\s*\*\*{len(rows):,}\*\*\s*\|",
            development,
        )
        is None
    ):
        errors.append(
            f"doc/DEVELOPMENT.md: retention total is not {len(rows):,}"
        )


def validate_repository_facts(errors: list[str]) -> None:
    result = subprocess.run(
        ["git", "ls-files", "-z", "--", "tests/fixtures/input"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    fixture_paths = [
        ROOT / raw.decode("utf-8")
        for raw in result.stdout.split(b"\0")
        if raw and (ROOT / raw.decode("utf-8")).exists()
    ]
    symlinks = sum(path.is_symlink() for path in fixture_paths)
    development = (ROOT / "doc" / "DEVELOPMENT.md").read_text(encoding="utf-8")
    if f"contains {len(fixture_paths):,} tracked paths" not in development:
        errors.append(
            "doc/DEVELOPMENT.md: canonical fixture path count is not "
            f"{len(fixture_paths):,}"
        )
    expected_symlink_text = (
        "no symlinks" if symlinks == 0 else f"{symlinks:,} symlinks"
    )
    if expected_symlink_text not in development:
        errors.append(
            "doc/DEVELOPMENT.md: canonical fixture symlink count is not "
            f"{symlinks:,}"
        )

    makefile = (ROOT / "Makefile").read_text(encoding="utf-8")
    fixture_targets = {
        target
        for target in TARGET.findall(makefile)
        if target.startswith("font-fixture-")
    }
    font_targets = fixture_targets - {"font-fixture-compressed"}
    expected = (
        f"exposes {len(font_targets):,} named font-generation targets plus "
        "the deterministic\ncompressed-payload target"
    )
    if expected not in development:
        errors.append(
            "doc/DEVELOPMENT.md: font-generation target count is not "
            f"{len(font_targets):,} plus compressed"
        )


def validate_authoritative_reachability(
    errors: list[str], classified: dict[str, str]
) -> None:
    root_text = (ROOT / "README.md").read_text(encoding="utf-8")
    index_text = INDEX.read_text(encoding="utf-8")
    root_links = {
        normalize_link(raw).split("#", 1)[0] for raw in MARKDOWN_LINK.findall(root_text)
    }
    if not any(
        link == "doc/README.md" or link.rstrip("/").endswith("/doc/README.md")
        for link in root_links
    ):
        errors.append("README.md: documentation index is not linked")
    index_links = {
        normalize_link(raw).split("#", 1)[0] for raw in MARKDOWN_LINK.findall(index_text)
    }
    for rel, lifecycle in sorted(classified.items()):
        if lifecycle == "authoritative" and rel != "README.md" and rel not in index_links:
            errors.append(
                f"doc/{rel}: authoritative guide is not reachable through doc/README.md"
            )


def validate_current_names(errors: list[str], files: list[Path], classified: dict[str, str]) -> None:
    former_parent_name = "pillow" + "-rs-freetype"
    former_c_directory = "`c" + "-abi/"
    former_ffi_directory = "`ffi" + "-c/"
    former_wasm_directory = "`wa" + "sm/"
    former_binding_name = "rust-" + "freetype"
    former_reference_project = "Ser" + "vo"
    removed_goal_doc = "PROJECT_" + "GOALS.md"
    forbidden = (
        former_parent_name,
        former_c_directory,
        former_ffi_directory,
        former_wasm_directory,
        former_binding_name,
        former_reference_project,
        removed_goal_doc,
    )
    for path in files:
        try:
            rel_doc = path.relative_to(ROOT / "doc").as_posix()
        except ValueError:
            rel_doc = None
        if rel_doc and classified.get(rel_doc) == "historical":
            continue
        text = path.read_text(encoding="utf-8")
        for value in forbidden:
            if value in text:
                errors.append(
                    f"{path.relative_to(ROOT)}: current text contains former name {value!r}"
                )


def main() -> int:
    errors: list[str] = []
    classified = validate_index(errors)
    files = candidate_text_files()
    validate_links(errors, files)
    validate_make_commands(errors, files, classified)
    validate_snapshot(errors)
    validate_retention_summary(errors)
    validate_repository_facts(errors)
    validate_authoritative_reachability(errors, classified)
    validate_current_names(errors, files, classified)
    if errors:
        for error in errors:
            print(f"documentation error: {error}", file=sys.stderr)
        return 1
    print(
        "documentation: "
        f"{len(classified)} doc files classified, "
        f"{len(files)} Markdown files checked, links, commands, snapshot, "
        "and repository facts clean"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
