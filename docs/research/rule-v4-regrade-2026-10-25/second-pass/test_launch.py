#!/usr/bin/env python3
"""Exercise launch.py against a fake second_pass.py and fake Claude meters; no model, no repository writes.

    python3 -I docs/research/rule-v4-regrade-2026-10-25/second-pass/test_launch.py
"""
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

LAUNCHER = Path(__file__).resolve().parent / "launch.py"
ROUND = "evaluator-audit-v2-2026-10-29"

FAKE_TOOL = r'''#!/usr/bin/env python3
import json, math, sys, time
from datetime import datetime, timezone
from pathlib import Path
meters = json.loads(Path("meters.json").read_text())
if sys.argv[1:3] != ["subscription-meters", "read"]:
    sys.exit(f"unexpected {sys.argv[1:]}")
five = meters["five_hour"]
if meters.get("high_five_hour_reads", 0) > 0:
    meters["high_five_hour_reads"] -= 1
    Path("meters.json").write_text(json.dumps(meters))
    five = 95.0
iso = lambda s: datetime.fromtimestamp(s, timezone.utc).isoformat()
print(json.dumps({"claude": {"five_hour": {"utilization": five, "resets_at": iso(time.time() + 1)},
                             "seven_day": {"utilization": float(math.floor(meters["week"])),
                                           "resets_at": iso(meters["week_resets_at"])}}}))
'''

FAKE_DRIVER = r'''import json, os, sys, time
from pathlib import Path
a = sys.argv[1:]
authorization = json.loads(Path(a[a.index("--authorization") + 1]).read_text())
limit = int(a[a.index("--limit") + 1])
plan = json.loads(Path(authorization["plan"]["path"]).read_text())
queue = Path(plan["queueDirectory"])
queue.mkdir(parents=True, exist_ok=True)
status_path = queue / "status.json"
status = json.loads(status_path.read_text()) if status_path.exists() else {"spentUpperUsd": 0.0}
cost, rate = float(os.environ["FAKE_COST"]), float(os.environ["FAKE_RATE"])
fail_target, fail_text = os.environ.get("FAKE_FAIL_TARGET"), os.environ.get("FAKE_FAIL_TEXT", "")
def saved(b):
    return (Path(plan["recordRoot"]) / "second" / Path(b["run"]).name / b["target"] / "assessment-1/receipt.json").exists()
def finish(state, code, reason=None):
    status.update({"state": state, "reason": reason, "saved": sum(map(saved, plan["order"])),
                   "planned": len(plan["order"]), "budgetCapUsd": authorization["budgetCapUsd"]})
    status_path.write_text(json.dumps(status))
    sys.exit(code)
launched = 0
for batch in plan["order"]:
    if saved(batch):
        continue
    folder = queue / "attempts" / Path(batch["run"]).name / batch["target"]
    attempts = sorted(folder.glob("attempt-*")) if folder.is_dir() else []
    if attempts and not (attempts[-1] / "disposition.json").exists():
        finish("failed", 1, f"{batch['target']}: unresolved")
    if launched >= limit:
        break
    attempt = folder / f"attempt-{len(attempts) + 1}"
    (attempt / "work").mkdir(parents=True)
    (attempt / "reservation.json").write_text(json.dumps({"at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}))
    launched += 1
    meters = json.loads(Path("meters.json").read_text())
    meters["week"] += cost * rate
    Path("meters.json").write_text(json.dumps(meters))
    status["spentUpperUsd"] += cost
    if batch["target"] == fail_target and len(attempts) == 0:
        (attempt / "work/dispatch.json").write_text("{}")
        (attempt / "work/stdout.txt").write_text(fail_text)
        finish("failed", 1, f"{batch['target']}: receipt-needs-investigation")
    out = Path(plan["recordRoot"]) / "second" / Path(batch["run"]).name / batch["target"] / "assessment-1"
    out.mkdir(parents=True)
    (out / "receipt.json").write_text("{}")
left = [b for b in plan["order"] if not saved(b)]
finish("complete" if not left else "limit-reached", 0)
'''


def build(root, batches, cap=275.0, week=2.0):
    (root / "bench/tools").mkdir(parents=True)
    (root / "bench/grading/current").mkdir(parents=True)
    (root / "bench/grading/current/audits.json").write_text("{}")
    tool = root / "bench/tools/bench-tools"
    tool.write_text(FAKE_TOOL)
    tool.chmod(0o755)
    (root / "driver.py").write_text(FAKE_DRIVER)
    (root / "home/.claude").mkdir(parents=True)
    (root / "home/.claude/.credentials.json").write_text(json.dumps(
        {"claudeAiOauth": {"expiresAt": (time.time() + 8 * 3600) * 1000}}))
    (root / "bin").mkdir()
    (root / "bin/claude").write_text("#!/bin/sh\necho '2.1.314 (Claude Code)'\n")
    (root / "bin/claude").chmod(0o755)
    (root / "meters.json").write_text(json.dumps({"five_hour": 3.0, "week": week,
                                                  "week_resets_at": time.time() + 7 * 86400}))
    (root / "plan.json").write_text(json.dumps({
        "recordRoot": f"bench/grading/current/audit/{ROUND}", "queueDirectory": ".local/second-pass-v4/queue",
        "order": [{"run": "runs/r1", "target": f"t{i:03}", "firstGradeUsd": 1.0} for i in range(batches)]}))
    (root / "auth.json").write_text(json.dumps({
        "plan": {"path": "plan.json"}, "budgetCapUsd": cap, "quotaStopPercent": 90,
        "grader": {"cliVersion": "2.1.314"}}))


def launch(root, *extra, cost=1.0, rate=0.05, **env):
    environment = {**os.environ, "PATH": f"{root / 'bin'}:{os.environ['PATH']}", "HOME": str(root / "home"),
                   "BENCH_RUNTIME_STORE": "/fake", "SECOND_PASS_DRIVER": str(root / "driver.py"),
                   "SECOND_PASS_LAUNCH_SLEEP_SCALE": "0", "FAKE_COST": str(cost), "FAKE_RATE": str(rate),
                   **{k: str(v) for k, v in env.items()}}
    done = subprocess.run([sys.executable, "-I", str(LAUNCHER), "run", "--authorization", "auth.json", *extra],
                          cwd=root, env=environment, capture_output=True, text=True, timeout=120)
    state_path = root / ".local/second-pass-v4/launcher.json"
    state = json.loads(state_path.read_text()) if state_path.exists() else {}
    return done.returncode, state, done.stdout + done.stderr


def saved(root):
    return len(list((root / f"bench/grading/current/audit/{ROUND}/second").glob("*/*/assessment-1/receipt.json")))


def low_rate_runs_the_full_pass(root):
    build(root, 40)
    code, state, out = launch(root, rate=0.05)
    assert code == 0 and state["phase"] == "finished" and saved(root) == 40, out
    assert len(state["decisions"]) == 1 and state["decisions"][0]["batches"] == 5, state
    assert state["decisions"][0]["projectedCostUsd"] == 40.0, state


def high_rate_stops_after_the_trial(root):
    build(root, 40)
    code, state, out = launch(root, rate=2.0)
    assert code == 0 and state["phase"] == "stopped" and saved(root) == 5, out
    assert "lowest rate" in state["stopped"]["reason"], state
    code, again, out = launch(root, rate=2.0)
    assert code == 0 and saved(root) == 5 and "Nothing to do" in out, out


def unsettled_trial_measures_more(root):
    build(root, 170)
    code, state, out = launch(root, rate=0.1)
    assert code == 0 and state["phase"] == "finished" and saved(root) == 170, out
    batches = [d["batches"] for d in state["decisions"]]
    assert batches[0] == 5 and len(batches) >= 2 and batches == sorted(batches), batches
    assert state["decisions"][-1]["projectedPointsHigh"] < 30 <= state["decisions"][0]["projectedPointsHigh"], state


def projected_cost_over_cap_stops(root):
    build(root, 40, cap=30)
    code, state, out = launch(root)
    assert code == 0 and state["phase"] == "stopped" and "cap" in state["stopped"]["reason"] and saved(root) == 5, out


def waits_out_the_five_hour_window(root):
    build(root, 10)
    meters = json.loads((root / "meters.json").read_text())
    meters["high_five_hour_reads"] = 1
    (root / "meters.json").write_text(json.dumps(meters))
    code, state, out = launch(root)
    assert code == 0 and state["phase"] == "finished" and "five-hour window is at 95%" in out, out


def retries_a_passing_failure(root):
    build(root, 10)
    code, state, out = launch(root, FAKE_FAIL_TARGET="t002", FAKE_FAIL_TEXT="API Error: 529 Overloaded")
    assert code == 0 and state["phase"] == "finished" and saved(root) == 10, out
    disposition = json.loads((root / ".local/second-pass-v4/queue/attempts/r1/t002/attempt-1/disposition.json").read_text())
    assert disposition["kind"] == "overloaded" and state["retries"] == {"runs/r1/t002": 1}, state


def stops_on_an_unknown_failure(root):
    build(root, 10)
    code, state, out = launch(root, FAKE_FAIL_TARGET="t002", FAKE_FAIL_TEXT="the verdicts did not validate")
    assert code == 0 and state["phase"] == "stopped" and "not known to pass" in state["stopped"]["reason"], out
    assert not (root / ".local/second-pass-v4/queue/attempts/r1/t002/attempt-1/disposition.json").exists()


def stops_at_the_weekly_quota(root):
    build(root, 10, week=90.0)
    code, state, out = launch(root)
    assert code == 0 and state["phase"] == "stopped" and "90%" in state["stopped"]["reason"] and saved(root) == 0, out


def does_nothing_before_its_start(root):
    build(root, 10)
    code, state, out = launch(root, "--not-before", "2099-01-01T00:00:00Z")
    assert code == 0 and saved(root) == 0 and not state, out


def waits_for_a_renewed_sign_in(root):
    build(root, 6)
    credentials = root / "home/.claude/.credentials.json"
    credentials.write_text(json.dumps({"claudeAiOauth": {"expiresAt": (time.time() + 600) * 1000}}))
    renew = subprocess.Popen([sys.executable, "-c", "import json, sys, time; time.sleep(1); "
                              "open(sys.argv[1], 'w').write(json.dumps({'claudeAiOauth': {'expiresAt': (time.time() + 8 * 3600) * 1000}}))",
                              str(credentials)])
    code, state, out = launch(root)
    renew.wait()
    assert code == 0 and state["phase"] == "finished" and "Only a person renews it" in out and "was renewed" in out, out


def case(name, check):
    with tempfile.TemporaryDirectory() as directory:
        try:
            check(Path(directory))
            print(f"ok   {name}")
            return True
        except AssertionError as error:
            print(f"FAIL {name}: {error}")
            return False


if __name__ == "__main__":
    results = [case(f.__name__, f) for f in (low_rate_runs_the_full_pass, high_rate_stops_after_the_trial,
                                              unsettled_trial_measures_more, projected_cost_over_cap_stops,
                                              waits_out_the_five_hour_window, retries_a_passing_failure,
                                              stops_on_an_unknown_failure, stops_at_the_weekly_quota,
                                              does_nothing_before_its_start, waits_for_a_renewed_sign_in)]
    shutil.rmtree(LAUNCHER.parent / "__pycache__", ignore_errors=True)
    sys.exit(0 if all(results) else 1)
