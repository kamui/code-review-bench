"""Verify Luna replay results and compare them with the fixed human labels."""

import json
from pathlib import Path
import statistics
import sys

ROOT = Path(__file__).resolve().parent
BASE = ROOT.parent / "jev-grading-2026-10-15"
sys.path.insert(0, str(BASE))
sys.path.insert(0, str(ROOT.parents[2] / "bench/tools"))
import evaluate
import codex_usage

COHORTS = (("choices", "v1"), ("choices", "v7"), ("choices_examples", "v7"))


def main():
    metrics = {}
    (ROOT / "metrics").mkdir(exist_ok=True)
    for mode, version in COHORTS:
        for partition in ("development", "qualification"):
            name = f"luna-{mode}-{partition}-{version}"
            output = ROOT / "results" / name
            kwargs = {"attempt_map": output / "attempt-map.json"} if (output / "attempt-map.json").exists() else {}
            result = evaluate.evaluate(output, partition, "luna",
                                       input_manifest=BASE / f"inputs/{partition}-{version}/manifest.json", **kwargs)
            (ROOT / "metrics" / f"{name}.json").write_text(json.dumps(result, indent=2) + "\n")
            metrics[name] = result
    total = {}
    for mode, version in COHORTS:
        rows = [row for partition in ("development", "qualification")
                for row in metrics[f"luna-{mode}-{partition}-{version}"]["rows"]]
        total[f"{mode}-{version}"] = {axis: {"correct": sum(r["correct"] for r in rows if r["axis"] == axis),
                                "total": sum(r["axis"] == axis for r in rows),
                                "abstentions": sum(r["axis"] == axis and r["actual"] == "cannot_tell" for r in rows)}
                          for axis in evaluate.AXES}
    requests, usage = 0, {}
    for path in (ROOT / "results").glob("**/rollout-*.jsonl"):
        for tokens, calls in codex_usage.read_rollout(path)["usage"]:
            requests += 1
            for key, value in tokens.items():
                if isinstance(value, int):
                    usage[key] = usage.get(key, 0) + value
    receipt_paths = list((ROOT / "results").glob("**/receipt.json"))
    receipts = [json.loads(path.read_text()) for path in receipt_paths]
    unknown_usage = []
    for path, receipt in zip(receipt_paths, receipts):
        rollouts = list((path.parent / "home/.codex/sessions").glob("**/rollout-*.jsonl"))
        if receipt["status"] != "completed" and (path.parent / "stdout.jsonl").exists() and not any(
                codex_usage.read_rollout(rollout)["usage"] for rollout in rollouts):
            unknown_usage.append(str(path.parent.relative_to(ROOT)))
    cohorts = {}
    for mode in ("choices", "choices_examples", "evidence"):
        selected = [r for r in receipts if r.get("response_mode", "evidence") == mode]
        latencies = [r["elapsed_seconds"] for r in selected if r["status"] == "completed"]
        cohorts[mode] = {
            "completed_attempts": sum(r["status"] == "completed" for r in selected),
            "failed_attempts": sum(r["status"] != "completed" for r in selected),
            "observed_completed_attempt_seconds": {
                "median": statistics.median(latencies), "min": min(latencies), "max": max(latencies),
                "scope": "Includes client setup and quota checks; no interleaved repeated comparison or end-to-end savings claim",
            },
        }
    result = {
        "scope": "Historical replay; no fresh blind validation or source-investigation trial",
        "metrics": {name: value["metrics"] for name, value in metrics.items()},
        "all_cases": total,
        "all_cases_limit": "Includes development examples used in input development; qualification split is the primary comparison",
        "recorded_requests": requests, "usage": usage, "unknown_usage_attempts": unknown_usage,
        "completed_attempts": sum(r["status"] == "completed" for r in receipts),
        "failed_attempts": sum(r["status"] != "completed" for r in receipts),
        "cohorts": cohorts,
        "automatic_grading_qualified": False,
        "missed_source_investigation_rate": None,
        "pipeline_cost_savings": None,
    }
    (ROOT / "metrics/summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"all_cases": total, "requests": requests, "usage": usage,
                      "failed_attempts": result["failed_attempts"]}, indent=2))


if __name__ == "__main__":
    main()
