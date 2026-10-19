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
import io
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
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


def write_family_version(directory, family, number, case_id="probe"):
    path = directory / f"{family}-decisions.v{number}.json"
    if number == 1:
        value = {"records": {case_id: decision({"id": case_id}, "version 1")}}
    else:
        previous_path = directory / f"{family}-decisions.v{number - 1}.json"
        old = json.loads(previous_path.read_text())["records"][case_id]
        value = {"previous": {"path": previous_path.name, "format": "decisions/2",
                              "sha256": contract.hashlib.sha256(previous_path.read_bytes()).hexdigest()},
                 "records": {case_id: decision({"id": case_id}, f"version {number}", supersedes=old["contract_sha256"])}}
    path.write_text(json.dumps(value))
    (directory / f"{family}.v1.json").write_text(json.dumps({"decisions": path.name}))
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

    def test_each_decision_family_keeps_its_own_published_history(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git = history_repo(root)
            directory = root / "tools/behavior_contracts"
            v1 = write_family_version(directory, "roster", 1)
            git("add", ".")
            git("commit", "-m", "test(bench): publish a second decision family")
            contract.verify_decision_history(root)
            published = v1.read_bytes()
            v1.write_text(json.dumps({"records": {}}))
            with self.assertRaisesRegex(ValueError, "roster-decisions.v1.json: published decision changed"):
                contract.verify_decision_history(root)
            v1.write_bytes(published)
            write_family_version(directory, "roster", 2)
            git("add", ".")
            git("commit", "-m", "test(bench): supersede the second family")
            contract.verify_decision_history(root)
            (directory / "roster.v1.json").write_text(json.dumps({"decisions": v1.name}))
            with self.assertRaisesRegex(ValueError, "roster.v1.json: corpus must use the latest published"):
                contract.verify_decision_history(root)
            (directory / "roster.v1.json").unlink()
            with self.assertRaisesRegex(ValueError, "roster: no corpus uses this decision history"):
                contract.verify_decision_history(root)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            git = history_repo(root)
            directory = root / "tools/behavior_contracts"
            v1 = write_family_version(directory, "roster", 1)
            value = json.loads(v1.read_text())
            value["previous"] = json.loads((directory / "usage-decisions.v2.json").read_text())["previous"]
            v1.write_text(json.dumps(value))
            git("add", ".")
            git("commit", "-m", "test(bench): give a first version a predecessor")
            with self.assertRaisesRegex(ValueError, "a first decision version has no predecessor"):
                contract.verify_decision_history(root)

    def test_prefix_pins_leading_bytes_only(self):
        expected = {"exit": 2, "stdout": {"mode": "bytes", "bytes_b64": ""},
                    "stderr": {"mode": "prefix", "bytes_b64": encoded(b"tool: command failed: git\n")}, "filesystem": []}
        actual = {"exit": 2, "stdout": b"", "stderr": b"tool: command failed: git\nfatal: any wording\n", "filesystem": []}
        self.assertEqual(contract.compare(expected, actual), [])
        self.assertTrue(contract.compare(expected, dict(actual, stderr=b"tool: command failed")))
        differences = contract.differential(actual, dict(actual, stderr=b"tool: command failed: git\nother\n"),
                                            {"expected": expected})
        self.assertTrue(any("stderr bytes" in diff for diff in differences), differences)

    def test_each_program_defaults_to_its_maintained_implementation(self):
        corpus = {"cases": [{"program": name} for name in ("codex", "diff-identity", "roster", "normalize-review",
                                                           "normalize-thermo")]}
        program = Path(self.enterContext(tempfile.TemporaryDirectory())) / "bench-tools"
        program.write_text("#!/bin/sh\n", encoding="utf-8")
        program.chmod(0o755)
        with patch.dict(contract.os.environ, {"BENCH_RUST_BENCH_TOOLS": str(program),
                                              "BENCH_RUST_BENCH_TOOLS_SHA256": contract.hashlib.sha256(program.read_bytes()).hexdigest()}):
            commands = contract.default_commands(corpus)
        self.assertEqual(commands, {"codex": [sys.executable, str(contract.REPO / "bench/tools/codex_usage.py")],
                                    "diff-identity": [sys.executable, str(contract.REPO / "bench/tools/diff_identity.py")],
                                    "roster": [str(program), "roster"],
                                    "normalize-review": [sys.executable, str(contract.REPO / "bench/tools/normalize_review.py")],
                                    "normalize-thermo": [str(program), "normalize-thermo"]})
        store = Path(self.enterContext(tempfile.TemporaryDirectory())) / "store"
        with patch.dict(contract.os.environ, {"BENCH_RUNTIME_STORE": str(store)}):
            contract.os.environ.pop("BENCH_RUST_BENCH_TOOLS", None)
            contract.os.environ.pop("BENCH_RUST_BENCH_TOOLS_SHA256", None)
            with self.assertRaisesRegex(ValueError, "(roster|normalize-thermo): bench-tools has no selected version.*install --crate rust/bench-tools"):
                contract.default_commands(corpus)

    def test_reader_corpora_hold_for_the_retained_libraries_and_reject_regressions(self):
        tools = contract.REPO / "bench/tools"
        mutants = {
            "check-manifest": ("check_manifest.py", "if isinstance(a, bool) or isinstance(b, bool):", "if False:",
                               "check-manifest-json-equality"),
            "rates": ("rates.py", "if LONG_PROMPT_ROW.search(name):", "if False:", "rates-anthropic-prompt-tiers"),
            "upstream": ("upstream.py", 'max(snapshots, key=lambda p: int(p.stem.removeprefix("snapshot.v")))',
                         "max(snapshots)", "upstream-report"),
        }
        for program, (module, before, after, case_id) in mutants.items():
            with self.subTest(program=program):
                corpus = contract.CORPUS.parent / f"{program}.v1.json"
                result = self.run_cli("--allow-known-defects", "--corpus", corpus)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn(f"PASS {case_id}", result.stdout)
                source = (tools / module).read_text(encoding="utf-8")
                self.assertIn(before, source)
                with tempfile.TemporaryDirectory() as temporary:
                    (Path(temporary) / module).write_text(source.replace(before, after), encoding="utf-8")
                    if program in contract.PYTHON_DRIVERS:
                        # The driver puts its directory first; the mutant's own imports still come from bench/tools.
                        driver = f"import sys; sys.path.insert(0, {str(tools)!r})\n" + contract.PYTHON_DRIVERS[program]
                        command = [sys.executable, "-c", driver, temporary]
                    else:
                        command = [sys.executable, str(Path(temporary) / module)]
                    commands = Path(temporary) / "commands.json"
                    commands.write_text(json.dumps({program: command}), encoding="utf-8")
                    result = self.run_cli("--corpus", corpus, "--implementations", commands)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn(f"FAIL {case_id}", result.stdout)

    def test_diff_identity_corpus_holds_for_the_retained_library_and_rejects_byte_order(self):
        corpus = contract.CORPUS.parent / "diff-identity.v1.json"
        result = self.run_cli("--allow-known-defects", "--corpus", corpus)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("PASS diff-identity-manifest", result.stdout)
        source = (contract.REPO / "bench/tools/diff_identity.py").read_text()
        before = "rows.sort(key=lambda row: row[1])"
        self.assertIn(before, source)
        with tempfile.TemporaryDirectory() as temporary:
            mutant = Path(temporary) / "diff_identity.py"
            mutant.write_text(source.replace(before, "rows.sort(key=lambda row: row[1].encode('utf-8', 'surrogateescape'))"))
            commands = Path(temporary) / "commands.json"
            commands.write_text(json.dumps({"diff-identity": [sys.executable, str(mutant)]}))
            result = self.run_cli("--corpus", corpus, "--implementations", commands)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("FAIL diff-identity-manifest", result.stdout)
        self.assertIn("PASS diff-identity-empty", result.stdout)

    def test_derive_packet_corpus_pins_every_committed_legacy_packet(self):
        corpus = contract.load_corpus(contract.CORPUS.parent / "derive-packet.v1.json")
        cases = {case["id"]: case for case in corpus["cases"]}
        legacy = sorted((contract.REPO / "bench/targets").glob("*/packet.legacy.md"))
        self.assertTrue(legacy)
        self.assertEqual({f"derive-packet-target-{path.parent.name}" for path in legacy},
                         {name for name in cases if name.startswith("derive-packet-target-")})
        for path in legacy:
            case = cases[f"derive-packet-target-{path.parent.name}"]
            files = {base64.b64decode(entry["path_b64"]): base64.b64decode(entry["bytes_b64"])
                     for entry in case["expected"]["filesystem"] if entry["kind"] == "file"}
            self.assertEqual(case["argv"], ["packet.legacy.md", "--out", "packet.md"])
            self.assertEqual(files, {b"packet.legacy.md": path.read_bytes(),
                                     b"packet.md": path.with_name("packet.md").read_bytes()})

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


    STORAGE_CORPORA = ("evidence-store", "native-artifacts", "evidence-inventory", "import-benchmark")
    ARCHIVE_WRITER = """import gzip, hashlib, io, json, os, sys, tarfile
level, variant = int(sys.argv[1]), sys.argv[2]
data = b"member bytez\\n" if variant == "byte" else b"member bytes\\n"
raw = io.BytesIO()
with gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0, compresslevel=level) as packed:
    with tarfile.open(fileobj=packed, mode="w") as bundle:
        info = tarfile.TarInfo("bench/a.txt")
        info.size, info.mode = len(data), 0o600 if variant == "mode" else 0o644
        bundle.addfile(info, io.BytesIO(data))
archive = raw.getvalue()
digest = hashlib.sha256(archive).hexdigest()
os.mkdir("out")
with open(f"out/{digest}.tar.gz", "wb") as handle:
    handle.write(archive)
named = hashlib.sha256(b"other bytes").hexdigest() if variant == "manifest" else digest
with open("out/manifest.json", "w") as handle:
    json.dump({"packages": [{"asset": digest + ".tar.gz", "sha256": named, "bytes": len(archive)}]}, handle, indent=2)
print(digest)
"""

    def test_archives_compare_by_members_and_manifests_name_the_written_archive(self):
        case = {"id": "probe", "program": "writer", "argv": [], "stdin_b64": "", "initial": [],
                "archives": [{"glob": "out/*.tar.gz", "format": "tar.gz"}], "manifests": ["out/manifest.json"]}
        def run(level, variant):
            return contract.execute(dict(case, argv=[str(level), variant]),
                                    {"writer": [sys.executable, "-c", self.ARCHIVE_WRITER]}, {"snapshot_modes": True})
        best, fastest = run(9, "same"), run(1, "same")
        self.assertEqual(best["stdout"], b"<archive 1>\n")
        self.assertNotIn("inconsistencies", best)
        self.assertEqual(contract.differential(best, fastest, {"expected": {"stdout": {"mode": "bytes"},
                                                                          "stderr": {"mode": "bytes"}}}), [])
        manifest = next(entry for entry in best["filesystem"] if entry["path_b64"] == encoded(b"out/manifest.json"))
        self.assertIn(b'"bytes": "<archive size>"', contract.decode(manifest["bytes_b64"]))
        archive = next(entry for entry in best["filesystem"] if entry["kind"] == "archive")
        self.assertEqual(contract.decode(archive["path_b64"]), b"out/<archive 1>.tar.gz")
        self.assertEqual([(member["name"], member["mode"]) for member in archive["members"]], [("bench/a.txt", 0o644)])
        for variant in ("byte", "mode"):
            with self.subTest(variant=variant):
                differences = contract.differential(best, run(9, variant), {"expected": {"stdout": {"mode": "bytes"},
                                                                                         "stderr": {"mode": "bytes"}}})
                self.assertTrue(any("filesystem effects" in diff for diff in differences), differences)
        wrong = run(9, "manifest")
        self.assertTrue(any("differ from the archive written beside it" in issue for issue in wrong["inconsistencies"]))
        self.assertTrue(any(diff.startswith("consistency:") for diff in contract.compare(
            {**best, "stdout": {"mode": "bytes", "bytes_b64": encoded(b"<archive 1>\n")},
             "stderr": {"mode": "bytes", "bytes_b64": ""}}, wrong)))

    def test_an_archive_member_name_that_is_not_utf8_is_listed_by_its_bytes(self):
        raw = io.BytesIO()
        with tarfile.open(fileobj=raw, mode="w:gz", format=tarfile.PAX_FORMAT) as bundle:
            for name in ("plain.txt", os.fsdecode(b"review-\xff.md")):
                info = tarfile.TarInfo(name)
                info.size = 1
                bundle.addfile(info, io.BytesIO(b"x"))
        members = contract.archive_members(raw.getvalue(), "tar.gz")
        self.assertEqual(members[0]["name"], "plain.txt")
        self.assertNotIn("name", members[1])
        self.assertEqual(contract.decode(members[1]["name_b64"]), b"review-\xff.md")
        self.assertEqual(len(contract.digest(members)), 64)

    NATIVE_WRITER = """import hashlib, io, json, os, sys, zipfile
raw = io.BytesIO()
with zipfile.ZipFile(raw, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=int(sys.argv[1])) as bundle:
    bundle.writestr("member", b"member bytes")
archive = raw.getvalue()
os.mkdir("out")
with open("out/native-scratch.v1.zip", "wb") as handle:
    handle.write(archive)
named = {"label": "<archive 1>", "path": "missing.zip"}.get(sys.argv[2], hashlib.sha256(archive).hexdigest())
path = "missing.zip" if sys.argv[2] == "path" else "native-scratch.v1.zip"
with open("out/native-artifact-storage.v1.json", "w") as handle:
    json.dump({"archive": {"path": path, "sha256": named}}, handle, indent=2)
"""

    def test_storage_references_name_the_written_archive_before_masking(self):
        case = {"id": "probe", "program": "writer", "argv": [], "stdin_b64": "", "initial": [],
                "archives": [{"glob": "out/native-scratch.v1.zip", "format": "zip"}],
                "references": [{"path": "out/native-artifact-storage.v1.json", "fields": ["archive"]}]}
        def run(level, variant):
            return contract.execute(dict(case, argv=[str(level), variant]),
                                    {"writer": [sys.executable, "-c", self.NATIVE_WRITER]}, {})
        streams = {"expected": {"stdout": {"mode": "bytes"}, "stderr": {"mode": "bytes"}}}
        best = run(9, "same")
        self.assertNotIn("inconsistencies", best)
        self.assertEqual(contract.differential(best, run(1, "same"), streams), [])
        for variant in ("label", "path"):
            with self.subTest(variant=variant):
                wrong = run(9, variant)
                self.assertTrue(any("differs from the file written beside it" in issue
                                    for issue in wrong["inconsistencies"]), wrong.get("inconsistencies"))
                self.assertTrue(any(diff.startswith("consistency:") for diff in contract.differential(best, wrong, streams)))

    def test_special_files_snapshot_without_blocking(self):
        import threading
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            contract.populate(root, [{"kind": "special", "path_b64": encoded(b"pipe"), "mode": 0o640}])
            result = []
            reader = threading.Thread(target=lambda: result.append(contract.snapshot(root)), daemon=True)
            reader.start()
            reader.join(5)
            self.assertFalse(reader.is_alive(), "snapshot blocked on a FIFO")
            self.assertEqual(result, [[{"path_b64": encoded(b"pipe"), "kind": "special", "mode": 0o640}]])

    def test_steps_share_one_root_with_modes_links_and_removals(self):
        writer = ("import os, sys\nfrom pathlib import Path\n"
                  "Path(sys.argv[1]).write_text(' '.join(sorted(os.listdir('.'))))\n")
        case = {"id": "probe", "program": "writer", "initial": [
            {"kind": "file", "path_b64": encoded(b"kept"), "bytes_b64": encoded(b"x"), "mode": 0o600},
            {"kind": "directory", "path_b64": encoded(b"gone")},
            {"kind": "symlink", "path_b64": encoded(b"link"), "target_b64": encoded(b"kept")}],
            "steps": [{"argv": ["first"], "stdin_b64": ""}, {"remove": [encoded(b"gone"), encoded(b"first")]},
                      {"write": [{"kind": "file", "path_b64": encoded(b"kept"), "bytes_b64": encoded(b"y"),
                                  "mode": 0o640}]},
                      {"argv": ["second"], "stdin_b64": ""}]}
        actual = contract.execute(case, {"writer": [sys.executable, "-c", writer]}, {"snapshot_modes": True})
        first, second = actual["steps"]
        self.assertEqual(contract.decode(next(e for e in first["filesystem"] if e["path_b64"] == encoded(b"first"))
                                         ["bytes_b64"]), b".home gone kept link")
        files = {contract.decode(e["path_b64"]): e for e in second["filesystem"]}
        self.assertEqual(sorted(files), [b".home", b"kept", b"link", b"second"])
        self.assertEqual((files[b"kept"]["mode"], contract.decode(files[b"kept"]["bytes_b64"])), (0o640, b"y"))
        self.assertEqual((files[b"second"]["mode"], files[b".home"]["mode"]), (0o644, 0o755))
        self.assertEqual(files[b"link"], {"path_b64": encoded(b"link"), "kind": "symlink", "target_b64": encoded(b"kept")})
        self.assertNotIn("mode", contract.execute(dict(case, steps=case["steps"][:1]),
                                                  {"writer": [sys.executable, "-c", writer]})["steps"][0]["filesystem"][0])
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "corpus.json"
            for broken in ({**case, "expected": {}}, {**case, "steps": [{"remove": []}]}):
                path.write_text(json.dumps({"format": "behavior-contract/1", "decisions": "d.json", "cases": [broken]}))
                (Path(temporary) / "d.json").write_text(json.dumps({"records": {"probe": decision(broken)}}))
                with self.assertRaisesRegex(ValueError, "step case"):
                    contract.load_corpus(path)
            path.write_text(json.dumps({"format": "behavior-contract/1", "decisions": "d.json", "cases": [],
                                        "snapshot_mode": True}))
            with self.assertRaisesRegex(ValueError, "unknown corpus option"):
                contract.load_corpus(path)

    def test_stub_gh_records_calls_and_injects_faults(self):
        caller = ("import subprocess, sys\n"
                  "for args in [['api', 'repos/o/r/releases/tags/evidence-t'],\n"
                  "             ['api', '--paginate', '--slurp', 'repos/o/r/releases/7/assets?per_page=100'],\n"
                  "             ['release', 'upload', 'evidence-t', 'a.bin', '--repo', 'o/r'],\n"
                  "             ['api', 'repos/o/r/releases/assets/101', '-H', 'Accept: application/octet-stream'],\n"
                  "             ['auth', 'token']]:\n"
                  "    result = subprocess.run(['gh', *args], capture_output=True)\n"
                  "    sys.stdout.buffer.write(b'%d %s\\n' % (result.returncode, result.stdout.strip()[:60]))\n")
        def run(**release):
            state = {"repository": "o/r", "tag": "evidence-t", "id": 7, **release}
            case = {"id": "probe", "program": "caller", "argv": [], "stdin_b64": "", "initial": [
                {"kind": "file", "path_b64": encoded(b"a.bin"), "bytes_b64": encoded(b"0123456789")},
                {"kind": "file", "path_b64": encoded(b"gh/release.json"), "bytes_b64": encoded(json.dumps(state).encode())}]}
            actual = contract.execute(case, {"caller": [sys.executable, "-c", caller]}, {"gh_stub_b64": encoded(b"gh")})
            files = {contract.decode(e["path_b64"]): e for e in actual["filesystem"]}
            return actual["stdout"].decode().splitlines(), files
        lines, files = run()
        self.assertEqual(lines, ['0 {"id": 7, "tag_name": "evidence-t", "draft": false}', "0 [[]]", "0 ",
                                 "0 0123456789", "2 "])
        self.assertEqual(contract.decode(files[b"gh/assets/101/a.bin"]["bytes_b64"]), b"0123456789")
        calls = [json.loads(line) for line in contract.decode(files[b"gh/calls.jsonl"]["bytes_b64"]).splitlines()]
        self.assertEqual(len(calls), 5)
        self.assertEqual(calls[2], ["release", "upload", "evidence-t", "a.bin", "--repo", "o/r"])
        lines, files = run(faults=["partial-upload"], filler_assets=150)
        self.assertEqual(lines[1][:5], '0 [[{')
        self.assertEqual(lines[2:4], ["1 ", "1 "])
        self.assertEqual(contract.decode(files[b"gh/assets/151/a.bin"]["bytes_b64"]), b"01234")
        lines, files = run(faults=["truncated-download"])
        self.assertEqual(lines[2:4], ["0 ", "0 01234"])
        lines, files = run(faults=["lost-upload-response", "oversized-download"])
        self.assertEqual(lines[2], "1 ")
        self.assertTrue(lines[3].startswith("0 0123456789\x00"), lines[3])
        lines, files = run(faults=["rejected-upload"])
        self.assertEqual(lines[2:4], ["1 ", "1 "])
        self.assertNotIn(b"gh/assets", files)

    def test_storage_corpora_never_reach_the_network(self):
        answered = re.compile(r'\["api", "repos/[^/"]+/[^/"]+/releases/tags/[^"/]+"\]'
                              r'|\["api", "--paginate", "--slurp", "repos/[^/"]+/[^/"]+/releases/\d+/assets\?per_page=100"\]'
                              r'|\["release", "upload", "[^"]+", "[^"]+", "--repo", "[^/"]+/[^/"]+"\]'
                              r'|\["api", "repos/[^/"]+/[^/"]+/releases/assets/\d+", "-H", "Accept: application/octet-stream"\]')
        calls = 0
        for name in self.STORAGE_CORPORA:
            corpus = contract.load_corpus(contract.CORPUS.parent / f"{name}.v1.json")
            self.assertEqual(corpus["gh_stub_b64"], encoded(b"gh"), name)
            for case in corpus["cases"]:
                for step in case.get("steps", [case]):
                    if "argv" not in step:
                        continue
                    self.assertFalse(any("://" in str(arg) for arg in step["argv"]), case["id"])
                    for entry in step["expected"]["filesystem"]:
                        if contract.decode(entry["path_b64"]) == b"gh/calls.jsonl":
                            for line in contract.decode(entry["bytes_b64"]).decode().splitlines():
                                self.assertRegex(line, f"^(?:{answered.pattern})$", case["id"])
                                calls += 1
        self.assertGreater(calls, 100)

    def storage_commands(self, name):
        """A storage corpus's default commands. Programs that moved to bench-tools need it installed for HEAD; the
        Pages build installs it, and the usage-pilot workflow, which does not, skips the corpus."""
        try:
            return contract.default_commands(contract.load_corpus(contract.CORPUS.parent / f"{name}.v1.json"))
        except ValueError as error:
            self.skipTest(f"{name}: {error}")

    def test_storage_corpora_hold_and_reject_regressions(self):
        for name in self.STORAGE_CORPORA:
            with self.subTest(corpus=name):
                self.storage_commands(name)
                result = self.run_cli("--allow-known-defects", "--corpus", contract.CORPUS.parent / f"{name}.v1.json")
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn("FAIL", result.stdout)
        commands = self.storage_commands("evidence-store")
        tools = contract.REPO / "bench/tools"
        source = (tools / "evidence_store.py").read_text(encoding="utf-8")
        credential = "if source.name in {'auth.json', '.credentials.json'}:"
        copy = "shutil.copyfileobj(source, target, 1024 * 1024)"
        self.assertIn(credential, source)
        self.assertIn(copy, source)
        with tempfile.TemporaryDirectory() as temporary:
            mutant = Path(temporary) / "bench/tools/evidence_store.py"
            mutant.parent.mkdir(parents=True)
            (Path(temporary) / "bench/schema").mkdir()
            mutant.write_text(source.replace(credential, "if False:").replace(
                copy, "raise RuntimeError('unrelated implementation crash')"), encoding="utf-8")
            (mutant.parent / "check_manifest.py").write_bytes((tools / "check_manifest.py").read_bytes())
            schema = "bench/schema/evidence-storage.schema.json"
            (Path(temporary) / schema).write_bytes((contract.REPO / schema).read_bytes())
            mapped = Path(temporary) / "commands.json"
            mapped.write_text(json.dumps({**commands, "evidence-store": [sys.executable, str(mutant)]}))
            result = self.run_cli("--allow-known-defects", "--corpus",
                                  contract.CORPUS.parent / "evidence-store.v1.json", "--implementations", mapped)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("FAIL pack-credential-auth-json", result.stdout)
        self.assertIn("FAIL fetch-archive-truncated-gzip", result.stdout)
        self.assertIn("KNOWN DEFECT fetch-existing-changed", result.stdout)
        self.assertIn("PASS pack-traversal-path", result.stdout)

    GRADING_WRITER = """import hashlib, json, os, random, sys
from pathlib import Path
seed, mutation = int(sys.argv[1]), sys.argv[2]
rng = random.Random(seed)
def token(prefix, size):
    return f"{prefix}-{rng.getrandbits(8 * size):0{2 * size}x}"
def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\\n", encoding="utf-8")
blinds = [token("blind", 3) for _ in range(2)]
given = Path("in/quote.txt").read_text(encoding="utf-8")
quotes = [given.replace("blind-abcdef", "<blind 2>") if mutation == "quote-label" else given,
          "Leaks the conn" if mutation != "quote" else "Leaks the socket"]
os.makedirs("out/reviews")
for blind, quote in zip(blinds, quotes):
    Path(f"out/reviews/{blind}.md").write_text(f"# Review\\n\\n{quote}\\n", encoding="utf-8")
source = Path("in/source.go").read_bytes()
source = {"source": source.replace(b"package main", b"package mains"),
          "source-label": source.replace(b'"review-abcdef"', b'"<review 3>"')}.get(mutation, source)
Path("out/source.go").write_bytes(source)
decoded = json.loads(Path("in/quote.json").read_text(encoding="utf-8"))["quote"]
Path("out/quote.txt").write_text(decoded.replace("blind-cafe01", "blind-10efac") if mutation == "quote-escaped"
                                 else decoded, encoding="utf-8")
write("out/verdicts.json", {"reviews": {blind: {"claims": [{"quote": quote}]} for blind, quote in zip(blinds, quotes)}})
write("out/receipt.json", {"mapped_at": f"2026-10-{seed:02d}T00:00:00Z",
                           "verdicts_sha256": "0" * 64 if mutation == "wrong-pin" else sha("out/verdicts.json"),
                           "prepared_files": {**{f"reviews/{b}.md": sha(f"out/reviews/{b}.md") for b in blinds},
                                              "source.go": sha("out/source.go"), "quote.txt": sha("out/quote.txt")},
                           "runner": {"mapping": {"version": 2, "bench_tools": f"{seed:064x}"}}})
pinned = "out/verdicts.json" if mutation == "pin" else "out/receipt.json"
links = [(token("review", 4), relation, reason) for relation, reason in
         [("same", "Same trigger and mechanism."), ("same", "Same consequence."), ("related", "Shares the close path.")]]
write("out/grades.json", {"assessor": {"receipt": {"path": pinned, "sha256": sha(pinned)}}, "reviews": [
    {"token": blind, "families": [{"id": "GT-t1", "outcome": "unresolved" if mutation == "unresolved" and n else "caught"}],
     "credit": {"says_what": "yes", "identifies_cause": "yes" if mutation == "credit" and n else "no"}}
    for n, blind in enumerate(blinds)], "links": [review for review, _relation, _reason in links]})
if mutation == "section":
    links[2] = (links[2][0], "related", "Shares the open path.")
if mutation == "section-drop":
    links.pop()
if mutation == "section-dup":
    links.append(links[0])
rng.shuffle(links)
sections = "".join(f"### {review} ({relation})\\n\\n{reason}\\n\\n" for review, relation, reason in links)
Path("out/dossier.md").write_text(f"# Dossier\\n\\n## CL-1 v1\\n\\n{sections}## CL-2 v1\\n\\nNo links.\\n", encoding="utf-8")
print(f"mapped {blinds[0]}")
"""
    STRUCTURED = {"pins": [{"glob": "out/receipt.json", "pointer": "/verdicts_sha256", "path": "verdicts.json",
                            "base": "record"},
                           {"glob": "out/receipt.json", "map": "/prepared_files", "base": "record"},
                           {"glob": "out/grades.json"}],
                  "tokens": [{"prefix": "blind", "sites": [{"glob": "out/grades.json",
                                                            "pointers": ["/reviews/0/token", "/reviews/1/token"]}],
                              "references": [{"stream": "stdout"}, {"glob": "out/reviews/*.md", "names": True},
                                             {"glob": "out/verdicts.json", "pointers": ["/reviews"], "keys": True},
                                             {"glob": "out/receipt.json", "pointers": ["/prepared_files"],
                                              "keys": True}]},
                             {"prefix": "review", "sites": [{"glob": "out/grades.json",
                                                             "pointers": ["/links/0", "/links/1", "/links/2"]}],
                              "references": [{"glob": "out/dossier.md"}]}],
                  "clocks": [{"glob": "out/receipt.json", "pointers": ["/mapped_at"]}],
                  "identity": [{"glob": "out/receipt.json", "pointers": ["/runner/mapping"],
                                "label": "grade-map-runner-identity-v2"}],
                  "sections": [{"glob": "out/dossier.md", "heading": "### "}]}
    STREAMS = {"expected": {"stdout": {"mode": "bytes"}, "stderr": {"mode": "bytes"}}}

    GRADING_INPUTS = [{"path_b64": encoded(b"in/quote.txt"), "kind": "file",
                       "bytes_b64": encoded(b"The output must contain blind-abcdef.")},
                      {"path_b64": encoded(b"in/quote.json"), "kind": "file",
                       "bytes_b64": encoded(b'{"quote": "The output must contain blind-\\u0063afe01."}')},
                      {"path_b64": encoded(b"in/source.go"), "kind": "file",
                       "bytes_b64": encoded(b'package main\n\nvar names = []string{"review-012345", "review-6789ab", '
                                            b'"review-abcdef"}\n')}]

    def grading_output(self, seed, mutation="none"):
        case = {"id": "probe", "program": "writer", "argv": [str(seed), mutation], "stdin_b64": "",
                "initial": self.GRADING_INPUTS, "structured": self.STRUCTURED}
        return contract.execute(case, {"writer": [sys.executable, "-c", self.GRADING_WRITER]})

    def test_structured_comparison_normalizes_only_the_declared_differences(self):
        first, second = self.grading_output(1), self.grading_output(2)
        self.assertNotIn("inconsistencies", first)
        raw = {"id": "raw", "program": "writer", "argv": ["1", "none"], "stdin_b64": "", "initial": self.GRADING_INPUTS}
        self.assertNotEqual(contract.execute(raw, {"writer": [sys.executable, "-c", self.GRADING_WRITER]})["stdout"],
                            first["stdout"])
        self.assertEqual(first["stdout"], b"mapped <blind 1>\n")
        self.assertEqual(contract.differential(first, second, self.STREAMS), [])
        files = {contract.decode(entry["path_b64"]): contract.decode(entry["bytes_b64"])
                 for entry in first["filesystem"] if entry["kind"] == "file"}
        self.assertIn(b"out/reviews/<blind 2>.md", files)
        receipt = json.loads(files[b"out/receipt.json"])
        self.assertEqual((receipt["mapped_at"], receipt["runner"]["mapping"]),
                         ("<volatile>", "grade-map-runner-identity-v2"))
        self.assertEqual(receipt["verdicts_sha256"], contract.hashlib.sha256(files[b"out/verdicts.json"]).hexdigest())
        grades = json.loads(files[b"out/grades.json"])
        self.assertEqual(grades["assessor"]["receipt"]["sha256"],
                         contract.hashlib.sha256(files[b"out/receipt.json"]).hexdigest())
        self.assertLess(files[b"out/dossier.md"].index(b"(related)"), files[b"out/dossier.md"].index(b"(same)"))
        self.assertIn(b'"review-abcdef"', files[b"out/source.go"])
        self.assertIn(b"The output must contain blind-abcdef.", files[b"out/verdicts.json"])
        self.assertEqual(files[b"out/quote.txt"], b"The output must contain blind-cafe01.")
        for mutation in ("credit", "unresolved", "quote", "source", "section", "pin", "section-drop", "section-dup"):
            with self.subTest(mutation=mutation):
                changed = self.grading_output(2, mutation)
                self.assertNotIn("inconsistencies", changed)
                self.assertTrue(any("filesystem effects" in d for d in contract.differential(first, changed, self.STREAMS)))

    def test_structured_comparison_keeps_given_text_that_matches_a_token_or_its_label(self):
        first = self.grading_output(1)
        for mutation in ("source-label", "quote-label", "quote-escaped"):
            with self.subTest(mutation=mutation):
                changed = self.grading_output(2, mutation)
                for left, right in ((first, changed), (changed, first)):
                    self.assertTrue(any("filesystem effects" in d for d in contract.differential(left, right, self.STREAMS)))

    COLLISION_WRITER = """import json, sys
from pathlib import Path
token = {"primary": "blind-abcdef", "candidate": "blind-fedcba", "plain": "blind-111111"}[sys.argv[1]]
mutation = sys.argv[2]
quote = json.loads(Path("input.json").read_bytes())["quote"]
quote = quote.replace("blind-abcdef", "blind-fedcba") if mutation == "quote" else quote
Path("ids.json").write_text(json.dumps({"token": token}))
Path("quote.txt").write_text(quote)
Path("source.go").write_text('package main; const quote = "' + quote + '"')
Path("notes.txt").write_text("minted " + token)
"""

    def collision_output(self, run, mutation="none"):
        case = {"id": "probe", "program": "writer", "argv": [run, mutation], "stdin_b64": "",
                "initial": [{"path_b64": encoded(b"input.json"), "kind": "file",
                             "bytes_b64": encoded(b'{"quote":"The output must contain blind-\\u0061bcdef."}')}],
                "structured": {"tokens": [{"prefix": "blind", "sites": [{"glob": "ids.json", "pointers": ["/token"]}],
                                           "references": []}]}}
        return contract.execute(case, {"writer": [sys.executable, "-c", self.COLLISION_WRITER]})

    def test_structured_comparison_fails_a_site_token_that_equals_an_input_literal(self):
        primary, candidate = self.collision_output("primary"), self.collision_output("candidate", "quote")
        self.assertTrue(any(issue.startswith("blind-abcdef: the run generated a token the case's inputs already hold")
                            for issue in primary["inconsistencies"]), primary.get("inconsistencies"))
        for left, right in ((primary, candidate), (candidate, primary)):
            differences = contract.differential(left, right, self.STREAMS)
            self.assertTrue(any("consistency: blind-abcdef" in d for d in differences), differences)
            self.assertTrue(any("filesystem effects" in d for d in differences), differences)

    def test_structured_comparison_keeps_undeclared_bytes_that_hold_a_site_token(self):
        output = self.collision_output("plain")
        self.assertNotIn("inconsistencies", output)
        files = {contract.decode(entry["path_b64"]): contract.decode(entry["bytes_b64"])
                 for entry in output["filesystem"] if entry["kind"] == "file"}
        self.assertEqual(files[b"ids.json"], b'{"token": "<blind 1>"}')
        self.assertEqual(files[b"notes.txt"], b"minted blind-111111")

    def test_structured_comparison_fails_a_missing_or_malformed_token_site(self):
        sites = [{"glob": "out/grades.json", "pointers": ["/reviews/0/token", "/reviews/2/token",
                                                          "/reviews/0/families/0/id"]},
                 {"glob": "out/missing.json", "pointers": ["/token"]}]
        case = {"id": "probe", "program": "writer", "argv": ["1", "none"], "stdin_b64": "",
                "initial": self.GRADING_INPUTS,
                "structured": {"tokens": [{"prefix": "blind", "sites": sites, "references": []}]}}
        issues = contract.execute(case, {"writer": [sys.executable, "-c", self.GRADING_WRITER]})["inconsistencies"]
        self.assertEqual(len(issues), 3, issues)
        self.assertTrue(any("/reviews/2/token should hold a blind token" in issue for issue in issues))
        self.assertIn('out/grades.json: /reviews/0/families/0/id holds "GT-t1", not a blind token', issues)
        self.assertIn("out/missing.json: no file holds the declared blind tokens", issues)

    def test_structured_comparison_fails_a_wrong_pin_in_either_output(self):
        first, wrong = self.grading_output(1), self.grading_output(2, "wrong-pin")
        self.assertTrue(any("/verdicts_sha256 pins 0000" in issue for issue in wrong["inconsistencies"]),
                        wrong.get("inconsistencies"))
        self.assertTrue(any(d.startswith("consistency:") for d in contract.differential(first, wrong, self.STREAMS)))
        self.assertTrue(any(d.startswith("primary consistency:")
                            for d in contract.differential(wrong, first, self.STREAMS)))
        expected = {**first, "stdout": {"mode": "bytes", "bytes_b64": encoded(first["stdout"])},
                    "stderr": {"mode": "bytes", "bytes_b64": ""}}
        self.assertEqual(contract.compare(expected, self.grading_output(3)), [])
        self.assertTrue(any(d.startswith("consistency:") for d in contract.compare(expected, wrong)))

    def test_structured_and_outcome_declarations_are_validated(self):
        base = {"id": "probe", "program": "writer", "argv": [], "stdin_b64": "", "initial": []}
        sites = [{"glob": "x", "pointers": ["/a"]}]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "corpus.json"
            for broken, message in (
                    ({**base, "structured": {"tokens": ["blind"]}, "volatile": []}, "structured comparison"),
                    ({**base, "structured": {"pins": [{"glob": "x", "pointer": "/a"}]}}, "pin rule"),
                    ({**base, "structured": {"tokens": ["blind"]}}, "token rule"),
                    ({**base, "structured": {"tokens": [{"prefix": "blind", "sites": [{"glob": "x"}]}]}},
                     "token rule"),
                    ({**base, "structured": {"tokens": [{"prefix": "blind", "sites": sites}]}}, "token rule"),
                    ({**base, "structured": {"tokens": [{"prefix": "blind", "sites": sites,
                                                         "references": [{"glob": "x", "keys": True}]}]}},
                     "token rule"),
                    ({**base, "structured": {"identity": [{"glob": "x", "pointers": ["/a"]}]}}, "decided label"),
                    ({**base, "structured": {"sections": [{"glob": "x", "heading": "Links"}]}}, "heading prefix"),
                    ({**base, "expected": {"outcomes": [{"kind": "score", "path": "g", "pointer": "", "value": 1}]}},
                     "outcome states its kind")):
                with self.subTest(message=message):
                    path.write_text(json.dumps({"format": "behavior-contract/1", "decisions": "d.json",
                                                "cases": [broken]}))
                    (Path(temporary) / "d.json").write_text(json.dumps({"records": {"probe": decision(broken)}}))
                    with self.assertRaisesRegex(ValueError, message):
                        contract.load_corpus(path)

    def test_hand_stated_outcomes_hold_for_every_implementation(self):
        first, credit = self.grading_output(1), self.grading_output(2, "credit")
        outcomes = [{"kind": "family-outcome", "path": "out/grades.json", "pointer": "/reviews/1/families/0/outcome",
                     "value": "caught"},
                    {"kind": "credit-fact", "path": "out/grades.json", "pointer": "/reviews/1/credit/identifies_cause",
                     "value": "no"}]
        expected = {**first, "stdout": {"mode": "bytes", "bytes_b64": encoded(first["stdout"])},
                    "stderr": {"mode": "bytes", "bytes_b64": ""}, "outcomes": outcomes}
        self.assertEqual(contract.compare(expected, first), [])
        self.assertIn('outcome credit-fact out/grades.json/reviews/1/credit/identifies_cause: expected "no", actual "yes"',
                      contract.compare(expected, credit))
        step = {"expected": {**self.STREAMS["expected"], "outcomes": outcomes}}
        self.assertTrue(any(d.startswith("outcome credit-fact") for d in contract.differential(first, credit, step)))
        missing = contract.outcome_differences([{**outcomes[0], "path": "out/missing.json"}], first["filesystem"])
        self.assertTrue(missing and missing[0].startswith("outcome family-outcome out/missing.json"), missing)

    ENTROPY_PROGRAM = ("import random, secrets\n"
                       "items = list(range(8))\n"
                       "random.SystemRandom().shuffle(items)\n"
                       "print(secrets.token_hex(3), secrets.token_hex(4), secrets.token_hex(), items)\n")

    def test_entropy_driver_records_the_same_python_output_twice(self):
        with tempfile.TemporaryDirectory() as temporary:
            script = Path(temporary) / "draws.py"
            script.write_text(self.ENTROPY_PROGRAM, encoding="utf-8")
            case = {"id": "probe", "program": "draws", "argv": [], "stdin_b64": "", "initial": []}
            fixed = {"draws": contract.entropy_command(script)}
            first, second = contract.execute(case, fixed), contract.execute(case, fixed)
            self.assertEqual(first["exit"], 0, first["stderr"])
            self.assertEqual(first, second)
            tokens = first["stdout"].split()
            self.assertEqual([len(token) for token in tokens[:3]], [6, 8, 64])
            self.assertEqual(len(set(tokens[:3])), 3)
            free = {"draws": [sys.executable, str(script)]}
            self.assertNotEqual(contract.execute(case, free)["stdout"], contract.execute(case, free)["stdout"])
            with patch.dict(contract.PYTHON_ENTROPY, {"draws": str(script)}):
                self.assertEqual(contract.default_commands({"cases": [case]})["draws"], contract.entropy_command(
                    contract.REPO / script))

    def test_fixture_git_commits_the_same_freeze_on_every_host(self):
        manifest = encoded(b'{"freeze_commit": "<freeze commit>"}\n')
        case = {"id": "probe", "program": "show", "initial": [
            {"kind": "file", "path_b64": encoded(b"bench/runs/r/manifest.json"), "bytes_b64": manifest}],
            "steps": [{"program": "fixture-git", "argv": ["bench/runs/r/manifest.json"], "stdin_b64": ""},
                      {"argv": ["show", "HEAD:bench/runs/r/manifest.json"], "stdin_b64": ""}]}
        actual = contract.execute(case, {"show": ["git"], "fixture-git": [sys.executable, "-c", "raise SystemExit(9)"]})
        again = contract.execute(case, {"show": ["git"]})
        self.assertEqual(actual, again)
        freeze, show = actual["steps"]
        self.assertEqual((freeze["exit"], show["exit"]), (0, 0), freeze["stderr"])
        self.assertEqual(show["stdout"], contract.decode(manifest))
        files = {contract.decode(e["path_b64"]): e for e in freeze["filesystem"]}
        commit = json.loads(contract.decode(files[b"bench/runs/r/manifest.json"]["bytes_b64"]))["freeze_commit"]
        self.assertRegex(commit, "^[0-9a-f]{40}$")
        self.assertEqual(contract.decode(files[b".git/refs/heads/main"]["bytes_b64"]), commit.encode() + b"\n")
        self.assertFalse({b".git/config", b".git/COMMIT_EDITMSG"} & set(files))
        self.assertEqual(contract.decode(files[b".git/index"]["bytes_b64"])[12:36], bytes(24))

    def real_record_cli(self, root, listing, primary, candidate):
        directory = root.parent
        (directory / "list.json").write_text(json.dumps(listing), encoding="utf-8")
        for name, source in (("primary", primary), ("candidate", candidate)):
            (directory / f"{name}.py").write_text(source, encoding="utf-8")
            (directory / f"{name}.json").write_text(json.dumps({"probe": [sys.executable, str(directory / f"{name}.py")]}))
        return self.run_cli("--real-record", directory / "list.json", "--root", root, "--implementations",
                            directory / "primary.json", "--differential", directory / "candidate.json")

    def test_real_record_mode_compares_streams_exit_and_outputs_byte_for_byte(self):
        listing = {"format": "behavior-real-record/1", "commands": [
            {"id": "status", "program": "probe", "argv": ["check", {"output": "status.json"}], "outputs": ["status.json"]}]}
        primary = ("import sys\nfrom pathlib import Path\n"
                   "record = Path('bench/grading/current/grades.json').read_text()\n"
                   "Path(sys.argv[2]).write_text(record.upper())\n"
                   "print('current evidence valid; 1/1 admitted reviews assessed')\n")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            grades = root / "bench/grading/current/grades.json"
            grades.parent.mkdir(parents=True)
            grades.write_text('{"batches": []}\n')
            (root / ".gitignore").write_text(".cache/\n")
            for command in (["init", "-q"], ["add", "-A"], ["-c", "user.name=k", "-c", "user.email=k@example.invalid",
                                                              "commit", "-qm", "record"]):
                subprocess.run(["git", "-C", str(root), *command], check=True)
            grades.write_text('{"batches": [], "edited": true}\n')
            (root / ".cache").mkdir()
            (root / ".cache/evidence.bin").write_bytes(b"evidence")
            listing["commands"].append({**listing["commands"][0], "id": "second"})
            same = self.real_record_cli(root, listing, primary, primary)
            self.assertEqual(same.returncode, 0, same.stdout + same.stderr)
            self.assertIn("PASS status", same.stdout)
            rewrite = "Path('{}').write_bytes(Path('{}').read_bytes())\n"
            for name, candidate, message in (
                    ("stdout", primary.replace("1/1", "0/1"), "stdout bytes"),
                    ("exit", primary + "sys.exit(1)\n", "exit: primary 0, candidate 1"),
                    ("output", primary.replace("record.upper()", "record.upper() + ' '"), "output status.json"),
                    ("dirty tracked file", primary + rewrite.format(grades.relative_to(root), grades.relative_to(root)),
                     "candidate changed the checked-out record"),
                    ("ignored file", primary + rewrite.format(".cache/evidence.bin", ".cache/evidence.bin"),
                     "candidate changed the checked-out record")):
                with self.subTest(name=name):
                    changed = self.real_record_cli(root, listing, primary, candidate)
                    self.assertEqual(changed.returncode, 1, changed.stdout + changed.stderr)
                    self.assertIn("DIFFERENTIAL status", changed.stdout)
                    self.assertIn(message, changed.stdout)
                    if "changed" in message:
                        self.assertIn("stopped before the next command", changed.stdout)
                        self.assertNotIn("second", changed.stdout)
                    else:
                        self.assertIn("DIFFERENTIAL second", changed.stdout)
            writer = primary + rewrite.format(".cache/evidence.bin", ".cache/evidence.bin")
            stopped = self.real_record_cli(root, listing, writer, primary + "Path('candidate-ran').touch()\n")
            self.assertEqual(stopped.returncode, 1, stopped.stdout + stopped.stderr)
            self.assertIn("primary changed the checked-out record; stopped before the candidate", stopped.stdout)
            self.assertFalse((root / "candidate-ran").exists())
            listing["commands"].pop()
            self.assertEqual(contract.load_real_record(contract.REAL_RECORD)["commands"], [])
            for broken in ({**listing, "format": "x"}, {**listing, "commands": listing["commands"] * 2},
                           {**listing, "commands": [{**listing["commands"][0], "outputs": ["../x"]}]}):
                (root.parent / "list.json").write_text(json.dumps(broken))
                with self.assertRaises(ValueError):
                    contract.load_real_record(root.parent / "list.json")


if __name__ == "__main__":
    unittest.main()
