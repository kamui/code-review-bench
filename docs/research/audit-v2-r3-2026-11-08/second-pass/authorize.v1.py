#!/usr/bin/env python3
"""Write the authorization of the Claude Sonnet second pass of audit round evaluator-audit-v2-2026-10-29-round-3.

    python3 -I docs/research/audit-v2-r3-2026-11-08/second-pass/authorize.v1.py

Run from the repository root after the plan and the no-charge preflight. Audit decision 16
(bench/grading/rulings/cohort-rebuild-audit.v11.md) chooses the grader and the $130 cap; decision 15 (v10) the round;
decision 14 (v9) the trpc archive. `runtimes` is the output of `bench-tools runtime pin`."""
import hashlib
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
RECEIPT = "bench/grading/rulings/cohort-rebuild-audit.v11.md"
PLAN_RECEIPT = "bench/grading/rulings/cohort-rebuild-audit.v10.md"
TRPC_RECEIPT = "bench/grading/rulings/cohort-rebuild-audit.v9.md"
CLIENT = "2.1.317"
PINNED = ["docs/research/audit-v2-r3-2026-11-08/second-pass/second_pass.py",
          "docs/research/audit-v2-r3-2026-11-08/second-pass/launch.py",
          "docs/research/audit-v2-r3-2026-11-08/second-pass/start.sh",
          "bench/tools/bench-tools", "bench/rates.json", "bench/policies/grading-commands.v5.json",
          "bench/policies/empty-harness-v1.md", "bench/grading/current/validation-policy.json",
          "bench/grading/current/remedy-policy.json", "bench/rubric/remedy-policy.v6.json",
          "bench/rubric/rules.pre-existing.v4.md", RECEIPT, PLAN_RECEIPT, TRPC_RECEIPT,
          "docs/research/trpc-cache-rebuild-2026-11-04/cache-replacements.v1.json"]


def pin(path):
    return {"path": path, "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}


def main():
    runtimes = json.loads(subprocess.run([str(ROOT / "bench/tools/bench-tools"), "runtime", "pin"], cwd=ROOT,
                                         capture_output=True, text=True, check=True).stdout)
    plan = json.loads((HERE / "plan.v1.json").read_text())
    value = {
        "schemaVersion": 1,
        "scope": f"Issue 30: the second pass of evaluator audit round {plan['round']} on the {len(plan['order'])} sampled batches of sample.json, reruns included; saves second assessments only and changes no grade",
        "budgetPolicy": "claude-list-price-cap",
        "budgetCapUsd": 130.0,
        "maxBudgetUsd": 6.0,
        "budgetReceipt": {"speaker": "user", "text": "Yes, $130 cap (Recommended)",
                          "context": "Audit decision 16, asked on 2026-11-08 at about 6:30 PM US Eastern: draw round 3 and run Claude Sonnet 5.5 at high effort, fast mode off, as its second grader, about $105 at list price under a $130 cap, stopping at 90% weekly use; as in round 2, random workspace names and one retry per failed batch.",
                          "ref": pin(RECEIPT)},
        "grader": {"client": "claude", "model": "claude-sonnet-5-5", "effort": "high", "cliVersion": CLIENT,
                   "timeoutSeconds": 2700,
                   "selection": {"speaker": "user", "text": "Yes, $130 cap (Recommended)",
                                 "context": "Audit decisions 15 and 16: Claude Sonnet 5.5 at high effort, without fast mode, is the second grader of round evaluator-audit-v2-2026-10-29-round-3; Claude Opus 5.5 graded every sampled batch first.",
                                 "ref": pin(RECEIPT)},
                   "fastMode": "off: start.sh exports CLAUDE_CODE_DISABLE_FAST_MODE=1, which grade dispatch passes to claude, and the driver refuses to run without it",
                   "client_pin": f"Claude Code {CLIENT}, the client of round 2 and of the rule-version-6 first grades, from its copy in /home/t3/.cache/code-review-bench/claude-{CLIENT} (sha256 eb224419...), the version the no-charge preflight passed; the driver copies it into the queue and refuses any other."},
        "plan": pin("docs/research/audit-v2-r3-2026-11-08/second-pass/plan.v1.json"),
        "preflight": pin("docs/research/audit-v2-r3-2026-11-08/preflight-second-pass.v1.json"),
        "runtimes": runtimes,
        "runnerDeviations": [pin(path) for path in PINNED],
        "quotaStopPercent": 90,
        "quotaReceipt": {"speaker": "user", "text": "You are able to use until there's 10% usage left of my weekly claude sub",
                         "ref": pin("docs/research/rule-v3-regrade-2026-10-23/quota-approval.v1.json")},
    }
    (HERE.parent / "authorization.second-pass.v1.json").write_text(json.dumps(value, indent=2) + "\n")
    print(f"cap ${value['budgetCapUsd']:.2f}, {len(plan['order'])} batches")


if __name__ == "__main__":
    main()
