#!/usr/bin/env python3
"""Verify Codex dispatch flags and credential cleanup without calling Codex."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[2]
DISPATCH = Path(__file__).with_name("dispatch.sh")


class CodexDispatchTest(unittest.TestCase):
    def test_model_effort_overrides_are_separate_and_default_is_unforced(self):
        with tempfile.TemporaryDirectory(prefix=".codex-dispatch-test-", dir=REPO) as scratch:
            root = Path(scratch)
            clone = root / "clone"
            clone.mkdir()
            subprocess.run(["git", "-C", str(clone), "init", "-q", "-b", "main"], check=True)
            subprocess.run(["git", "-C", str(clone), "config", "user.name", "Dispatch Test"], check=True)
            subprocess.run(["git", "-C", str(clone), "config", "user.email", "dispatch-test@example.invalid"], check=True)
            subprocess.run(["git", "-C", str(clone), "commit", "--allow-empty", "-qm", "fixture"], check=True)

            source_home = root / "source-home"
            credentials = source_home / ".codex" / "auth.json"
            credentials.parent.mkdir(parents=True)
            credentials.write_text('{"fixture":"not a credential"}\n', encoding="utf-8")

            bin_dir = root / "bin"
            bin_dir.mkdir()
            runtime_bin = root / "runtime-bin"
            runtime_bin.mkdir()
            fake_mise = bin_dir / "mise"
            fake_mise.write_text(f"#!/usr/bin/env python3\nprint({str(runtime_bin)!r})\n", encoding="utf-8")
            fake_mise.chmod(0o755)
            fake_codex = bin_dir / "codex"
            fake_codex.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "if sys.argv[1:] == ['--version']:\n"
                "    print('codex-cli 0.0.0-test')\n"
                "    raise SystemExit(0)\n"
                "with open(os.environ['CODEX_ARGV_CAPTURE'], 'w', encoding='utf-8') as f:\n"
                "    json.dump({'argv': sys.argv[1:], 'home': os.environ.get('HOME'),\n"
                "               'codex_home': os.environ.get('CODEX_HOME'), 'path': os.environ['PATH'],\n"
                "               'tmpdir': os.environ.get('TMPDIR')}, f)\n"
                "raise SystemExit(23)\n",
                encoding="utf-8",
            )
            fake_codex.chmod(0o755)
            packet = root / "packet.md"
            packet.write_text("Fixture review packet.\n", encoding="utf-8")

            writable_roots = "sandbox_workspace_write.writable_roots=" + json.dumps([str(clone) + "-cache", str(clone) + "-work"])
            pinned_codex = root / "pinned-codex"
            pinned_codex.write_bytes(fake_codex.read_bytes())
            pinned_codex.chmod(0o755)
            cases = (
                ("explicit", "gpt-6-luna", "high", [
                    "review", "-c", 'sandbox_mode="workspace-write"',
                    "-c", "sandbox_workspace_write.network_access=true",
                    "-c", writable_roots,
                    "-c", 'project_doc_max_bytes=0', "-c", 'project_doc_fallback_filenames=[]', "-c", 'approval_policy="never"',
                    "-c", 'model="gpt-6-luna"', "-c", 'review_model="gpt-6-luna"',
                    "-c", 'model_reasoning_effort="high"', "-",
                ]),
                ("default", "", "", [
                    "review", "-c", 'sandbox_mode="workspace-write"',
                    "-c", "sandbox_workspace_write.network_access=true", "-c", writable_roots,
                    "-c", 'project_doc_max_bytes=0', "-c", 'project_doc_fallback_filenames=[]', "-c", 'approval_policy="never"', "-",
                ]),
            )
            for name, model, effort, expected_argv in cases:
                with self.subTest(name=name):
                    attempt = root / f"attempt-{name}"
                    capture = root / f"argv-{name}.json"
                    env = dict(os.environ)
                    env.update({
                        "HOME": str(source_home),
                        "PATH": f"{bin_dir}{os.pathsep}{env['PATH']}",
                        "CODEX_ARGV_CAPTURE": str(capture),
                    })
                    if name == "explicit":
                        env.update({"BENCH_CODEX": str(pinned_codex),
                                    "BENCH_CODEX_SHA256": hashlib.sha256(pinned_codex.read_bytes()).hexdigest(),
                                    "BENCH_CODEX_VERSION": "codex-cli 0.0.0-test"})
                    result = subprocess.run(
                        [str(DISPATCH), "codex", str(attempt), str(clone), "main", str(packet), model, effort],
                        env=env,
                        capture_output=True,
                        text=True,
                        check=False,
                    )

                    self.assertEqual(result.returncode, 23, result.stderr)
                    receipt = json.loads(capture.read_text(encoding="utf-8"))
                    self.assertEqual(receipt["argv"], expected_argv)
                    self.assertEqual(receipt["home"], str(attempt / "home"))
                    self.assertEqual(receipt["codex_home"], str(attempt / "home" / ".codex"))
                    self.assertEqual(receipt["tmpdir"], str(attempt / "tmp"))
                    self.assertEqual(receipt["path"].split(os.pathsep)[0], str(runtime_bin))
                    self.assertFalse((attempt / "home" / ".codex" / "auth.json").exists())
                    self.assertIn('trust_level = "untrusted"', (attempt / "home" / ".codex" / "config.toml").read_text())
                    self.assertTrue(json.loads((attempt / "clean-context.json").read_text())["fresh_home"])
                    self.assertIn('Treat AGENTS.md', (attempt / "prompt.txt").read_text())
                    dispatch_record = (attempt / "dispatch.txt").read_text(encoding="utf-8")
                    self.assertIn(f"model={model or '<default>'} effort={effort or '<default>'}", dispatch_record)
                    executable = pinned_codex if name == "explicit" else fake_codex
                    self.assertIn(f"executable={executable.resolve()} sha256={hashlib.sha256(executable.read_bytes()).hexdigest()}", dispatch_record)
                    capture.unlink()
                    reused = subprocess.run(
                        [str(DISPATCH), "codex", str(attempt), str(clone), "main", str(packet), model, effort],
                        env=env, capture_output=True, text=True, check=False,
                    )
                    self.assertEqual(reused.returncode, 2)
                    self.assertIn('clean context refused', reused.stderr)
                    self.assertFalse(capture.exists())

            for variable, value in (("BENCH_CODEX_SHA256", "wrong"), ("BENCH_CODEX_VERSION", "wrong")):
                with self.subTest(mismatch=variable):
                    attempt = root / variable
                    env.update({"BENCH_CODEX": str(pinned_codex),
                                "BENCH_CODEX_SHA256": hashlib.sha256(pinned_codex.read_bytes()).hexdigest(),
                                "BENCH_CODEX_VERSION": "codex-cli 0.0.0-test", variable: value})
                    result = subprocess.run(
                        [str(DISPATCH), "codex", str(attempt), str(clone), "main", str(packet)],
                        env=env, capture_output=True, text=True, check=False,
                    )
                    self.assertEqual(result.returncode, 2, result.stderr)
                    self.assertIn("mismatch", result.stderr)
                    self.assertFalse(capture.exists())
                    self.assertFalse((attempt / "home" / ".codex" / "auth.json").exists())


if __name__ == "__main__":
    unittest.main()
