#!/usr/bin/env python3
"""Plan the regrade of the batches whose rule-version-6 graders could not run node.

    BENCH_RUNTIME_STORE=<store> python3 -I docs/research/node-regrade-2026-11-07/plan.py

Run from the repository root after requeue.py. This is the rule-version-6 regrade's plan.py with this regrade's
workspace and archive roots and the same cache root. Each batch keeps the cache manifest of its rule-version-6 queue,
`trpc` included. It refuses a batch with no earlier queue, one whose plan state is neither `missing` nor `current`, and
a missing batch that is neither in batches.v1.json nor the j-trpc-5017 batch the rule-version-6 regrade left on a
blocked check."""
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
TOOL = ROOT / "bench/tools/bench-tools"
WANTED = {(b["run"], b["target"]) for b in json.loads((Path(__file__).resolve().parent / "batches.v1.json").read_text())}
WANTED.add(("runs/2026-09-29-claude-fable-high", "j-trpc-5017"))
QUEUES = {
    "docs/research/provision-sandbox-cache-2026-10-22/cache-replacements.v2.json": "rebuilt",
    "docs/research/selected-cache-rebuild-2026-10-02/cache-replacements.v1.json": "selected",
    None: "plain",
    "docs/research/trpc-cache-rebuild-2026-11-04/cache-replacements.v1.json": "trpc",
}


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


def committed_time(path):
    out = subprocess.run(["git", "log", "-1", "--format=%ct", "--", str(path)], cwd=ROOT, capture_output=True, text=True, check=True)
    return int(out.stdout.strip() or 0)


def earlier_queues():
    rows = []
    for path in (ROOT / "docs/research").rglob("authorization*.json"):
        if path.parent == HERE:
            continue
        try:
            value = load(path)
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        if not isinstance(value, dict) or "executionPlan" not in value or not (ROOT / value["executionPlan"]["path"]).exists():
            continue
        replacement = (value.get("cacheReplacements") or {}).get("path")
        for batch in load(ROOT / value["executionPlan"]["path"]).get("order", []):
            rows.append((committed_time(path.relative_to(ROOT)), (batch["run"], batch["target"]), replacement))
    latest = {}
    for _, key, replacement in sorted(rows, key=lambda row: row[0]):
        latest[key] = replacement
    return latest


def main():
    full = HERE / "source-plan.full.v1.json"
    if full.exists():
        raise SystemExit(f"{full.name} exists")
    subprocess.run([str(TOOL), "methodology", "--out", str(full)], cwd=ROOT, check=True)
    plan = load(full)
    latest = earlier_queues()
    members = {name: [] for name in QUEUES.values()}
    for batch in plan["batches"]:
        key = (batch["run"], batch["target"])
        if batch["state"] == "current":
            continue
        if batch["state"] != "missing":
            raise SystemExit(f"{key}: state {batch['state']}, not missing")
        if key not in latest:
            raise SystemExit(f"{key}: no earlier regrade queue names this batch")
        if key not in WANTED:
            raise SystemExit(f"{key}: missing, but not a batch this regrade is for")
        members[QUEUES[latest[key]]].append(key)
    for name, keys in members.items():
        if not keys:
            continue
        wanted = set(keys)
        part = dict(plan)
        part["batches"] = [batch for batch in plan["batches"] if (batch["run"], batch["target"]) in wanted]
        part["reviews"] = [review for review in plan["reviews"] if (review["run"], review["target"]) in wanted]
        targets = {target for _, target in keys}
        part["sharedClaimItems"] = [item for item in plan["sharedClaimItems"] if item["target"] in targets]
        write(HERE / f"source-plan.{name}.v1.json", part)
        write(HERE / f"execution-plan.{name}.v1.json", {
            "workspaceRoot": f".local/node-regrade/{name}/workspaces",
            "archiveRoot": f"bench/regrading/node-regrade-2026-11-07-{name}",
            "cacheRoot": ".local/regrade-v5-2/dependency-cache",
            "order": [{"run": run, "target": target} for run, target in keys],
        })
        print(f"{name}: {len(keys)} batches, {len(part['reviews'])} reviews, {len(part['sharedClaimItems'])} shared-claim items")


if __name__ == "__main__":
    main()
