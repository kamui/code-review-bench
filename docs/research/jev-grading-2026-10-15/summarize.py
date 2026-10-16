"""Recompute verified metrics, routing diagnostics and complete attempt accounting."""

import json
from pathlib import Path
import statistics
import sys

import evaluate

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parents[2] / "bench/tools"))
import codex_usage


def clear(row):
    probability = row.get("raw_noul")
    if probability is not None:
        return probability >= 0.95 or probability <= 0.05
    return row["actual"] != "cannot_tell" and (row.get("confidence") or 0) >= 0.95


def main():
    metrics = {}
    for partition, versions in (("development", range(1, 13)), ("qualification", (1, 3, 6, 10, 11))):
        for number in versions:
            if number == 2:
                continue
            name = f"jev-{partition}-v{number}"
            value = evaluate.evaluate(ROOT / "results" / name, partition, "jev")
            (ROOT / "metrics" / f"{name}.json").write_text(json.dumps(value, indent=2) + "\n")
            metrics[name] = value
    for partition in ("development", "qualification"):
        name = f"sol-{partition}-v1"
        output = ROOT / "results" / (name + "-replacement1" if partition == "qualification" else name)
        kwargs = {"attempt_map": output / "attempt-map.json"} if partition == "qualification" else {}
        value = evaluate.evaluate(output, partition, "sol", **kwargs)
        (ROOT / "metrics" / f"{name}.json").write_text(json.dumps(value, indent=2) + "\n")
        metrics[name] = value
    routing = {}
    for name, value in metrics.items():
        if not name.startswith("jev-"):
            continue
        routing[name] = {}
        for axis in evaluate.AXES:
            rows = [row for row in value["rows"] if row["axis"] == axis]
            accepted = [row for row in rows if clear(row)]
            escalated = [row for row in rows if not clear(row)]
            routing[name][axis] = {
                "eligible_labelled_axes": len(rows), "shadow_cleared": len(accepted),
                "incorrect_cleared": sum(not row["correct"] for row in accepted),
                "escalated": len(escalated), "correct_but_escalated": sum(row["correct"] for row in escalated),
                "incorrect_escalated": sum(not row["correct"] for row in escalated),
                "missed_source_investigations": None,
                "unavailable_reason": "This experiment checks packet judgments, not source investigation necessity.",
            }
    agreement = {}
    for partition in ("development", "qualification"):
        sol = {(r["case_id"], r["axis"]): r for r in metrics[f"sol-{partition}-v1"]["rows"]}
        for version in (1, 10, 11):
            name = f"jev-{partition}-v{version}"
            agreement[name] = {}
            for axis in evaluate.AXES:
                rows = [r for r in metrics[name]["rows"] if r["axis"] == axis]
                agreement[name][axis] = {"agree": sum(r["actual"] == sol[(r["case_id"], axis)]["actual"] for r in rows), "total": len(rows)}
    attempts = [json.loads(path.read_text()) for path in (ROOT / "results").glob("jev-*/*/result.json")]
    successful = [attempt for attempt in attempts if attempt["status"] == "completed"]
    latency = [attempt["elapsed_seconds"] for attempt in successful]
    ledger = evaluate.jev_runner.Ledger(ROOT / "spend-ledger")
    with ledger.locked():
        accounting = ledger.accounting()
    codex_receipts = [json.loads(path.read_text()) for path in (ROOT / "results").glob("sol-*/**/receipt.json")]
    usage = {}
    requests = 0
    for path in (ROOT / "results").glob("sol-*/**/rollout-*.jsonl"):
        rollout = codex_usage.read_rollout(path)
        for tokens, calls in rollout["usage"]:
            requests += 1
            for key, value in tokens.items():
                if isinstance(value, int):
                    usage[key] = usage.get(key, 0) + value
    output = {
        "metrics": {name: value["metrics"] for name, value in metrics.items()},
        "shadow_routing": routing, "jev_sol_agreement": agreement,
        "jev_accounting": {**accounting, "attempts": len(attempts), "completed": len(successful),
                           "failed_or_unknown": len(attempts) - len(successful),
                           "validated_input_tokens": sum(a["usage"]["input_tokens"] for a in successful)},
        "jev_observed_request_seconds": {"count": len(latency), "median": statistics.median(latency),
                                        "min": min(latency), "max": max(latency),
                                        "scope": "Single HTTP call including network and response validation; excludes evidence preparation, extraction, investigators and audits. Not a controlled speed comparison."},
        "codex_accounting": {"requests": requests, "usage": usage,
                             "completed": sum(r["status"] == "completed" for r in codex_receipts),
                             "failed_receipts": sum(r["status"] != "completed" for r in codex_receipts),
                             "cash_cost": None, "cash_cost_reason": "Subscription quota; token counts are not a weekly quota conversion."},
        "production_qualified": False, "source_investigation_trial_run": False,
        "pipeline_savings": None,
    }
    (ROOT / "metrics/summary.json").write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps({"jev": output["jev_accounting"], "codex": output["codex_accounting"],
                      "latency": output["jev_observed_request_seconds"]}, indent=2))


if __name__ == "__main__":
    main()
