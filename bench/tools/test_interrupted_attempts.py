"""Prove unknown usage stays unknown when an attempt is interrupted, with fake clients and local process failures.

Every attempt directory is a temporary fixture shaped like the saved built-in attempts; no reviewer is called.
"""
from __future__ import annotations

import argparse
from contextlib import redirect_stderr, redirect_stdout
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import file_attempt
import run_cell

TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parents[1]
RUN_ID = "2026-01-01-interrupted"
PROMPT = "`high effort variant`\n\nReview the diff."
PROMPT_HASH = hashlib.sha256(PROMPT.encode("utf-8")).hexdigest()
RUBRIC = "You are acting as a reviewer for a proposed code change"
RUBRIC_HASH = hashlib.sha256(RUBRIC.encode("utf-8")).hexdigest()
RATE = {"model": "m-1", "as_of": "2026-01-01", "input": 2.0, "output": 10.0, "cache_read": 0.2,
        "cache_write_5m": 2.5, "cache_write_1h": 4.0, "billing": "api-dollars"}
CAPTURED = round((10 * 2 + 100 * 2.5 + 1000 * 0.2 + 50 * 10) / 1e6, 6)
CODEX_ROLLOUT = round((600 * 2 + 400 * 0.2 + 50 * 10) / 1e6, 6)
CODEX_CAPTURED = 2 * CODEX_ROLLOUT
ARMS = {"claude": {"id": "arm-claude", "kind": "claude-builtin", "model": "m-1", "effort": "high",
                   "adapter": {"expected_prompt_variants": [PROMPT_HASH]}},
        "codex": {"id": "arm-codex", "kind": "codex", "model": "m-1", "effort": "high",
                  "adapter": {"expected_prompt_variants": [RUBRIC_HASH]}}}


def lines(*records) -> str:
    return "".join(json.dumps(record) + "\n" for record in records)


def reply(request: str, stop_reason: str | None) -> dict:
    return {"type": "assistant", "requestId": request, "effort": "high", "timestamp": "2026-01-01T00:00:01Z",
            "message": {"model": "m-1", "stop_reason": stop_reason, "content": [{"type": "text", "text": "[]"}],
                        "usage": {"input_tokens": 10, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 1000,
                                  "cache_creation": {"ephemeral_5m_input_tokens": 100, "ephemeral_1h_input_tokens": 0},
                                  "output_tokens": 50}}}


def claude_transcript(stop_reason: str | None = "end_turn") -> str:
    return lines({"type": "user", "message": {"content": f"Review target: `main...review-head high`\n\n{PROMPT}\n"}},
                 reply("r1", stop_reason))


def rollout(thread: str, parent: str | None, *events: str, input_tokens: int = 1000) -> str:
    meta = {"id": thread, "base_instructions": {"text": RUBRIC}}
    if parent:
        meta["parent_thread_id"] = parent
    usage = {"input_tokens": input_tokens, "cached_input_tokens": 400, "cache_write_input_tokens": 0, "output_tokens": 50,
             "reasoning_output_tokens": 0}
    return lines({"type": "session_meta", "payload": meta},
                 {"type": "turn_context", "payload": {"model": "m-1", "effort": "high", "sandbox_policy": {"type": "workspace-write"}}},
                 {"type": "token_usage_record", "payload": {"response_id": f"{thread}-r1", "usage": usage}},
                 {"type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": usage}}},
                 *({"type": "event_msg", "payload": {"type": event}} for event in events))


class Fixture(unittest.TestCase):
    """A frozen fixture run under a temporary root: ``bench/runs/<run>`` and the work directory ``work/<run>``."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=ROOT, prefix=".interrupted-attempt-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.run_dir = self.root / "bench/runs" / RUN_ID
        self.work = self.root / "work" / RUN_ID
        (self.run_dir / "fixture").mkdir(parents=True)
        (self.root / "bench/arms").mkdir()
        for arm in ARMS.values():
            (self.root / "bench/arms" / f"{arm['id']}.json").write_text(json.dumps(arm), encoding="utf-8")
        self.enterContext(patch.object(run_cell, "BENCH", self.root / "bench"))
        self.enterContext(patch.dict(os.environ, {"BENCH_ARCHIVE_ROOT": str(self.root / "archive")}))
        self.manifest = {"run_id": RUN_ID, "frozen_at": "2026-01-01T00:00:00Z",
                         "arms": [{"id": arm["id"], "expected_cli_version": "9.9.9", "resolved_skill_tree": None}
                                  for arm in ARMS.values()],
                         "planned_cells": [{"target": "t-fixture", "arm": "arm-claude", "replicate": 1}],
                         "sealed_order": ["t-fixture/arm-claude/1"],
                         "caps": {"max_attempts": 4, "replacements": 2, "spend_usd": 50.0, "closeout_reserve_usd": 0.0,
                                  "max_in_flight": 1, "attempt_usd": 5.0}}
        (self.run_dir / "manifest.json").write_text(json.dumps(self.manifest), encoding="utf-8")

    def clone(self, attempt: Path) -> Path:
        clone = attempt / "clone"
        git = ["git", "-C", str(clone), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid"]
        dated = {**os.environ, "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z", "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z"}
        subprocess.run(["git", "init", "-q", "-b", "main", str(clone)], check=True)
        subprocess.run([*git, "commit", "--allow-empty", "-qm", "base"], check=True, env=dated)
        subprocess.run([*git, "checkout", "-q", "-b", "review-head"], check=True)
        subprocess.run([*git, "commit", "--allow-empty", "-qm", "head"], check=True, env=dated)
        revision = lambda name: subprocess.check_output([*git, "rev-parse", name], text=True).strip()  # noqa: E731
        (self.run_dir / "fixture/target.json").write_text(json.dumps(
            {"id": "t-fixture", "head": revision("review-head"), "merge_base": revision("main")}), encoding="utf-8")
        return clone

    def attempt(self, attempt_id: str = "att-001", harness: str = "claude", *, exit_code: int | None = 0,
                predecessor: str | None = None) -> Path:
        """A claimed attempt whose reviewer ran to its end; ``exit_code`` None leaves the dispatch unreturned."""
        attempt = self.work / attempt_id
        attempt.mkdir(parents=True)
        self.clone(attempt)
        claim = {"cell": {"target": "t-fixture", "arm": ARMS[harness]["id"], "replicate": 1}, "reserved_usd": 5.0,
                 "predecessor": predecessor, "retry_reason": "interrupted" if predecessor else None,
                 "replacement_index": 1 if predecessor else None, "billing_mode": "subscription"}
        (attempt / "cell.json").write_text(json.dumps(claim), encoding="utf-8")
        (attempt / "rates.json").write_text(json.dumps({"rates": [RATE]}), encoding="utf-8")
        (attempt / "timing.json").write_text(json.dumps({"root_dispatched_at": "2026-01-01T00:00:00Z", "payload_validated_at": None,
                                                         "completed_at": None if exit_code is None else "2026-01-01T00:00:05Z"}),
                                             encoding="utf-8")
        (attempt / "tree-before.txt").write_text("abc\n", encoding="utf-8")
        ended = "" if exit_code is None else f"exit={exit_code}\n"
        if harness == "claude":
            (attempt / "dispatch.txt").write_text("claude 9.9.9 (Claude Code)\nmodel=m-1 effort=high\n" + ended, encoding="utf-8")
            subagents = attempt / "home/.claude/projects/p/s1/subagents"
            subagents.mkdir(parents=True)
            (attempt / "home/.claude/projects/p/s1.jsonl").write_text("", encoding="utf-8")
            (subagents / "agent-a1.jsonl").write_text(claude_transcript(), encoding="utf-8")
            (attempt / "home/.claude/.credentials.json").write_text("{}", encoding="utf-8")
            (attempt / "stdout.jsonl").write_text(lines({"type": "system"}, {"type": "result", "subtype": "success"}), encoding="utf-8")
        else:
            (attempt / "dispatch.txt").write_text("codex-cli 9.9.9\nmodel=m-1 effort=high\n" + ended, encoding="utf-8")
            sessions = attempt / "home/.codex/sessions/2026/01/01"
            sessions.mkdir(parents=True)
            (sessions / "rollout-root.jsonl").write_text(rollout("root", None, "task_complete"), encoding="utf-8")
            (sessions / "rollout-child.jsonl").write_text(rollout("child", "root", "task_complete"), encoding="utf-8")
            (attempt / "home/.codex/auth.json").write_text("{}", encoding="utf-8")
            (attempt / "stdout.txt").write_text("No findings.\n", encoding="utf-8")
        if exit_code is not None:
            (attempt / "native-return.json").write_text(json.dumps({"returned_at": "2026-01-01T00:00:04Z", "exit_code": exit_code}),
                                                        encoding="utf-8")
            (attempt / "tree-after.txt").write_text("abc\n", encoding="utf-8")
            (attempt / "audit.json").write_text(json.dumps({"violations": [], "guidance_probes": [], "network_commands": [],
                                                            "diff_commands": ["git diff main...review-head"]}), encoding="utf-8")
            (attempt / "normalized.json").write_text(json.dumps({"parse_status": "parsed", "items": []}), encoding="utf-8")
            if harness == "claude":
                (attempt / "payload.json").write_text("[]", encoding="utf-8")
        return attempt

    def stop(self, attempt: Path, exit_code: int, reason: str) -> None:
        (attempt / "stop.json").write_text(json.dumps({"stopped_at": "2026-01-01T00:00:06Z", "exit_code": exit_code, "reason": reason}),
                                           encoding="utf-8")

    def evidence(self, attempt_id: str = "att-001", **changed) -> Path:
        path = self.root / f"process-stop-{len(list(self.root.glob('process-stop-*')))}.json"
        path.write_text(json.dumps({"run_id": RUN_ID, "attempt_id": attempt_id, "observed_at": datetime.now(timezone.utc).isoformat(),
                                    "status": "absent", "verified_by": "fixture: process table of the dispatch host", **changed}),
                        encoding="utf-8")
        return path

    def filer(self, attempt: Path, harness: str = "claude") -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(TOOLS / "file_attempt.py"), "--attempt-dir", str(attempt), "--clone", str(attempt / "clone"),
             "--target", str(self.run_dir / "fixture"), "--arm", str(self.root / "bench/arms" / f"{ARMS[harness]['id']}.json"),
             "--run-id", RUN_ID, "--attempt-id", attempt.name, "--replicate", "1", "--out", str(self.run_dir / "attempts" / attempt.name),
             "--rates", str(attempt / "rates.json"), "--archive-root", str(self.root / "archive"), "--billing-mode", "subscription"],
            capture_output=True, text=True, encoding="utf-8")

    def filed(self, attempt: Path, harness: str = "claude") -> dict:
        done = self.filer(attempt, harness)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        return json.loads((self.run_dir / "attempts" / attempt.name / "attempt.json").read_text(encoding="utf-8"))

    def runner(self, *args: str) -> subprocess.CompletedProcess:
        """``run_cell.py`` from its command line, in this process so that it reads the fixture's arms."""
        argv = ["run_cell.py", "--run", str(self.run_dir), "--work", str(self.work), *args]
        with patch.object(sys, "argv", argv), redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()) as err:
            code = run_cell.main()
        return subprocess.CompletedProcess(argv, code, out.getvalue(), err.getvalue())

    def run_state(self) -> run_cell.Run:
        return run_cell.Run(self.run_dir, self.work)

    def assert_unknown_total(self, record: dict, minimum) -> None:
        usage = record["usage"]
        self.assertEqual((usage["metering_status"], usage["priced_total_usd"], usage["cost_bounds_usd"]),
                         ("incomplete", None, {"low": minimum, "high": None}), record["notes"])


class CompleteUsage(Fixture):
    def assert_complete(self, record: dict, total: float) -> None:
        usage = record["usage"]
        self.assertEqual((usage["metering_status"], usage["priced_total_usd"]), ("complete", total), record["notes"])

    def test_audit_invalid_completion_keeps_its_complete_usage(self):
        attempt = self.attempt()
        (attempt / "audit.json").write_text(json.dumps({"violations": ["read outside the clone: /etc/hosts"],
                                                        "diff_commands": ["git diff main...review-head"]}), encoding="utf-8")
        record = self.filed(attempt)
        self.assertTrue(record["disposition"].startswith("harness-invalid: read audit"), record["disposition"])
        self.assert_complete(record, CAPTURED)

    def test_native_completion_keeps_its_usage_when_normalization_fails(self):
        attempt = self.attempt()
        malformed = b'[{"title": "an "unescaped" quote"}]'
        (attempt / "payload.json").write_bytes(malformed)
        (attempt / "normalized.json").unlink()
        self.stop(attempt, 0, "normalization exit 1: unresolved")
        record = self.filed(attempt)
        self.assertEqual(record["disposition"], "stopped: normalization exit 1: unresolved")
        self.assertEqual(record["normalized"]["parse_status"], "unresolved")
        self.assertEqual(record["native_payload"]["sha256"], hashlib.sha256(malformed).hexdigest())
        self.assertEqual((self.run_dir / "attempts/att-001/native-return.json").read_text(encoding="utf-8"),
                         (attempt / "native-return.json").read_text(encoding="utf-8"))
        self.assert_complete(record, CAPTURED)

    def test_nonzero_native_failure_keeps_usage_its_streams_prove_complete(self):
        claude = self.attempt("att-001", exit_code=1)
        (claude / "stdout.jsonl").write_text(lines({"type": "result", "subtype": "error_during_execution", "is_error": True}), encoding="utf-8")
        self.stop(claude, 1, "reviewer exit 1")
        codex = self.attempt("att-002", "codex", exit_code=1)
        self.stop(codex, 1, "reviewer exit 1")
        for attempt, harness, total in ((claude, "claude", CAPTURED), (codex, "codex", CODEX_CAPTURED)):
            with self.subTest(harness=harness):
                record = self.filed(attempt, harness)
                self.assertEqual(record["disposition"], "stopped: reviewer exit 1")
                self.assert_complete(record, total)


class UnprovenUsage(Fixture):
    def test_partial_parent_stream_prices_only_a_minimum(self):
        for name, stream in (("no result record", lines({"type": "system"}, {"type": "rate_limit_event"})),
                             ("result cut mid-line", lines({"type": "system"}) + '{"type": "result", "subty'),
                             ("missing", None)):
            with self.subTest(stream=name):
                attempt = self.attempt(f"att-00{len(list(self.work.glob('att-*'))) + 1}")
                (attempt / "stdout.jsonl").unlink()
                if stream is not None:
                    (attempt / "stdout.jsonl").write_text(stream, encoding="utf-8")
                record = self.filed(attempt)
                self.assertEqual(record["disposition"], "valid completed")
                self.assert_unknown_total(record, CAPTURED)
                requests = (self.run_dir / "attempts" / attempt.name / "usage-requests.jsonl").read_text(encoding="utf-8")
                self.assertEqual([json.loads(line)["request_id"] for line in requests.splitlines()], ["r1"])

    def test_missing_child_terminal_evidence_leaves_the_total_unknown(self):
        claude = self.attempt("att-001")
        (claude / "home/.claude/projects/p/s1/subagents/agent-a2.jsonl").write_text(claude_transcript("tool_use"), encoding="utf-8")
        codex = self.attempt("att-002", "codex")
        (codex / "home/.codex/sessions/2026/01/01/rollout-child.jsonl").write_text(rollout("child", "root", "turn_aborted"), encoding="utf-8")
        for attempt, harness, minimum, gap in (
                (claude, "claude", 2 * CAPTURED, "agent-a2.jsonl ends without end_turn"),
                (codex, "codex", CODEX_CAPTURED, "rollout-child.jsonl does not end with task_complete")):
            with self.subTest(harness=harness):
                record = self.filed(attempt, harness)
                self.assertEqual(record["disposition"], "valid completed")
                self.assert_unknown_total(record, minimum)
                self.assertTrue(any(gap in note for note in record["notes"]), record["notes"])

    def test_transcript_cut_mid_line_is_filed_with_what_it_captured(self):
        attempt = self.attempt()
        transcript = attempt / "home/.claude/projects/p/s1/subagents/agent-a1.jsonl"
        transcript.write_text(claude_transcript() + '{"type": "assistant", "requestId": "r2", "mess', encoding="utf-8")
        record = self.filed(attempt)
        self.assert_unknown_total(record, CAPTURED)
        self.assertTrue(any("agent-a1.jsonl has an unreadable line" in note for note in record["notes"]), record["notes"])

    def test_capture_the_meter_cannot_read_keeps_the_floor_of_its_readable_requests(self):
        claude = self.attempt("att-001")
        (claude / "stdout.jsonl").unlink()
        (claude / "home/.claude/projects/p/s1/subagents/agent-a2.jsonl").write_text(
            lines({"type": "user", "message": {"content": "never answered"}}), encoding="utf-8")
        codex = self.attempt("att-002", "codex")
        (codex / "home/.codex/sessions/2026/01/01/rollout-child.jsonl").write_text(
            rollout("child", "root", input_tokens=4_000_400) + '{"type": "event_msg", "payl', encoding="utf-8")
        over = round((4_000_000 * 2 + 400 * 0.2 + 50 * 10) / 1e6, 6)
        for attempt, harness, floor in ((claude, "claude", CAPTURED), (codex, "codex", over)):
            with self.subTest(harness=harness):
                record = self.filed(attempt, harness)
                self.assert_unknown_total(record, floor)
                self.assertTrue(any(note.startswith("usage not priced: the meter could not read the capture") for note in record["notes"]),
                                record["notes"])
        state = self.run_state()
        self.assertEqual((state.over_reservation(), state.spend()["attempts"]), (["att-002"], round(5.0 + over, 4)))

    def test_missing_root_rollout_keeps_the_child_requests(self):
        attempt = self.attempt(harness="codex", exit_code=1)
        self.stop(attempt, 1, "reviewer exit 1")
        (attempt / "home/.codex/sessions/2026/01/01/rollout-root.jsonl").unlink()
        record = self.filed(attempt, "codex")
        self.assertEqual((record["disposition"], record["observed"]["models"]), ("stopped: reviewer exit 1", ["m-1"]))
        self.assert_unknown_total(record, CODEX_ROLLOUT)
        requests = (self.run_dir / "attempts/att-001/usage-requests.jsonl").read_text(encoding="utf-8").splitlines()
        self.assertEqual([json.loads(line)["thread"] for line in requests], ["rollout-child.jsonl"])

    def test_missing_transcripts_never_price_zero(self):
        for harness, home in (("claude", "home/.claude/projects"), ("codex", "home/.codex/sessions")):
            with self.subTest(harness=harness):
                attempt = self.attempt(f"att-00{len(list(self.work.glob('att-*'))) + 1}", harness, exit_code=1)
                self.stop(attempt, 1, "reviewer exit 1")
                for path in sorted((attempt / home).rglob("*.jsonl")):
                    path.unlink()
                record = self.filed(attempt, harness)
                self.assertEqual(record["disposition"], "stopped: reviewer exit 1")
                self.assert_unknown_total(record, None)


class InterruptionFiling(Fixture):
    def interrupted(self, attempt_id: str = "att-001") -> Path:
        attempt = self.attempt(attempt_id, exit_code=None)
        (attempt / "stdout.jsonl").write_text(lines({"type": "system"}, {"type": "rate_limit_event"}), encoding="utf-8")
        (attempt / "home/.claude/projects/p/s1/subagents/agent-a1.jsonl").write_text(claude_transcript("tool_use"), encoding="utf-8")
        return attempt

    def test_filer_refuses_a_dispatch_that_neither_ended_nor_was_observed_interrupted(self):
        done = self.filer(self.interrupted())
        self.assertEqual(done.returncode, 2, done.stdout)
        self.assertIn("no exit= line", done.stderr)
        self.assertFalse((self.run_dir / "attempts/att-001").exists())

    def test_filer_refuses_an_interruption_record_without_verified_absence(self):
        attempt = self.interrupted()
        (attempt / "interruption.json").write_text(json.dumps({"reason": "host restarted", "process_stop": {
            "run_id": RUN_ID, "attempt_id": "att-001", "observed_at": "2026-01-01T00:30:00Z", "status": "unknown",
            "verified_by": "fixture"}}), encoding="utf-8")
        done = self.filer(attempt)
        self.assertEqual(done.returncode, 2, done.stdout)
        self.assertIn("status is 'unknown'", done.stderr)

    def test_filer_refuses_a_stop_observation_older_than_the_saved_output(self):
        attempt = self.interrupted()
        sidecar = {"reason": "host restarted", "last_output_at": None,
                   "process_stop": json.loads(self.evidence(observed_at="2026-01-01T00:30:00Z").read_text(encoding="utf-8"))}
        (attempt / "interruption.json").write_text(json.dumps(sidecar), encoding="utf-8")
        done = self.filer(attempt)
        self.assertEqual(done.returncode, 2, done.stdout)
        self.assertIn("observed_at precedes the attempt's last output", done.stderr)
        run_cell.close_interruption(self.run_state(), "att-001", str(self.evidence()), "host restarted")
        later = time.time() + 60
        os.utime(attempt / "stdout.jsonl", (later, later))
        done = self.filer(attempt)
        self.assertEqual(done.returncode, 2, done.stdout)
        self.assertIn("observed_at precedes the attempt's last output", done.stderr)
        self.assertFalse((self.run_dir / "attempts/att-001").exists())

    def test_absent_or_unverified_process_evidence_is_refused(self):
        attempt = self.interrupted()
        cases = {"absent": str(self.root / "no-such-evidence.json"),
                 "unknown visibility": str(self.evidence(status="unknown")),
                 "still running": str(self.evidence(status="running")),
                 "another attempt": str(self.evidence(attempt_id="att-009")),
                 "another run": str(self.evidence(run_id="2026-01-01-other")),
                 "no method": str(self.evidence(verified_by=" ")),
                 "no time": str(self.evidence(observed_at="soon")),
                 "no offset": str(self.evidence(observed_at="2999-01-01T00:00:00")),
                 "observed before the last output": str(self.evidence(observed_at="2026-01-01T00:30:00Z"))}
        for name, path in cases.items():
            with self.subTest(evidence=name):
                done = self.runner("--file", "att-001", "--interrupted", path, "--reason", "host restarted")
                self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
                self.assertIn("refused: att-001", done.stdout)
                self.assertFalse((self.run_dir / "attempts/att-001").exists())
                self.assertFalse((attempt / "interruption.json").exists())
                self.assertTrue((attempt / "home/.claude/.credentials.json").exists())
        done = self.runner("--file", "att-001", "--interrupted", str(self.evidence()))
        self.assertEqual((done.returncode, done.stdout.strip()), (1, "refused: --interrupted needs --reason"))

    def test_interruption_is_filed_with_exit_and_stop_time_unknown(self):
        attempt = self.interrupted()
        (attempt / "home/.codex").mkdir()
        (attempt / "home/.codex/auth.json").write_text("{}", encoding="utf-8")
        done = self.runner("--file", "att-001", "--interrupted", str(self.evidence()), "--reason", "host restarted during the review")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        filed = self.run_dir / "attempts/att-001"
        record = json.loads((filed / "attempt.json").read_text(encoding="utf-8"))
        self.assertEqual(record["disposition"], "stopped: host restarted during the review")
        self.assertEqual(record["timing"], {"dispatched_at": "2026-01-01T00:00:00Z", "payload_validated_at": None,
                                            "completed_at": None, "stopped_at": None})
        self.assertEqual(record["observed"]["tree_identity_after"], "not recorded")
        self.assertIsNone(record["native_payload"])
        self.assert_unknown_total(record, CAPTURED)
        self.assertNotIn("exit=", (attempt / "dispatch.txt").read_text(encoding="utf-8"))
        self.assertNotIn("exit=", (filed / "dispatch.txt").read_text(encoding="utf-8"))
        closed = json.loads((filed / "interruption.json").read_text(encoding="utf-8"))
        self.assertEqual(closed["process_stop"]["status"], "absent")
        moment = lambda text: datetime.fromisoformat(text.replace("Z", "+00:00"))  # noqa: E731
        self.assertLessEqual(moment(closed["last_output_at"]), moment(closed["process_stop"]["observed_at"]))
        self.assertFalse((attempt / "home/.claude/.credentials.json").exists())
        self.assertFalse((attempt / "home/.codex/auth.json").exists())
        self.assertTrue((attempt / "clone").is_dir())
        self.assertFalse((attempt / "workspace-pruned.json").exists())

    def test_a_completed_interruption_filing_is_reused(self):
        self.interrupted()
        first = self.runner("--file", "att-001", "--interrupted", str(self.evidence()), "--reason", "host restarted")
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        record = self.run_dir / "attempts/att-001/attempt.json"
        before = (record.read_bytes(), record.stat().st_mtime_ns)
        again = self.runner("--file", "att-001", "--interrupted", str(self.root / "no-such-evidence.json"), "--reason", "another reason")
        self.assertEqual((again.returncode, json.loads(again.stdout)), (0, json.loads(first.stdout)), again.stderr)
        self.assertEqual((record.read_bytes(), record.stat().st_mtime_ns), before)

    def test_an_ended_dispatch_is_not_filed_as_an_interruption(self):
        self.attempt()
        done = self.runner("--file", "att-001", "--interrupted", str(self.evidence()), "--reason", "host restarted")
        self.assertEqual(done.returncode, 1, done.stderr)
        self.assertIn("the dispatch ended", done.stdout)

    def test_interruption_after_the_native_return_keeps_the_known_exit_and_usage(self):
        attempt = self.attempt()
        (attempt / "dispatch.txt").write_text("claude 9.9.9 (Claude Code)\nmodel=m-1 effort=high\n", encoding="utf-8")
        (attempt / "timing.json").write_text(json.dumps({"root_dispatched_at": "2026-01-01T00:00:00Z", "payload_validated_at": None,
                                                         "completed_at": None}), encoding="utf-8")
        for name in ("tree-after.txt", "audit.json", "normalized.json", "payload.json"):
            (attempt / name).unlink()
        done = self.runner("--file", "att-001", "--interrupted", str(self.evidence()), "--reason", "wrapper killed while auditing")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        record = json.loads((self.run_dir / "attempts/att-001/attempt.json").read_text(encoding="utf-8"))
        self.assertEqual(record["disposition"], "stopped: wrapper killed while auditing")
        self.assertEqual(record["timing"]["stopped_at"], "2026-01-01T00:00:04Z")
        self.assertEqual((record["usage"]["metering_status"], record["usage"]["priced_total_usd"]), ("complete", CAPTURED))

    def test_a_reviewer_killed_under_the_wrapper_leaves_no_return_and_is_closed_out(self):
        attempt = self.work / "att-001"
        attempt.mkdir(parents=True)
        clone = self.clone(attempt)
        subprocess.run(["git", "-C", str(clone), "checkout", "-q", "review-head"], check=True)
        (attempt / "cell.json").write_text(json.dumps({"cell": {"target": "t-fixture", "arm": "arm-claude", "replicate": 1},
                                                       "reserved_usd": 5.0, "predecessor": None, "billing_mode": "subscription"}),
                                           encoding="utf-8")
        (attempt / "rates.json").write_text(json.dumps({"rates": [RATE]}), encoding="utf-8")
        source = self.root / "source-home"
        (source / ".claude").mkdir(parents=True)
        (source / ".claude/.credentials.json").write_text('{"token": "fixture"}', encoding="utf-8")
        (source / ".claude.json").write_text("{}", encoding="utf-8")
        packet = self.root / "packet.md"
        packet.write_text("Review fixture.", encoding="utf-8")
        client = self.root / "fake-claude"
        client.write_text(f"""#!/usr/bin/env python3
import json, os, sys, time
if sys.argv[1:] == ["--version"]:
    print("9.9.9 (Claude Code)"); sys.exit(0)
session = sys.argv[sys.argv.index("--session-id") + 1]
subagents = os.path.join(os.environ["HOME"], ".claude", "projects", "p", session, "subagents")
os.makedirs(subagents)
open(os.path.join(os.environ["HOME"], ".claude", "projects", "p", session + ".jsonl"), "w").close()
with open(os.path.join(subagents, "agent-a1.jsonl"), "w") as handle:
    handle.write({claude_transcript("tool_use")!r})
print(json.dumps({{"type": "system", "subtype": "task_started", "pid": os.getpid()}}), flush=True)
time.sleep(120)
""", encoding="utf-8")
        client.chmod(0o755)
        wrapper = subprocess.Popen(
            [str(TOOLS / "dispatch.sh"), "claude-builtin", str(attempt), str(clone), "main", str(packet), "m-1", "high"],
            env={**os.environ, "HOME": str(source), "BENCH_CLAUDE": str(client)}, start_new_session=True,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        started = ""
        try:
            deadline = time.monotonic() + 30
            while not started.endswith("\n"):
                self.assertIsNone(wrapper.poll(), "dispatch.sh ended before the fake reviewer started")
                self.assertLess(time.monotonic(), deadline, "the fake reviewer never started")
                time.sleep(0.05)
                started = (attempt / "stdout.jsonl").read_text(encoding="utf-8") if (attempt / "stdout.jsonl").is_file() else ""
        finally:
            os.killpg(wrapper.pid, signal.SIGKILL)
            wrapper.wait()
            if started.endswith("\n"):
                # timeout(1) runs the reviewer in a process group of its own, which outlives the wrapper's.
                os.killpg(os.getpgid(json.loads(started)["pid"]), signal.SIGKILL)
        self.assertNotIn("exit=", (attempt / "dispatch.txt").read_text(encoding="utf-8"))
        self.assertFalse((attempt / "native-return.json").exists())
        self.assertTrue((attempt / "home/.claude/.credentials.json").exists())
        refused = self.runner("--file", "att-001")
        self.assertEqual(refused.returncode, 1, refused.stderr)
        self.assertIn("the dispatch has not ended", refused.stdout)
        done = self.runner("--file", "att-001", "--interrupted", str(self.evidence()), "--reason", "reviewer and wrapper killed")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        record = json.loads((self.run_dir / "attempts/att-001/attempt.json").read_text(encoding="utf-8"))
        self.assertEqual(record["disposition"], "stopped: reviewer and wrapper killed")
        self.assert_unknown_total(record, CAPTURED)
        self.assertFalse((attempt / "home/.claude/.credentials.json").exists())


class Accounting(Fixture):
    def choose(self, **wanted):
        return run_cell.choose(self.run_state(), argparse.Namespace(**{"next": False, "cell": None, "replace": None, "reason": None,
                                                                       "stopped_by_harness": False, **wanted}))

    def file_record(self, attempt_id: str, priced, low=None, disposition: str = "stopped: host restarted", predecessor=None) -> None:
        directory = self.run_dir / "attempts" / attempt_id
        directory.mkdir(parents=True)
        (directory / "attempt.json").write_text(json.dumps({
            "cell": {"target": "t-fixture", "arm": "arm-claude", "replicate": 1}, "disposition": disposition, "predecessor": predecessor,
            "usage": {"priced_total_usd": priced, "cost_bounds_usd": {"low": low, "high": None if priced is None else priced}},
            "timing": {"dispatched_at": "2026-01-01T00:00:00Z", "completed_at": None}}), encoding="utf-8")

    def claim(self, attempt_id: str, predecessor=None) -> None:
        (self.work / attempt_id).mkdir(parents=True)
        (self.work / attempt_id / "cell.json").write_text(json.dumps({
            "cell": {"target": "t-fixture", "arm": "arm-claude", "replicate": 1}, "reserved_usd": 5.0, "predecessor": predecessor}),
            encoding="utf-8")

    def test_restart_and_replacement_count_each_reservation_once(self):
        (self.run_dir / "charges.jsonl").write_text(json.dumps({"at": "2026-01-01", "step": "setup probe", "usd": 1.0}) + "\n", encoding="utf-8")
        self.claim("att-001")
        in_flight = {"attempts": 0.0, "charges": 1.0, "in_flight_reserved": 5.0, "total": 6.0}
        self.assertEqual(self.run_state().spend(), in_flight)
        with self.assertRaisesRegex(run_cell.Refused, "still in flight"):
            self.choose(replace="att-001", reason="host restarted", stopped_by_harness=True)
        self.file_record("att-001", None, low=0.25)
        unknown = {"attempts": 5.0, "charges": 1.0, "in_flight_reserved": 0.0, "total": 6.0}
        self.assertEqual(self.run_state().spend(), unknown)
        self.assertEqual(self.run_state().spend(), unknown)
        pick = self.choose(replace="att-001", reason="host restarted", stopped_by_harness=True)
        self.assertEqual((pick["predecessor"], pick["replacement_index"], pick["reserved_usd"], pick["spent_before"]),
                         ("att-001", 1, 5.0, unknown))
        self.claim("att-002", predecessor="att-001")
        replacing = {"attempts": 5.0, "charges": 1.0, "in_flight_reserved": 5.0, "total": 11.0}
        for _ in range(2):
            state = self.run_state()
            self.assertEqual((state.spend(), state.replacements_used(), state.in_flight()), (replacing, 1, ["att-002"]))
        self.file_record("att-002", 0.4, disposition="valid completed", predecessor="att-001")
        state = self.run_state()
        self.assertEqual(state.spend(), {"attempts": 5.4, "charges": 1.0, "in_flight_reserved": 0.0, "total": 6.4})
        self.assertEqual((state.replacements_used(), state.attempt_ids()), (1, ["att-001", "att-002"]))

    def test_usage_beyond_its_reservation_stops_launches_until_reconciled(self):
        for name, priced, low, counted in (("priced total", 6.5, 6.5, 6.5), ("captured minimum of an unknown total", None, 6.5, 6.5)):
            with self.subTest(observed=name):
                attempt = self.run_dir / "attempts/att-001"
                if attempt.exists():
                    (attempt / "attempt.json").unlink()
                    attempt.rmdir()
                (self.run_dir / "charges.jsonl").write_text("", encoding="utf-8")
                self.file_record("att-001", priced, low=low)
                self.assertEqual(self.run_state().spend()["attempts"], counted)
                self.assertEqual(json.loads(self.runner("--status").stdout)["over_reservation"], ["att-001"])
                with self.assertRaisesRegex(run_cell.Refused, "att-001 used more than was reserved"):
                    self.choose(replace="att-001", reason="host restarted", stopped_by_harness=True)
                (self.run_dir / "charges.jsonl").write_text(json.dumps({"at": "2026-01-02", "step": "reconcile att-001 with the provider meter",
                                                                        "usd": 0.0, "reconciles": "att-001"}) + "\n", encoding="utf-8")
                self.assertEqual(self.run_state().over_reservation(), [])
                self.assertEqual(self.choose(replace="att-001", reason="host restarted", stopped_by_harness=True)["predecessor"], "att-001")

    def test_a_partly_written_record_is_not_a_filed_attempt(self):
        directory = self.run_dir / "attempts/att-001"
        directory.mkdir(parents=True)
        self.claim("att-001")
        (directory / "attempt.json").write_text('{"disposition": "stopped: earlier filing"}', encoding="utf-8")
        with patch.object(file_attempt.os, "fsync", side_effect=OSError("disk full")), self.assertRaises(OSError):
            file_attempt.publish(directory / "attempt.json", '{"disposition": "valid completed"}')
        self.assertEqual(json.loads((directory / "attempt.json").read_text(encoding="utf-8")), {"disposition": "stopped: earlier filing"})
        (directory / "attempt.json").unlink()
        state = self.run_state()
        self.assertEqual((state.filed, state.in_flight()), ({}, ["att-001"]))


class FixtureCleanup(Fixture):
    def test_valid_fixture_filed_through_the_runner_prunes_with_its_resolved_target(self):
        attempt = self.attempt()
        self.assertFalse((self.root / "bench/targets").exists())
        record = run_cell.file(self.run_state(), "att-001")
        self.assertEqual(record["disposition"], "valid completed")
        receipt = json.loads((attempt / "workspace-pruned.json").read_text(encoding="utf-8"))
        self.assertTrue(receipt["applied"])
        self.assertFalse((attempt / "clone").exists())
        self.assertTrue((attempt / "home").is_dir())

    def test_fixture_that_no_longer_matches_its_target_remains(self):
        attempt = self.attempt()
        (attempt / "clone/untracked.txt").write_text("left by the reviewer\n", encoding="utf-8")
        with self.assertRaisesRegex(run_cell.InputError, "evidence filed, but workspace cleanup failed: clone revision or working tree changed"):
            run_cell.file(self.run_state(), "att-001")
        self.assertTrue((attempt / "clone").is_dir())
        self.assertFalse((attempt / "workspace-pruned.json").exists())

    def test_failed_fixtures_remain(self):
        invalid = self.attempt("att-001")
        (invalid / "audit.json").write_text(json.dumps({"violations": ["read outside the clone: /etc/hosts"],
                                                        "diff_commands": ["git diff main...review-head"]}), encoding="utf-8")
        stopped = self.attempt("att-002", exit_code=1)
        self.stop(stopped, 1, "reviewer exit 1")
        unknown = self.attempt("att-003")
        (unknown / "stdout.jsonl").unlink()
        for attempt, disposition in ((invalid, "harness-invalid"), (stopped, "stopped")):
            with self.subTest(disposition=disposition):
                record = run_cell.file(self.run_state(), attempt.name)
                self.assertTrue(record["disposition"].startswith(disposition), record["disposition"])
                self.assertTrue((attempt / "clone").is_dir())
                self.assertFalse((attempt / "workspace-pruned.json").exists())
        with self.assertRaisesRegex(run_cell.InputError, "workspace cleanup failed: usage is incomplete"):
            run_cell.file(self.run_state(), "att-003")
        self.assertTrue((unknown / "clone").is_dir())


if __name__ == "__main__":
    unittest.main()
