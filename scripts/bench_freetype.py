#!/usr/bin/env python3
"""Run fontdone operation benchmarks.

The Rust benchmark path is always available and emits JSONL rows.  The C
FreeType comparison path is optional and uses scripts/bench_ft_ops.c as a
standalone helper; it is never linked into the Rust runtime crate.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import pathlib
import platform
import re
import shutil
import subprocess
import sys
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_MATRIX = ROOT / "tests" / "data" / "perf_operation_matrix.json"
DEFAULT_OUT = ROOT / "target" / "fontdone-bench" / "latest.json"
DEFAULT_REPORT = ROOT / "target" / "fontdone-bench" / "latest.md"
COMPATIBILITY_SNAPSHOT = ROOT / "doc" / "compatibility_snapshot.json"
README = ROOT / "README.md"
ROADMAP = ROOT / "doc" / "ROADMAP.md"
PERFORMANCE_README_PATTERN = re.compile(
    r"<!-- performance-baseline:start -->.*?<!-- performance-baseline:end -->",
    re.DOTALL,
)
PERFORMANCE_ROADMAP_PATTERN = re.compile(
    r"<!-- performance-roadmap:start -->.*?<!-- performance-roadmap:end -->",
    re.DOTALL,
)
HELPER_SRC = ROOT / "scripts" / "bench_ft_ops.c"
HELPER_BIN = ROOT / "target" / "fontdone-bench" / "bench_ft_ops"
CARGO_MANIFEST = ROOT / "Cargo.toml"
RUST_BENCH_BIN = (
    ROOT
    / "target"
    / "release"
    / "examples"
    / ("bench_ops.exe" if os.name == "nt" else "bench_ops")
)
WASM_TARGET = "wasm32-unknown-unknown"
MEMORY_MARKER = "__FONTDONE_MAX_RSS_KIB__="
MEMORY_WRAPPER = f"""
import resource
import subprocess
import sys

completed = subprocess.run(
    sys.argv[1:],
    check=False,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)
sys.stdout.buffer.write(completed.stdout)
sys.stderr.buffer.write(completed.stderr)
peak_rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
print({MEMORY_MARKER!r} + str(peak_rss), file=sys.stderr)
raise SystemExit(completed.returncode)
"""
REQUIRED_REGRESSION_THRESHOLDS = (
    "minimum_weighted_speedup_vs_c",
    "minimum_total_throughput_ratio_vs_c",
    "maximum_peak_rss_ratio_vs_c",
    "maximum_shared_library_size_ratio_vs_c",
    "maximum_wasm_bytes",
)
REQUIRED_ARTIFACT_IDS = (
    "fontdone-rust-benchmark",
    "freetype-c-benchmark",
    "fontdone-c-abi-shared",
    "freetype-shared",
    "fontdone-wasm",
)
REQUIRED_ENVIRONMENT_IDENTITY = (
    "platform",
    "machine",
    "cpu_model",
    "runner_image",
    "rustc_version",
    "c_compiler_version",
)


def run(cmd: list[str], *, cwd: pathlib.Path = ROOT, env: dict[str, str] | None = None) -> str:
    proc = subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return proc.stdout


def run_measured(
    cmd: list[str],
    *,
    cwd: pathlib.Path = ROOT,
    env: dict[str, str] | None = None,
) -> tuple[str, dict[str, Any]]:
    """Run one benchmark process and return stdout plus exact peak-RSS evidence."""

    system = platform.system()
    if system == "Linux":
        scale = 1024
        source = "Python resource.getrusage(RUSAGE_CHILDREN).ru_maxrss (KiB)"
    elif system == "Darwin":
        scale = 1
        source = "Python resource.getrusage(RUSAGE_CHILDREN).ru_maxrss (bytes)"
    else:
        raise RuntimeError(
            f"peak RSS measurement is not implemented for {system or 'this platform'}"
        )

    proc = subprocess.run(
        [sys.executable, "-c", MEMORY_WRAPPER, *cmd],
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        raise subprocess.CalledProcessError(
            proc.returncode,
            cmd,
            output=proc.stdout,
            stderr=proc.stderr,
        )
    match = re.search(
        rf"^{re.escape(MEMORY_MARKER)}(\d+)$",
        proc.stderr,
        re.MULTILINE,
    )
    peak_rss_bytes = int(match.group(1)) * scale if match else None
    if peak_rss_bytes is None:
        raise RuntimeError(f"could not parse peak RSS from {source}")
    return (
        proc.stdout,
        {
            "command": cmd,
            "peak_rss_bytes": peak_rss_bytes,
            "measurement_source": source,
        },
    )


def run_optional(cmd: list[str], *, cwd: pathlib.Path = ROOT) -> str | None:
    try:
        return run(cmd, cwd=cwd).strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None


def repo_root() -> pathlib.Path:
    root = run_optional(["git", "rev-parse", "--show-toplevel"], cwd=ROOT)
    return pathlib.Path(root) if root else ROOT


REPO_ROOT = repo_root()


def build_rust_benchmark() -> pathlib.Path:
    run(
        [
            "cargo",
            "build",
            "--manifest-path",
            str(CARGO_MANIFEST),
            "--example",
            "bench_ops",
            "--release",
            "--locked",
        ],
        cwd=ROOT,
    )
    if not RUST_BENCH_BIN.is_file():
        raise RuntimeError(f"Rust benchmark executable is missing: {RUST_BENCH_BIN}")
    return RUST_BENCH_BIN


def run_rust(
    matrix: pathlib.Path,
    binary: pathlib.Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    stdout, measurement = run_measured([str(binary), str(matrix)], cwd=ROOT)
    return (
        [json.loads(line) for line in stdout.splitlines() if line.strip()],
        measurement,
    )


def compile_c_helper(include_dir: pathlib.Path, lib_dir: pathlib.Path) -> pathlib.Path:
    HELPER_BIN.parent.mkdir(parents=True, exist_ok=True)
    compiler = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if compiler is None:
        raise RuntimeError("no C compiler found")
    command = [
        compiler,
        "-O3",
        "-std=c11",
        f"-I{include_dir}",
        str(HELPER_SRC),
        f"-L{lib_dir}",
        f"-Wl,-rpath,{lib_dir.resolve()}",
        "-lfreetype",
        "-o",
        str(HELPER_BIN),
    ]
    run(command)
    return HELPER_BIN


def run_c(
    matrix: pathlib.Path,
    helper: pathlib.Path,
    lib_dir: pathlib.Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    env = os.environ.copy()
    loader_variable = "DYLD_LIBRARY_PATH" if sys.platform == "darwin" else "LD_LIBRARY_PATH"
    old_ld = env.get(loader_variable)
    env[loader_variable] = (
        str(lib_dir) if not old_ld else f"{lib_dir}{os.pathsep}{old_ld}"
    )
    stdout, measurement = run_measured([str(helper), str(matrix)], env=env)
    return (
        [json.loads(line) for line in stdout.splitlines() if line.strip()],
        measurement,
    )


def load_matrix(matrix: pathlib.Path) -> dict[str, Any]:
    return json.loads(matrix.read_text())


def load_weights(matrix_data: dict[str, Any], profile: str) -> dict[str, float]:
    profiles = matrix_data.get("workload_profiles", {})
    if profile in profiles:
        weights = profiles[profile].get("weights", {})
        row_ids = {row["id"] for row in matrix_data.get("rows", [])}
        weight_ids = set(weights)
        missing = sorted(row_ids - weight_ids)
        unknown = sorted(weight_ids - row_ids)
        if missing or unknown:
            raise ValueError(
                f"workload profile {profile!r} does not exactly match matrix rows; "
                f"missing={missing}, unknown={unknown}"
            )
        parsed = {row_id: float(weight) for row_id, weight in weights.items()}
        invalid = sorted(row_id for row_id, weight in parsed.items() if weight <= 0.0)
        if invalid:
            raise ValueError(
                f"workload profile {profile!r} has non-positive weights: {invalid}"
            )
        return parsed
    if profile != "row_weight":
        available = ", ".join(sorted([*profiles.keys(), "row_weight"]))
        raise ValueError(f"unknown workload profile {profile!r}; available: {available}")
    return {
        row["id"]: float(row.get("weight", 1.0))
        for row in matrix_data.get("rows", [])
    }


def matrix_rows_by_id(matrix_data: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {row["id"]: row for row in matrix_data.get("rows", [])}


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def artifact_record(
    artifact_id: str,
    implementation: str,
    kind: str,
    path: pathlib.Path,
) -> dict[str, Any]:
    resolved = path.resolve()
    if not resolved.is_file():
        raise RuntimeError(f"benchmark artifact is missing: {path}")
    return {
        "id": artifact_id,
        "implementation": implementation,
        "kind": kind,
        "path": str(path.relative_to(ROOT)),
        "resolved_path": str(resolved),
        "bytes": resolved.stat().st_size,
        "sha256": sha256_file(resolved),
    }


def find_freetype_shared(lib_dir: pathlib.Path) -> pathlib.Path:
    patterns = (
        "libfreetype.so*",
        "libfreetype*.dylib",
        "freetype*.dll",
    )
    candidates: dict[pathlib.Path, pathlib.Path] = {}
    for pattern in patterns:
        for path in lib_dir.glob(pattern):
            if path.is_file():
                candidates.setdefault(path.resolve(), path)
    if not candidates:
        raise RuntimeError(f"FreeType shared library is missing under {lib_dir}")
    if len(candidates) != 1:
        names = ", ".join(str(path) for path in sorted(candidates))
        raise RuntimeError(f"ambiguous FreeType shared libraries: {names}")
    return next(iter(candidates.values()))


def fontdone_shared_path() -> pathlib.Path:
    if sys.platform == "darwin":
        name = "libfontdone_c_abi.dylib"
    elif os.name == "nt":
        name = "fontdone_c_abi.dll"
    else:
        name = "libfontdone_c_abi.so"
    return ROOT / "target" / "release" / name


def build_and_measure_artifacts(
    rust_binary: pathlib.Path,
    c_helper: pathlib.Path,
    freetype_lib_dir: pathlib.Path,
) -> dict[str, Any]:
    run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "fontdone-c-abi",
            "--locked",
        ]
    )
    run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "fontdone-wasm",
            "--target",
            WASM_TARGET,
            "--locked",
        ]
    )
    records = [
        artifact_record(
            "fontdone-rust-benchmark",
            "fontdone",
            "workload-executable",
            rust_binary,
        ),
        artifact_record(
            "freetype-c-benchmark",
            "freetype",
            "workload-executable",
            c_helper,
        ),
        artifact_record(
            "fontdone-c-abi-shared",
            "fontdone",
            "shared-library",
            fontdone_shared_path(),
        ),
        artifact_record(
            "freetype-shared",
            "freetype",
            "shared-library",
            find_freetype_shared(freetype_lib_dir),
        ),
        artifact_record(
            "fontdone-wasm",
            "fontdone",
            "webassembly-module",
            ROOT
            / "target"
            / WASM_TARGET
            / "release"
            / "fontdone_wasm.wasm",
        ),
    ]
    by_id = {row["id"]: row for row in records}
    shared_ratio = (
        by_id["fontdone-c-abi-shared"]["bytes"]
        / by_id["freetype-shared"]["bytes"]
    )
    workload_ratio = (
        by_id["fontdone-rust-benchmark"]["bytes"]
        / by_id["freetype-c-benchmark"]["bytes"]
    )
    return {
        "measurement": "exact unstripped release-build file bytes",
        "artifacts": records,
        "summary": {
            "fontdone_to_freetype_shared_library_size_ratio": shared_ratio,
            "rust_to_c_workload_executable_size_ratio": workload_ratio,
            "fontdone_wasm_bytes": by_id["fontdone-wasm"]["bytes"],
        },
    }


def summarize_process_memory(samples: list[dict[str, Any]]) -> dict[str, Any]:
    rust_values = [
        int(sample["rust"]["peak_rss_bytes"])
        for sample in samples
        if sample.get("rust", {}).get("peak_rss_bytes") is not None
    ]
    c_values = [
        int(sample["c"]["peak_rss_bytes"])
        for sample in samples
        if sample.get("c", {}).get("peak_rss_bytes") is not None
    ]
    rust_median = median([float(value) for value in rust_values])
    c_median = median([float(value) for value in c_values])
    return {
        "boundary": "complete direct benchmark process for the full operation matrix",
        "samples": samples,
        "summary": {
            "rust_peak_rss_bytes_min": min(rust_values) if rust_values else None,
            "rust_peak_rss_bytes_median": int(rust_median) if rust_values else None,
            "rust_peak_rss_bytes_max": max(rust_values) if rust_values else None,
            "c_peak_rss_bytes_min": min(c_values) if c_values else None,
            "c_peak_rss_bytes_median": int(c_median) if c_values else None,
            "c_peak_rss_bytes_max": max(c_values) if c_values else None,
            "rust_to_c_peak_rss_ratio": (
                rust_median / c_median if rust_median and c_median else None
            ),
        },
    }


def ops_per_second(ns_per_iter: float) -> float:
    return 1_000_000_000.0 / ns_per_iter if ns_per_iter > 0.0 else 0.0


def evaluate_regression_policy(
    matrix_data: dict[str, Any],
    summary: dict[str, Any],
    process_memory: dict[str, Any],
    artifact_sizes: dict[str, Any],
) -> dict[str, Any]:
    policy = matrix_data.get("regression_policy")
    if not isinstance(policy, dict):
        return {
            "status": "missing",
            "complete": False,
            "passed": False,
            "debt": ["perf_operation_matrix.json has no regression_policy object"],
            "checks": [],
        }

    status = str(policy.get("status", "missing"))
    thresholds = policy.get("thresholds")
    missing = []
    if status != "active":
        missing.append(
            f"regression policy status is {status!r}; collect and review the required clean baselines"
        )
    if not isinstance(thresholds, dict):
        missing.append("regression policy thresholds are not defined")
        thresholds = {}
    missing.extend(
        f"missing threshold {key}"
        for key in REQUIRED_REGRESSION_THRESHOLDS
        if key not in thresholds
    )

    memory_summary = process_memory["summary"]
    artifact_summary = artifact_sizes["summary"]
    observations = {
        "minimum_weighted_speedup_vs_c": summary["overall"][
            "weighted_speedup_vs_c"
        ],
        "minimum_total_throughput_ratio_vs_c": summary["overall"][
            "throughput_ratio_vs_c"
        ],
        "maximum_peak_rss_ratio_vs_c": memory_summary[
            "rust_to_c_peak_rss_ratio"
        ],
        "maximum_shared_library_size_ratio_vs_c": artifact_summary[
            "fontdone_to_freetype_shared_library_size_ratio"
        ],
        "maximum_wasm_bytes": artifact_summary["fontdone_wasm_bytes"],
    }
    checks = []
    comparators = {
        "minimum_weighted_speedup_vs_c": ">=",
        "minimum_total_throughput_ratio_vs_c": ">=",
        "maximum_peak_rss_ratio_vs_c": "<=",
        "maximum_shared_library_size_ratio_vs_c": "<=",
        "maximum_wasm_bytes": "<=",
    }
    for key in REQUIRED_REGRESSION_THRESHOLDS:
        if key not in thresholds:
            continue
        observed = observations[key]
        threshold = thresholds[key]
        if observed is None or not isinstance(threshold, (int, float)):
            passed = False
        elif comparators[key] == ">=":
            passed = float(observed) >= float(threshold)
        else:
            passed = float(observed) <= float(threshold)
        checks.append(
            {
                "metric": key,
                "comparator": comparators[key],
                "threshold": threshold,
                "observed": observed,
                "passed": passed,
            }
        )
    complete = not missing and len(checks) == len(REQUIRED_REGRESSION_THRESHOLDS)
    return {
        "status": status,
        "minimum_clean_runs_per_environment": policy.get(
            "minimum_clean_runs_per_environment"
        ),
        "complete": complete,
        "passed": complete and all(check["passed"] for check in checks),
        "debt": missing,
        "observations": observations,
        "checks": checks,
    }


def merge_rows(
    rust_rows: list[dict[str, Any]], c_rows: list[dict[str, Any]] | None
) -> list[dict[str, Any]]:
    c_by_id = {row["id"]: row for row in c_rows or []}
    merged = []
    for rust in rust_rows:
        row = dict(rust)
        c_row = c_by_id.get(row["id"])
        if c_row is not None:
            row["c_ns_total"] = c_row["c_ns_total"]
            row["c_ns_per_iter"] = c_row["c_ns_per_iter"]
            row["c_output_fingerprint"] = c_row.get("output_fingerprint")
            if c_row.get("output_sha256") and row.get("output_sha256") == c_row.get("output_sha256"):
                row["output_match"] = True
            elif c_row.get("output_sha256"):
                row["output_match"] = False
                row["c_output_sha256"] = c_row.get("output_sha256")
            if row["c_ns_per_iter"]:
                row["ratio_rust_to_c"] = row["rust_ns_per_iter"] / row["c_ns_per_iter"]
        merged.append(row)
    return merged


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (pct / 100.0) * (len(ordered) - 1)
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    fraction = rank - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * fraction


def mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0


def weighted_mean(values: list[float], weights: list[float]) -> float:
    if len(values) != len(weights):
        raise ValueError("weighted mean values and weights must have equal lengths")
    total_weight = sum(weights)
    if not values or total_weight == 0.0:
        return 0.0
    return sum(value * weight for value, weight in zip(values, weights)) / total_weight


def weighted_percentile(values: list[float], weights: list[float], pct: float) -> float:
    if len(values) != len(weights):
        raise ValueError("weighted percentile values and weights must have equal lengths")
    if not values:
        return 0.0
    pairs = sorted(zip(values, weights), key=lambda item: item[0])
    total_weight = sum(weight for _, weight in pairs)
    if total_weight == 0.0:
        return 0.0
    threshold = total_weight * pct / 100.0
    cumulative = 0.0
    for value, weight in pairs:
        cumulative += weight
        if cumulative >= threshold:
            return value
    return pairs[-1][0]


def median(values: list[float]) -> float:
    return percentile(values, 50)


def trimmed_mean(values: list[float], trim_fraction: float = 0.1) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    trim = int(len(ordered) * trim_fraction)
    if trim == 0 or trim * 2 >= len(ordered):
        return mean(ordered)
    return mean(ordered[trim:-trim])


def stddev(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    avg = mean(values)
    variance = sum((value - avg) ** 2 for value in values) / (len(values) - 1)
    return variance ** 0.5


def summarize_rows(
    sample_rows: list[list[dict[str, Any]]],
    weights: dict[str, float],
    matrix_by_id: dict[str, dict[str, Any]] | None = None,
) -> dict[str, Any]:
    by_id: dict[str, list[dict[str, Any]]] = {}
    order: list[str] = []
    for sample in sample_rows:
        for row in sample:
            row_id = row["id"]
            if row_id not in by_id:
                order.append(row_id)
                by_id[row_id] = []
            by_id[row_id].append(row)

    summary_rows: list[dict[str, Any]] = []
    rust_total_ns = 0.0
    c_total_ns = 0.0
    operation_count = 0
    weighted_rust_total = 0.0
    weighted_c_total = 0.0
    weighted_total = 0.0
    all_rust_per_iter = []
    all_c_per_iter = []
    all_speedups = []
    all_sample_weights = []
    groups: dict[str, dict[str, Any]] = {}

    for row_id in order:
        samples = by_id[row_id]
        first = samples[0]
        matrix_row = (matrix_by_id or {}).get(row_id, {})
        timing_category = timing_category_for_row(first, matrix_row)
        rust_per_iter = [float(row["rust_ns_per_iter"]) for row in samples]
        c_per_iter = [
            float(row["c_ns_per_iter"])
            for row in samples
            if row.get("c_ns_per_iter") is not None
        ]
        rust_throughput = [ops_per_second(value) for value in rust_per_iter]
        c_throughput = [ops_per_second(value) for value in c_per_iter]
        speedups = [
            float(row["c_ns_per_iter"]) / float(row["rust_ns_per_iter"])
            for row in samples
            if row.get("c_ns_per_iter") and row.get("rust_ns_per_iter")
        ]
        c_output_has_sha = any(row.get("c_output_sha256") for row in samples)
        output_match_checked = any(row.get("output_match") is not None for row in samples)
        rust_total = sum(float(row["rust_ns_total"]) for row in samples)
        c_total = sum(float(row.get("c_ns_total", 0)) for row in samples)
        iterations = int(first["iterations"])
        total_iterations = iterations * len(samples)
        weight = weights.get(row_id, 1.0)
        sample_weights = [float(iterations)] * len(samples)

        all_rust_per_iter.extend(rust_per_iter)
        all_c_per_iter.extend(c_per_iter)
        all_speedups.extend(speedups)
        all_sample_weights.extend(sample_weights)

        group = groups.setdefault(
            timing_category,
            {
                "operation_count": 0,
                "rust_ns_total": 0.0,
                "c_ns_total": 0.0,
                "weighted_operation_weight": 0.0,
                "weighted_rust_total": 0.0,
                "weighted_c_total": 0.0,
                "rust_per_iter": [],
                "c_per_iter": [],
                "speedups": [],
                "sample_weights": [],
            },
        )
        group["operation_count"] += total_iterations
        group["rust_ns_total"] += rust_total
        group["c_ns_total"] += c_total
        group["weighted_operation_weight"] += weight
        group["weighted_rust_total"] += weight * mean(rust_per_iter)
        if c_per_iter:
            group["weighted_c_total"] += weight * mean(c_per_iter)
        group["rust_per_iter"].extend(rust_per_iter)
        group["c_per_iter"].extend(c_per_iter)
        group["speedups"].extend(speedups)
        group["sample_weights"].extend(sample_weights)

        rust_total_ns += rust_total
        c_total_ns += c_total
        operation_count += total_iterations
        weighted_rust_total += weight * mean(rust_per_iter)
        if c_per_iter:
            weighted_c_total += weight * mean(c_per_iter)
        weighted_total += weight

        summary_rows.append(
            {
                "id": row_id,
                "operation": first["operation"],
                "timing_category": timing_category,
                "comparison_trust": matrix_row.get("comparison_trust", "unspecified"),
                "timing_boundary": matrix_row.get("timing_boundary", ""),
                "output_match_checked": output_match_checked,
                "c_output_has_sha256": c_output_has_sha,
                "iterations_per_sample": iterations,
                "sample_count": len(samples),
                "operation_count": total_iterations,
                "weight": weight,
                "rust_ns_per_iter_min": min(rust_per_iter),
                "rust_ns_per_iter_max": max(rust_per_iter),
                "rust_ns_per_iter_mean": mean(rust_per_iter),
                "rust_ns_per_iter_median": median(rust_per_iter),
                "rust_ns_per_iter_trimmed_mean": trimmed_mean(rust_per_iter),
                "rust_ns_per_iter_stddev": stddev(rust_per_iter),
                "rust_ns_per_iter_p90": percentile(rust_per_iter, 90),
                "rust_ns_per_iter_p99": percentile(rust_per_iter, 99),
                "rust_operations_per_second_mean": mean(rust_throughput),
                "rust_operations_per_second_median": median(rust_throughput),
                "rust_operations_per_second_p10": percentile(rust_throughput, 10),
                "rust_operations_per_second_p90": percentile(rust_throughput, 90),
                "c_ns_per_iter_min": min(c_per_iter) if c_per_iter else 0.0,
                "c_ns_per_iter_max": max(c_per_iter) if c_per_iter else 0.0,
                "c_ns_per_iter_mean": mean(c_per_iter),
                "c_ns_per_iter_median": median(c_per_iter),
                "c_ns_per_iter_trimmed_mean": trimmed_mean(c_per_iter),
                "c_ns_per_iter_stddev": stddev(c_per_iter),
                "c_ns_per_iter_p90": percentile(c_per_iter, 90),
                "c_ns_per_iter_p99": percentile(c_per_iter, 99),
                "c_operations_per_second_mean": mean(c_throughput),
                "c_operations_per_second_median": median(c_throughput),
                "c_operations_per_second_p10": percentile(c_throughput, 10),
                "c_operations_per_second_p90": percentile(c_throughput, 90),
                "speedup_vs_c_min": min(speedups) if speedups else 0.0,
                "speedup_vs_c_max": max(speedups) if speedups else 0.0,
                "speedup_vs_c_mean": mean(speedups),
                "speedup_vs_c_median": median(speedups),
                "speedup_vs_c_trimmed_mean": trimmed_mean(speedups),
                "speedup_vs_c_stddev": stddev(speedups),
                "speedup_vs_c_p90": percentile(speedups, 90),
                "speedup_vs_c_p99": percentile(speedups, 99),
                "rust_ns_total": int(rust_total),
                "c_ns_total": int(c_total),
            }
        )

    overall_speedup = c_total_ns / rust_total_ns if rust_total_ns else 0.0
    weighted_speedup = (
        weighted_c_total / weighted_rust_total if weighted_rust_total and weighted_c_total else 0.0
    )
    return {
        "rows": summary_rows,
        "overall": {
            "operation_count": operation_count,
            "rust_ns_total": int(rust_total_ns),
            "c_ns_total": int(c_total_ns),
            "speedup_vs_c_total": overall_speedup,
            "rust_operations_per_second": (
                operation_count * 1_000_000_000.0 / rust_total_ns
                if rust_total_ns
                else 0.0
            ),
            "c_operations_per_second": (
                operation_count * 1_000_000_000.0 / c_total_ns
                if c_total_ns
                else 0.0
            ),
            "throughput_ratio_vs_c": overall_speedup,
            "weighted_operation_weight": weighted_total,
            "weighted_speedup_vs_c": weighted_speedup,
            **distribution_stats(all_rust_per_iter, all_c_per_iter, all_speedups, all_sample_weights),
        },
        "groups": summarize_groups(groups),
    }


def timing_category_for_row(row: dict[str, Any], matrix_row: dict[str, Any]) -> str:
    boundary = matrix_row.get("timing_boundary", "")
    if row.get("operation") == "load_font" or "construct" in boundary and "timed loop" in boundary:
        return "font_load_path_dependent"
    return "cached_font_operation"


def distribution_stats(
    rust_per_iter: list[float],
    c_per_iter: list[float],
    speedups: list[float],
    sample_weights: list[float],
) -> dict[str, float]:
    speedup_weights = sample_weights[: len(speedups)]
    return {
        "rust_ns_per_iter_mean": weighted_mean(rust_per_iter, sample_weights),
        "rust_ns_per_iter_median": weighted_percentile(rust_per_iter, sample_weights, 50),
        "rust_ns_per_iter_p90": weighted_percentile(rust_per_iter, sample_weights, 90),
        "rust_ns_per_iter_p99": weighted_percentile(rust_per_iter, sample_weights, 99),
        "c_ns_per_iter_mean": weighted_mean(c_per_iter, sample_weights[: len(c_per_iter)]),
        "c_ns_per_iter_median": weighted_percentile(c_per_iter, sample_weights[: len(c_per_iter)], 50),
        "c_ns_per_iter_p90": weighted_percentile(c_per_iter, sample_weights[: len(c_per_iter)], 90),
        "c_ns_per_iter_p99": weighted_percentile(c_per_iter, sample_weights[: len(c_per_iter)], 99),
        "speedup_vs_c_mean": weighted_mean(speedups, speedup_weights),
        "speedup_vs_c_median": weighted_percentile(speedups, speedup_weights, 50),
        "speedup_vs_c_p90": weighted_percentile(speedups, speedup_weights, 90),
        "speedup_vs_c_p99": weighted_percentile(speedups, speedup_weights, 99),
    }


def summarize_groups(groups: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    labels = {
        "cached_font_operation": "Cached font operations",
        "font_load_path_dependent": "Font load / path-dependent setup",
    }
    rows = []
    for category, group in sorted(groups.items()):
        rust_total = float(group["rust_ns_total"])
        c_total = float(group["c_ns_total"])
        weighted_rust = float(group["weighted_rust_total"])
        weighted_c = float(group["weighted_c_total"])
        rows.append(
            {
                "category": category,
                "label": labels.get(category, category),
                "operation_count": group["operation_count"],
                "rust_ns_total": int(rust_total),
                "c_ns_total": int(c_total),
                "speedup_vs_c_total": c_total / rust_total if rust_total else 0.0,
                "rust_operations_per_second": (
                    group["operation_count"] * 1_000_000_000.0 / rust_total
                    if rust_total
                    else 0.0
                ),
                "c_operations_per_second": (
                    group["operation_count"] * 1_000_000_000.0 / c_total
                    if c_total
                    else 0.0
                ),
                "throughput_ratio_vs_c": (
                    c_total / rust_total if rust_total else 0.0
                ),
                "weighted_operation_weight": group["weighted_operation_weight"],
                "weighted_speedup_vs_c": weighted_c / weighted_rust if weighted_rust and weighted_c else 0.0,
                **distribution_stats(
                    group["rust_per_iter"],
                    group["c_per_iter"],
                    group["speedups"],
                    group["sample_weights"],
                ),
            }
        )
    return rows


def read_cpu_model() -> str | None:
    cpuinfo = pathlib.Path("/proc/cpuinfo")
    if not cpuinfo.exists():
        return platform.processor() or None
    for line in cpuinfo.read_text(errors="ignore").splitlines():
        if line.startswith("model name"):
            return line.split(":", 1)[1].strip()
    return platform.processor() or None


def read_cpu_governor() -> str | None:
    governors = sorted(pathlib.Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_governor"))
    values = []
    for governor in governors[:8]:
        try:
            values.append(governor.read_text().strip())
        except OSError:
            continue
    return ",".join(sorted(set(values))) if values else None


def read_khz_file(path: pathlib.Path) -> int | None:
    try:
        return int(path.read_text().strip())
    except (OSError, ValueError):
        return None


def format_mhz(khz: float | int | None) -> str | None:
    if khz is None:
        return None
    return f"{float(khz) / 1000.0:.0f} MHz"


def read_cpu_frequencies() -> dict[str, Any]:
    policies = sorted(pathlib.Path("/sys/devices/system/cpu/cpufreq").glob("policy*"))
    current = []
    maximum = []
    for policy in policies:
        cur = read_khz_file(policy / "scaling_cur_freq")
        max_freq = read_khz_file(policy / "cpuinfo_max_freq")
        if cur is not None:
            current.append(cur)
        if max_freq is not None:
            maximum.append(max_freq)
    if not current and not maximum:
        return {}
    return {
        "current_min_mhz": format_mhz(min(current)) if current else None,
        "current_max_mhz": format_mhz(max(current)) if current else None,
        "current_mean_mhz": format_mhz(mean([float(value) for value in current])) if current else None,
        "cpuinfo_max_mhz": format_mhz(max(maximum)) if maximum else None,
        "policy_count": len(policies),
    }


def read_memory_info() -> dict[str, Any]:
    meminfo = pathlib.Path("/proc/meminfo")
    info: dict[str, Any] = {
        "total": None,
        "available": None,
        "speed": None,
        "clock": None,
        "source": "/proc/meminfo; speed/clock not exposed",
    }
    if meminfo.exists():
        values = {}
        for line in meminfo.read_text(errors="ignore").splitlines():
            key, _, rest = line.partition(":")
            values[key] = rest.strip()
        info["total"] = values.get("MemTotal")
        info["available"] = values.get("MemAvailable")

    # Desktop/server DIMM speed is usually exposed through SMBIOS/EDAC only
    # with elevated privileges or platform-specific drivers. Keep explicit
    # unknowns rather than manufacturing a number.
    for candidate in (
        pathlib.Path("/sys/class/dmi/id/product_version"),
        pathlib.Path("/sys/class/dmi/id/board_name"),
    ):
        try:
            value = candidate.read_text(errors="ignore").strip()
        except OSError:
            continue
        if value:
            info.setdefault("platform_hint", value)
            break
    return info


def build_metadata(args: argparse.Namespace, matrix_data: dict[str, Any]) -> dict[str, Any]:
    cc = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    return {
        "schema_version": 3,
        "created_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "git_sha": run_optional(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT),
        "git_dirty": bool(run_optional(["git", "status", "--short"], cwd=REPO_ROOT)),
        "repository_root": str(REPO_ROOT),
        "crate_root": str(ROOT),
        "matrix": str(args.matrix),
        "matrix_version": matrix_data.get("version"),
        "workload_profile": args.profile,
        "sample_count": args.samples,
        "cached_row_warmup_iterations": 1,
        "compare_c": args.compare_c,
        "rustc_version": run_optional(["rustc", "--version"], cwd=ROOT),
        "cargo_version": run_optional(["cargo", "--version"], cwd=ROOT),
        "python_version": sys.version.split()[0],
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu_model": read_cpu_model(),
        "cpu_governor": read_cpu_governor(),
        "cpu_frequency": read_cpu_frequencies(),
        "memory": read_memory_info(),
        "c_compiler": cc,
        "c_compiler_version": run_optional([cc, "--version"], cwd=ROOT).splitlines()[0] if cc else None,
        "ci": {
            "github_actions": os.environ.get("GITHUB_ACTIONS") == "true",
            "github_run_id": os.environ.get("GITHUB_RUN_ID"),
            "github_run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
            "github_sha": os.environ.get("GITHUB_SHA"),
            "runner_image": os.environ.get("ImageOS"),
            "runner_arch": os.environ.get("RUNNER_ARCH"),
            "runner_name": os.environ.get("RUNNER_NAME"),
        },
        "ft_include": str(args.ft_include),
        "ft_lib": str(args.ft_lib),
        "timing_notes": [
            "Rust and C workload executables are built before measurement and executed directly.",
            "C helper is standalone tooling compiled by this script and never linked into runtime code.",
            "Cached-font rows run one untimed warmup operation in both Rust and C before timing.",
            "Operation latency/throughput uses in-process timers; peak RSS covers the complete direct process.",
            "Artifact size is the exact unstripped release-build file byte count.",
            "Rows marked timing_only have C timing/fingerprint but not exact comparable C SHA-256 output parity.",
            "Exact correctness remains enforced by fixture parity tests.",
        ],
    }


def format_table(summary: dict[str, Any], *, include_aggregate: bool = True) -> str:
    headers = [
        "id",
        "op",
        "count",
        "weight",
        "trust",
        "rust total ms",
        "c total ms",
        "rust median ns",
        "rust mean ns",
        "rust stddev",
        "rust p90 ns",
        "rust p99 ns",
        "c median ns",
        "c mean ns",
        "c stddev",
        "c p90 ns",
        "c p99 ns",
        "rust median ops/s",
        "c median ops/s",
        "median speedup vs C",
        "mean speedup vs C",
        "stddev speedup",
        "p90 speedup",
        "p99 speedup",
    ]
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join(["---"] * len(headers)) + " |",
    ]
    for row in summary["rows"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    row["id"],
                    row["operation"],
                    str(row["operation_count"]),
                    f"{row['weight']:.2f}",
                    row["comparison_trust"],
                    f"{row['rust_ns_total'] / 1_000_000.0:.3f}",
                    f"{row['c_ns_total'] / 1_000_000.0:.3f}",
                    f"{row['rust_ns_per_iter_median']:.1f}",
                    f"{row['rust_ns_per_iter_mean']:.1f}",
                    f"{row['rust_ns_per_iter_stddev']:.1f}",
                    f"{row['rust_ns_per_iter_p90']:.1f}",
                    f"{row['rust_ns_per_iter_p99']:.1f}",
                    f"{row['c_ns_per_iter_median']:.1f}",
                    f"{row['c_ns_per_iter_mean']:.1f}",
                    f"{row['c_ns_per_iter_stddev']:.1f}",
                    f"{row['c_ns_per_iter_p90']:.1f}",
                    f"{row['c_ns_per_iter_p99']:.1f}",
                    f"{row['rust_operations_per_second_median']:.1f}",
                    f"{row['c_operations_per_second_median']:.1f}",
                    f"{row['speedup_vs_c_median']:.3f}x",
                    f"{row['speedup_vs_c_mean']:.3f}x",
                    f"{row['speedup_vs_c_stddev']:.3f}x",
                    f"{row['speedup_vs_c_p90']:.3f}x",
                    f"{row['speedup_vs_c_p99']:.3f}x",
                ]
            )
            + " |"
        )
    if include_aggregate:
        overall = summary["overall"]
        lines.extend(
            [
                "",
                "| aggregate | value |",
                "| --- | --- |",
                f"| total operation count | {overall['operation_count']} |",
                f"| rust total ns | {overall['rust_ns_total']} |",
                f"| c total ns | {overall['c_ns_total']} |",
                f"| total speedup vs C | {overall['speedup_vs_c_total']:.3f}x |",
                f"| rust operations/s | {overall['rust_operations_per_second']:.3f} |",
                f"| c operations/s | {overall['c_operations_per_second']:.3f} |",
                f"| throughput ratio vs C | {overall['throughput_ratio_vs_c']:.3f}x |",
                f"| weighted operation weight | {overall['weighted_operation_weight']:.2f} |",
                f"| weighted speedup vs C | {overall['weighted_speedup_vs_c']:.3f}x |",
            ]
        )
    return "\n".join(lines)


def format_ms(ns: int | float) -> str:
    return f"{float(ns) / 1_000_000.0:.3f} ms"


def aggregate_table(summary: dict[str, Any]) -> str:
    overall = summary["overall"]
    rows = [
        ("Total operation count", overall["operation_count"]),
        ("Rust total time", format_ms(overall["rust_ns_total"])),
        ("C total time", format_ms(overall["c_ns_total"])),
        ("Total speedup vs C", f"{overall['speedup_vs_c_total']:.3f}x"),
        (
            "Rust aggregate throughput",
            f"{overall['rust_operations_per_second']:.3f} operations/s",
        ),
        (
            "C aggregate throughput",
            f"{overall['c_operations_per_second']:.3f} operations/s",
        ),
        (
            "Throughput ratio vs C",
            f"{overall['throughput_ratio_vs_c']:.3f}x",
        ),
        ("Weighted operation weight", f"{overall['weighted_operation_weight']:.2f}"),
        ("Weighted speedup vs C", f"{overall['weighted_speedup_vs_c']:.3f}x"),
    ]
    lines = ["| Metric | Value |", "| --- | --- |"]
    for key, value in rows:
        lines.append(f"| {key} | {value} |")
    return "\n".join(lines)


def group_summary_table(summary: dict[str, Any]) -> str:
    lines = [
        "| Group | count | rust total ms | c total ms | rust ops/s | c ops/s | throughput ratio vs C | weighted speedup vs C |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for group in summary.get("groups", []):
        lines.append(
            f"| {group['label']} | {group['operation_count']} | "
            f"{group['rust_ns_total'] / 1_000_000.0:.3f} | "
            f"{group['c_ns_total'] / 1_000_000.0:.3f} | "
            f"{group['rust_operations_per_second']:.3f} | "
            f"{group['c_operations_per_second']:.3f} | "
            f"{group['throughput_ratio_vs_c']:.3f}x | "
            f"{group['weighted_speedup_vs_c']:.3f}x |"
        )
    return "\n".join(lines)


def overall_distribution_table(summary: dict[str, Any]) -> str:
    overall = summary["overall"]
    rows = [
        (
            "Rust ns/iter",
            overall["rust_ns_per_iter_mean"],
            overall["rust_ns_per_iter_median"],
            overall["rust_ns_per_iter_p90"],
            overall["rust_ns_per_iter_p99"],
        ),
        (
            "C ns/iter",
            overall["c_ns_per_iter_mean"],
            overall["c_ns_per_iter_median"],
            overall["c_ns_per_iter_p90"],
            overall["c_ns_per_iter_p99"],
        ),
        (
            "Per-row speedup vs C",
            overall["speedup_vs_c_mean"],
            overall["speedup_vs_c_median"],
            overall["speedup_vs_c_p90"],
            overall["speedup_vs_c_p99"],
        ),
    ]
    lines = ["| Distribution | mean | median | p90 | p99 |", "| --- | --- | --- | --- | --- |"]
    for label, avg, med, p90, p99 in rows:
        suffix = "x" if "speedup" in label else " ns"
        lines.append(
            f"| {label} | {avg:.3f}{suffix} | {med:.3f}{suffix} | "
            f"{p90:.3f}{suffix} | {p99:.3f}{suffix} |"
        )
    return "\n".join(lines)


def group_distribution_table(group: dict[str, Any]) -> str:
    rows = [
        (
            "Rust ns/iter",
            group["rust_ns_per_iter_mean"],
            group["rust_ns_per_iter_median"],
            group["rust_ns_per_iter_p90"],
            group["rust_ns_per_iter_p99"],
        ),
        (
            "C ns/iter",
            group["c_ns_per_iter_mean"],
            group["c_ns_per_iter_median"],
            group["c_ns_per_iter_p90"],
            group["c_ns_per_iter_p99"],
        ),
        (
            "Per-row speedup vs C",
            group["speedup_vs_c_mean"],
            group["speedup_vs_c_median"],
            group["speedup_vs_c_p90"],
            group["speedup_vs_c_p99"],
        ),
    ]
    lines = ["| Distribution | mean | median | p90 | p99 |", "| --- | --- | --- | --- | --- |"]
    for label, avg, med, p90, p99 in rows:
        suffix = "x" if "speedup" in label else " ns"
        lines.append(
            f"| {label} | {avg:.3f}{suffix} | {med:.3f}{suffix} | "
            f"{p90:.3f}{suffix} | {p99:.3f}{suffix} |"
        )
    return "\n".join(lines)


def split_operation_tables(summary: dict[str, Any]) -> str:
    labels = {
        "cached_font_operation": "Cached Font Operations",
        "font_load_path_dependent": "Font Load / Path-Dependent Setup",
    }
    lines = []
    for category in ("cached_font_operation", "font_load_path_dependent"):
        rows = [row for row in summary["rows"] if row.get("timing_category") == category]
        if not rows:
            continue
        lines.extend(["", f"### {labels.get(category, category)}", ""])
        lines.append(format_table({"rows": rows, "overall": summary["overall"]}, include_aggregate=False))
    return "\n".join(lines)


def metadata_table(metadata: dict[str, Any]) -> str:
    cpu_frequency = metadata.get("cpu_frequency") or {}
    memory = metadata.get("memory") or {}
    rows = [
        ("Created UTC", metadata.get("created_utc")),
        ("Git SHA", metadata.get("git_sha")),
        ("Git dirty", metadata.get("git_dirty")),
        ("Repository root", metadata.get("repository_root")),
        ("Crate root", metadata.get("crate_root")),
        ("Workload profile", metadata.get("workload_profile")),
        ("Samples", metadata.get("sample_count")),
        ("Cached row warmup iterations", metadata.get("cached_row_warmup_iterations")),
        ("Matrix", metadata.get("matrix")),
        ("Matrix version", metadata.get("matrix_version")),
        ("Platform", metadata.get("platform")),
        ("Machine", metadata.get("machine")),
        ("CPU", metadata.get("cpu_model")),
        ("CPU governor", metadata.get("cpu_governor") or "not available"),
        ("CPU current min", cpu_frequency.get("current_min_mhz") or "not available"),
        ("CPU current max", cpu_frequency.get("current_max_mhz") or "not available"),
        ("CPU current mean", cpu_frequency.get("current_mean_mhz") or "not available"),
        ("CPU max", cpu_frequency.get("cpuinfo_max_mhz") or "not available"),
        ("CPU policy count", cpu_frequency.get("policy_count") or "not available"),
        ("Memory total", memory.get("total") or "not available"),
        ("Memory available", memory.get("available") or "not available"),
        ("Memory speed", memory.get("speed") or "not available"),
        ("Memory clock", memory.get("clock") or "not available"),
        ("Memory source", memory.get("source") or "not available"),
        ("Rust", metadata.get("rustc_version")),
        ("Cargo", metadata.get("cargo_version")),
        ("Python", metadata.get("python_version")),
        ("C compiler", metadata.get("c_compiler_version") or metadata.get("c_compiler")),
        ("FreeType include", metadata.get("ft_include")),
        ("FreeType lib", metadata.get("ft_lib")),
    ]
    lines = ["| Parameter | Value |", "| --- | --- |"]
    for key, value in rows:
        lines.append(f"| {key} | {value} |")
    return "\n".join(lines)


def benchmark_configuration_table(metadata: dict[str, Any]) -> str:
    rows = [
        ("Workload profile", metadata.get("workload_profile")),
        ("Samples", metadata.get("sample_count")),
        ("Cached row warmup iterations", metadata.get("cached_row_warmup_iterations")),
        ("Compare C", metadata.get("compare_c")),
        ("Matrix", metadata.get("matrix")),
        ("Matrix version", metadata.get("matrix_version")),
        ("FreeType include", metadata.get("ft_include")),
        ("FreeType lib", metadata.get("ft_lib")),
    ]
    lines = ["| Parameter | Value |", "| --- | --- |"]
    for key, value in rows:
        lines.append(f"| {key} | {value} |")
    return "\n".join(lines)


def format_bytes(value: int | float | None) -> str:
    if value is None:
        return "not measured"
    return f"{int(value):,} bytes"


def process_memory_table(process_memory: dict[str, Any]) -> str:
    summary = process_memory["summary"]
    rows = [
        ("Boundary", process_memory["boundary"]),
        (
            "Rust peak RSS min",
            format_bytes(summary["rust_peak_rss_bytes_min"]),
        ),
        (
            "Rust peak RSS median",
            format_bytes(summary["rust_peak_rss_bytes_median"]),
        ),
        (
            "Rust peak RSS max",
            format_bytes(summary["rust_peak_rss_bytes_max"]),
        ),
        ("C peak RSS min", format_bytes(summary["c_peak_rss_bytes_min"])),
        ("C peak RSS median", format_bytes(summary["c_peak_rss_bytes_median"])),
        ("C peak RSS max", format_bytes(summary["c_peak_rss_bytes_max"])),
        (
            "Rust / C median peak RSS",
            (
                f"{summary['rust_to_c_peak_rss_ratio']:.3f}x"
                if summary["rust_to_c_peak_rss_ratio"] is not None
                else "not measured"
            ),
        ),
    ]
    lines = ["| Measurement | Value |", "| --- | --- |"]
    lines.extend(f"| {key} | {value} |" for key, value in rows)
    return "\n".join(lines)


def artifact_size_table(artifact_sizes: dict[str, Any]) -> str:
    lines = [
        "| Artifact | Implementation | Kind | Bytes | SHA-256 |",
        "| --- | --- | --- | ---: | --- |",
    ]
    for row in artifact_sizes["artifacts"]:
        lines.append(
            f"| {row['id']} | {row['implementation']} | {row['kind']} | "
            f"{row['bytes']:,} | `{row['sha256']}` |"
        )
    summary = artifact_sizes["summary"]
    lines.extend(
        [
            "",
            "| Ratio | Value |",
            "| --- | ---: |",
            "| Fontdone / FreeType shared-library bytes | "
            f"{summary['fontdone_to_freetype_shared_library_size_ratio']:.3f}x |",
            "| Rust / C workload-executable bytes | "
            f"{summary['rust_to_c_workload_executable_size_ratio']:.3f}x |",
            f"| Fontdone WASM bytes | {summary['fontdone_wasm_bytes']:,} |",
        ]
    )
    return "\n".join(lines)


def regression_table(regression: dict[str, Any]) -> str:
    lines = [
        "| Field | Value |",
        "| --- | --- |",
        f"| Status | {regression['status']} |",
        f"| Complete | {str(regression['complete']).lower()} |",
        f"| Passed | {str(regression['passed']).lower()} |",
    ]
    for debt in regression.get("debt", []):
        lines.append(f"| Debt | {debt} |")
    if regression.get("checks"):
        lines.extend(
            [
                "",
                "| Metric | Observed | Contract | Passed |",
                "| --- | ---: | --- | --- |",
            ]
        )
        for check in regression["checks"]:
            lines.append(
                f"| {check['metric']} | {check['observed']} | "
                f"{check['comparator']} {check['threshold']} | "
                f"{str(check['passed']).lower()} |"
            )
    return "\n".join(lines)


def format_report(
    metadata: dict[str, Any],
    summary: dict[str, Any] | None,
    process_memory: dict[str, Any] | None,
    artifact_sizes: dict[str, Any] | None,
    regression: dict[str, Any] | None,
) -> str:
    lines = [
        "# fontdone Benchmark Report",
        "",
        "This report is generated by `scripts/bench_freetype.py`. Raw samples and",
        "machine-readable summaries are stored in the paired JSON artifact.",
        "",
        "## Benchmark Configuration",
        "",
        benchmark_configuration_table(metadata),
        "",
        "## Environment",
        "",
        metadata_table(metadata),
        "",
        "## Trust Notes",
        "",
    ]
    for note in metadata.get("timing_notes", []):
        lines.append(f"- {note}")
    if metadata.get("git_dirty"):
        lines.append("- Warning: the benchmark was generated from a dirty worktree.")
    lines.extend(
        [
            "",
            "## Interpretation Notes",
            "",
            "- Aggregate speedup is the ratio of total C time to total Rust time.",
            "- Weighted speedup uses the selected workload profile weights.",
            "- Distribution rows are operation-count weighted. They describe the",
            "  distribution of row-level timings, not a replacement for aggregate speedup.",
            "- Per-row speedup percentiles are useful for spotting operation families,",
            "  but they are not mathematically equivalent to total speedup.",
            "- Font load/path-dependent setup is separated from cached font operations",
            "  because path-backed face creation can include filesystem and OS page-cache effects.",
        ]
    )
    lines.extend(["", "## Aggregate Summary", ""])
    if summary is None:
        lines.append("No C comparison summary was generated. Run with `--compare-c`.")
    else:
        lines.append(aggregate_table(summary))
        lines.extend(["", "## Operation Groups", ""])
        lines.append(group_summary_table(summary))
        lines.extend(["", "## Overall Distribution", ""])
        lines.append(overall_distribution_table(summary))
        for group in summary.get("groups", []):
            lines.extend(["", f"## {group['label']} Distribution", ""])
            lines.append(group_distribution_table(group))
        lines.extend(["", "## Per-Operation Results", ""])
        lines.append(split_operation_tables(summary))
    if process_memory is not None:
        lines.extend(["", "## Peak Process Memory", ""])
        lines.append(process_memory_table(process_memory))
    if artifact_sizes is not None:
        lines.extend(["", "## Binary Size", ""])
        lines.append(artifact_size_table(artifact_sizes))
    if regression is not None:
        lines.extend(["", "## Regression Contract", ""])
        lines.append(regression_table(regression))
    lines.extend(
        [
            "",
            "## Reproduction",
            "",
            "```bash",
            "make bench BENCH_SAMPLES=10 BENCH_PROFILE=default",
            "```",
            "",
        ]
    )
    return "\n".join(lines)


def write_output(
    path: pathlib.Path,
    report_path: pathlib.Path,
    rows: list[dict[str, Any]],
    metadata: dict[str, Any],
    summary: dict[str, Any] | None = None,
    process_memory: dict[str, Any] | None = None,
    artifact_sizes: dict[str, Any] | None = None,
    regression: dict[str, Any] | None = None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    payload: dict[str, Any] = {"metadata": metadata, "rows": rows}
    if summary is not None:
        payload["summary"] = summary
        payload["summary_markdown"] = format_table(summary)
    if process_memory is not None:
        payload["process_memory"] = process_memory
    if artifact_sizes is not None:
        payload["artifact_sizes"] = artifact_sizes
    if regression is not None:
        payload["regression"] = regression
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    report_path.write_text(
        format_report(
            metadata,
            summary,
            process_memory,
            artifact_sizes,
            regression,
        )
        + "\n"
    )


def require_record(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(f"cannot record performance baseline: {message}")


def compact_performance_baseline(
    payload: dict[str, Any],
    report_sha256: str,
    matrix_data: dict[str, Any],
    matrix_sha256: str,
) -> dict[str, Any]:
    metadata = payload.get("metadata", {})
    policy = matrix_data.get("regression_policy", {})
    require_record(isinstance(metadata, dict), "report metadata is missing")
    require_record(isinstance(policy, dict), "matrix regression policy is missing")
    minimum_samples = policy.get("minimum_samples_per_run")
    required_profile = policy.get("required_workload_profile")
    sample_count = metadata.get("sample_count")
    require_record(metadata.get("schema_version") == 3, "report schema is not 3")
    require_record(metadata.get("compare_c") is True, "report did not compare C")
    require_record(metadata.get("git_dirty") is False, "measured worktree was dirty")
    require_record(
        re.fullmatch(r"[0-9a-f]{40}", str(metadata.get("git_sha", ""))) is not None,
        "report does not contain a full source commit",
    )
    require_record(
        metadata.get("matrix_version") == matrix_data.get("version"),
        "report matrix version differs from the maintained matrix",
    )
    require_record(
        isinstance(minimum_samples, int) and minimum_samples > 0,
        "matrix policy has no valid minimum_samples_per_run",
    )
    require_record(
        isinstance(sample_count, int) and sample_count >= minimum_samples,
        f"report has {sample_count!r} samples; at least {minimum_samples} are required",
    )
    require_record(
        metadata.get("workload_profile") == required_profile,
        f"report profile is not the required {required_profile!r} profile",
    )
    require_record(
        set(policy.get("environment_identity", []))
        == set(REQUIRED_ENVIRONMENT_IDENTITY),
        "matrix environment identity fields are incomplete",
    )

    matrix_rows = matrix_rows_by_id(matrix_data)
    rows = payload.get("rows")
    require_record(isinstance(rows, list), "raw timing rows are missing")
    seen: dict[tuple[int, str], int] = {}
    for row in rows:
        row_id = row.get("id")
        sample_index = row.get("sample_index")
        require_record(row_id in matrix_rows, f"unknown timing row {row_id!r}")
        require_record(
            isinstance(sample_index, int) and 0 <= sample_index < sample_count,
            f"invalid sample index for {row_id!r}",
        )
        require_record(
            float(row.get("rust_ns_per_iter", 0)) > 0.0
            and float(row.get("c_ns_per_iter", 0)) > 0.0,
            f"non-positive Rust or C timing for {row_id!r}",
        )
        require_record(
            row.get("output_match") is not False,
            f"output mismatch for {row_id!r}",
        )
        key = (sample_index, row_id)
        seen[key] = seen.get(key, 0) + 1
    expected_keys = {
        (sample_index, row_id)
        for sample_index in range(sample_count)
        for row_id in matrix_rows
    }
    require_record(
        set(seen) == expected_keys and all(count == 1 for count in seen.values()),
        "timing rows do not exactly cover every matrix row and sample",
    )

    summary = payload.get("summary")
    process_memory = payload.get("process_memory")
    artifact_sizes = payload.get("artifact_sizes")
    regression = payload.get("regression")
    require_record(isinstance(summary, dict), "C comparison summary is missing")
    require_record(
        isinstance(process_memory, dict), "process-memory evidence is missing"
    )
    require_record(
        isinstance(artifact_sizes, dict), "artifact-size evidence is missing"
    )
    require_record(
        isinstance(regression, dict), "regression-policy evaluation is missing"
    )

    memory_samples = process_memory.get("samples")
    require_record(
        isinstance(memory_samples, list) and len(memory_samples) == sample_count,
        "peak-RSS samples do not match timing sample count",
    )
    for sample in memory_samples:
        for implementation in ("rust", "c"):
            measurement = sample.get(implementation)
            require_record(
                isinstance(measurement, dict)
                and isinstance(measurement.get("peak_rss_bytes"), int)
                and measurement["peak_rss_bytes"] > 0,
                f"{implementation} peak RSS is missing or non-positive",
            )

    artifacts = artifact_sizes.get("artifacts")
    require_record(
        isinstance(artifacts, list)
        and all(isinstance(row, dict) for row in artifacts),
        "artifact records are missing or malformed",
    )
    artifact_by_id = {row.get("id"): row for row in artifacts}
    require_record(
        set(artifact_by_id) == set(REQUIRED_ARTIFACT_IDS)
        and len(artifacts) == len(REQUIRED_ARTIFACT_IDS),
        "artifact records do not exactly match the required artifact set",
    )
    compact_artifacts = []
    for artifact_id in REQUIRED_ARTIFACT_IDS:
        artifact = artifact_by_id[artifact_id]
        require_record(
            isinstance(artifact.get("bytes"), int) and artifact["bytes"] > 0,
            f"artifact {artifact_id!r} has no positive byte size",
        )
        require_record(
            re.fullmatch(r"[0-9a-f]{64}", str(artifact.get("sha256", "")))
            is not None,
            f"artifact {artifact_id!r} has no valid SHA-256",
        )
        compact_artifacts.append(
            {
                "id": artifact_id,
                "bytes": artifact["bytes"],
                "sha256": artifact["sha256"],
            }
        )

    overall = summary.get("overall", {})
    observations = regression.get("observations", {})
    for metric in REQUIRED_REGRESSION_THRESHOLDS:
        value = observations.get(metric)
        require_record(
            isinstance(value, (int, float)) and float(value) > 0.0,
            f"regression observation {metric!r} is missing or non-positive",
        )
    environment = {
        "platform": metadata.get("platform"),
        "machine": metadata.get("machine"),
        "cpu_model": metadata.get("cpu_model"),
        "runner_image": metadata.get("ci", {}).get("runner_image"),
        "rustc_version": metadata.get("rustc_version"),
        "c_compiler_version": metadata.get("c_compiler_version"),
    }
    require_record(
        all(
            environment[field] not in (None, "")
            for field in policy.get("environment_identity", [])
            if field != "runner_image"
        ),
        "required non-CI environment identity is incomplete",
    )
    environment_id = sha256_bytes(
        json.dumps(environment, sort_keys=True, separators=(",", ":")).encode()
    )
    per_operation = []
    for row in summary.get("rows", []):
        per_operation.append(
            {
                "id": row["id"],
                "comparison_trust": row["comparison_trust"],
                "rust_ns_per_iter_median": row["rust_ns_per_iter_median"],
                "rust_ns_per_iter_p90": row["rust_ns_per_iter_p90"],
                "rust_ns_per_iter_p99": row["rust_ns_per_iter_p99"],
                "c_ns_per_iter_median": row["c_ns_per_iter_median"],
                "c_ns_per_iter_p90": row["c_ns_per_iter_p90"],
                "c_ns_per_iter_p99": row["c_ns_per_iter_p99"],
                "rust_operations_per_second_median": row[
                    "rust_operations_per_second_median"
                ],
                "c_operations_per_second_median": row[
                    "c_operations_per_second_median"
                ],
                "speedup_vs_c_median": row["speedup_vs_c_median"],
            }
        )
    require_record(
        {row["id"] for row in per_operation} == set(matrix_rows)
        and len(per_operation) == len(matrix_rows),
        "summary does not exactly cover the maintained matrix",
    )
    memory_summary = process_memory["summary"]
    artifact_summary = artifact_sizes["summary"]
    return {
        "report_sha256": report_sha256,
        "measured_utc": metadata.get("created_utc"),
        "recorded_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "source_commit": metadata["git_sha"],
        "clean_source": True,
        "matrix_version": metadata["matrix_version"],
        "matrix_sha256": matrix_sha256,
        "workload_profile": metadata["workload_profile"],
        "sample_count": sample_count,
        "environment_id": environment_id,
        "environment": environment,
        "latency": {
            "rust_ns_per_iter_mean": overall["rust_ns_per_iter_mean"],
            "rust_ns_per_iter_median": overall["rust_ns_per_iter_median"],
            "rust_ns_per_iter_p90": overall["rust_ns_per_iter_p90"],
            "rust_ns_per_iter_p99": overall["rust_ns_per_iter_p99"],
            "c_ns_per_iter_mean": overall["c_ns_per_iter_mean"],
            "c_ns_per_iter_median": overall["c_ns_per_iter_median"],
            "c_ns_per_iter_p90": overall["c_ns_per_iter_p90"],
            "c_ns_per_iter_p99": overall["c_ns_per_iter_p99"],
            "weighted_speedup_vs_c": overall["weighted_speedup_vs_c"],
        },
        "throughput": {
            "operation_count": overall["operation_count"],
            "rust_operations_per_second": overall[
                "rust_operations_per_second"
            ],
            "c_operations_per_second": overall["c_operations_per_second"],
            "ratio_vs_c": overall["throughput_ratio_vs_c"],
        },
        "peak_process_memory": {
            **memory_summary,
            "boundary": process_memory["boundary"],
        },
        "binary_size": {
            "measurement": artifact_sizes["measurement"],
            "artifacts": compact_artifacts,
            **artifact_summary,
        },
        "regression": regression,
        "per_operation": per_operation,
    }


def record_performance_baseline(
    report_path: pathlib.Path,
    matrix_path: pathlib.Path,
) -> None:
    require_record(report_path.is_file(), f"report is missing: {report_path}")
    report_bytes = report_path.read_bytes()
    report_sha256 = sha256_bytes(report_bytes)
    snapshot = json.loads(COMPATIBILITY_SNAPSHOT.read_text(encoding="utf-8"))
    existing = snapshot.get("performance", {}).get("clean_runs", [])
    if any(run.get("report_sha256") == report_sha256 for run in existing):
        print(
            f"performance baseline {report_sha256} is already recorded in "
            f"{COMPATIBILITY_SNAPSHOT.relative_to(ROOT)}"
        )
        return

    current_status = run_optional(["git", "status", "--short"], cwd=REPO_ROOT)
    require_record(not current_status, "current worktree is dirty")
    current_sha = run_optional(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT)
    payload = json.loads(report_bytes)
    require_record(
        payload.get("metadata", {}).get("git_sha") == current_sha,
        "report source commit is not the current HEAD",
    )
    matrix_data = load_matrix(matrix_path)
    matrix_sha256 = sha256_file(matrix_path)
    baseline = compact_performance_baseline(
        payload,
        report_sha256,
        matrix_data,
        matrix_sha256,
    )
    runs = [*existing, baseline]
    policy = matrix_data["regression_policy"]
    profile = policy["required_workload_profile"]
    current_runs = [
        run
        for run in runs
        if run.get("matrix_sha256") == matrix_sha256
        and run.get("workload_profile") == profile
    ]
    environment_run_counts: dict[str, int] = {}
    for run in current_runs:
        environment_id = run["environment_id"]
        environment_run_counts[environment_id] = (
            environment_run_counts.get(environment_id, 0) + 1
        )
    minimum_runs = policy["minimum_clean_runs_per_environment"]
    snapshot["performance"] = {
        "schema_version": 1,
        "command": "make bench BENCH_SAMPLES=10 BENCH_PROFILE=default",
        "record_command": "make record-performance-baseline",
        "matrix": str(matrix_path.relative_to(ROOT)),
        "matrix_version": matrix_data["version"],
        "matrix_sha256": matrix_sha256,
        "required_workload_profile": profile,
        "minimum_samples_per_run": policy["minimum_samples_per_run"],
        "minimum_clean_runs_per_environment": minimum_runs,
        "regression_status": policy["status"],
        "thresholds": policy.get("thresholds"),
        "current_matrix_clean_run_count": len(current_runs),
        "environment_run_counts": environment_run_counts,
        "ready_for_threshold_review": any(
            count >= minimum_runs for count in environment_run_counts.values()
        ),
        "clean_runs": runs,
    }
    readme = README.read_text(encoding="utf-8")
    readme_block = format_performance_readme(snapshot["performance"])
    updated_readme, replacement_count = PERFORMANCE_README_PATTERN.subn(
        readme_block,
        readme,
    )
    require_record(
        replacement_count == 1,
        "README performance baseline marker is missing or duplicated",
    )
    roadmap = ROADMAP.read_text(encoding="utf-8")
    roadmap_block = format_performance_roadmap(snapshot["performance"])
    updated_roadmap, roadmap_replacement_count = (
        PERFORMANCE_ROADMAP_PATTERN.subn(roadmap_block, roadmap)
    )
    require_record(
        roadmap_replacement_count == 1,
        "ROADMAP performance baseline marker is missing or duplicated",
    )
    COMPATIBILITY_SNAPSHOT.write_text(
        json.dumps(snapshot, indent=2) + "\n",
        encoding="utf-8",
    )
    README.write_text(updated_readme, encoding="utf-8")
    ROADMAP.write_text(updated_roadmap, encoding="utf-8")
    print(
        f"recorded clean performance baseline {report_sha256} in "
        f"{COMPATIBILITY_SNAPSHOT.relative_to(ROOT)}"
    )


def format_performance_readme(performance: dict[str, Any]) -> str:
    minimum_runs = performance["minimum_clean_runs_per_environment"]
    environment_counts = performance["environment_run_counts"]
    qualifying_count = max(environment_counts.values(), default=0)
    lines = [
        "<!-- performance-baseline:start -->",
        f"The committed ledger contains **{qualifying_count} / {minimum_runs} clean runs**",
        "for its most-sampled current environment. Five runs from the same environment",
        "are required before regression thresholds can be reviewed.",
    ]
    current_runs = [
        run
        for run in performance["clean_runs"]
        if run.get("matrix_sha256") == performance["matrix_sha256"]
        and run.get("workload_profile")
        == performance["required_workload_profile"]
    ]
    if current_runs:
        latest = current_runs[-1]
        observations = latest["regression"]["observations"]
        lines.extend(
            [
                "",
                "| Latest clean measurement | Value |",
                "|---|---:|",
                f"| Source commit | `{latest['source_commit']}` |",
                f"| Samples | {latest['sample_count']} |",
                "| Weighted latency speedup versus C | "
                f"{observations['minimum_weighted_speedup_vs_c']:.3f}x |",
                "| Total throughput ratio versus C | "
                f"{observations['minimum_total_throughput_ratio_vs_c']:.3f}x |",
                "| Median peak-RSS ratio versus C | "
                f"{observations['maximum_peak_rss_ratio_vs_c']:.3f}x |",
                "| Shared-library byte-size ratio versus C | "
                f"{observations['maximum_shared_library_size_ratio_vs_c']:.3f}x |",
                "| Fontdone WASM size | "
                f"{int(observations['maximum_wasm_bytes']):,} bytes |",
            ]
        )
    else:
        lines.extend(
            [
                "",
                "No qualifying clean ten-sample run has been committed yet. Dirty smoke",
                "runs are useful diagnostics but cannot enter this ledger.",
            ]
        )
    lines.extend(
        [
            "",
            f"The regression policy is `{performance['regression_status']}`. "
            "`make bench-regression`",
            "therefore fails closed until reviewed thresholds become active.",
            "<!-- performance-baseline:end -->",
        ]
    )
    return "\n".join(lines)


def format_performance_roadmap(performance: dict[str, Any]) -> str:
    minimum_runs = performance["minimum_clean_runs_per_environment"]
    qualifying_count = max(
        performance["environment_run_counts"].values(),
        default=0,
    )
    return "\n".join(
        [
            "<!-- performance-roadmap:start -->",
            "The most-sampled current environment has "
            f"**{qualifying_count} / {minimum_runs} qualifying clean runs**.",
            "<!-- performance-roadmap:end -->",
        ]
    )


def run_self_test() -> int:
    samples = [
        [
            {
                "id": "a",
                "operation": "op",
                "iterations": 10,
                "rust_ns_total": 100,
                "rust_ns_per_iter": 10,
                "c_ns_total": 200,
                "c_ns_per_iter": 20,
            },
            {
                "id": "b",
                "operation": "op",
                "iterations": 5,
                "rust_ns_total": 100,
                "rust_ns_per_iter": 20,
                "c_ns_total": 50,
                "c_ns_per_iter": 10,
            },
        ],
        [
            {
                "id": "a",
                "operation": "op",
                "iterations": 10,
                "rust_ns_total": 200,
                "rust_ns_per_iter": 20,
                "c_ns_total": 400,
                "c_ns_per_iter": 40,
            },
            {
                "id": "b",
                "operation": "op",
                "iterations": 5,
                "rust_ns_total": 200,
                "rust_ns_per_iter": 40,
                "c_ns_total": 100,
                "c_ns_per_iter": 20,
            },
        ],
    ]
    summary = summarize_rows(
        samples,
        {"a": 2.0, "b": 1.0},
        {
            "a": {"comparison_trust": "exact_sha256", "timing_boundary": "test"},
            "b": {"comparison_trust": "timing_only", "timing_boundary": "test"},
        },
    )
    assert summary["overall"]["operation_count"] == 30
    assert summary["overall"]["rust_ns_total"] == 600
    assert summary["overall"]["c_ns_total"] == 750
    assert round(summary["overall"]["speedup_vs_c_total"], 6) == 1.25
    assert round(summary["overall"]["weighted_speedup_vs_c"], 6) == 1.25
    assert round(summary["overall"]["rust_operations_per_second"], 3) == 50_000_000.0
    assert round(summary["overall"]["c_operations_per_second"], 3) == 40_000_000.0
    assert round(summary["overall"]["throughput_ratio_vs_c"], 6) == 1.25
    assert summary["overall"]["rust_ns_per_iter_mean"] == 20.0
    assert summary["overall"]["rust_ns_per_iter_median"] == 20.0
    assert summary["overall"]["rust_ns_per_iter_p90"] == 40.0
    assert summary["overall"]["c_ns_per_iter_p99"] == 40.0
    assert summary["overall"]["speedup_vs_c_p90"] == 2.0
    assert summary["groups"][0]["category"] == "cached_font_operation"
    assert summary["rows"][0]["speedup_vs_c_mean"] == 2.0
    assert summary["rows"][1]["speedup_vs_c_mean"] == 0.5
    process_memory = summarize_process_memory(
        [
            {
                "sample_index": 0,
                "rust": {"peak_rss_bytes": 200},
                "c": {"peak_rss_bytes": 100},
            },
            {
                "sample_index": 1,
                "rust": {"peak_rss_bytes": 300},
                "c": {"peak_rss_bytes": 200},
            },
        ]
    )
    assert process_memory["summary"]["rust_peak_rss_bytes_median"] == 250
    assert process_memory["summary"]["c_peak_rss_bytes_median"] == 150
    assert round(process_memory["summary"]["rust_to_c_peak_rss_ratio"], 6) == round(
        250 / 150, 6
    )
    artifact_sizes = {
        "summary": {
            "fontdone_to_freetype_shared_library_size_ratio": 2.0,
            "rust_to_c_workload_executable_size_ratio": 3.0,
            "fontdone_wasm_bytes": 400,
        }
    }
    collecting = evaluate_regression_policy(
        {
            "regression_policy": {
                "status": "collecting_baseline",
                "minimum_clean_runs_per_environment": 5,
                "thresholds": None,
            }
        },
        summary,
        process_memory,
        artifact_sizes,
    )
    assert collecting["complete"] is False
    active = evaluate_regression_policy(
        {
            "regression_policy": {
                "status": "active",
                "minimum_clean_runs_per_environment": 5,
                "thresholds": {
                    "minimum_weighted_speedup_vs_c": 1.0,
                    "minimum_total_throughput_ratio_vs_c": 1.0,
                    "maximum_peak_rss_ratio_vs_c": 2.0,
                    "maximum_shared_library_size_ratio_vs_c": 2.5,
                    "maximum_wasm_bytes": 500,
                },
            }
        },
        summary,
        process_memory,
        artifact_sizes,
    )
    assert active["complete"] is True
    assert active["passed"] is True
    print("bench_freetype.py self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", type=pathlib.Path, default=DEFAULT_MATRIX)
    parser.add_argument("--out", type=pathlib.Path, default=DEFAULT_OUT)
    parser.add_argument("--report", type=pathlib.Path, default=DEFAULT_REPORT)
    parser.add_argument("--compare-c", action="store_true")
    parser.add_argument("--profile", default="default")
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--record",
        action="store_true",
        help="record the latest qualifying clean run in the compatibility snapshot",
    )
    parser.add_argument(
        "--require-regression-thresholds",
        action="store_true",
        help="fail unless the reviewed regression policy is active and every threshold passes",
    )
    parser.add_argument("--table", action="store_true", help="print comparative summary table")
    parser.add_argument("--ft-include", type=pathlib.Path, default=ROOT / "freetype/include")
    parser.add_argument("--ft-lib", type=pathlib.Path, default=ROOT / "freetype/build")
    args = parser.parse_args()
    if args.self_test:
        return run_self_test()
    if args.record:
        try:
            record_performance_baseline(args.out, args.matrix)
        except (ValueError, json.JSONDecodeError) as err:
            parser.error(str(err))
        return 0
    if args.samples < 1:
        parser.error("--samples must be >= 1")
    matrix_data = load_matrix(args.matrix)
    try:
        weights = load_weights(matrix_data, args.profile)
    except ValueError as err:
        parser.error(str(err))

    rust_binary = build_rust_benchmark()
    helper = None
    artifact_sizes = None
    if args.compare_c:
        helper = compile_c_helper(args.ft_include, args.ft_lib)
        artifact_sizes = build_and_measure_artifacts(
            rust_binary,
            helper,
            args.ft_lib,
        )

    sample_rows = []
    process_samples = []
    rows: list[dict[str, Any]] = []
    for sample_index in range(args.samples):
        rust_rows, rust_measurement = run_rust(args.matrix, rust_binary)
        c_rows = None
        c_measurement = None
        if args.compare_c and helper is not None:
            c_rows, c_measurement = run_c(args.matrix, helper, args.ft_lib)
        process_samples.append(
            {
                "sample_index": sample_index,
                "rust": rust_measurement,
                "c": c_measurement,
            }
        )
        merged = merge_rows(rust_rows, c_rows)
        for row in merged:
            row["sample_index"] = sample_index
        sample_rows.append(merged)
        rows.extend(merged)

    mismatches = [row for row in rows if row.get("output_match") is False]
    if mismatches:
        print("benchmark output mismatches:", file=sys.stderr)
        for row in mismatches:
            print(f"  {row['id']}", file=sys.stderr)
        return 1

    metadata = build_metadata(args, matrix_data)
    summary = summarize_rows(sample_rows, weights, matrix_rows_by_id(matrix_data)) if args.compare_c else None
    process_memory = summarize_process_memory(process_samples)
    regression = (
        evaluate_regression_policy(
            matrix_data,
            summary,
            process_memory,
            artifact_sizes,
        )
        if summary is not None and artifact_sizes is not None
        else None
    )
    write_output(
        args.out,
        args.report,
        rows,
        metadata,
        summary,
        process_memory,
        artifact_sizes,
        regression,
    )
    if args.table and summary is not None:
        print(format_table(summary))
    print(f"wrote {args.out}")
    print(f"wrote {args.report}")
    if args.require_regression_thresholds and (
        regression is None or not regression["passed"]
    ):
        print("performance regression contract is incomplete or failed", file=sys.stderr)
        if regression is not None:
            for debt in regression.get("debt", []):
                print(f"  {debt}", file=sys.stderr)
            for check in regression.get("checks", []):
                if not check["passed"]:
                    print(
                        f"  {check['metric']}: observed={check['observed']} "
                        f"required {check['comparator']} {check['threshold']}",
                        file=sys.stderr,
                    )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
