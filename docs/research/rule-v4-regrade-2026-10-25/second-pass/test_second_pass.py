#!/usr/bin/env python3
"""Exercise second_pass.py's run loop against a fake bench-tools and a fake claude; no model, no repository writes.

    python3 -I docs/research/rule-v4-regrade-2026-10-25/second-pass/test_second_pass.py
"""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

DRIVER = Path(__file__).resolve().parent / "second_pass.py"
ROUND = "evaluator-audit-v2-2026-10-29"
PIN = {"bench-tools": {"sha256": "a" * 64, "receipt_sha256": "b" * 64}}

FAKE_TOOL = r'''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
a = sys.argv[1:]
def opt(name):
    return a[a.index(name) + 1]
used = float(os.environ.get("FAKE_USED", "1"))
if a[:2] == ["runtime", "pin"]:
    print(json.dumps(PIN))
elif a[:2] == ["subscription-meters", "read"]:
    assert "claude" in json.loads(opt("--clients"))
    print(json.dumps({"at": "now", "claude": {"five_hour": {"utilization": 1.0}, "seven_day": {"utilization": used}},
                      "claude_sign_in_expires_at": 4102444800000}))
elif a[:2] == ["subscription-meters", "stop"]:
    reading = json.load(sys.stdin)
    hit = reading["claude"]["seven_day"]["utilization"] >= float(opt("--percent"))
    print(json.dumps({"stop": "Claude usage reached the quota stop" if hit else None, "sign_in_expiring": False}))
elif a[:2] == ["evaluator-audit", "claims"]:
    Path(opt("--out")).write_text("{}")
elif a[:2] == ["grade", "prepare"]:
    Path(opt("--work")).mkdir(parents=True)
    Path(opt("--key")).write_text(json.dumps({"run": opt("--run"), "target": opt("--target")}))
elif a[:2] == ["grade", "check-prepared"]:
    pass
elif a[:2] == ["grade", "dispatch"]:
    assert "--allow-unbounded-codex" not in a and opt("--model") == "claude-opus-5-5"
    Path(os.environ["FAKE_BUDGETS"]).open("a").write(opt("--max-budget-usd") + "\n")
    work = Path(opt("--work"))
    target = json.loads(Path(opt("--key")).read_text())["target"]
    (work / "home").mkdir()
    failed = target == os.environ.get("FAKE_DISPATCH_FAIL")
    (work / "dispatch.json").write_text(json.dumps({"exit_code": 1 if failed else 0, "verdicts_present": not failed,
        "audit_violations": [], "usage": {"high": float(os.environ.get("FAKE_COST", "0.5"))}}))
    sys.exit(1 if failed else 0)
elif a[:2] == ["evaluator-audit", "second"]:
    key = json.loads(Path(opt("--key")).read_text())
    if key["target"] == os.environ.get("FAKE_SECOND_FAIL"):
        sys.exit(1)
    out = Path("bench/grading/current/audit/ROUND/second") / Path(key["run"]).name / key["target"] / "assessment-1"
    out.mkdir(parents=True)
    (out / "receipt.json").write_text("{}")
elif a[:2] == ["regrade", "archive-attempt"]:
    pass
else:
    sys.exit(f"unexpected {a}")
'''.replace("PIN", repr(PIN)).replace("ROUND", ROUND)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def build(root, targets):
    (root / "bench/tools").mkdir(parents=True)
    tool = root / "bench/tools/bench-tools"
    tool.write_text(FAKE_TOOL)
    tool.chmod(0o755)
    bin_dir = root / "bin"
    bin_dir.mkdir()
    (bin_dir / "claude").write_text("#!/bin/sh\necho '2.1.314 (Claude Code)'\n")
    (bin_dir / "claude").chmod(0o755)
    current = root / "bench/grading/current"
    (current / "audit" / ROUND).mkdir(parents=True)
    (current / "audits.json").write_text("{}")
    (current / "grades.json").write_text("{}")
    (current / "audit" / ROUND / "sample.json").write_text("{}")
    (root / "receipt.json").write_text("{}")
    plan = {
        "contract": "evaluator-audit-second-pass-plan/v1", "round": ROUND,
        "sample": {"path": f"bench/grading/current/audit/{ROUND}/sample.json",
                   "sha256": sha(current / "audit" / ROUND / "sample.json")},
        "grades": {"path": "bench/grading/current/grades.json", "sha256": sha(current / "grades.json")},
        "recordRoot": f"bench/grading/current/audit/{ROUND}", "queueDirectory": ".local/q",
        "workspaceRoot": ".local/w", "archiveRoot": "archive", "cacheReplacements": {},
        "order": [{"run": "runs/r1", "target": t, "units": 1, "queue": "plain", "cacheReplacements": None}
                  for t in targets],
    }
    (root / "plan.json").write_text(json.dumps(plan))


def authorize(root, cap, reserve=6.0):
    (root / "auth.json").write_text(json.dumps({
        "schemaVersion": 1, "budgetPolicy": "claude-list-price-cap", "budgetCapUsd": cap, "maxBudgetUsd": reserve,
        "budgetReceipt": {"ref": {"path": "receipt.json", "sha256": sha(root / "receipt.json")}},
        "grader": {"client": "claude", "model": "claude-opus-5-5", "effort": "high", "cliVersion": "2.1.314",
                   "timeoutSeconds": 60},
        "plan": {"path": "plan.json", "sha256": sha(root / "plan.json")}, "runtimes": PIN, "quotaStopPercent": 90,
    }))


def run(root, *extra, **env):
    environment = {**os.environ, "PATH": f"{root / 'bin'}:{os.environ['PATH']}", "BENCH_RUNTIME_STORE": "/fake",
                   "FAKE_BUDGETS": str(root / "budgets.txt"),
                   **{k: str(v) for k, v in env.items()}}
    done = subprocess.run([sys.executable, str(DRIVER), "run", "--authorization", "auth.json", *extra], cwd=root,
                          env=environment, capture_output=True, text=True)
    status = json.loads((root / ".local/q/status.json").read_text()) if (root / ".local/q/status.json").exists() else {}
    return done.returncode, status, done.stdout + done.stderr


def saved(root):
    return len(list((root / f"bench/grading/current/audit/{ROUND}/second").glob("*/*/assessment-1/receipt.json")))


def case(name, check):
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        try:
            check(root)
            print(f"ok   {name}")
            return True
        except AssertionError as error:
            print(f"FAIL {name}: {error}")
            return False


def completes(root):
    build(root, [f"t{i}" for i in range(5)])
    authorize(root, 200)
    code, status, out = run(root, "--workers", "2")
    assert code == 0 and status["state"] == "complete" and saved(root) == 5, out
    assert abs(status["spentUpperUsd"] - 2.5) < 1e-9 and status["reservedUsd"] == 0, status
    assert (root / "budgets.txt").read_text().split() == ["6.0"] * 5, "a session ran without the $6 limit"
    assert json.loads((root / ".local/q/client.json").read_text())["version"] == "2.1.314"
    code, status, out = run(root)
    assert code == 0 and status["state"] == "complete" and len(status["invocations"]) == 2, out


def limit_then_resume(root):
    build(root, ["t0", "t1", "t2"])
    authorize(root, 200)
    code, status, out = run(root, "--limit", "1")
    assert code == 0 and status["state"] == "limit-reached" and saved(root) == 1, out
    code, status, out = run(root)
    assert code == 0 and saved(root) == 3, out


def budget_stop(root):
    build(root, [f"t{i}" for i in range(5)])
    authorize(root, 12, reserve=6)
    code, status, out = run(root, FAKE_COST=2)
    assert code == 3 and status["state"] == "budget-stopped", out
    assert saved(root) == 3 and status["spentUpperUsd"] == 6, status


def quota_stop(root):
    build(root, ["t0"])
    authorize(root, 200)
    code, status, out = run(root, FAKE_USED=90)
    assert code == 4 and status["state"] == "quota-stopped" and saved(root) == 0, out
    assert not (root / ".local/q/attempts").exists(), "a batch was prepared after the quota stop"


def failure_stops_and_retries(root):
    build(root, ["t0", "t1", "t2"])
    authorize(root, 200)
    code, status, out = run(root, FAKE_DISPATCH_FAIL="t1")
    assert code == 1 and status["state"] == "failed" and saved(root) == 1, out
    code, status, out = run(root)
    assert code == 1 and "disposition.json" in status["reason"] and saved(root) == 1, out
    (root / ".local/q/attempts/r1/t1/attempt-1/disposition.json").write_text('{"decision": "retry"}')
    code, status, out = run(root)
    assert code == 0 and saved(root) == 3, out
    assert (root / ".local/q/attempts/r1/t1/attempt-2").is_dir()
    assert abs(status["spentUpperUsd"] - 2.0) < 1e-9, status


def resave_after_second_fails(root):
    build(root, ["t0"])
    authorize(root, 200)
    code, status, out = run(root, FAKE_SECOND_FAIL="t0")
    assert code == 1 and saved(root) == 0, out
    code, status, out = run(root)
    assert code == 0 and saved(root) == 1, out
    assert not (root / ".local/q/attempts/r1/t0/attempt-2").exists(), "the batch was dispatched again"
    assert abs(status["spentUpperUsd"] - 0.5) < 1e-9, status


def runs_after_grades_change(root):
    build(root, ["t0", "t1"])
    authorize(root, 200)
    code, status, out = run(root, "--limit", "1")
    assert code == 0 and saved(root) == 1, out
    (root / "bench/grading/current/grades.json").write_text('{"batches": ["a new batch"]}')
    code, status, out = run(root)
    assert code == 0 and status["state"] == "complete" and saved(root) == 2, out


def refuses_changed_runtime(root):
    build(root, ["t0"])
    authorize(root, 200)
    auth = json.loads((root / "auth.json").read_text())
    auth["runtimes"] = {"bench-tools": {"sha256": "c" * 64, "receipt_sha256": "d" * 64}}
    (root / "auth.json").write_text(json.dumps(auth))
    code, _, out = run(root)
    assert code == 2 and "runtime" in out and not (root / ".local/q").exists(), out


if __name__ == "__main__":
    results = [case(f.__name__, f) for f in (completes, limit_then_resume, budget_stop, quota_stop,
                                              failure_stops_and_retries, resave_after_second_fails,
                                              refuses_changed_runtime, runs_after_grades_change)]
    shutil.rmtree(DRIVER.parent / "__pycache__", ignore_errors=True)
    sys.exit(0 if all(results) else 1)
