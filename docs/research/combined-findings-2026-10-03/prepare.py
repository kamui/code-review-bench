#!/usr/bin/env python3
"""Pin the six complete-target batches named by the combined-finding correction."""

import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
PREVIOUS = ROOT / "docs/research/skill-matrix-2026-10-02"


def read(path):
    return json.loads(path.read_text())


def ref(path):
    return {"path": path.relative_to(ROOT).as_posix(),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def check(reference):
    assert ref(ROOT / reference["path"]) == reference, reference["path"]


def write(name, value):
    with (HERE / name).open("x") as handle:
        handle.write(json.dumps(value, indent=2) + "\n")


def main():
    correction_path = PREVIOUS / "combined-findings-correction.v1.json"
    batches = [b for b in read(correction_path)["grading_batches"] if b["previous_mapping"]]
    wanted = {(f"bench/runs/{b['run']}", b["target"]): b for b in batches}
    source = read(PREVIOUS / "source-plan.selected.v2.json")
    reviews = []
    for original in source["reviews"]:
        batch = wanted.get((original["run"], original["target"]))
        if batch is None:
            continue
        row = copy.deepcopy(original)
        check(row["review"])
        check(row["record"])
        check(batch["previous_mapping"])
        mapping = read(ROOT / batch["previous_mapping"]["path"])
        previous = next(a for a in mapping["attempts"] if a["attempt_id"] == row["attempt"])
        row.update(priorMapping=batch["previous_mapping"],
                   priorAssignments=[item["assignment"] for item in previous["items"]],
                   reason="Reassess each assertion after combined items were linked as related")
        reviews.append(row)
    assert len(reviews) == 28 and len(batches) == 6
    for (run, target), batch in wanted.items():
        assert {r["attempt"] for r in reviews if (r["run"], r["target"]) == (run, target)} == set(batch["attempts"])
        assert not (ROOT / run / "scoring" / target / f"mapping.v{batch['next_mapping_version']}.json").exists()
    registry = ROOT / "bench/claims/registry.json"
    source.update(registry=ref(registry), cases=read(registry)["cases"], reviews=reviews,
                  targets=[t for t in source["targets"] if t["target"] in {b["target"] for b in batches}])
    for target in source["targets"]:
        check(target["nextRegister"])
    write("source-plan.v1.json", source)
    write("execution-plan.v1.json", {
        "workspaceRoot": ".local/combined-findings-2026-10-03/workspaces",
        "archiveRoot": "bench/regrading/combined-findings-2026-10-03",
        "order": [{"run": run, "target": target} for run, target in wanted]})
    previous_auth = read(PREVIOUS / "authorization.selected.v2.json")
    audit_paths = [PREVIOUS / "audit" / f"{name}.jsonl" for name in ("reviews", "grading")]
    prior_cost = sum(row["cost_usd"] for path in audit_paths for row in
                     (json.loads(line) for line in path.read_text().splitlines()))
    assert round(prior_cost, 6) == 203.414533
    previous_grading = read(PREVIOUS / "grading-completion.v1.json")
    assert round(previous_grading["settledChargeUpperUsd"], 6) == 44.945779
    authorization = {
        "schemaVersion": 1,
        "scope": "Regrade 28 saved reviews in six complete-target batches after combined-finding link corrections, then publish verified scores; no new reviewer benchmarks",
        "budgetCapUsd": 30,
        "budgetReceipt": {
            "speaker": "user", "text": "go",
            "context": "Response to the proposal to regrade the 28 saved reviews, regenerate scores, publish and verify the site",
            "accountPolicy": previous_auth["budgetReceipt"],
            "note": "The $30 limit is a conservative controller limit for list-price-equivalent usage, not a new invoice budget or a reset of historical spend. Stop on an account usage limit."},
        "priorUsage": {"reviewAndGradingListPriceUsd": round(prior_cost, 6),
                       "sources": [ref(path) for path in audit_paths],
                       "gradingCompletion": ref(PREVIOUS / "grading-completion.v1.json")},
        "grader": {"model": "claude-opus-5-5", "effort": "high", "cliVersion": "2.1.287", "timeoutSeconds": 2700,
                   "selection": {"speaker": "user", "text": "i think we should settle on opus 5.5 high as the grader?"}},
        "correction": ref(correction_path),
        "sourcePlan": ref(HERE / "source-plan.v1.json"),
        "executionPlan": ref(HERE / "execution-plan.v1.json"),
        "graderTemplate": ref(ROOT / previous_auth["graderTemplate"]["path"]),
        "cacheReplacements": previous_auth["cacheReplacements"],
        "runnerDeviations": [ref(ROOT / r["path"]) for r in previous_auth["runnerDeviations"]]
            + [ref(ROOT / "bench/tools/native_artifacts.py"), ref(Path(__file__).resolve())]}
    check(authorization["cacheReplacements"])
    write("authorization.v1.json", authorization)
    print("Pinned six complete-target batches, 28 reviews, Claude Opus 5.5 High, $30 accounting limit")


if __name__ == "__main__":
    main()
