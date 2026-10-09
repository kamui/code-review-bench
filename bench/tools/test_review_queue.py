"""Prove the serial queue recovers without a duplicate review or a lost reservation, with a fake client and local process failures.

The runner is the real ``run_cell.py`` with its provisioning, its price check and the reviewer replaced, so the claims, the
caps, the filing and the cleanup are the real ones; no reviewer is called. Run as a script with ``cell`` or ``--queue`` in
its arguments, this file is that fake runner or the controller that launches it.
"""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
import unittest
from unittest.mock import patch

import prune_workspace
import review_queue
import run_cell
import test_interrupted_attempts as fixtures

RUN_ID = fixtures.RUN_ID
CAPTURED = fixtures.CAPTURED
FAKE = [sys.executable, str(Path(__file__).resolve())]
# The fixture replaces the freeze-commit comparison; one test exercises the real one.
RUNTIME_PROBLEM = review_queue.runtime_problem


# --- the fake runner and controller, in a process of their own ----------------------------------

def plan_root() -> Path:
    return Path(os.environ["FAKE_QUEUE_ROOT"])


def take(key: str):
    """Use up one instruction of the fixture's plan, so that it applies to one dispatch only."""
    path = plan_root() / "plan.json"
    plan = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}
    value = plan.pop(key, None)
    if value is not None:
        path.write_text(json.dumps(plan), encoding="utf-8")
    return value


def crash(point: str, attempt: Path | None = None) -> None:
    """Kill the controller and this runner at one point of a dispatch, as losing the host's session would."""
    path = plan_root() / "plan.json"
    plan = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}
    if plan.get("kill") != point:
        return
    take("kill")
    if take("survivor"):
        # A reviewer child in a session of its own: it outlives the runner and keeps the launch's environment.
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(300)"], cwd=attempt, start_new_session=True,
                                 stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        (plan_root() / "survivor.pid").write_text(str(child.pid), encoding="utf-8")
    (plan_root() / "crashed.pid").write_text(str(os.getpid()), encoding="utf-8")
    if os.environ.get("FAKE_CONTROLLER") == str(os.getppid()):
        os.kill(os.getppid(), signal.SIGKILL)
    os.kill(os.getpid(), signal.SIGKILL)


def review(argv: list) -> subprocess.CompletedProcess:
    """What ``dispatch.sh`` leaves for a built-in Claude review, written in its order."""
    attempt = Path(argv[2])
    with (plan_root() / "reviews.log").open("a", encoding="utf-8") as log:
        log.write(attempt.name + "\n")
    subagents = attempt / "home/.claude/projects/p/s1/subagents"
    subagents.mkdir(parents=True)
    (attempt / "home/.claude/projects/p/s1.jsonl").write_text("", encoding="utf-8")
    (attempt / "home/.claude/.credentials.json").write_text("{}", encoding="utf-8")
    (attempt / "tree-before.txt").write_text("abc\n", encoding="utf-8")
    (attempt / "dispatch.txt").write_text("claude 9.9.9 (Claude Code)\nmodel=m-1 effort=high\n", encoding="utf-8")
    timing = {"root_dispatched_at": "2026-01-01T00:00:00Z", "payload_validated_at": None, "completed_at": None}
    (attempt / "timing.json").write_text(json.dumps(timing), encoding="utf-8")
    (subagents / "agent-a1.jsonl").write_text(fixtures.claude_transcript("tool_use"), encoding="utf-8")
    (attempt / "stdout.jsonl").write_text(fixtures.lines({"type": "system"}), encoding="utf-8")
    crash("during_review", attempt)
    if take("hold"):
        while not (plan_root() / "release").exists():
            time.sleep(0.02)
    stopped = take("stop_next")
    code = 1 if stopped else 0
    (subagents / "agent-a1.jsonl").write_text(fixtures.claude_transcript(), encoding="utf-8")
    result = {"type": "result", "subtype": "error_during_execution", "is_error": True} if stopped else {"type": "result", "subtype": "success"}
    (attempt / "stdout.jsonl").write_text(fixtures.lines({"type": "system"}, result), encoding="utf-8")
    (attempt / "native-return.json").write_text(json.dumps({"returned_at": "2026-01-01T00:00:04Z", "exit_code": code}), encoding="utf-8")
    crash("after_native_return")
    (attempt / "tree-after.txt").write_text("abc\n", encoding="utf-8")
    (attempt / "audit.json").write_text(json.dumps({"violations": [], "guidance_probes": [], "network_commands": [],
                                                    "diff_commands": ["git diff main...review-head"]}), encoding="utf-8")
    (attempt / "normalized.json").write_text(json.dumps({"parse_status": "parsed", "items": []}), encoding="utf-8")
    (attempt / "payload.json").write_text("[]", encoding="utf-8")
    if stopped:
        (attempt / "stop.json").write_text(json.dumps({"stopped_at": "2026-01-01T00:00:06Z", "exit_code": 1, "reason": stopped}),
                                           encoding="utf-8")
    else:
        (attempt / "timing.json").write_text(json.dumps({**timing, "completed_at": "2026-01-01T00:00:05Z"}), encoding="utf-8")
    (attempt / "home/.claude/.credentials.json").unlink()
    with (attempt / "dispatch.txt").open("a", encoding="utf-8") as dispatched:
        dispatched.write(f"exit={code}\n")
    return subprocess.CompletedProcess(argv, code, "", "")


def fake_cell(argv: list) -> int:
    real = {name: getattr(run_cell, name) for name in ("tool", "choose", "dispatch", "prune")}
    real_remove = prune_workspace.remove

    def tool(command: list, env=None) -> subprocess.CompletedProcess:
        name = Path(command[1] if command[0] == sys.executable else command[0]).name
        if name == "provision.py":
            shutil.copytree(plan_root() / "source/clone", command[command.index("--out") + 1], symlinks=True)
            return subprocess.CompletedProcess(command, 0, "{}", "")
        if name == "dispatch.sh":
            return review(command)
        done = real["tool"](command, env)
        if name == "file_attempt.py":
            crash("after_filing")
        return done

    def frozen(run) -> None:
        if not run.manifest.get("frozen_at"):
            raise run_cell.Refused("the manifest has no frozen_at")

    def choose(run, args):
        crash("before_claim")
        return real["choose"](run, args)

    def dispatch(run, attempt_id, claim):
        crash("after_claim")
        return real["dispatch"](run, attempt_id, claim)

    def prune(run, attempt_id, target_id):
        crash("before_cleanup")
        return real["prune"](run, attempt_id, target_id)

    def remove(path) -> None:
        real_remove(path)
        crash("during_cleanup")

    run_cell.BENCH = plan_root() / "bench"
    run_cell.tool, run_cell.choose, run_cell.dispatch, run_cell.prune, run_cell.check_frozen = tool, choose, dispatch, prune, frozen
    run_cell.check_target = lambda run, target_id: run.target_dir(target_id)
    run_cell.check_dispatch_rates = lambda run, cell: ({"policy": "fixture"}, {"rates": [fixtures.RATE]})
    prune_workspace.remove = remove
    with patch.object(sys, "argv", ["run_cell.py", *argv]):
        return run_cell.main()


def faked() -> dict:
    """What a controller under test replaces: the runner it launches, the meters, and the freeze-commit comparison."""
    return {"RUN_CELL": [*FAKE, "cell"], "POLL_SECONDS": 0.02, "SCAN_GAP_SECONDS": 0.01,
            "runtime_problem": lambda run: None,
            "meters": lambda queue, env: json.loads((plan_root() / "meters.json").read_text(encoding="utf-8"))}


def fake_queue(argv: list) -> int:
    real_write = review_queue.write

    def write(path, value) -> None:
        real_write(path, value)
        if isinstance(value, dict) and value.get("state") == "intent":
            crash("before_spawn")

    run_cell.BENCH = plan_root() / "bench"
    review_queue.write = write
    for name, value in faked().items():
        setattr(review_queue, name, value)
    os.environ["FAKE_CONTROLLER"] = str(os.getpid())
    return review_queue.main(argv)


# --- the fixture --------------------------------------------------------------------------------

class QueueFixture(fixtures.Fixture):
    """A frozen two-cell run of one Claude arm, a queue file for it, and state and work directories under the fixture root."""

    def setUp(self):
        super().setUp()
        (self.root / "source").mkdir()
        self.clone(self.root / "source")
        target = json.loads((self.run_dir / "fixture/target.json").read_text(encoding="utf-8"))
        target["provisioning"] = {"allowance": "Run the tests in <clone>.", "unavailable": "The network."}
        (self.run_dir / "fixture/target.json").write_text(json.dumps(target), encoding="utf-8")
        (self.run_dir / "fixture/packet.md").write_text("# Packet\n", encoding="utf-8")
        cells = [{"target": "t-fixture", "arm": "arm-claude", "replicate": replicate} for replicate in (1, 2)]
        self.manifest.update(
            arms=[{"id": "arm-claude", "expected_cli_version": "9.9.9", "resolved_skill_tree": None, "billing_mode": "subscription"}],
            planned_cells=cells, sealed_order=[run_cell.cell_key(cell) for cell in cells],
            execution_policy={"allowance": "Five minutes per command.", "branch_layout": "`main` is the merge-base."},
            caps={**self.manifest["caps"], "max_attempts": 6})
        (self.run_dir / "manifest.json").write_text(json.dumps(self.manifest), encoding="utf-8")
        client = self.root / "fake-claude"
        client.write_text("#!/bin/sh\n", encoding="utf-8")
        self.spec = {"queue_id": "fixture", "runs": [str(self.run_dir)],
                     "clients": {"claude": {"path": str(client), "sha256": run_cell.sha256_file(client), "version": "9.9.9 (Claude Code)"}},
                     "ceilings_usd": {"claude": 40.0}, "environment": {"BENCH_ARCHIVE_ROOT": str(self.root / "archive")},
                     "work_root": str(self.root / "work"), "state_dir": str(self.root / "state")}
        self.queue = self.root / "queue.json"
        self.queue.write_text(json.dumps(self.spec), encoding="utf-8")
        self.state = self.root / "state"
        self.meters(10)
        self.enterContext(patch.dict(os.environ, {"FAKE_QUEUE_ROOT": str(self.root)}))
        os.environ.pop("FAKE_CONTROLLER", None)
        for name, value in faked().items():
            self.enterContext(patch.object(review_queue, name, value))
        self.addCleanup(self.reap)

    def reap(self) -> None:
        """End what this fixture's own fake processes left running."""
        for name in ("survivor.pid", "crashed.pid"):
            if (self.root / name).is_file():
                self.end(int((self.root / name).read_text(encoding="utf-8")))

    def end(self, pid: int) -> None:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        self.gone(pid)

    def gone(self, pid: int) -> None:
        deadline = time.monotonic() + 30
        while (review_queue.stat_of(pid) or {"state": "X"})["state"] not in "ZX":
            self.assertLess(time.monotonic(), deadline, f"process {pid} never ended")
            time.sleep(0.01)

    def meters(self, percent: float) -> None:
        (self.root / "meters.json").write_text(json.dumps({
            "at": "2026-01-01T00:00:00Z", "claude": {"five_hour": {"utilization": percent}, "seven_day": {"utilization": 20}}}),
            encoding="utf-8")

    def plan(self, **plan) -> None:
        (self.root / "plan.json").write_text(json.dumps(plan), encoding="utf-8")

    def controller(self, *args: str) -> subprocess.CompletedProcess:
        """The controller in a process of its own, which a planned crash can kill."""
        return subprocess.run([*FAKE, *args, "--queue", str(self.queue)], capture_output=True, text=True, encoding="utf-8")

    def command(self, *args: str) -> tuple:
        """The controller in this process: its exit code and the JSON lines it printed."""
        with redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()) as err:
            code = review_queue.main([*args, "--queue", str(self.queue)])
        return code, self.events(out.getvalue()), err.getvalue()

    @staticmethod
    def events(text: str) -> list:
        """The JSON lines a controller printed, or the one indented report of ``check`` and ``status``."""
        try:
            return [json.loads(text)]
        except ValueError:
            return [json.loads(line) for line in text.splitlines() if line.startswith("{")]

    def crashed(self, point: str, **plan) -> None:
        self.plan(kill=point, **plan)
        done = self.controller("run")
        self.assertEqual(done.returncode, -signal.SIGKILL, done.stdout + done.stderr)
        self.gone(int((self.root / "crashed.pid").read_text(encoding="utf-8")))

    def launch_records(self) -> list:
        return [json.loads(path.read_text(encoding="utf-8")) for path in sorted((self.state / "launches").glob("launch-*.json"))]

    def reviews(self) -> list:
        log = self.root / "reviews.log"
        return log.read_text(encoding="utf-8").split() if log.is_file() else []

    def second_run(self) -> str:
        """Queue a copy of the fixture run behind it; call before the queue is first run, which pins it."""
        other = "2026-01-02-second"
        shutil.copytree(self.run_dir, self.run_dir.parent / other)
        (self.run_dir.parent / other / "manifest.json").write_text(json.dumps({**self.manifest, "run_id": other}), encoding="utf-8")
        self.queue.write_text(json.dumps({**self.spec, "runs": [str(self.run_dir), str(self.run_dir.parent / other)]}), encoding="utf-8")
        return other

    def diagnose(self, attempt_id: str, cause: str, reason: str = "The host session ended during the review.") -> Path:
        path = self.run_dir / "deviations" / f"{attempt_id}-{cause}.v1.json"
        path.parent.mkdir(exist_ok=True)
        path.write_text(json.dumps({"version": 1, "predecessor": attempt_id, "cause": cause, "reason": reason}), encoding="utf-8")
        return path

    def assert_blocked(self, result: tuple, needle: str) -> None:
        code, events, err = result
        blocked = [reason for event in events for reason in event.get("blocked", [])]
        self.assertEqual(code, 1, (events, err))
        self.assertTrue(any(needle in reason for reason in blocked), blocked)

    def assert_complete(self, attempts: list, reviews: list) -> None:
        """Every planned cell has one valid review, each rerun names its predecessor, and no reviewer ran unrecorded."""
        code, events, err = self.command("run")
        self.assertEqual((code, events[-1].get("completed")), (0, True), (events, err))
        self.assertFalse(any("launched" in event for event in events), events)
        state = self.run_state()
        self.assertEqual((state.attempt_ids(), state.in_flight(), self.reviews()), (attempts, [], reviews))
        valid = [run_cell.cell_key(record["cell"]) for record in state.filed.values() if record["disposition"] == "valid completed"]
        self.assertEqual(sorted(valid), sorted(self.manifest["sealed_order"]))
        seen = set()
        for attempt_id in attempts:
            key = run_cell.cell_key(state.filed[attempt_id]["cell"])
            self.assertEqual(bool(state.filed[attempt_id].get("predecessor")), key in seen, attempt_id)
            seen.add(key)
        self.assertEqual(review_queue.unpruned(state), [])
        self.assertEqual([record["state"] for record in self.launch_records() if record["state"] in ("intent", "spawned")], [])


# --- killed dispatches --------------------------------------------------------------------------

class KilledDispatch(QueueFixture):
    def test_killed_before_the_runner_starts_leaves_an_intent_that_the_restart_closes(self):
        self.crashed("before_spawn")
        intent = self.launch_records()[0]
        self.assertEqual((intent["state"], intent["process"]["pid"], self.run_state().attempt_ids()), ("intent", None, []))
        self.assertEqual(json.loads(UncertainExecution.status(self))["open_launches"][0]["process"]["status"], "absent")
        restarted = self.controller("run")
        self.assertEqual(restarted.returncode, 0, restarted.stdout + restarted.stderr)
        self.assertEqual(self.launch_records()[0]["outcome"], "no claim was made")
        self.assert_complete(["att-001", "att-002"], ["att-001", "att-002"])

    def test_killed_before_the_claim_leaves_nothing_to_file_and_the_cell_runs_once(self):
        self.crashed("before_claim")
        self.assertEqual((self.run_state().attempt_ids(), self.launch_records()[0]["state"]), ([], "spawned"))
        restarted = self.controller("run")
        self.assertEqual(restarted.returncode, 0, restarted.stdout + restarted.stderr)
        self.assertEqual(self.launch_records()[0]["outcome"], "no claim was made")
        self.assert_complete(["att-001", "att-002"], ["att-001", "att-002"])

    def test_killed_after_the_claim_releases_it_and_keeps_no_reservation_for_a_reviewer_that_never_started(self):
        self.crashed("after_claim")
        state = self.run_state()
        self.assertEqual((state.in_flight(), state.spend()["in_flight_reserved"], self.reviews()), (["att-001"], 5.0, []))
        restarted = self.controller("run")
        self.assertEqual(restarted.returncode, 0, restarted.stdout + restarted.stderr)
        closed = self.launch_records()[0]
        self.assertEqual((closed["state"], closed["outcome"]), ("recovered", "released: no reviewer had started"))
        saved = json.loads(next((self.state / "recoveries").glob("*.json")).read_text(encoding="utf-8"))
        self.assertEqual((saved["status"], saved["claim"]["cell"]["replicate"]), ("absent", 1))
        self.assert_complete(["att-001", "att-002"], ["att-001", "att-002"])

    def interrupted(self, point: str) -> dict:
        """A review killed after its reviewer started: filed once as stopped, never rerun without a diagnosis."""
        self.crashed(point)
        state = self.run_state()
        self.assertEqual((state.in_flight(), state.spend()["in_flight_reserved"]), (["att-001"], 5.0))
        self.assertTrue((self.work / "att-001/home/.claude/.credentials.json").exists())
        record_path = self.run_dir / "attempts/att-001/attempt.json"
        self.assert_blocked(self.command("run"), "has no diagnosis in deviations/")
        first = (record_path.read_bytes(), record_path.stat().st_mtime_ns)
        self.assert_blocked(self.command("run"), "has no diagnosis in deviations/")
        code, events, err = self.command("recover")
        self.assertEqual((code, events[-1]["settled"], len(events[-1]["waiting_on_diagnosis_or_replacement"])), (0, True, 1), (events, err))
        self.assertEqual((record_path.read_bytes(), record_path.stat().st_mtime_ns), first)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        self.assertTrue(record["disposition"].startswith("stopped: the controller stopped and the dispatch never returned"))
        closed = json.loads((self.run_dir / "attempts/att-001/interruption.json").read_text(encoding="utf-8"))
        self.assertEqual(closed["process_stop"]["status"], "absent")
        self.assertIn("twice", closed["process_stop"]["verified_by"])
        self.assertNotIn("exit=", (self.work / "att-001/dispatch.txt").read_text(encoding="utf-8"))
        self.assertFalse((self.work / "att-001/home/.claude/.credentials.json").exists())
        self.assertTrue((self.work / "att-001/clone").is_dir())
        self.assertEqual((self.run_state().attempt_ids(), self.reviews()), (["att-001"], ["att-001"]))
        self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-001"), "has no diagnosis in deviations/")
        diagnosis = self.diagnose("att-001", "harness-stop")
        self.assert_blocked(self.command("run"), "diagnosed as harness-stop; replace it before the queue goes on")
        code, events, err = self.command("replace", "--run", RUN_ID, "--attempt", "att-001")
        self.assertEqual((code, events[-1]["disposition"]), (0, "valid completed"), (events, err))
        claim = json.loads((self.work / "att-002/cell.json").read_text(encoding="utf-8"))
        self.assertEqual((claim["predecessor"], claim["cell"], claim["reserved_usd"], claim["kind"]),
                         ("att-001", record["cell"], 5.0, "replacement 1 of 2"))
        self.assertIn(f"sha256 {run_cell.sha256_file(diagnosis)}", claim["retry_reason"])
        code, events, err = self.command("run")
        self.assertEqual((code, [event["run"] for event in events if "launched" in event]), (0, [RUN_ID]), (events, err))
        self.assert_complete(["att-001", "att-002", "att-003"], ["att-001", "att-002", "att-003"])
        return record

    def test_killed_during_the_review_keeps_the_whole_reservation_of_its_unknown_usage(self):
        record = self.interrupted("during_review")
        fixtures.Fixture.assert_unknown_total(self, record, CAPTURED)
        self.assertEqual(self.run_state().spend()["attempts"], round(5.0 + 2 * CAPTURED, 4))
        self.assertEqual(review_queue.spend(review_queue.Queue(self.queue), [self.run_state()])["claude"]["unknown_reserved"], 5.0)

    def test_killed_after_the_native_return_keeps_the_usage_the_return_proves(self):
        record = self.interrupted("after_native_return")
        self.assertEqual((record["usage"]["metering_status"], record["usage"]["priced_total_usd"]), ("complete", CAPTURED))
        self.assertEqual(record["timing"]["stopped_at"], "2026-01-01T00:00:04Z")

    def filed_before_cleanup(self, point: str, clone_left: bool) -> None:
        """A review killed once its record was filed: the record is reused and only its cleanup is finished."""
        self.crashed(point)
        record_path = self.run_dir / "attempts/att-001/attempt.json"
        before = (record_path.read_bytes(), record_path.stat().st_mtime_ns)
        self.assertEqual(((self.work / "att-001/clone").exists(), (self.work / "att-001/workspace-pruned.json").exists()),
                         (clone_left, False))
        restarted = self.controller("run")
        self.assertEqual(restarted.returncode, 0, restarted.stdout + restarted.stderr)
        self.assertEqual((record_path.read_bytes(), record_path.stat().st_mtime_ns), before)
        self.assertEqual(self.launch_records()[0]["outcome"], "filed before the controller stopped")
        self.assertTrue(json.loads((self.work / "att-001/workspace-pruned.json").read_text(encoding="utf-8"))["applied"])
        self.assert_complete(["att-001", "att-002"], ["att-001", "att-002"])

    def test_killed_after_filing_reuses_the_record_and_finishes_its_cleanup(self):
        self.filed_before_cleanup("after_filing", clone_left=True)

    def test_killed_before_cleanup_reuses_the_record_and_finishes_its_cleanup(self):
        self.filed_before_cleanup("before_cleanup", clone_left=True)

    def test_killed_between_removing_the_clone_and_its_receipt_writes_the_receipt(self):
        self.filed_before_cleanup("during_cleanup", clone_left=False)


class UncertainExecution(QueueFixture):
    def test_a_surviving_reviewer_child_blocks_every_restart_and_is_never_signalled(self):
        self.crashed("during_review", survivor=True)
        survivor = int((self.root / "survivor.pid").read_text(encoding="utf-8"))
        with patch.object(os, "kill") as kill, patch.object(os, "killpg") as killpg:
            for action in ("run", "recover", "run"):
                self.assert_blocked(self.command(action), "is claimed and not filed, and its processes are running")
            status = json.loads(self.status())
        self.assertEqual((kill.call_count, killpg.call_count), (0, 0))
        claim = status["unfiled_claims"][0]
        self.assertEqual((claim["attempt"], claim["process"]["status"], claim["process"]["wrapper"]), ("att-001", "running", "gone"))
        self.assertEqual([row["pid"] for row in claim["process"]["processes"]], [survivor])
        self.assertEqual(review_queue.stat_of(survivor)["state"] in "ZX", False)
        state = self.run_state()
        self.assertEqual((state.in_flight(), state.filed, state.spend()["in_flight_reserved"], self.reviews()),
                         (["att-001"], {}, 5.0, ["att-001"]))
        self.end(survivor)
        self.assert_blocked(self.command("run"), "has no diagnosis in deviations/")
        self.assertEqual(self.run_state().filed["att-001"]["disposition"].split(":")[0], "stopped")

    def status(self) -> str:
        with redirect_stdout(io.StringIO()) as out:
            self.assertEqual(review_queue.main(["status", "--queue", str(self.queue)]), 0)
        return out.getvalue()

    def test_a_claim_whose_launch_record_is_missing_is_never_filed_by_the_queue(self):
        self.crashed("during_review")
        (self.state / "launches/launch-0001.json").unlink()
        for action in ("run", "recover", "run"):
            self.assert_blocked(self.command(action), "its processes are unknown: no launch record names this claim")
        state = self.run_state()
        self.assertEqual((state.in_flight(), state.filed, state.spend()["in_flight_reserved"]), (["att-001"], {}, 5.0))
        done = self.runner("--file", "att-001", "--interrupted", str(self.evidence()), "--reason", "host restarted")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assert_blocked(self.command("run"), "has no diagnosis in deviations/")
        self.assertEqual(self.reviews(), ["att-001"])

    def test_a_launch_this_process_cannot_see_into_is_unknown_and_blocks(self):
        self.crashed("during_review")
        here = review_queue.identity()
        for hidden, needle in (({"pid_namespace": "pid:[1]"}, "which this process cannot see into"),
                               ({"uid": here["uid"] + 1}, "cannot inspect"),
                               ({"host": "another-host"}, "observe it there")):
            with self.subTest(hidden=hidden), patch.object(review_queue, "identity", return_value={**here, **hidden}):
                self.assert_blocked(self.command("recover"), needle)
                self.assertEqual((self.run_state().in_flight(), self.run_state().filed), (["att-001"], {}))
        code, events, err = self.command("recover")
        self.assertEqual((code, events[-1]["settled"]), (0, True), (events, err))
        self.assertEqual(list(self.run_state().filed), ["att-001"])

    def test_a_reused_pid_is_another_process_and_is_left_alone(self):
        self.crashed("during_review")
        stranger = subprocess.Popen(["sleep", "300"], start_new_session=True)
        self.addCleanup(stranger.wait)
        self.addCleanup(stranger.kill)
        path = self.state / "launches/launch-0001.json"
        record = json.loads(path.read_text(encoding="utf-8"))
        self.assertNotEqual(record["process"]["start_ticks"], review_queue.stat_of(stranger.pid)["start_ticks"])
        record["process"].update(pid=stranger.pid, session=stranger.pid)
        path.write_text(json.dumps(record), encoding="utf-8")
        with patch.object(os, "kill") as kill, patch.object(os, "killpg") as killpg:
            code, events, err = self.command("recover")
        self.assertEqual((code, kill.call_count, killpg.call_count), (0, 0, 0), (events, err))
        self.assertIsNone(stranger.poll())
        seen = self.launch_records()[0]["observation"]
        self.assertEqual((seen["status"], seen["wrapper"], seen["processes"]), ("absent", "reused by another process", []))
        self.assertEqual(self.run_state().filed["att-001"]["disposition"].split(":")[0], "stopped")

    def test_check_reports_a_recoverable_claim_and_changes_nothing(self):
        code, events, err = self.command("check")
        self.assertEqual((code, events[0]["ready"], events[0]["blocked"]), (0, True, []), err)
        self.assertFalse((self.state / "queue.pin.json").exists())
        self.assertEqual(self.run_state().attempt_ids(), [])
        self.crashed("during_review")
        code, (report,), err = self.command("check")
        self.assertEqual((code, report["ready"]), (1, False), err)
        self.assertIn("its processes are gone, so recover or run settles it", report["blocked"][0])
        self.assertEqual((self.run_state().in_flight(), self.launch_records()[0]["state"]), (["att-001"], "spawned"))


class ProcessObservation(unittest.TestCase):
    def setUp(self):
        self.enterContext(patch.object(review_queue, "SCAN_GAP_SECONDS", 0.01))

    def launched(self, argv: list) -> tuple:
        child = subprocess.Popen(argv, start_new_session=True, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE)
        self.addCleanup(child.wait)
        self.addCleanup(child.stdout.close)
        return child, {**review_queue.identity(), **review_queue.started(child.pid), "command": argv, "session": child.pid, "marker": None}

    def test_a_zombie_wrapper_does_not_hide_the_child_it_left_running(self):
        wrapper, evidence = self.launched(["sh", "-c", "sleep 300 & echo $!"])
        sleeper = int(wrapper.stdout.readline())
        self.addCleanup(lambda: review_queue.stat_of(sleeper) and os.kill(sleeper, signal.SIGKILL))
        deadline = time.monotonic() + 30
        while review_queue.stat_of(wrapper.pid)["state"] != "Z":
            self.assertLess(time.monotonic(), deadline, "the wrapper never exited")
            time.sleep(0.01)
        seen = review_queue.observe(evidence)
        self.assertEqual((seen["status"], seen["wrapper"]), ("running", "a zombie"))
        self.assertEqual({row["pid"]: row["matched_by"] for row in seen["processes"]},
                         {wrapper.pid: "the recorded PID and start time", sleeper: f"session {wrapper.pid}"})
        os.kill(sleeper, signal.SIGKILL)
        deadline = time.monotonic() + 30
        while (review_queue.stat_of(sleeper) or {"state": "X"})["state"] not in "ZX":
            self.assertLess(time.monotonic(), deadline, "the sleeper never ended")
            time.sleep(0.01)
        seen = review_queue.observe(evidence)
        self.assertEqual((seen["status"], seen["wrapper"]), ("absent", "a zombie"))

    def test_a_live_wrapper_is_running_with_its_expected_command(self):
        wrapper, evidence = self.launched(["sleep", "300"])
        self.addCleanup(wrapper.kill)
        seen = review_queue.observe(evidence)
        self.assertEqual((seen["status"], seen["processes"][0]["is_expected_command"], seen["processes"][0]["command"]),
                         ("running", True, "sleep 300"))
        self.assertEqual(review_queue.observe({**evidence, "command": ["sleep", "1"]})["processes"][0]["is_expected_command"], False)

    def test_a_reboot_ends_every_process_of_the_boot_before_it(self):
        evidence = {**review_queue.identity(), **review_queue.started(os.getpid()), "boot_id": "an-earlier-boot"}
        seen = review_queue.observe(evidence)
        self.assertEqual(seen["status"], "absent")
        self.assertIn("no process outlives the boot it started in", seen["verified_by"])

    def test_no_record_and_no_process_table_are_unknown(self):
        self.assertEqual(review_queue.observe(None)["status"], "unknown")
        with patch.object(review_queue, "PROC", Path("/nonexistent-proc")):
            evidence = {**review_queue.identity(), "boot_id": "b", "pid_namespace": "n", "pid": 1, "start_ticks": 1}
            self.assertEqual(review_queue.observe(evidence)["reason"], "this host exposes no process table to read")

    def test_a_process_of_this_user_that_cannot_be_inspected_is_unknown_not_absent(self):
        temp = self.enterContext(fixtures.tempfile.TemporaryDirectory())
        proc = Path(temp)
        (proc / "sys/kernel/random").mkdir(parents=True)
        (proc / "sys/kernel/random/boot_id").write_text("boot\n", encoding="utf-8")
        (proc / "self/ns").mkdir(parents=True)
        os.symlink("pid:[7]", proc / "self/ns/pid")

        def process(pid: int, parent: int, environ: bytes | None) -> None:
            (proc / str(pid)).mkdir()
            (proc / str(pid) / "stat").write_text(f"{pid} (a b) S {parent} {pid} {pid} " + "0 " * 15 + f"{pid * 10} 0\n", encoding="utf-8")
            (proc / str(pid) / "cmdline").write_bytes(b"reviewer\0--flag\0")
            os.symlink("/", proc / str(pid) / "cwd")
            if environ is None:
                (proc / str(pid) / "environ").mkdir()
            else:
                (proc / str(pid) / "environ").write_bytes(environ)

        with patch.object(review_queue, "PROC", proc):
            evidence = {**review_queue.identity(), "marker": "m1", "pid": 50, "start_ticks": 500, "session": 50}
            process(60, 1, b"HOME=/x\0")
            self.assertEqual(review_queue.observe(evidence)["status"], "absent")
            process(70, 1, None)
            seen = review_queue.observe(evidence)
            self.assertEqual((seen["status"], seen["reason"]), ("unknown", "processes [70] of this user could not be inspected"))
            process(80, 1, f"HOME=/x\0{review_queue.MARKER}=m1\0".encode())
            process(81, 80, b"")
            seen = review_queue.observe(evidence)
            self.assertEqual((seen["status"], [row["pid"] for row in seen["processes"]]), ("running", [80, 81]))


# --- one controller, pinned ---------------------------------------------------------------------

class SerialOwnership(QueueFixture):
    def test_a_second_starter_is_refused_while_the_first_holds_the_queue(self):
        self.plan(hold=True)
        first = subprocess.Popen([*FAKE, "run", "--queue", str(self.queue)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        self.addCleanup(first.wait)
        self.addCleanup(first.stdout.close)
        self.addCleanup((self.root / "release").touch)
        deadline = time.monotonic() + 30
        while not self.reviews():
            self.assertIsNone(first.poll(), "the first controller ended before its review started")
            self.assertLess(time.monotonic(), deadline, "the first review never started")
            time.sleep(0.02)
        for action in (["run"], ["recover"], ["replace", "--run", RUN_ID, "--attempt", "att-001"], ["check"]):
            with self.subTest(action=action[0]):
                self.assert_blocked(self.command(*action), "another serial controller holds")
        status = json.loads(UncertainExecution.status(self))
        self.assertEqual((status["controller"]["process"]["status"], status["unfiled_claims"][0]["process"]["status"],
                          status["spend_usd"]["claude"]["in_flight_reserved"]), ("running", "running", 5.0))
        self.assertEqual(status["unfiled_claims"][0]["process"]["wrapper"], "running")
        (self.root / "release").touch()
        self.assertEqual(first.wait(60), 0, first.stdout.read())
        self.assert_complete(["att-001", "att-002"], ["att-001", "att-002"])

    def test_a_changed_queue_manifest_client_or_volatile_state_is_refused_before_any_launch(self):
        code, events, err = self.command("run", "--count", "1")
        self.assertEqual(code, 0, (events, err))
        changes = {
            "the queue file or a run manifest changed": lambda: self.queue.write_text(
                json.dumps({**self.spec, "ceilings_usd": {"claude": 400.0}}), encoding="utf-8"),
            "the queue file or a run manifest changed ": lambda: (self.run_dir / "manifest.json").write_text(
                json.dumps({**self.manifest, "caps": {**self.manifest["caps"], "spend_usd": 500.0}}), encoding="utf-8"),
            "the claude executable no longer hashes to its pin": lambda: (self.root / "fake-claude").write_text("#!/bin/sh\n:\n", encoding="utf-8"),
            "the state directory is not persistent storage": lambda: self.queue.write_text(
                json.dumps({**self.spec, "state_dir": "/tmp/review-queue-test-never-created"}), encoding="utf-8"),
        }
        for needle, change in changes.items():
            with self.subTest(change=needle.strip()):
                saved = {path: path.read_bytes() for path in (self.queue, self.run_dir / "manifest.json", self.root / "fake-claude")}
                change()
                self.assert_blocked(self.command("run"), needle.strip())
                for path, content in saved.items():
                    path.write_bytes(content)
        self.assertFalse(Path("/tmp/review-queue-test-never-created").exists())
        self.assertEqual(self.reviews(), ["att-001"])

    def test_runner_files_that_differ_from_the_freeze_commit_are_refused(self):
        repo = self.root / "repo"
        (repo / "bench/tools").mkdir(parents=True)
        git = ["git", "-C", str(repo), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid"]
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        (repo / "bench/tools/run_cell.py").write_text("frozen\n", encoding="utf-8")
        subprocess.run([*git, "add", "."], check=True)
        subprocess.run([*git, "commit", "-qm", "freeze"], check=True)
        commit = subprocess.check_output([*git, "rev-parse", "HEAD"], text=True).strip()
        run = self.run_state()
        with patch.object(review_queue, "REPO", repo):
            self.assertIn("has no freeze_commit", RUNTIME_PROBLEM(run))
            run.manifest["freeze_commit"] = commit
            self.assertIsNone(RUNTIME_PROBLEM(run))
            (repo / "bench/tools/run_cell.py").write_text("edited\n", encoding="utf-8")
            self.assertIn(f"the runner files differ from freeze commit {commit}", RUNTIME_PROBLEM(run))
            run.manifest["freeze_commit"] = "0" * 40
            self.assertIn("cannot compare the runner files", RUNTIME_PROBLEM(run))

    def test_detached_controller_finishes_the_queue_and_records_where_it_ran(self):
        done = self.controller("run", "--detach")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        detached = json.loads(done.stdout)
        self.addCleanup(self.end, detached["detached"]["pid"])
        self.assertEqual(detached["survives"], "the launching shell; not a host or sandbox restart")
        self.assertTrue(Path(detached["log"]).is_relative_to(self.state / "logs"))
        self.assertEqual(review_queue.stat_of(detached["detached"]["pid"])["session"], detached["detached"]["pid"])
        self.gone(detached["detached"]["pid"])
        self.assertIn('"completed": true', Path(detached["log"]).read_text(encoding="utf-8"))
        self.assert_complete(["att-001", "att-002"], ["att-001", "att-002"])


# --- what governs a launch ----------------------------------------------------------------------

class LaunchGates(QueueFixture):
    def test_quota_at_the_stop_ends_the_queue_without_a_launch(self):
        self.meters(95)
        code, events, err = self.command("run")
        self.assertEqual((code, events[-1].get("stopped"), self.reviews()), (0, "Claude usage reached the quota stop", []), (events, err))
        self.assertEqual(self.run_state().attempt_ids(), [])

    def test_no_launch_starts_on_a_meter_reading_older_than_an_hour(self):
        readings, clock = [], [1_000_000.0]

        def meters(queue, env):
            readings.append(clock[0])
            if len(readings) == 3:
                raise OSError("usage endpoint unavailable")
            return {"at": review_queue.stamp(clock[0]), "claude": {"five_hour": {"utilization": 10}}}

        with patch.object(review_queue, "meters", meters), redirect_stdout(io.StringIO()):
            session = review_queue.Session(review_queue.Queue(self.queue), "run", lambda: clock[0], time.sleep)
            self.assertIsNone(session.quota())
            clock[0] += review_queue.QUOTA_MAX_AGE_SECONDS
            self.assertIsNone(session.quota())
            self.assertEqual(len(readings), 1)
            clock[0] += 1
            self.assertIsNone(session.quota())
            self.assertEqual(len(readings), 2)
            clock[0] += review_queue.QUOTA_MAX_AGE_SECONDS + 1
            with self.assertRaisesRegex(review_queue.Blocked, "could not be read, and no launch starts on a reading older than 3600 s"):
                session.quota()

    def test_probe_charges_failures_and_unknown_reservations_count_against_the_client_ceiling(self):
        self.queue.write_text(json.dumps({**self.spec, "ceilings_usd": {"claude": 11.0}}), encoding="utf-8")
        (self.run_dir / "charges.jsonl").write_text(json.dumps({"at": "2026-01-01", "step": "setup probe", "usd": 1.5}) + "\n", encoding="utf-8")
        self.crashed("during_review")
        self.assert_blocked(self.command("run"), "has no diagnosis in deviations/")
        self.diagnose("att-001", "harness-stop")
        self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-001"),
                            "claude spend 6.50 + bound 5.00 exceeds the queue ceiling 11.00")
        totals = review_queue.spend(review_queue.Queue(self.queue), [self.run_state()])["claude"]
        self.assertEqual((totals["total"], totals["charges"], totals["unknown_attempts"], totals["unknown_reserved"]), (6.5, 1.5, 1, 5.0))
        self.assertEqual((self.run_state().attempt_ids(), self.reviews()), (["att-001"], ["att-001"]))

    def test_every_run_files_one_valid_review_before_any_run_takes_its_second(self):
        other = self.second_run()
        code, events, err = self.command("run")
        self.assertEqual((code, [event["run"] for event in events if "launched" in event]), (0, [RUN_ID, other, RUN_ID, other]),
                         (events, err))
        queue = review_queue.Queue(self.queue)
        totals = review_queue.spend(queue, queue.runs())["claude"]
        self.assertEqual((totals["total"], totals["unknown_attempts"]), (round(2 * round(2 * CAPTURED, 4), 4), 0))
        self.assertEqual(self.reviews(), ["att-001", "att-001", "att-002", "att-002"])

    def test_incomplete_cleanup_of_a_valid_review_blocks_the_next_launch(self):
        code, events, err = self.command("run", "--count", "1")
        self.assertEqual(code, 0, (events, err))
        (self.work / "att-001/workspace-pruned.json").unlink()
        os.symlink(self.root / "source/clone", self.work / "att-001/clone-cache")
        self.assert_blocked(self.command("run"), "cleanup incomplete: att-001: evidence filed, but workspace cleanup failed: clone or cache is a symlink")
        self.assertTrue((self.root / "source/clone").is_dir())
        self.assertEqual(self.reviews(), ["att-001"])


class Replacements(QueueFixture):
    def stopped_review(self, reason: str = "reviewer exit 1: Selected model is at capacity") -> None:
        self.plan(stop_next=reason)
        self.assert_blocked(self.command("run"), "save a diagnosis before the queue goes on")

    def test_capacity_after_a_valid_review_gets_one_fresh_replacement_on_the_frozen_arm(self):
        code, events, err = self.command("run", "--count", "1")
        self.assertEqual(code, 0, (events, err))
        self.stopped_review()
        self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-002"), "has no diagnosis in deviations/")
        self.diagnose("att-002", "transient-capacity", "Selected model is at capacity after one valid review.")
        code, events, err = self.command("replace", "--run", RUN_ID, "--attempt", "att-002")
        self.assertEqual((code, events[-1]["disposition"]), (0, "valid completed"), (events, err))
        claim = json.loads((self.work / "att-003/cell.json").read_text(encoding="utf-8"))
        failed = self.run_state().filed["att-002"]
        self.assertEqual((claim["cell"], claim["predecessor"], claim["reserved_usd"], claim["billing_mode"], claim["kind"]),
                         (failed["cell"], "att-002", 5.0, "subscription", "replacement 1 of 2"))
        self.assertTrue(claim["retry_reason"].startswith("Selected model is at capacity after one valid review. (diagnosis deviations/"))
        replacement = self.run_state().filed["att-003"]
        self.assertEqual((replacement["observed"]["models"], replacement["observed"]["effort"], replacement["continuity"]),
                         (failed["observed"]["models"], failed["observed"]["effort"], failed["continuity"]))
        self.assertTrue((self.work / "att-002/home").is_dir() and (self.work / "att-003/home").is_dir())
        self.assertTrue((self.work / "att-002/clone").is_dir())
        self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-002"), "att-002 already has the replacement att-003")
        self.assert_complete(["att-001", "att-002", "att-003"], ["att-001", "att-002", "att-003"])

    def test_rejection_on_the_first_call_stops_the_arm_instead_of_replacing(self):
        self.stopped_review()
        for cause in ("transient-capacity", "setup-rejection"):
            with self.subTest(cause=cause):
                path = self.diagnose("att-001", cause)
                self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-001"),
                                    "rejected before any of its reviews completed validly, so the arm stops")
                code, events, err = self.command("run")
                self.assertEqual((code, events[-1].get("completed"), list(events[-1]["stopped_arms"])), (0, True, [RUN_ID]), (events, err))
                path.unlink()
        self.assertEqual((self.run_state().attempt_ids(), self.reviews()), (["att-001"], ["att-001"]))

    def test_a_stopped_arm_is_skipped_while_the_other_runs_go_on(self):
        other = self.second_run()
        self.stopped_review("reviewer exit 1: the model is not available to this account")
        self.diagnose("att-001", "setup-rejection")
        code, events, err = self.command("run")
        self.assertEqual((code, [event["run"] for event in events if "launched" in event], list(events[-1]["stopped_arms"])),
                         (0, [other, other], [RUN_ID]), (events, err))
        self.assertEqual((self.run_state().attempt_ids(), self.reviews()), (["att-001"], ["att-001", "att-001", "att-002"]))

    def test_a_skill_timeout_and_a_valid_review_are_never_replaced(self):
        code, events, err = self.command("run", "--count", "1")
        self.assertEqual(code, 0, (events, err))
        self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-001"), "a valid attempt is never replaced")
        self.diagnose("att-001", "harness-stop")
        self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-001"), "a valid attempt is never replaced")
        self.stopped_review("the skill ran out of time")
        self.diagnose("att-002", "skill-timeout")
        self.assert_blocked(self.command("replace", "--run", RUN_ID, "--attempt", "att-002"), "not a rerun opportunity")
        code, events, err = self.command("run")
        self.assertEqual((code, events[-1].get("completed"), events[-1]["stopped_arms"]), (0, True, {}), (events, err))
        self.assertEqual((self.run_state().attempt_ids(), self.reviews()), (["att-001", "att-002"], ["att-001", "att-002"]))

    def test_a_diagnosis_names_one_known_cause_and_a_reason(self):
        self.stopped_review()
        self.diagnose("att-001", "bad-luck")
        self.assert_blocked(self.command("run"), "a diagnosis gives a reason and a cause among")
        self.diagnose("att-001", "harness-stop", reason=" ")
        self.assert_blocked(self.command("run"), "has 2 diagnoses in deviations/; keep one")
        (self.run_dir / "deviations/att-001-bad-luck.v1.json").unlink()
        self.assert_blocked(self.command("run"), "a diagnosis gives a reason and a cause among")
        self.assertEqual(self.reviews(), ["att-001"])


# --- status -------------------------------------------------------------------------------------

class Status(QueueFixture):
    def test_snapshots_are_saved_every_thirty_minutes_while_one_review_runs(self):
        self.plan(hold=True)
        clock, naps = [1_800_000_000.0], []

        def sleep(_seconds: float) -> None:
            naps.append(clock[0])
            if not self.reviews():
                time.sleep(0.02)
                return
            clock[0] += 600
            if len(naps) >= 9:
                (self.root / "release").touch()
            time.sleep(0.02)

        queue = review_queue.Queue(self.queue)
        with redirect_stdout(io.StringIO()):
            code = review_queue.controlled(queue, "run", lambda session: review_queue.run_queue(session, 1), lambda: clock[0], sleep)
        self.assertEqual(code, 0)
        saved = [json.loads(path.read_text(encoding="utf-8")) for path in sorted((self.state / "status").glob("*.json"))]
        moments = [fixtures.datetime.fromisoformat(row["at"].replace("Z", "+00:00")).timestamp() for row in saved]
        self.assertGreaterEqual(len(saved), 3, moments)
        self.assertEqual({later - earlier for earlier, later in zip(moments, moments[1:])}, {1800.0})
        during = saved[1]
        self.assertEqual((during["valid"], during["remaining"], during["unfiled_claims"][0]["process"]["status"],
                          during["spend_usd"]["claude"]["in_flight_reserved"], during["controller"]["process"]["status"]),
                         (0, 2, "running", 5.0, "running"))
        self.assertEqual(self.reviews(), ["att-001"])

    def test_a_snapshot_is_not_due_before_thirty_minutes_and_its_delivery_is_recorded_apart(self):
        queue = review_queue.Queue(self.queue)
        with redirect_stdout(io.StringIO()):
            first = review_queue.snapshot(queue, 1_800_000_000.0, due_only=True)
            self.assertIsNone(review_queue.snapshot(queue, 1_800_000_000.0 + 1799, due_only=True))
            second = review_queue.snapshot(queue, 1_800_000_000.0 + 1800, due_only=True)
        saved = first.read_bytes()
        report = json.loads(UncertainExecution.status(self))
        self.assertEqual(report["snapshots"], {"saved": 2, "unacknowledged": [first.name, second.name]})
        self.assertIn("nothing here sends it", report["delivery"])
        code, _, err = self.command("status", "--acknowledge", first.name, "--note", "posted to the owner in chat")
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(UncertainExecution.status(self))["snapshots"]["unacknowledged"], [second.name])
        self.assertEqual(first.read_bytes(), saved)
        receipt = json.loads((self.state / "status/acknowledged.jsonl").read_text(encoding="utf-8"))
        self.assertEqual((receipt["snapshot"], receipt["note"]), (first.name, "posted to the owner in chat"))
        code, _, err = self.command("status", "--acknowledge", "20260101T000000Z.json", "--note", "sent")
        self.assertEqual((code, "no saved snapshot" in err), (2, True))

    def test_a_stale_heartbeat_does_not_end_a_live_controller_and_a_fresh_one_does_not_keep_a_dead_one(self):
        queue = review_queue.Queue(self.queue)
        gone = subprocess.Popen(["true"])
        ended = {**review_queue.identity(), **review_queue.started(gone.pid)}
        gone.wait()
        now = float(int(time.time()))
        for name, process, beat, expected in (("live", {**review_queue.identity(), **review_queue.started(os.getpid())}, now - 86400, "running"),
                                              ("dead", ended, now, "absent")):
            with self.subTest(controller=name):
                review_queue.write(self.state / "invocations" / f"{name}.json", {
                    "invocation_id": name, "action": "run", "ended_at": None, "outcome": None,
                    "heartbeat_at": review_queue.stamp(beat), "process": process})
                controller = review_queue.report(queue, now)["controller"]
                self.assertEqual((controller["process"]["status"], controller["heartbeat_age_seconds"]), (expected, round(now - beat)))
                (self.state / "invocations" / f"{name}.json").unlink()


class Pins(unittest.TestCase):
    def test_memory_and_scratch_file_systems_are_not_persistent_storage(self):
        self.assertIn("is under /tmp", review_queue.volatile(Path("/tmp/review-queue-test-never-created/state")))
        self.assertFalse(Path("/tmp/review-queue-test-never-created").exists())
        temp = self.enterContext(fixtures.tempfile.TemporaryDirectory(dir=fixtures.ROOT, prefix=".review-queue-test-"))
        with patch.object(review_queue, "filesystem", return_value="tmpfs"):
            self.assertIn("is on tmpfs", review_queue.volatile(Path(temp) / "state"))
        self.assertIsNone(review_queue.volatile(Path(temp) / "state"))
        self.assertEqual(list((Path(temp) / "state").iterdir()), [])

    def test_the_file_system_is_the_one_mounted_deepest_over_the_path(self):
        temp = self.enterContext(fixtures.tempfile.TemporaryDirectory())
        (Path(temp) / "self").mkdir()
        (Path(temp) / "self/mountinfo").write_text(
            "21 1 8:1 / / rw - ext4 /dev/sda1 rw\n22 21 0:5 / /data rw - tmpfs tmpfs rw\n23 22 8:2 / /data/kept\\040here rw - xfs /dev/sdb1 rw\n",
            encoding="utf-8")
        with patch.object(review_queue, "PROC", Path(temp)):
            self.assertEqual([review_queue.filesystem(Path(path)) for path in ("/home/x", "/data/x", "/data/kept here/x")],
                             ["ext4", "tmpfs", "xfs"])
        with patch.object(review_queue, "PROC", Path("/nonexistent-proc")):
            self.assertIsNone(review_queue.filesystem(Path("/home/x")))

    def test_quota_stop_reads_each_client_the_queue_pins(self):
        codex = {"ordinaryUsageAllowed": True, "rateLimits": {"primary": {"usedPercent": 40}, "secondary": None}}
        self.assertIsNone(review_queue.quota_stop({"claude": {"five_hour": {"utilization": 94.9}, "seven_day": None}, "codex": codex}, 95))
        self.assertEqual(review_queue.quota_stop({"codex": {**codex, "ordinaryUsageAllowed": False}}, 95), "ChatGPT ordinary usage is unavailable")
        codex["rateLimits"]["primary"]["usedPercent"] = 95
        self.assertEqual(review_queue.quota_stop({"codex": codex}, 95), "ChatGPT usage reached the quota stop")


if __name__ == "__main__":
    if sys.argv[1:2] == ["cell"]:
        sys.exit(fake_cell(sys.argv[2:]))
    if "--queue" in sys.argv:
        sys.exit(fake_queue(sys.argv[1:]))
    unittest.main()
