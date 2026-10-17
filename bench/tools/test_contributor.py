"""Exercise contributor behavior with disposable repositories and no provider calls."""
from __future__ import annotations

import contextlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import contributor as cli
import contributor_adapters as adapters
import contributor_common as common
import contributor_submit as submit
import evidence_store
from contributor_common import command, digest, read, write
from test_evidence_store import FakeGitHub

REPOSITORY = Path(__file__).resolve().parents[2]


def git(root, *arguments):
    return command(["git", "-C", str(root), *arguments])


class ContributorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="contributor-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.state = self.root / ".bench"
        self.patches = [patch.object(common, "ROOT", self.root), patch.object(common, "STATE", self.state)]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)
        git(self.root, "init", "-b", "main")
        git(self.root, "config", "user.name", "Contributor")
        git(self.root, "config", "user.email", "contributor@example.test")
        (self.root / "source.txt").write_text("base\n")
        git(self.root, "add", "source.txt")
        git(self.root, "commit", "-m", "test: create source")
        self.base = git(self.root, "rev-parse", "HEAD")
        (self.root / "source.txt").write_text("head\n")
        git(self.root, "add", "source.txt")
        git(self.root, "commit", "-m", "test: update source")
        self.head = git(self.root, "rev-parse", "HEAD")
        self.profiles = read(REPOSITORY / "bench/contributor-profiles.json")
        write(self.root / "bench/contributor-profiles.json", self.profiles)
        for directory in ("bench/tools", "bench/schema", "bench/policies", "docs"):
            (self.root / directory).mkdir(parents=True, exist_ok=True)
        (self.root / "bench/tools/fixture.py").write_text("print('pinned runtime')\n")
        for name in ("clean-context.md", "evidence-storage.md"):
            (self.root / "docs" / name).write_text("Frozen policy\n")
        (self.root / "bench/policies/empty-harness-v1.md").write_text("No ambient configuration.\n")
        self.tasks = []
        for name in ("task-a", "task-b"):
            target = self.root / "bench/targets" / name
            target.mkdir(parents=True)
            (target / "packet.md").write_text(f"Review {name}\n")
            write(target / "target.json", {"id": name, "head": self.head, "merge_base": self.base,
                  "diff_manifest_sha256": "f" * 64, "provisioning": {"allowance": "read only", "unavailable": "tests"}})
            self.tasks.append({"id": name, "revision": {"head": self.head, "base_sha": self.base,
                               "packet_sha256": digest(target / "packet.md"), "diff_manifest_sha256": "f" * 64}})
        write(self.root / "bench/scoreboard.current.json", {"tasks": self.tasks})
        self.executable = self.root / "client"
        self.executable.write_text("#!/bin/sh\nexit 0\n")
        self.executable.chmod(0o755)
        self.runtime = {"path": str(self.executable), "sha256": digest(self.executable), "version": "0.161.0"}
        self.rate = {"provider": "native", "rate": {"input": 1, "output": 1, "cache_read": .1, "cache_write_5m": 1.25}}
        self.calls = []
        self.real_subprocess_run = subprocess.run

    def invoke(self, *arguments):
        output = io.StringIO()
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
            code = cli.main(list(arguments))
        return code, output.getvalue()

    def fake_provision(self, argv, **kwargs):
        if "prepare" not in argv or not any(str(part).endswith("provision.py") for part in argv):
            return self.real_subprocess_run(argv, **kwargs)
        clone = Path(argv[argv.index("--out") + 1])
        git(self.root, "clone", "--no-hardlinks", str(self.root), str(clone))
        git(clone, "checkout", "-b", "review-head", self.head)
        git(clone, "branch", "-f", "main", self.base)
        git(clone, "remote", "remove", "origin")
        (clone.parent / "clone-cache").mkdir()
        (clone.parent / "clone-work").mkdir()
        return subprocess.CompletedProcess(argv, 0, "prepared", "")

    def fake_execute(self, config, attempt, target, packet, skill):
        self.calls.append((target["id"], config["effort"]))
        (attempt / "home").mkdir()
        (attempt / "stdout.jsonl").write_text("original transcript\n")
        write(attempt / "completion.json", {"exit_code": 0, "tree_before": "same", "tree_after": "same"}, immutable=True)
        return 0

    def fake_collect(self, config, attempt):
        usage = {"metering_status": "complete", "priced_total_usd": .01}
        normalized = {"parse_status": "empty", "items": []}
        write(attempt / "usage.json", usage, immutable=True)
        write(attempt / "normalized.json", normalized, immutable=True)
        return usage, normalized

    @contextlib.contextmanager
    def no_charge_run(self):
        with patch.object(adapters, "prepare", return_value=self.runtime), \
             patch.object(adapters, "rates_snapshot", return_value=self.rate), \
             patch.object(adapters, "verify_rates", return_value=self.rate), \
             patch.object(adapters, "execute", side_effect=self.fake_execute), \
             patch.object(adapters, "collect", side_effect=self.fake_collect), \
             patch.object(subprocess, "run", side_effect=self.fake_provision):
            yield

    def run_complete(self, *extra):
        with self.no_charge_run():
            code, output = self.invoke("run", "review", "--harness", "codex", "--model", "gpt-6.1-sol", "--attempt-usd", "1", "--yes", *extra)
        self.assertEqual(code, 0, output)
        return self.state / "runs" / f"R{len(list((self.state / 'runs').iterdir()))}"

    def test_complete_scope_two_efforts_cleanup_and_pending_exclusion(self):
        run = self.run_complete("--effort", "medium,high", "--tasks", "task-a", "--repeat", "2")
        self.assertEqual(self.calls, [("task-a", "medium")] * 2 + [("task-a", "high")] * 2)
        self.assertEqual(len(read(run / "manifest.json")["cells"]), 4)
        for attempt in cli.attempts(run):
            self.assertFalse((attempt / "clone").exists())
            self.assertFalse((attempt / "clone-cache").exists())
            self.assertTrue((attempt / "home").is_dir())
            self.assertTrue((attempt / "clone-work").is_dir())
            self.assertTrue((attempt / "workspace-pruned.json").is_file())
        self.assertEqual(self.invoke("status")[0], 0)
        shutil.rmtree(self.state / "pending/R1")
        self.assertEqual(self.invoke("status")[0], 0)
        self.assertFalse((self.state / "pending/R1").exists())
        self.assertEqual(self.invoke("run", "--resume", "R1", "--yes")[0], 0)
        self.assertEqual(len(self.calls), 4)
        self.assertFalse((self.state / "pending/R1").exists())

    def test_frozen_evidence_and_runner_drift_block_submission_and_resume(self):
        run = self.run_complete("--tasks", "task-a")
        (run / "attempts/att-001/stdout.jsonl").write_text("changed transcript")
        with self.assertRaisesRegex(common.Refused, "frozen evidence changed"):
            submit.snapshot()
        code, output = self.invoke("run", "--resume", "R1", "--yes")
        self.assertEqual(code, 1, output)
        self.assertIn("frozen evidence changed", output)

    def test_runtime_change_blocks_even_unchanged_plan(self):
        run = self.run_complete("--tasks", "task-a")
        (self.root / "bench/tools/fixture.py").write_text("different runtime")
        with self.assertRaisesRegex(common.Refused, "runtime differs"):
            cli.verify_run(run)

    def test_invalid_invocations_models_efforts_never_probe_or_launch(self):
        cases = [("arbitrary prompt", "codex", "gpt-6.1-sol", "high"),
                 ("review", "claude-code", "claude-opus-5-5", "high"),
                 ("/code-review", "codex", "gpt-6.1-sol", "high"),
                 ("review", "codex", "new-unapproved-model", "high"),
                 ("review", "codex", "gpt-6.1-sol", "max"),
                 ("review", "codex", "gpt-6.1-sol", "high,high")]
        with patch.object(adapters, "prepare") as probe:
            for invocation, harness, model, effort in cases:
                code, output = self.invoke("run", invocation, "--harness", harness, "--model", model, "--effort", effort, "--yes")
                self.assertEqual(code, 1, output)
            probe.assert_not_called()

    def test_partial_results_keep_full_plan_and_failed_predecessor(self):
        def fail_second(config, attempt, target, packet, skill):
            result = self.fake_execute(config, attempt, target, packet, skill)
            if target["id"] == "task-b":
                raise OSError("controller lost after review completed")
            return result
        with self.no_charge_run(), patch.object(adapters, "execute", side_effect=fail_second):
            code, output = self.invoke("run", "review", "--harness", "codex", "--model", "gpt-6.1-sol", "--attempt-usd", "1", "--yes")
        self.assertEqual(code, 2, output)
        self.assertFalse((self.state / "pending/R1").exists())
        # Recovery files the already completed second review rather than paying for it again.
        with self.no_charge_run():
            code, output = self.invoke("run", "--resume", "R1", "--yes")
        self.assertEqual(code, 0, output)
        self.assertEqual(len(self.calls), 2)
        self.assertTrue((self.state / "pending/R1").exists())

    def test_partial_inclusion_and_unknown_usage_block_dispatch(self):
        calls = 0
        def stop_after_first(config, attempt, target, packet, skill):
            nonlocal calls
            calls += 1
            if calls == 2:
                (attempt / "stdout.jsonl").write_text("partial unknown usage")
                raise OSError("interrupted before completion")
            return self.fake_execute(config, attempt, target, packet, skill)
        with self.no_charge_run(), patch.object(adapters, "execute", side_effect=stop_after_first):
            code, output = self.invoke("run", "review", "--harness", "codex", "--model", "gpt-6.1-sol", "--attempt-usd", "1", "--yes")
        self.assertEqual(code, 2, output)
        with self.no_charge_run():
            code, output = self.invoke("run", "--resume", "R1", "--yes")
        self.assertEqual(code, 1, output)
        self.assertIn("unknown usage", output)
        self.assertEqual(calls, 2)
        code, output = self.invoke("run", "--resume", "R1", "--include-partial")
        self.assertEqual(code, 0, output)
        entry = read(self.state / "pending/R1/entry.json")
        self.assertTrue(entry["partial"])
        self.assertEqual(len(entry["missing"]), 1)
        self.assertEqual(len(entry["attempts"]), 2)
        self.assertEqual(len(read(self.state / "runs/R1/manifest.json")["cells"]), 2)

    def test_submission_snapshot_excludes_runs_completed_later_and_recovers_receipts(self):
        self.run_complete("--tasks", "task-a")
        saved = submit.snapshot()
        self.run_complete("--tasks", "task-b")
        self.assertEqual([row["run_id"] for row in submit.snapshot()["entries"]], ["R1"])
        saved.update(staging_repository="contributor/code-review-bench", login="contributor")
        submit.save(saved)
        local = submit.package(saved)
        remote_root = self.root / "remote"
        remote_root.mkdir()
        remote = FakeGitHub(remote_root)
        with patch.object(evidence_store, "GitHub", return_value=remote), patch.object(submit, "api", return_value={"draft": False}):
            submit.publish_assets(saved, local)
            submit.publish_assets(saved, local)
        self.assertEqual(remote.uploads, 1)
        saved.update(pr_url="https://github.com/kamui/code-review-bench/pull/999", head=self.head)
        submit.finish(saved)
        submit.finish(saved)
        self.assertFalse((self.state / "pending/R1").exists())
        self.assertTrue((self.state / "pending/R2").exists())
        self.assertEqual(read(self.state / "submitted/R1/receipt.json")["pr_url"], saved["pr_url"])
        cold = self.root / "cold"
        evidence_store.materialize(read(local / "records" / saved["manifest"]), cold, remote=remote)
        payloads = list(cold.rglob("stdout.jsonl"))
        self.assertEqual(len(payloads), 1)
        self.assertEqual(payloads[0].read_text(), "original transcript\n")

    def test_ambiguous_pr_write_reads_before_retrying(self):
        saved = {"login": "contributor", "branch": "contributions/x", "head": self.head,
                 "entries": [], "staging_repository": "contributor/code-review-bench", "tag": "tag"}
        existing = [{"url": "https://github.com/kamui/code-review-bench/pull/999", "headRefOid": self.head, "state": "OPEN"}]
        calls = []
        def fake_gh(*arguments):
            calls.append(arguments)
            if arguments[:2] == ("pr", "list"):
                return json.dumps(existing if len(calls) > 1 else [])
            raise common.Refused("lost response after server created PR")
        with patch.object(submit, "gh", side_effect=fake_gh):
            self.assertEqual(submit.create_pr(saved, self.root), existing[0]["url"])
        self.assertEqual(sum(args[:2] == ("pr", "create") for args in calls), 1)

    def test_invalid_pending_entry_blocks_instead_of_disappearing(self):
        self.run_complete("--tasks", "task-a")
        (self.state / "pending/broken").mkdir()
        with self.assertRaisesRegex(common.Refused, "invalid pending"):
            submit.snapshot()
        self.assertTrue((self.state / "pending/R1/entry.json").exists())

    def test_budget_refusal_preserves_progress_without_pending(self):
        with self.no_charge_run():
            code, output = self.invoke("run", "review", "--harness", "codex", "--model", "gpt-6.1-sol", "--attempt-usd", "1", "--budget-usd", "1", "--yes")
        self.assertEqual(code, 1, output)
        self.assertEqual(len(self.calls), 1)
        self.assertFalse((self.state / "pending/R1").exists())


class AdapterTests(unittest.TestCase):
    def test_published_skill_and_entire_task_suite_prepare_without_a_model_call(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(common, "STATE", Path(temporary)), \
             patch.object(adapters, "prepare", return_value={"path": "/client", "sha256": "a" * 64, "version": "0.161.0"}), \
             patch.object(adapters, "rates_snapshot", return_value={}), patch.object(adapters, "execute") as execute:
            output = io.StringIO()
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
                code = cli.main(["run", "/ce-code-review", "--harness", "codex", "--model", "gpt-6.1-sol"])
            self.assertEqual(code, 1, output.getvalue())
            manifest = read(Path(temporary) / "runs/R1/manifest.json")
            self.assertEqual(len(manifest["tasks"]), len(read(REPOSITORY / "bench/scoreboard.current.json")["tasks"]))
            self.assertTrue((Path(temporary) / "runs/R1/inputs/skill/SKILL.md").is_file())
            provenance = read(Path(temporary) / "runs/R1/inputs/skill-provenance.json")
            self.assertEqual(provenance["date"], "2026-09-28")
            execute.assert_not_called()

    @unittest.skipUnless(shutil.which("bwrap") and sys.platform == "linux", "adapter isolation requires Linux bubblewrap")
    def test_openrouter_adapters_execute_fresh_subprocesses_and_collect_reports(self):
        scratch = REPOSITORY / ".local"
        scratch.mkdir(exist_ok=True)
        for harness in ("pi", "opencode"):
            with self.subTest(harness=harness), tempfile.TemporaryDirectory(dir=scratch) as temporary:
                attempt = Path(temporary) / "att-001"
                attempt.mkdir()
                clone = attempt / "clone"
                clone.mkdir()
                git(clone, "init", "-b", "review-head")
                git(clone, "config", "user.name", "Contributor")
                git(clone, "config", "user.email", "contributor@example.test")
                (clone / "source.txt").write_text("source\n")
                (clone / "AGENTS.md").write_text("AMBIENT-INSTRUCTION-MARKER")
                (clone / ".opencode").mkdir()
                (clone / ".opencode/settings.json").write_text("AMBIENT-CONFIG-MARKER")
                git(clone, "add", ".")
                git(clone, "commit", "-m", "test: create target")
                packet = Path(temporary) / "packet.md"
                packet.write_text("Review source.txt\n")
                skill = Path(temporary) / "skill"
                skill.mkdir()
                (skill / "SKILL.md").write_text("Produce a review.\n")
                executable = attempt / "fake-client"
                executable.write_text('''#!/usr/bin/python3
import json, os, pathlib, sys
assert '--resume' not in sys.argv and '--continue' not in sys.argv
home = pathlib.Path(os.environ['HOME'])
assert not (home / 'ambient-marker').exists()
assert pathlib.Path('AGENTS.md').read_text() == ''
assert list(pathlib.Path('.opencode').iterdir()) == []
try:
    pathlib.Path('source.txt').write_text('forbidden source edit')
except OSError:
    pass
else:
    raise AssertionError('review clone must be read-only')
report = pathlib.Path(os.environ['CE_REVIEW_ARTIFACT_ROOT'])
report.mkdir(parents=True)
(report / 'review.json').write_text(json.dumps({'status':'complete','verdict':'clear','findings':[]}))
if '--provider' in sys.argv:
    print(json.dumps({'type':'message_end','message':{'role':'assistant','model':'openai/gpt-6.1-sol','thinkingLevel':'high','usage':{'cost':{'total':0.01}}}}))
    print(json.dumps({'type':'agent_settled','aborted':False}))
else:
    print(json.dumps({'type':'step_finish','part':{'reason':'stop','cost':0.01}}))
''')
                executable.chmod(0o755)
                config = {"harness": harness, "model": "openai/gpt-6.1-sol", "effort": "high", "kind": "skill",
                          "runtime": {"path": str(executable), "sha256": digest(executable)}, "policy": "Frozen policy"}
                with patch.object(adapters, "credentials", return_value=[]):
                    result = adapters.execute(config, attempt, {"merge_base": "base", "provisioning": {"allowance": "read only", "unavailable": "tests"}}, packet, skill)
                self.assertEqual(result, 0, (attempt / "stderr.txt").read_text())
                usage, normalized = adapters.collect(config, attempt)
                self.assertEqual(usage["priced_total_usd"], .01)
                self.assertEqual(normalized["parse_status"], "empty")
                context = read(attempt / "clean-context.json")
                self.assertTrue(context["fresh_home"])
                self.assertEqual(read(attempt / "completion.json")["tree_before"], read(attempt / "completion.json")["tree_after"])
                with self.assertRaises(FileExistsError):
                    adapters.execute(config, attempt, {"provisioning": {}}, packet, skill)

    def test_actual_client_command_paths_are_explicit_and_fresh(self):
        with tempfile.TemporaryDirectory() as temporary:
            attempt = Path(temporary)
            home, clone = attempt / "home", attempt / "clone"
            home.mkdir()
            clone.mkdir()
            for harness in ("codex", "claude-code", "pi", "opencode"):
                config = {"harness": harness, "model": "openai/gpt-6.1-sol" if harness in ("pi", "opencode") else "gpt-6.1-sol",
                          "effort": "high", "runtime": {"path": "/client"}, "kind": "skill", "policy": "frozen", "reserve_usd": 1}
                argv, env = adapters.client_command(config, attempt, home, clone, attempt / "skill")
                self.assertEqual(argv[0], "/client")
                self.assertNotIn("--resume", argv)
                self.assertNotIn("--continue", argv)
                self.assertNotIn("OPENAI_API_KEY", env)
                self.assertEqual(env["HOME"], str(home))
                if harness == "pi":
                    self.assertIn("--no-context-files", argv)
                    self.assertIn("--no-extensions", argv)
                if harness == "opencode":
                    self.assertIn("--standalone", argv)
                    self.assertIn("openrouter/openai/gpt-6.1-sol#high", argv)
                if harness == "codex":
                    config["kind"] = "native"
                    native, _ = adapters.client_command(config, attempt, home, clone, None)
                    self.assertEqual(native[-2:], ["review", "-"])

    def test_pi_and_opencode_meter_authoritative_completion_once(self):
        for harness in ("pi", "opencode"):
            with self.subTest(harness=harness), tempfile.TemporaryDirectory() as temporary:
                attempt = Path(temporary)
                (attempt / "clone-work/report").mkdir(parents=True)
                write(attempt / "clone-work/report/review.json", {"status": "complete", "verdict": "clear", "findings": []})
                if harness == "pi":
                    message = {"role": "assistant", "model": "openai/gpt-6.1-sol", "thinkingLevel": "high", "usage": {"cost": {"total": .02}}}
                    rows = [{"type": "message_update", "usage": message["usage"]}, {"type": "message_end", "message": message},
                            {"type": "agent_settled", "aborted": False}]
                else:
                    rows = [{"type": "step_finish", "part": {"reason": "stop", "cost": .02, "tokens": {"input": 5}}}]
                (attempt / "stdout.jsonl").write_text("\n".join(map(json.dumps, rows)))
                usage, report = adapters.collect({"harness": harness, "model": "openai/gpt-6.1-sol", "effort": "high"}, attempt)
                self.assertEqual(usage["priced_total_usd"], .02)
                self.assertEqual(report["parse_status"], "empty")

    def test_pi_unknown_cost_stays_unknown(self):
        with tempfile.TemporaryDirectory() as temporary:
            attempt = Path(temporary)
            (attempt / "clone-work/report").mkdir(parents=True)
            write(attempt / "clone-work/report/review.json", {"status": "complete", "verdict": "clear", "findings": []})
            rows = [{"type": "message_end", "message": {"role": "assistant", "model": "openai/gpt-6.1-sol"}},
                    {"type": "agent_settled", "aborted": False}]
            (attempt / "stdout.jsonl").write_text("\n".join(map(json.dumps, rows)))
            usage, _ = adapters.collect({"harness": "pi", "model": "openai/gpt-6.1-sol", "effort": "high"}, attempt)
            self.assertIsNone(usage["priced_total_usd"])
            self.assertEqual(usage["metering_status"], "incomplete")


if __name__ == "__main__":
    unittest.main()
