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
import errno
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import behavior_contract as contract


def encoded(value):
    return base64.b64encode(value).decode("ascii")


def decision(case, reason="test", **extra):
    payload = dict(reason=reason, contract_sha256=contract.digest(case), **extra)
    return dict(decision="decision-" + contract.digest(payload), **payload)


def write_decision_version(directory, number):
    previous_path = directory / f"usage-decisions.v{number - 1}.json"
    previous = json.loads(previous_path.read_text())
    old = previous if number == 2 else previous["records"]
    value = {"previous": {"path": previous_path.name,
                          "format": "decisions/1" if number == 2 else "decisions/2",
                          "sha256": contract.hashlib.sha256(previous_path.read_bytes()).hexdigest()},
             "records": {"probe": decision({"id": "probe"}, f"version {number}",
                                           supersedes=old["probe"]["contract_sha256"])}}
    path = directory / f"usage-decisions.v{number}.json"
    path.write_text(json.dumps(value))
    (directory / "usage.v1.json").write_text(json.dumps({"decisions": path.name}))
    return path


def history_repo(root):
    def git(*args):
        return subprocess.check_output(["git", "-C", str(root), "-c", "user.name=kamui",
                                        "-c", "user.email=2255+kamui@users.noreply.github.com", *args],
                                       stderr=subprocess.PIPE)
    git("init", "--initial-branch=main")
    directory = root / "tools/behavior_contracts"
    directory.mkdir(parents=True)
    (directory / "usage-decisions.v1.json").write_text(json.dumps({"probe": decision({"id": "probe"}, "legacy")}))
    write_decision_version(directory, 2)
    git("add", ".")
    git("commit", "-m", "test(bench): adopt decision history")
    return git


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
            decisions.write_text(json.dumps({"records": {"probe": decision(base)}}), encoding="utf-8")
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

    def test_canonical_digest_vectors(self):
        values = [None, True, 1, 1.0, 4e-5, 9007199254740993, -0.0, "雪😀",
                  {"雪": [True, None, 10.0], "a": 1}]
        vectors = json.loads((contract.CORPUS.parent / "digest-vectors.json").read_text())
        self.assertEqual(len(values), len(vectors))
        for value, vector in zip(values, vectors):
            self.assertEqual(contract.canonical(value).hex(), vector["canonical_hex"], vector["name"])
            self.assertEqual(contract.digest(value), vector["sha256"], vector["name"])
        for value in (float("inf"), float("nan"), "\udcff"):
            with self.assertRaises((ValueError, UnicodeError)):
                contract.canonical(value)

    def test_removed_case_needs_decision(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            corpus = json.loads(contract.CORPUS.read_text(encoding="utf-8"))
            for decisions in contract.CORPUS.parent.glob("usage-decisions.*.json"):
                (root / decisions.name).write_bytes(decisions.read_bytes())
            corpus["cases"] = [case for case in corpus["cases"] if case["id"] != "unknown-is-not-zero"]
            path = root / "corpus.json"
            path.write_text(json.dumps(corpus), encoding="utf-8")
            result = self.run_cli("--corpus", path, "--allow-known-defects")
            self.assertEqual(result.returncode, 2, result.stdout)
            self.assertIn("removal needs an explicit decision", result.stderr)

    def test_changed_digest_requires_new_superseding_decision(self):
        case = {"id": "probe", "expected": {"exit": 2}}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            previous = root / "previous.json"
            previous.write_text(json.dumps({"records": {"probe": decision(case, "Reject unknown")}}))
            corpus = {"format": "behavior-contract/1", "decisions": "decisions.json", "cases": [case]}
            case["expected"]["exit"] = 0
            path = root / "corpus.json"
            path.write_text(json.dumps(corpus))
            old = json.loads(previous.read_text())["records"]["probe"]
            changed = dict(old, contract_sha256=contract.digest(case))
            decisions = {"previous": {"path": previous.name, "format": "decisions/2",
                                      "sha256": contract.hashlib.sha256(previous.read_bytes()).hexdigest()},
                         "records": {"probe": changed}}
            target = root / "decisions.json"
            for record in (changed, decision(case, "Reject unknown"),
                           decision(case, "Approve known zero"),
                           decision(case, "Reject unknown", supersedes=old["contract_sha256"])):
                decisions["records"]["probe"] = record
                target.write_text(json.dumps(decisions))
                with self.assertRaisesRegex(ValueError, "explicit decision|supersede"):
                    contract.load_corpus(path)
            decisions["records"]["probe"] = decision(case, "Approve known zero", supersedes=old["contract_sha256"])
            target.write_text(json.dumps(decisions))
            self.assertEqual(contract.load_corpus(path)["cases"], [case])
            previous.write_text("{}")
            with self.assertRaisesRegex(ValueError, "previous decisions changed"):
                contract.load_corpus(path)

    def test_open_defects_retain_independent_cases(self):
        register = (contract.REPO / "docs/python-migration/contracts/defects.md").read_text()
        corpus = contract.load_corpus(contract.CORPUS)
        registered = set()
        for section in register.split("\n## ")[1:]:
            if "- Status: open" not in section:
                continue
            identifier = section.split(":", 1)[0]
            matches = [case for case in corpus["cases"] if
                       ("#" + identifier.lower() + "-") in case.get("baseline_defect", {}).get("resolution", "")]
            self.assertEqual(len(matches), 1, identifier)
            registered.add(matches[0]["id"])
            self.assertIn("`" + matches[0]["id"] + "`", section)
            self.assertTrue(contract.compare(matches[0]["expected"], {
                "exit": matches[0]["baseline_defect"]["observed"]["exit"],
                "stdout": json.dumps(matches[0]["baseline_defect"]["observed"]["stdout"]["value"]).encode(),
                "stderr": b"", "filesystem": matches[0]["expected"]["filesystem"]}), identifier)
        self.assertEqual(registered, {case["id"] for case in corpus["cases"] if "baseline_defect" in case})

    def test_published_decisions_cannot_be_edited_or_removed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git = history_repo(root)
            directory = root / "tools/behavior_contracts"
            v1 = directory / "usage-decisions.v1.json"
            v2 = directory / "usage-decisions.v2.json"
            published = v2.read_bytes()
            contract.verify_decision_history(root)
            v2.write_text("same reason with recomputed digest and ID")
            with self.assertRaisesRegex(ValueError, "published decision changed"):
                contract.verify_decision_history(root)
            git("add", ".")
            git("commit", "-m", "test(bench): simulate an in-place decision edit")
            with self.assertRaisesRegex(ValueError, "published decision changed"):
                contract.verify_decision_history(root)
            v2.write_bytes(published)
            v3 = write_decision_version(directory, 3)
            published_v3 = v3.read_bytes()
            git("add", ".")
            git("commit", "-m", "test(bench): retain history and add a decision")
            contract.verify_decision_history(root)
            v3.write_text("edited new case with no earlier record")
            with self.assertRaisesRegex(ValueError, "published decision changed"):
                contract.verify_decision_history(root)
            v3.write_bytes(published_v3)
            v1.unlink()
            with self.assertRaisesRegex(ValueError, "published decision changed"):
                contract.verify_decision_history(root)

    def test_active_history_cannot_rollback_or_skip_predecessors(self):
        for scenario in ("rollback", "new-root", "skip-predecessor"):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                git = history_repo(root)
                directory = root / "tools/behavior_contracts"
                write_decision_version(directory, 3)
                git("add", ".")
                git("commit", "-m", "test(bench): publish another decision")
                contract.verify_decision_history(root)
                if scenario == "rollback":
                    (directory / "usage.v1.json").write_text(json.dumps({"decisions": "usage-decisions.v2.json"}))
                else:
                    v4 = write_decision_version(directory, 4)
                    value = json.loads(v4.read_text())
                    if scenario == "new-root":
                        del value["previous"]
                    else:
                        value["previous"] = json.loads((directory / "usage-decisions.v2.json").read_text())["previous"]
                    v4.write_text(json.dumps(value))
                git("add", ".")
                git("commit", "-m", "test(bench): simulate bypassing active history")
                with self.assertRaisesRegex(ValueError, "latest published|immediate predecessor"):
                    contract.verify_decision_history(root)

    def test_corpus_rejects_duplicate_json_keys(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "corpus.json"
            path.write_text('{"format":"behavior-contract/1","format":"behavior-contract/1"}')
            with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
                contract.load_corpus(path)

    def test_unavailable_filesystem_case_continues(self):
        original = Path.write_bytes
        def refuse(path, data):
            if path.name == contract.os.fsdecode(b"probe-\xff"):
                raise OSError(errno.EILSEQ, "invalid filename")
            return original(path, data)
        with patch.object(Path, "write_bytes", refuse), patch.object(sys, "argv", ["behavior_contract", "--allow-known-defects"]):
            import contextlib
            import io
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                self.assertEqual(contract.main(), 0)
            self.assertIn("SKIP UNAVAILABLE non-utf8-path", output.getvalue())
            self.assertNotIn("PASS non-utf8-path", output.getvalue())
            self.assertIn("PASS transcript-duplicate-maxima", output.getvalue())
        with tempfile.TemporaryDirectory() as temporary:
            case = {"requires": ["non-utf8-paths"]}
            with patch.object(Path, "write_bytes", refuse), self.assertRaises(contract.UnavailableFixture):
                contract.require_capabilities(case, Path(temporary))

    def test_escaped_filename_case_reports_unavailable_and_continues(self):
        original = contract.os.listdir
        def escaped_names(path):
            return [b"probe-%FF" if name == b"probe-\xff" else name
                    for name in original(path)]
        with patch.object(contract.os, "listdir", escaped_names):
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with self.assertRaisesRegex(contract.UnavailableFixture, "non-utf8-paths"):
                    contract.require_capabilities({"requires": ["non-utf8-paths"]}, root)
                self.assertEqual(list(root.iterdir()), [])
            with patch.object(sys, "argv", ["behavior_contract", "--allow-known-defects"]):
                import contextlib
                import io
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    self.assertEqual(contract.main(), 0)
                self.assertIn("SKIP UNAVAILABLE non-utf8-path", output.getvalue())
                self.assertNotIn("PASS non-utf8-path", output.getvalue())
                self.assertIn("PASS transcript-duplicate-maxima", output.getvalue())

    def test_invalid_primary_json_reports_each_case_and_continues(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            commands = root / "commands.json"
            commands.write_text(json.dumps({name: [sys.executable, "-c", "print('not-json')"]
                                            for name in ("codex", "transcript")}))
            result = self.run_cli("--implementations", commands, "--differential", commands)
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn("DIFFERENTIAL duplicate-response", result.stdout)
            self.assertIn("stdout primary semantic:", result.stdout)
            self.assertIn("FAIL transcript-duplicate-maxima", result.stdout)
            self.assertEqual(result.stderr, "")
        left = {"exit": 0, "stdout": b"invalid", "stderr": b"left", "filesystem": []}
        right = {"exit": 9, "stdout": b"{}", "stderr": b"right", "filesystem": [{"kind": "directory"}]}
        differences = contract.differential(left, right, {"expected": {"stdout": {"mode": "json"},
                                                                    "stderr": {"mode": "bytes"}}})
        for detail in ("primary semantic", "exit: expected 0, actual 9", "stderr bytes", "filesystem effects"):
            self.assertTrue(any(detail in diff for diff in differences), differences)

    def test_duplicate_rules_reject_first_last_and_maximum_mutants(self):
        cases = contract.load_corpus(contract.CORPUS)["cases"]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for program, case_id, before, after in (
                ("codex", "duplicate-response-last", "usage_by_response[key] = usage",
                 "usage_by_response.setdefault(key, usage)"),
                ("codex", "duplicate-response-last", "usage_by_response[key] = usage",
                 "usage_by_response[key] = {k: max(v, usage_by_response.get(key, {}).get(k, 0)) for k, v in usage.items()}"),
                ("transcript", "transcript-duplicate-maxima", 'max(turn["output"], int(used.get("output_tokens") or 0))',
                 'int(used.get("output_tokens") or 0) if turn["lines"] == 1 else turn["output"]'),
                ("transcript", "transcript-duplicate-maxima", 'max(turn["input"], int(used.get("input_tokens") or 0))',
                 'int(used.get("input_tokens") or 0)'),
            ):
                source = (contract.REPO / "bench/tools" / f"{program}_usage.py").read_text()
                self.assertIn(before, source)
                mutant = root / f"{program}_usage.py"
                mutant.write_text(source.replace(before, after))
                case = next(case for case in cases if case["id"] == case_id)
                actual = contract.execute(case, {program: [sys.executable, str(mutant)]})
                self.assertEqual(actual["exit"], 0, actual["stderr"])
                differences = contract.compare(case["expected"], actual)
                self.assertTrue(any("stdout semantic:" in diff for diff in differences), differences)

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
