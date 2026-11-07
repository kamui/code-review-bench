#!/usr/bin/env python3
"""Return to the queue every batch whose rule-version-6 grade says its grader could not run node.

    python3 -I docs/research/node-regrade-2026-11-07/requeue.py

The grading sandbox left a node under /usr/ off its PATH (fixed by PR #442), so on this host every node, tsc and vitest
run failed. This finds each current grade whose verdicts.json reports that failure, writes the list to
batches.v1.json, and removes those batches from bench/grading/current/grades.json, as `bench-tools grade invalidate`
removes a stale batch. The assessments stay on disk."""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
SIGN = re.compile(r"node: not found|execvp node|node is not (on|available|installed)|no node binary|node: No such file", re.I)
grades_path = ROOT / "bench/grading/current/grades.json"
grades = json.loads(grades_path.read_text(encoding="utf-8"))
hit, kept = [], []
for batch in grades["batches"]:
    verdicts = ROOT / batch["assessor"]["verdicts"]["path"]
    (hit if SIGN.search(verdicts.read_text(encoding="utf-8")) else kept).append(batch)
out = HERE / "batches.v1.json"
if out.exists():
    raise SystemExit(f"{out.name} exists")
out.write_text(json.dumps([{"run": b["run"], "target": b["target"], "verdicts": b["assessor"]["verdicts"]["path"]} for b in hit], indent=1) + "\n")
grades["batches"] = kept
grades_path.write_text(json.dumps(grades, indent=2) + "\n", encoding="utf-8")
print(f"{len(hit)} batches returned to the queue; {len(kept)} stay current")
