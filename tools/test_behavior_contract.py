#!/usr/bin/env python3
"""Exercise the contract CLI, including deliberately wrong local implementations.

Usage: python3 -m unittest discover -s tools -p test_behavior_contract.py -v
Inputs: synthetic programs, fixture bytes and disposable filesystem roots.
Exit codes: unittest's 0 success or 1 failure.
"""
from __future__ import annotations

import base64
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import behavior_contract as contract


def encoded(value):
    return base64.b64encode(value).decode("ascii")


class ContractTests(unittest.TestCase):
    def run_cli(self, *args):
        return subprocess.run([sys.executable, str(Path(contract.__file__)), *map(str, args)],
                              capture_output=True, text=True, encoding="utf-8", timeout=60)

    def test_baseline_independent_and_differential(self):
        with tempfile.TemporaryDirectory() as temporary:
            commands = Path(temporary) / "commands.json"
            commands.write_text(json.dumps({name: [sys.executable, str(contract.REPO / "bench/tools" / f"{name}_usage.py")]
                                           for name in ("codex", "transcript")}), encoding="utf-8")
            result = self.run_cli("--allow-known-defects", "--differential", commands)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("KNOWN DEFECT unknown-is-not-zero", result.stdout)
            self.assertNotIn("FAIL", result.stdout)
            strict = self.run_cli()
            self.assertEqual(strict.returncode, 1, strict.stderr)
            self.assertIn("FAIL unknown-is-not-zero", strict.stdout)
            self.assertIn("exit: expected 2, actual 0", strict.stdout)

    def test_wrong_outputs_bytes_and_duplicate_effects(self):
        initial = [{"kind": "file", "path_b64": encoded(b"hashed"), "bytes_b64": encoded(b"a\n")}]
        base = {"id": "probe", "program": "probe", "argv": [], "stdin_b64": encoded(b"input\x00\xff"),
                "initial": initial, "derivation": "Synthetic local negative demonstration"}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            program = directory / "probe.py"
            corpus = directory / "corpus.json"
            decisions = directory / "decisions.json"
            commands = directory / "commands.json"
            commands.write_text(json.dumps({"probe": [sys.executable, str(program)]}), encoding="utf-8")
            correct = "from pathlib import Path\nimport sys\nassert sys.stdin.buffer.read() == b'input\\x00\\xff'\nPath('journal').write_bytes(b'publish-once\\n')\nprint('{\"count\":1,\"unknown\":null}')\n"
            program.write_text(correct, encoding="utf-8")
            actual = contract.execute(base, {"probe": [sys.executable, str(program)]})
            base["expected"] = {"exit": 0, "stdout": {"mode": "json", "value": {"count": 1, "unknown": None}},
                                "stderr": {"mode": "bytes", "bytes_b64": ""}, "filesystem": actual["filesystem"]}
            # Only this synthetic driver snapshots setup; pilot expectations are independently derived.
            corpus.write_text(json.dumps({"format": "behavior-contract/1", "decisions": "decisions.json", "cases": [base]}), encoding="utf-8")
            decisions.write_text(json.dumps({"probe": {"reason": "test", "contract_sha256": contract.digest(base)}}), encoding="utf-8")
            for source, expected in (
                (correct, "PASS probe"),
                (correct.replace('count":1', 'count":2'), "semantic"),
                (correct + "Path('hashed').write_bytes(b'a')\n", "filesystem effects"),
                (correct + "with Path('journal').open('ab') as f: f.write(b'publish-once\\n')\n", "filesystem effects"),
            ):
                program.write_text(source, encoding="utf-8")
                result = self.run_cli("--corpus", corpus, "--implementations", commands)
                self.assertEqual(result.returncode, 0 if expected == "PASS probe" else 1, result.stdout + result.stderr)
                self.assertIn(expected, result.stdout)
            changed = copy.deepcopy(base)
            changed["expected"]["exit"] = 9
            corpus.write_text(json.dumps({"format": "behavior-contract/1", "decisions": "decisions.json", "cases": [changed]}), encoding="utf-8")
            result = self.run_cli("--corpus", corpus, "--implementations", commands)
            self.assertEqual(result.returncode, 2)
            self.assertIn("explicit decision", result.stderr)

    def test_comparison_contract(self):
        actual = {"exit": 0, "stdout": b'{"snow":"\\u96ea","missing":null}\n', "stderr": b"", "filesystem": []}
        expected = {"exit": 0, "stdout": {"mode": "json", "value": {"missing": None, "snow": "雪"}},
                    "stderr": {"mode": "bytes", "bytes_b64": ""}, "filesystem": []}
        self.assertEqual(contract.compare(expected, actual), [])
        for value in ({"snow": "雪"}, {"snow": "雪", "missing": 0}):
            changed = copy.deepcopy(expected)
            changed["stdout"]["value"] = value
            self.assertTrue(contract.compare(changed, actual))
        for raw in (b'{"missing":null,"missing":null}', b'{"value":NaN}'):
            self.assertTrue(contract.compare(expected, dict(actual, stdout=raw)))
        exact = dict(expected, stdout={"mode": "bytes", "bytes_b64": encoded(actual["stdout"])})
        self.assertTrue(contract.compare(exact, dict(actual, stdout=actual["stdout"].rstrip())))
        for raw in (b"../outside", b"/absolute", b"a/../b", b"a\0b"):
            with self.assertRaises(ValueError):
                contract.relative(encoded(raw))

    def test_removed_case_needs_decision(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            corpus = json.loads(contract.CORPUS.read_text(encoding="utf-8"))
            decisions = contract.CORPUS.parent / corpus["decisions"]
            (root / corpus["decisions"]).write_bytes(decisions.read_bytes())
            corpus["cases"] = [case for case in corpus["cases"] if case["id"] != "unknown-is-not-zero"]
            path = root / "corpus.json"
            path.write_text(json.dumps(corpus), encoding="utf-8")
            result = self.run_cli("--corpus", path, "--allow-known-defects")
            self.assertEqual(result.returncode, 2, result.stdout)
            self.assertIn("removal needs an explicit decision", result.stderr)

    def test_differential_recognizes_only_independent_defect_fixes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wrapper = root / "fixed.py"
            corpus = contract.load_corpus(contract.CORPUS)
            defects = [case for case in corpus["cases"] if "baseline_defect" in case]
            errors = {case["argv"][0]: contract.decode(case["expected"]["stderr"]["bytes_b64"]).decode("utf-8")
                      for case in defects}
            source = ("import os, sys\n"
                      f"errors = {errors!r}\n"
                      "program, args = sys.argv[1], sys.argv[2:]\n"
                      "if args[0] in errors:\n"
                      "    sys.stderr.write(errors[args[0]])\n"
                      "    sys.exit(2)\n"
                      f"script = {str(contract.REPO / 'bench/tools')!r} + '/' + program + '_usage.py'\n"
                      "os.execv(sys.executable, [sys.executable, script, *args])\n")
            wrapper.write_text(source, encoding="utf-8")
            commands = root / "fixed.json"
            commands.write_text(json.dumps({name: [sys.executable, str(wrapper), name]
                                            for name in ("codex", "transcript")}), encoding="utf-8")
            result = self.run_cli("--allow-known-defects", "--differential", commands)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            for case in defects:
                self.assertIn("APPROVED FIX " + case["id"], result.stdout)
            baseline = root / "baseline.json"
            baseline.write_text(json.dumps({name: [sys.executable, str(contract.REPO / "bench/tools" / f"{name}_usage.py")]
                                            for name in ("codex", "transcript")}), encoding="utf-8")
            regression = self.run_cli("--implementations", commands, "--allow-known-defects", "--differential", baseline)
            self.assertEqual(regression.returncode, 1, regression.stdout + regression.stderr)
            self.assertIn("DIFFERENTIAL unknown-is-not-zero", regression.stdout)
            wrapper.write_text(source.replace("sys.exit(2)", "sys.exit(9)"), encoding="utf-8")
            wrong = self.run_cli("--allow-known-defects", "--differential", commands)
            self.assertEqual(wrong.returncode, 1, wrong.stdout + wrong.stderr)
            self.assertIn("DIFFERENTIAL unknown-is-not-zero", wrong.stdout)


if __name__ == "__main__":
    unittest.main()
