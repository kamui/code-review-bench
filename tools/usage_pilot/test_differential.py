"""Independent generated accounting checks for both pilot implementations."""
from __future__ import annotations

import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
BINARY = Path(os.environ.get("USAGE_PILOT_BINARY", REPO / "rust/usage-pilot/target/release/usage-pilot"))


class DifferentialTests(unittest.TestCase):
    def run_core(self, engine: str, path: Path) -> subprocess.CompletedProcess:
        prefix = [str(BINARY)] if engine == "rust" else [sys.executable, str(ROOT / "python_core.py"), "--engine", engine]
        return subprocess.run(prefix + [str(path), "10", "50", "0.1", "1.25"], capture_output=True, check=False)

    def test_seeded_successes_equal_independent_totals_and_baseline(self):
        rng = random.Random(152)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "input.jsonl"
            for trial in range(24):
                requests = rng.randrange(1, 25)
                rows = []
                expected = {"turns": requests, "tool_calls": 0, "text_only_turns": requests,
                            "input": 0, "cache_write": 0, "cache_read": 0, "output": 0, "thinking": 0,
                            "models": [], "wall_seconds": None}
                for i in range(requests):
                    values = [rng.randrange(100000) for _ in range(4)]
                    ordinary, cached, write, output = values
                    thinking = rng.randrange(output + 1)
                    usage = dict(input_tokens=ordinary + cached + write, cached_input_tokens=cached,
                                 cache_write_input_tokens=write, output_tokens=output, reasoning_output_tokens=thinking)
                    row = {"type": "token_usage_record", "payload": {"response_id": f"r-{i}", "usage": usage}}
                    if i % 3 == 0:
                        rows.append({"type": "token_usage_record", "payload": {"response_id": f"r-{i}",
                            "usage": dict(usage, input_tokens=ordinary + cached + write + 1, output_tokens=output + 1)}})
                    rows.append(row)
                    for field, value in zip(("input", "cache_read", "cache_write", "output", "thinking"), (*values, thinking)):
                        expected[field] += value
                expected["cost"] = (expected["input"] * 10.0 + expected["cache_read"] * 10.0 * 0.1
                                    + expected["cache_write"] * 10.0 * 1.25 + expected["output"] * 50.0) / 1_000_000
                path.write_text("\n".join(json.dumps(r) for r in rows) + ("\n" if trial % 2 else ""), encoding="utf-8")
                for engine in ("existing", "typed", "rust"):
                    with self.subTest(trial=trial, engine=engine):
                        result = self.run_core(engine, path)
                        self.assertEqual(result.returncode, 0, result.stderr)
                        self.assertEqual(json.loads(result.stdout), expected)

    def test_bad_counts_and_truncated_logs_never_return_totals(self):
        valid = {"type": "token_usage_record", "payload": {"response_id": "r", "usage": {
            "input_tokens": 1, "cached_input_tokens": 0, "cache_write_input_tokens": 0,
            "output_tokens": 1, "reasoning_output_tokens": 0}}}
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "bad.jsonl"
            inputs = []
            for field in valid["payload"]["usage"]:
                for bad in (None, -1, True, 1.5, "1", 2**64):
                    row = json.loads(json.dumps(valid))
                    row["payload"]["usage"][field] = bad
                    inputs.append(json.dumps(row).encode())
            raw = json.dumps(valid).encode()
            inputs.extend(raw[:i] for i in range(1, len(raw), 11))
            inputs += [raw + b"\n{\"type\":", b"\xff", b"[]", b"null", b'{"type":"token_usage_record","payload":[]}']
            for index, data in enumerate(inputs):
                path.write_bytes(data)
                for engine in ("typed", "rust"):
                    with self.subTest(case=index, engine=engine):
                        result = self.run_core(engine, path)
                        self.assertEqual(result.returncode, 2, result.stderr)
                        self.assertEqual(result.stdout, b"")

    def test_payload_validation_only_for_consumed_record_types(self):
        usage = dict(input_tokens=1, cached_input_tokens=0, cache_write_input_tokens=0,
                     output_tokens=0, reasoning_output_tokens=0)
        valid = {"type": "token_usage_record", "payload": {"response_id": "r", "usage": usage}}
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "input.jsonl"
            for kind in ("event_msg", "future_record", "session_meta", "turn_context",
                         "response_item", "token_usage_record"):
                consumed = kind not in ("event_msg", "future_record")
                for payload in ([], "text", 42, True):
                    path.write_text(json.dumps({"type": kind, "payload": payload}) + "\n"
                                    + json.dumps(valid), encoding="utf-8")
                    engines = ("typed", "rust") if consumed else ("existing", "typed", "rust")
                    for engine in engines:
                        with self.subTest(kind=kind, payload=payload, engine=engine):
                            result = self.run_core(engine, path)
                            self.assertEqual(result.returncode, 2 if consumed else 0, result.stderr)
                            if consumed:
                                self.assertEqual(result.stdout, b"")
                            else:
                                summary = json.loads(result.stdout)
                                self.assertEqual(summary["turns"], 1)
                                self.assertEqual(summary["input"], 1)

    def test_large_ordinal_and_timestamp_forms(self):
        usage = dict(input_tokens=1, cached_input_tokens=0, cache_write_input_tokens=0,
                     output_tokens=0, reasoning_output_tokens=0)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "input.jsonl"
            for first, last, valid in [
                ("20261018T100000Z", "20261018T100004Z", True),
                ("2026-10-18T10:00:00", "2026-10-18T10:00:04", True),
                ("2026-10-18T10:00:00", "2026-10-18T10:00:04Z", False),
            ]:
                rows = [{"type": "token_usage_record", "ordinal": 2**100 + i,
                         "timestamp": stamp, "payload": {"usage": usage}}
                        for i, stamp in enumerate((first, last))]
                path.write_text("\n".join(json.dumps(r) for r in rows), encoding="utf-8")
                for engine in ("typed", "rust"):
                    result = self.run_core(engine, path)
                    self.assertEqual(result.returncode, 0 if valid else 2, result.stderr)
                    if valid:
                        summary = json.loads(result.stdout)
                        self.assertEqual(summary["turns"], 2)
                        self.assertEqual(summary["wall_seconds"], 4)
                    else:
                        self.assertEqual(result.stdout, b"")


if __name__ == "__main__":
    unittest.main()
