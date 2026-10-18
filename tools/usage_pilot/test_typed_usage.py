"""Behavioral checks for the isolated Python usage pilot and CLI boundaries."""

from contextlib import redirect_stdout
from dataclasses import is_dataclass
import io
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from bench.tools import codex_usage, transcript_usage
from tools.usage_pilot import adapter, typed_usage as typed

ROOT = Path(__file__).resolve().parents[2]


def record(response="r", **counts):
    usage = dict.fromkeys(typed.COUNT_FIELDS, 0)
    usage.update(counts)
    return {"type": "token_usage_record", "payload": {"response_id": response, "usage": usage}}


def assistant(**counts):
    usage = dict.fromkeys(adapter.TRANSCRIPT_COUNTS, 0)
    usage.update(counts)
    return {"type": "assistant", "requestId": "r", "message": {
        "model": "m", "usage": usage, "content": [],
    }}


class UsageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.path = self.directory / "log.jsonl"

    def write(self, *records):
        self.path.write_text("".join(json.dumps(item) + "\n" for item in records), encoding="utf-8")
        return self.path

    def summary(self, *records, prices=(10.0, 50.0)):
        self.write(*records)
        return typed.summarize(typed.read_rollout(self.path), prices, 0.1, 1.25)

    def cli(self, program="codex", *args):
        command = [sys.executable, str(ROOT / "tools/usage_pilot/adapter.py"), program]
        if program == "codex":
            command += ["--engine", "typed"]
        return subprocess.run([*command, str(self.path), *args, "--prices", "10,50", "--json"],
                              capture_output=True, text=True)

    def test_known_zero_and_summary_shape(self):
        summary = self.summary(record())
        self.assertTrue(is_dataclass(summary))
        self.assertEqual(summary.turns, 1)
        self.assertEqual(summary.cost, 0.0)
        self.assertIs(type(summary.cost), float)
        baseline = codex_usage.summarize(codex_usage.read_rollout(self.path), (10.0, 50.0), 0.1, 1.25)
        self.assertEqual(summary.to_dict(), baseline)
        self.assertEqual(list(summary.to_dict()), list(baseline))

    def test_each_unknown_count_is_retained_until_summary(self):
        for field in typed.COUNT_FIELDS:
            for absent in (False, True):
                with self.subTest(field=field, absent=absent):
                    item = record(**{field: None})
                    if absent:
                        del item["payload"]["usage"][field]
                    self.write(item)
                    rollout = typed.read_rollout(self.path)
                    self.assertIsInstance(getattr(rollout.requests[0].tokens, field), typed.UnknownCount)
                    with self.assertRaisesRegex(codex_usage.RolloutError, f"request 1 missing {field}$"):
                        typed.summarize(rollout, (10.0, 50.0), 0.1, 1.25)

    def test_unknown_can_be_replaced_but_bad_count_cannot(self):
        summary = self.summary(record(input_tokens=None), record(input_tokens=9))
        self.assertEqual(summary.input, 9)
        with self.assertRaisesRegex(codex_usage.RolloutError, "missing input_tokens"):
            self.summary(record(), record(input_tokens=None))
        for value in (True, False, -1, 0.0, 1.5, "2", [], {}, typed.U64_MAX + 1):
            for field in typed.COUNT_FIELDS:
                with self.subTest(value=value, field=field):
                    self.write(record(**{field: value}), record())
                    with self.assertRaisesRegex(codex_usage.RolloutError, field):
                        typed.read_rollout(self.path)

    def test_integer_limits_and_aggregate_overflow(self):
        for count in (0, (1 << 53) + 1, typed.U64_MAX):
            summary = self.summary(record(input_tokens=count, output_tokens=count), prices=(0.0, 0.0))
            self.assertEqual((summary.input, summary.output), (count, count))
        for field in typed.COUNT_FIELDS:
            values = {field: typed.U64_MAX}
            if field in ("cached_input_tokens", "cache_write_input_tokens"):
                values["input_tokens"] = typed.U64_MAX
            other = {field: 1}
            if field in ("cached_input_tokens", "cache_write_input_tokens"):
                other["input_tokens"] = 1
            with self.subTest(field=field), self.assertRaisesRegex(codex_usage.RolloutError, "exceeds u64"):
                self.summary(record("a", **values), record("b", **other))

    def test_cache_subset_error_is_exact(self):
        with self.assertRaises(codex_usage.RolloutError) as error:
            self.summary(record(input_tokens=10, cached_input_tokens=8, cache_write_input_tokens=8))
        self.assertEqual(str(error.exception), f"{self.path}: request 1 has cached+cache_write (8+8) above input_tokens (10)")

    def test_last_record_first_tool_attribution_and_pending_tools(self):
        tool = {"type": "response_item", "payload": {"type": "function_call"}}
        summary = self.summary(tool, record("a", input_tokens=1), tool,
                               record("a", input_tokens=8), record("b", input_tokens=2), record("c"))
        self.assertEqual((summary.turns, summary.input, summary.tool_calls, summary.text_only_turns), (3, 10, 2, 1))
        rollout = typed.read_rollout(self.path)
        self.assertEqual([item.tool_calls for item in rollout.requests], [1, 1, 0])

    def test_fallback_identity_never_collides_with_response_id(self):
        summary = self.summary(record("ordinal-None", input_tokens=7), record(None, input_tokens=3),
                               record("", input_tokens=5))
        self.assertEqual((summary.turns, summary.input), (2, 12))
        first = record(None, input_tokens=1)
        first["ordinal"] = 2
        second = record(None, input_tokens=4)
        second["ordinal"] = "2"
        self.assertEqual(self.summary(first, second).input, 4)
        first["ordinal"] = 1 << 100
        self.assertEqual(self.summary(first).input, 1)
        for ordinal in (True, 1.0, [], {}):
            first["ordinal"] = ordinal
            with self.subTest(ordinal=ordinal), self.assertRaisesRegex(codex_usage.RolloutError, "ordinal"):
                self.summary(first)
        for identity in (False, 0, 42, [], {}):
            with self.subTest(identity=identity), self.assertRaisesRegex(codex_usage.RolloutError, "response_id"):
                self.summary(record(identity))

    def test_session_ids_are_validated_and_distinct(self):
        meta = {"type": "session_meta", "payload": {"id": "s", "parent_thread_id": "p"}}
        self.write(meta, record())
        rollout = typed.read_rollout(self.path)
        self.assertEqual(rollout.session_id, typed.SessionId("s"))
        self.assertNotEqual(typed.SessionId("s"), typed.ResponseId("s"))
        for field in ("id", "session_id", "parent_thread_id"):
            meta["payload"] = {field: 4}
            with self.subTest(field=field), self.assertRaisesRegex(codex_usage.RolloutError, field):
                self.summary(meta, record())

    def test_timestamps_include_replaced_records_and_truncate(self):
        records = [record("a"), record("b"), record("a"), record("c")]
        stamps = ["2026-10-18T10:00:00.2Z", "2026-10-18T12:00:03.9+02:00", "garbage", None]
        for item, stamp in zip(records, stamps):
            item["timestamp"] = stamp
        self.assertEqual(self.summary(*records).wall_seconds, 3)
        records[0]["timestamp"] = 42
        self.assertEqual(self.summary(*records).wall_seconds, 0)

    def test_nonfinite_prices_and_cost(self):
        for price in (float("nan"), float("inf"), -1.0, True):
            with self.subTest(price=price), self.assertRaises(codex_usage.RolloutError):
                self.summary(record(), prices=(price, 1.0))
        with self.assertRaisesRegex(codex_usage.RolloutError, "cost must be finite"):
            self.summary(record(input_tokens=typed.U64_MAX), prices=(1e308, 1.0))

    def test_malformed_input_and_unknown_cli_have_no_stdout(self):
        self.write(record())
        with self.path.open("a") as handle:
            handle.write('{"type":\n')
        result = self.cli()
        self.assertEqual((result.returncode, result.stdout), (2, ""))
        self.assertIn(f"{self.path}:2: malformed JSON", result.stderr)
        self.write(record(input_tokens=None))
        result = self.cli()
        self.assertEqual((result.returncode, result.stdout), (2, ""))
        self.assertEqual(result.stderr, f"codex_usage.py: {self.path}: request 1 missing input_tokens\n")

    def test_malformed_shapes_and_utf8_fail_cleanly(self):
        for item in ([], {"type": "token_usage_record", "payload": []},
                     {"type": "token_usage_record", "payload": {}},
                     {"type": "turn_context", "payload": {"model": False}}):
            with self.subTest(item=item):
                self.write(item)
                result = self.cli()
                self.assertEqual((result.returncode, result.stdout), (2, ""))
                self.assertNotIn("Traceback", result.stderr)
        self.path.write_bytes(b"\xff\n")
        result = self.cli()
        self.assertEqual((result.returncode, result.stdout), (2, ""))
        self.assertIn("invalid UTF-8", result.stderr)
        self.assertEqual(self.summary({"type": "unrelated", "payload": []}, record()).turns, 1)

    def test_transcript_u2_u3_and_known_zero(self):
        self.write(assistant())
        result = self.cli("transcript")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["total"]["input"], 0)
        with self.path.open("a") as handle:
            handle.write("{\n")
        result = self.cli("transcript")
        self.assertEqual((result.returncode, result.stdout), (2, ""))
        self.assertEqual(result.stderr, f"transcript_usage: {self.path}:2: malformed JSON\n")
        for field in adapter.TRANSCRIPT_COUNTS:
            self.write(assistant(**{field: None}))
            result = self.cli("transcript")
            self.assertEqual((result.returncode, result.stdout), (2, ""))
            self.assertEqual(result.stderr, f"transcript_usage: {self.path}: request 1 missing {field}\n")

    def test_transcript_unknown_tier_is_preserved(self):
        self.write(assistant(cache_creation_input_tokens=9))
        result = self.cli("transcript")
        self.assertEqual(result.returncode, 0, result.stderr)
        total = json.loads(result.stdout)["total"]
        self.assertEqual(total["cache_write_unknown"], 9)
        self.assertFalse(total["cost_bounds"]["tiers_known"])
        self.assertLess(total["cost_bounds"]["low"], total["cost_bounds"]["high"])

    def test_adapter_restores_production_functions(self):
        self.write(record())
        functions = (codex_usage.read_rollout, codex_usage.summarize, codex_usage.add,
                     transcript_usage.read_transcript)
        with redirect_stdout(io.StringIO()):
            self.assertEqual(adapter.main(["codex", "--engine", "typed", str(self.path), "--prices", "1,1", "--json"]), 0)
        self.assertEqual(functions, (codex_usage.read_rollout, codex_usage.summarize, codex_usage.add,
                                     transcript_usage.read_transcript))

    def test_cli_checks_cross_file_totals_before_rendering(self):
        self.write(record(input_tokens=typed.U64_MAX))
        result = self.cli("codex", str(self.path))
        self.assertEqual((result.returncode, result.stdout), (2, ""))
        self.assertIn("TOTAL: input exceeds u64", result.stderr)

    def test_rust_boundary_arguments_and_structured_error(self):
        summary = self.summary(record(input_tokens=4)).to_dict()
        binary = self.directory / "rust-pilot"
        with patch.object(adapter.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, json.dumps(summary).encode(), b"")) as run:
            self.assertEqual(adapter.summarize_rust({"path": str(self.path)}, (10.0, 50.0), 0.1, 1.25, binary=binary), summary)
            self.assertEqual(run.call_args.args[0], [str(binary), str(self.path), "10.0", "50.0", "0.1", "1.25"])
        error = {"kind": "usage", "detail": "path: request 1 missing input_tokens"}
        with patch.object(adapter.subprocess, "run", return_value=subprocess.CompletedProcess([], 2, b"", json.dumps(error).encode())):
            with self.assertRaises(codex_usage.RolloutError) as raised:
                adapter.summarize_rust({"path": "path"}, (1.0, 1.0), 0.1, 1.25, binary=binary)
            self.assertEqual(str(raised.exception), error["detail"])

    def test_non_utf8_path_is_not_reencoded(self):
        if os.name != "posix":
            self.skipTest("POSIX filename bytes required")
        self.path = Path(os.fsdecode(os.fsencode(self.directory) + b"/log-\xff.jsonl"))
        self.write(record())
        result = self.cli()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["threads"][0]["path"], str(self.path))

    def test_deterministic_valid_rollouts_match_baseline(self):
        rng = random.Random(152)
        for _ in range(40):
            records = []
            for _ in range(rng.randrange(1, 40)):
                cached, write, ordinary = (rng.randrange(10000) for _ in range(3))
                if rng.randrange(2):
                    records.append({"type": "response_item", "payload": {"type": "custom_tool_call"}})
                records.append(record(str(rng.randrange(8)), input_tokens=ordinary + cached + write,
                                      cached_input_tokens=cached, cache_write_input_tokens=write,
                                      output_tokens=rng.randrange(1000), reasoning_output_tokens=rng.randrange(100)))
            result = self.summary(*records).to_dict()
            baseline = codex_usage.summarize(codex_usage.read_rollout(self.path), (10.0, 50.0), 0.1, 1.25)
            self.assertEqual(result, baseline)

    def test_core_repeats_and_counts_only_measured_work(self):
        self.write(record(input_tokens=40))
        for engine in ("existing", "typed"):
            result = subprocess.run([
                sys.executable, str(ROOT / "tools/usage_pilot/python_core.py"), "--engine", engine,
                str(self.path), "10", "50", "0.1", "1.25", "3", "2",
            ], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            summaries = [json.loads(line) for line in result.stdout.splitlines()]
            self.assertEqual(len(summaries), 3)
            self.assertTrue(all(summary["input"] == 40 for summary in summaries))
            metrics = json.loads(result.stderr)
            self.assertEqual(metrics["completed_iterations"], 3)
            self.assertEqual(metrics["bytes_processed"], 3 * self.path.stat().st_size)
            self.assertGreater(metrics["elapsed_seconds"], 0)
            self.assertGreater(metrics["cpu_seconds"], 0)

    def test_unchanged_behavior_corpus(self):
        command_map = self.directory / "commands.json"
        prefix = [sys.executable, str(ROOT / "tools/usage_pilot/adapter.py")]
        command_map.write_text(json.dumps({"codex": [*prefix, "codex", "--engine", "typed"],
                                           "transcript": [*prefix, "transcript"]}), encoding="utf-8")
        result = subprocess.run([sys.executable, str(ROOT / "tools/behavior_contract.py"),
                                 "--implementations", str(command_map)], capture_output=True, text=True, cwd=ROOT)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.count("PASS "), 23, result.stdout)


if __name__ == "__main__":
    unittest.main()
