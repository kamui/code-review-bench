#!/usr/bin/env python3
"""Check metering against duplicate requests, incomplete dispatches, and changed evidence."""

import importlib.util
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("calibration_measure", Path(__file__).with_name("measure.py"))
measure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(measure)


def fixture(root, session="session-1", timeout=False):
    path = root / session / "work/dispatch.json"
    path.parent.mkdir(parents=True)
    rate = {"model": "gpt-6-astra", "input": 10.0, "output": 50.0, "cache_read": 1.0, "cache_write_5m": 12.5}
    record = {"session_id": session, "model": "gpt-6-astra", "models_observed": ["gpt-6-astra"],
              "subagents": 0, "exit_code": None if timeout else 0, "audit_violations": [],
              "verdicts_present": not timeout, "dispatched_at": "2026-10-02T09:00:00Z",
              "completed_at": "2026-10-02T09:15:00Z",
              "usage": {"rate": rate, "requests": 1, "priced_total_usd": 0.00915,
                        "low": 0.00915, "high": None if timeout else 0.00915}}
    path.write_text(json.dumps(record))
    usage = {"type": "token_usage_record", "payload": {"response_id": session + "-response-1", "usage": {
        "input_tokens": 1000, "cached_input_tokens": 400, "cache_write_input_tokens": 100,
        "output_tokens": 50, "reasoning_output_tokens": 10}}}
    events = [{"type": "session_meta", "payload": {"id": session}},
              {"type": "turn_context", "payload": {"model": "gpt-6-astra"}},
              {"type": "response_item", "payload": {"type": "function_call"}}, usage, usage,
              {"type": "response_item", "payload": {"type": "function_call"}}]
    transcript = path.parent / "home/.codex/sessions/rollout-test.jsonl"
    transcript.parent.mkdir(parents=True)
    transcript.write_text("".join(json.dumps(row) + "\n" for row in events))
    return path, transcript


def archive_fixture(root, dispatch, transcript):
    archive = root / "evidence.tar.gz"
    files = {"work/dispatch.json": dispatch.read_bytes(),
             "work/home/.codex/sessions/rollout-test.jsonl": transcript.read_bytes()}
    write_archive(archive, files)
    receipt = root / "evidence.json"
    receipt.write_text(json.dumps({"archive": {"path": str(archive), "sha256": measure.digest(archive.read_bytes())},
                                   "files": [{"path": name, "sha256": measure.digest(data)}
                                             for name, data in files.items()]}))
    return receipt, archive, files


def write_archive(path, files):
    with tarfile.open(path, "w:gz") as bundle:
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            bundle.addfile(info, io.BytesIO(data))


class MeterTest(unittest.TestCase):
    def test_duplicate_response_is_one_billed_turn_and_late_tools_are_unattributed(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            dispatch, _ = fixture(root)
            attempt = measure.measure_attempt(dispatch, root, "accepted")
            counts = attempt["observed"]
            self.assertEqual(counts["turns"], 1)
            self.assertEqual(counts["input"], 500)
            self.assertEqual(counts["cache_read"], 400)
            self.assertEqual(counts["cache_write"], 100)
            self.assertEqual(counts["output"], 50)
            self.assertEqual(counts["thinking"], 10)
            self.assertEqual(counts["tool_turns"], 1)
            self.assertEqual(counts["tool_calls"], 1)
            self.assertEqual(counts["recorded_tool_calls"], 2)
            self.assertEqual(counts["unattributed_tool_calls"], 1)
            self.assertEqual(attempt["observed_cost_usd"], 0.00915)

    def test_timeout_subtotal_does_not_make_the_total_charge_known(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            accepted, _ = fixture(root, "accepted")
            failed, _ = fixture(root, "timeout", timeout=True)
            report = measure.report([(accepted, "accepted"), (failed, "failed")], root)
            total = report["raw_total"]
            self.assertEqual(total["known_observed_cost_usd_sum"], 0.0183)
            self.assertEqual(total["unknown_charge_upper_attempts"], 1)
            self.assertIsNone(total["charge_upper_usd"])
            self.assertEqual(report["by_disposition"]["accepted"]["charge_upper_usd"], 0.00915)
            self.assertEqual(report["by_disposition"]["failed"]["known_observed_cost_usd_sum"], 0.00915)
            self.assertEqual(report["by_disposition"]["failed"]["dispatch_seconds_known_sum"], 900)

    def test_archive_and_selected_member_hashes_are_both_enforced(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            dispatch, transcript = fixture(root)
            receipt, archive, files = archive_fixture(root, dispatch, transcript)
            self.assertEqual(measure.measure_attempt(receipt, root)["observed"]["turns"], 1)
            archive.write_bytes(archive.read_bytes() + b"changed")
            with self.assertRaisesRegex(ValueError, "archive hash differs"):
                measure.measure_attempt(receipt, root)
            files["work/home/.codex/sessions/rollout-test.jsonl"] += b"\n"
            write_archive(archive, files)
            saved = json.loads(receipt.read_text())
            saved["archive"]["sha256"] = measure.digest(archive.read_bytes())
            receipt.write_text(json.dumps(saved))
            with self.assertRaisesRegex(ValueError, "member hash differs"):
                measure.measure_attempt(receipt, root)

    def test_wrong_session_or_model_cannot_be_metered_as_this_dispatch(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            dispatch, transcript = fixture(root)
            original = transcript.read_text()
            transcript.write_text(original.replace('"id": "session-1"', '"id": "another-session"'))
            with self.assertRaisesRegex(ValueError, "session differs"):
                measure.measure_attempt(dispatch, root)
            transcript.write_text(original.replace('"model": "gpt-6-astra"', '"model": "other-model"'))
            with self.assertRaisesRegex(ValueError, "model differs"):
                measure.measure_attempt(dispatch, root)

    def test_duplicate_attempts_are_refused_and_timeout_cannot_be_accepted(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            dispatch, _ = fixture(root)
            with self.assertRaisesRegex(ValueError, "repeated session or response"):
                measure.report([(dispatch, "accepted"), (dispatch, "failed")], root)
            failed, _ = fixture(root, "timeout", timeout=True)
            with self.assertRaisesRegex(ValueError, "accepted attempt"):
                measure.measure_attempt(failed, root, "accepted")

    def test_missing_saved_usage_is_not_inferred_from_tool_calls(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            dispatch, transcript = fixture(root, timeout=True)
            events = [json.loads(line) for line in transcript.read_text().splitlines()]
            transcript.write_text("".join(json.dumps(row) + "\n" for row in events
                                          if row["type"] != "token_usage_record"))
            record = json.loads(dispatch.read_text())
            record["usage"].update(requests=0, priced_total_usd=None, low=None)
            dispatch.write_text(json.dumps(record))
            attempt = measure.measure_attempt(dispatch, root)
            self.assertIsNone(attempt["observed_cost_usd"])
            self.assertEqual(attempt["observed"]["turns"], 0)
            self.assertEqual(attempt["observed"]["recorded_tool_calls"], 2)
            self.assertEqual(measure.summarize([attempt])["attempts_without_observed_cost"], 1)


if __name__ == "__main__":
    unittest.main()
