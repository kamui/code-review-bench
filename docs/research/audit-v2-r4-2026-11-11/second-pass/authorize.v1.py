#!/usr/bin/env python3
"""Write the authorization of the Claude Sonnet second pass of audit round evaluator-audit-v2-2026-10-29-round-4.

    python3 -I docs/research/audit-v2-r4-2026-11-11/second-pass/authorize.v1.py

Run from the repository root after the plan and the no-charge preflight. Audit decision 17
(bench/grading/rulings/cohort-rebuild-audit.v12.md) draws the round and reserves its second grader and spend for a
separate owner approval; decision 15 (v10) sets the plan; decision 14 (v9) the trpc archive. `runtimes` is the
output of `bench-tools runtime pin`.

APPROVAL stays None until the owner's approval of this round's grader and cap is saved as a ruling receipt; then it
holds that receipt's path, the owner's words, their context and the cap as {"receipt", "text", "context", "capUsd"}.
Until then the helper refuses and writes nothing. It is the only gate: second_pass.py checks the receipt's hash, not
which round it approves."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
APPROVAL = None
ROUND_RECEIPT = "bench/grading/rulings/cohort-rebuild-audit.v12.md"
PLAN_RECEIPT = "bench/grading/rulings/cohort-rebuild-audit.v10.md"
TRPC_RECEIPT = "bench/grading/rulings/cohort-rebuild-audit.v9.md"
CLIENT = "2.1.317"
PINNED = ["docs/research/audit-v2-r4-2026-11-11/second-pass/second_pass.py",
          "docs/research/audit-v2-r4-2026-11-11/second-pass/launch.py",
          "docs/research/audit-v2-r4-2026-11-11/second-pass/start.sh",
          "bench/tools/bench-tools", "bench/rates.json", "bench/policies/grading-commands.v5.json",
          "bench/policies/empty-harness-v1.md", "bench/grading/current/validation-policy.json",
          "bench/grading/current/remedy-policy.json", "bench/rubric/remedy-policy.v6.json",
          "bench/rubric/rules.pre-existing.v4.md", ROUND_RECEIPT, PLAN_RECEIPT, TRPC_RECEIPT,
          "docs/research/trpc-cache-rebuild-2026-11-04/cache-replacements.v1.json"]


def pin(path):
    return {"path": path, "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}


def main():
    if APPROVAL is None:
        sys.exit("refused: no owner approval of round 4's second grader and spend is filed. Audit decision 17 "
                 f"({ROUND_RECEIPT}) reserves both; save the approval as a ruling receipt and set APPROVAL first.")
    runtimes = json.loads(subprocess.run([str(ROOT / "bench/tools/bench-tools"), "runtime", "pin"], cwd=ROOT,
                                         capture_output=True, text=True, check=True).stdout)
    plan = json.loads((HERE / "plan.v1.json").read_text())
    approval = {"speaker": "user", "text": APPROVAL["text"], "context": APPROVAL["context"],
                "ref": pin(APPROVAL["receipt"])}
    value = {
        "schemaVersion": 1,
        "scope": f"Issue 30: the second pass of evaluator audit round {plan['round']} on the {len(plan['order'])} sampled batches of sample.json, reruns included; saves second assessments only and changes no grade",
        "budgetPolicy": "claude-list-price-cap",
        "budgetCapUsd": APPROVAL["capUsd"],
        "maxBudgetUsd": 6.0,
        "budgetReceipt": approval,
        "grader": {"client": "claude", "model": "claude-sonnet-5-5", "effort": "high", "cliVersion": CLIENT,
                   "timeoutSeconds": 2700,
                   "selection": approval,
                   "fastMode": "off: start.sh exports CLAUDE_CODE_DISABLE_FAST_MODE=1, which grade dispatch passes to claude, and the driver refuses to run without it",
                   "client_pin": f"Claude Code {CLIENT}, the client of round 2 and of the rule-version-6 first grades, from its copy in /home/t3/.cache/code-review-bench/claude-{CLIENT} (sha256 eb224419...), the version the no-charge preflight passed; the driver copies it into the queue and refuses any other."},
        "plan": pin("docs/research/audit-v2-r4-2026-11-11/second-pass/plan.v1.json"),
        "preflight": pin("docs/research/audit-v2-r4-2026-11-11/preflight-second-pass.v1.json"),
        "runtimes": runtimes,
        "runnerDeviations": [pin(path) for path in PINNED + [APPROVAL["receipt"]]],
        "quotaStopPercent": 90,
        "quotaReceipt": {"speaker": "user", "text": "You are able to use until there's 10% usage left of my weekly claude sub",
                         "ref": pin("docs/research/rule-v3-regrade-2026-10-23/quota-approval.v1.json")},
    }
    (HERE.parent / "authorization.second-pass.v1.json").write_text(json.dumps(value, indent=2) + "\n")
    print(f"cap ${value['budgetCapUsd']:.2f}, {len(plan['order'])} batches")


if __name__ == "__main__":
    main()
