#!/usr/bin/env python3
"""Promote the current generated C-ABI scorecard into the committed snapshot."""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCORECARD = ROOT / "target" / "api-abi-audit" / "c_abi_contract_status.json"
SNAPSHOT = ROOT / "doc" / "compatibility_snapshot.json"


def scorecard_metrics(scorecard: dict[str, object]) -> dict[str, int | bool]:
    """Extract the stable C-contract fields kept in compatibility_snapshot."""

    categories = scorecard.get("categories")
    if not isinstance(categories, list) or len(categories) != 12:
        raise ValueError("scorecard must contain exactly 12 categories")
    metrics: dict[str, dict[str, object]] = {}
    for category in categories:
        if not isinstance(category, dict):
            raise ValueError("scorecard category is not an object")
        rows = category.get("metrics")
        if not isinstance(rows, list):
            raise ValueError("scorecard category metrics are not a list")
        for row in rows:
            if not isinstance(row, dict) or not isinstance(row.get("id"), str):
                raise ValueError("scorecard metric is not a valid object")
            metrics[row["id"]] = row

    values: dict[str, int | bool] = {}
    for key in ("categories_complete", "categories_total", "is_complete"):
        value = scorecard.get(key)
        if not isinstance(value, (int, bool)):
            raise ValueError(f"scorecard field {key} is missing or invalid")
        values[key] = value
    for metric_id, complete_key, total_key in (
        ("C01.7", "runtime_contract_rows_complete", "runtime_contract_rows_total"),
        ("C11.3", "binary_artifact_items_complete", "binary_artifact_items_total"),
        ("C12.3", "platform_lanes_complete", "platform_lanes_total"),
    ):
        metric = metrics.get(metric_id)
        if metric is None:
            raise ValueError(f"scorecard is missing metric {metric_id}")
        complete = metric.get("complete")
        total = metric.get("total")
        if not isinstance(complete, int) or not isinstance(total, int):
            raise ValueError(f"scorecard metric {metric_id} has invalid counts")
        values[complete_key] = complete
        values[total_key] = total
    values["runtime_contract_rows_pending"] = (
        int(values["runtime_contract_rows_total"])
        - int(values["runtime_contract_rows_complete"])
    )
    return values


def main() -> int:
    if not SCORECARD.is_file():
        print(f"missing {SCORECARD.relative_to(ROOT)}; run make c-abi-contract", file=sys.stderr)
        return 1
    snapshot = json.loads(SNAPSHOT.read_text(encoding="utf-8"))
    scorecard = json.loads(SCORECARD.read_text(encoding="utf-8"))
    if scorecard.get("pinned_freetype_version") != snapshot.get("freetype_version"):
        raise ValueError("scorecard and compatibility snapshot FreeType versions differ")
    contract = snapshot.get("c_contract")
    if not isinstance(contract, dict):
        raise ValueError("compatibility snapshot is missing c_contract")
    contract.update(scorecard_metrics(scorecard))
    SNAPSHOT.write_text(json.dumps(snapshot, indent=2) + "\n", encoding="utf-8")
    print(
        "recorded C-ABI contract snapshot: "
        f"{contract['categories_complete']}/{contract['categories_total']} categories, "
        f"C01.7={contract['runtime_contract_rows_complete']}/"
        f"{contract['runtime_contract_rows_total']}, "
        f"C11.3={contract['binary_artifact_items_complete']}/"
        f"{contract['binary_artifact_items_total']}, "
        f"C12.3={contract['platform_lanes_complete']}/"
        f"{contract['platform_lanes_total']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
