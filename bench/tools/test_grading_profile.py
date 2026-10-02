#!/usr/bin/env python3
"""Drive grading_profile.py through subprocess on synthetic grading archives.

Usage: python3 bench/tools/test_grading_profile.py
Exit codes: 0 every test passed; 1 a test failed.
"""

from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("grading_profile.py")
MODEL = "test-model"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def line(request, usage, command=None, tool_id="t1"):
    content = [{"type": "tool_use", "id": tool_id, "name": "Bash", "input": {"command": command}}] if command else []
    return {"type": "assistant", "requestId": request, "timestamp": "2026-01-01T00:00:01Z",
            "message": {"model": MODEL, "usage": usage, "content": content}}


def usage(fresh, write, read, output, thinking=0, tier="ephemeral_1h_input_tokens"):
    return {"input_tokens": fresh, "cache_creation_input_tokens": write, "cache_read_input_tokens": read,
            "output_tokens": output, "output_tokens_details": {"thinking_tokens": thinking},
            "cache_creation": {tier: write}}


ACCEPTED = [line("r1", usage(10, 1000, 0, 40, 20), "cat reviews/blind-000000.md"),
            line("r1", usage(10, 1000, 0, 100, 20), "cat reviews/blind-000000.md"),
            line("r2", usage(0, 500, 1010, 50), "cd clone && sed -n 1,9p main.go", "t2"),
            line("r3", usage(0, 300, 1510, 200), "cat > verdicts.json <<'EOF'\n{}\nEOF", "t3"),
            line("r4", usage(0, 250, 1810, 30))]
REJECTED = [line("r5", usage(5, 100, 0, 10, tier="ephemeral_5m_input_tokens"), "ls")]


class Profile(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.write("bench/rates.json", {"rates": [{"model": MODEL, "as_of": "2026-01-01", "input": 4.0, "output": 20.0,
                                                    "cache_read": 0.2, "cache_write_5m": 5.0, "cache_write_1h": 8.0}]})
        mapping = self.write("bench/runs/run-a/scoring/t-one/mapping.v1.json", {"attempts": [{"items": [
            {"claims": [{}, {}, {}]}, {"claims": [{}, {}]}, {"claims": [{}]}]}]})
        accepted = self.archive("run-a/t-one/attempt-2", ACCEPTED, 0.024906, reviews=[2, 1])
        self.archive("run-a/t-one/attempt-1", REJECTED, 0.00072, reviews=[2, 1], violations=["read outside the workspace"])
        self.archive("run-b/t-one/attempt-1", None, None)
        self.completion = self.write("completion.json", {"batches": [{
            "evidence": {"path": str(accepted.relative_to(self.root)), "sha256": digest(accepted)},
            "mapping": {"path": str(mapping.relative_to(self.root)), "sha256": digest(mapping)}}]})

    def write(self, name, value) -> Path:
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def archive(self, name, lines, cost, reviews=(), violations=()) -> Path:
        directory = self.root / "archives" / name
        directory.mkdir(parents=True)
        files = {"prepare.log": b"prepared\n"}
        if lines is not None:
            files["key.json"] = json.dumps({"reviews": [{"items": count} for count in reviews]}).encode()
            files["reservation.json"] = json.dumps({"maxBudgetUsd": 2.0}).encode()
            files["work/dispatch.json"] = json.dumps({
                "model": MODEL, "exit_code": 0, "audit_violations": list(violations), "verdicts_present": True,
                "usage": {"priced_total_usd": cost}, "dispatched_at": "2026-01-01T00:00:00Z",
                "completed_at": "2026-01-01T00:01:40Z"}).encode()
            files["work/home/.claude/projects/-work/session.jsonl"] = "".join(
                json.dumps(entry) + "\n" for entry in lines).encode()
        with tarfile.open(directory / "evidence.tar.gz", "w:gz") as bundle:
            for member, data in files.items():
                info = tarfile.TarInfo(member)
                info.size = len(data)
                bundle.addfile(info, io.BytesIO(data))
        archive = directory / "evidence.tar.gz"
        return self.write(f"archives/{name}/evidence.json", {
            "archive": {"path": str(archive.relative_to(self.root)), "sha256": digest(archive)}})

    def run_cli(self, *options) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, str(SCRIPT), "--archives", str(self.root / "archives"), "--completion",
                               str(self.completion), "--root", str(self.root), *options],
                              capture_output=True, text=True, encoding="utf-8")

    def report(self) -> dict:
        done = self.run_cli()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        return json.loads(done.stdout)

    def test_repeated_lines_of_one_request_are_billed_once(self):
        accepted = self.report()["accepted"]
        self.assertEqual((accepted["transcript_lines"], accepted["billed_requests"]), (5, 4))
        self.assertEqual(accepted["tokens"], {"input": 10, "cache_write_5m": 0, "cache_write_1h": 2050,
                                              "cache_write_unknown": 0, "cache_read": 4330, "output": 380,
                                              "thinking": 20})
        self.assertEqual(accepted["cost_usd"], 0.024906)
        self.assertEqual(accepted["cost_by_category_usd"], {"fresh_input": 0.00004, "cache_write": 0.0164,
                                                            "cache_read": 0.000866, "visible_output": 0.0072,
                                                            "thinking": 0.0004})
        self.assertEqual((accepted["tool_calls"], accepted["text_only_requests"]), (3, 1))

    def test_accepted_and_failed_attempts_are_reported_apart(self):
        report = self.report()
        accepted, failed = report["accepted"], report["failed"]
        self.assertEqual((accepted["attempts"], accepted["reviews"], accepted["items"], accepted["claims"]), (1, 2, 3, 6))
        self.assertEqual((accepted["cost_per_review_usd"], accepted["cost_per_item_usd"], accepted["cost_per_claim_usd"]),
                         (0.012453, 0.008302, 0.004151))
        self.assertEqual(accepted["dispatch_seconds"]["median"], 100)
        self.assertEqual((failed["attempts"], failed["attempts_with_billed_requests"], failed["unpriced_attempts"]), (2, 1, 1))
        self.assertEqual(failed["reasons"], {"access violation": 1, "not dispatched": 1})
        self.assertEqual((failed["cost_usd"], failed["tokens"]["cache_write_5m"]), (0.00072, 100))
        self.assertEqual(report["retry_cost_share"], 0.0281)

    def test_context_growth_is_attributed_to_the_turn_that_caused_it(self):
        estimate = self.report()["accepted"]["exploration_estimate"]
        rows = estimate["by_turn"]
        self.assertEqual({name: (row["turns"], row["context_growth_tokens"], row["carried_cost_usd"])
                          for name, row in rows.items() if row["turns"]},
                         {"inputs": (1, 500, 0.0042), "source": (1, 300, 0.00246), "verdicts": (1, 250, 0.002),
                          "none": (1, 0, 0)})
        self.assertEqual(estimate["initial_context_carried_cost_usd"], 0.008646)
        self.assertEqual(estimate["unattributed_cache_cost_usd"], 0)
        self.assertEqual(estimate["source_share_of_cost"], {"low": 0.0988, "high": 0.0988})

    def test_changed_archive_mapping_or_recorded_cost_fails(self):
        archive = self.root / "archives/run-a/t-one/attempt-1/evidence.tar.gz"
        archive.write_bytes(archive.read_bytes() + b"\0")
        mapping = self.root / "bench/runs/run-a/scoring/t-one/mapping.v1.json"
        mapping.write_text(json.dumps({"attempts": []}))
        done = self.run_cli()
        self.assertEqual(done.returncode, 1)
        self.assertEqual(done.stdout.splitlines(), ["mapping changed: bench/runs/run-a/scoring/t-one/mapping.v1.json",
                                                    "archive changed: archives/run-a/t-one/attempt-1/evidence.tar.gz"])

    def test_request_billed_in_two_attempts_fails(self):
        self.archive("run-c/t-one/attempt-1", REJECTED, 0.00072, reviews=[1])
        done = self.run_cli()
        self.assertEqual((done.returncode, done.stdout.strip()),
                         (1, "1 request id(s) appear in more than one attempt and would be billed twice"))

    def test_markdown_reports_the_same_totals_and_unreadable_input_exits_2(self):
        done = self.run_cli("--markdown")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("| Total | $0.024906 | $0.000720 |", done.stdout)
        self.assertIn("| source | 1 | 1 | 300 | $0.002460 | 9.9% |", done.stdout)
        self.completion.unlink()
        done = self.run_cli()
        self.assertEqual(done.returncode, 2)
        self.assertIn("grading_profile.py:", done.stderr)


if __name__ == "__main__":
    unittest.main()
