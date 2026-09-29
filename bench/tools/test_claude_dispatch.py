"""Check pinned native Claude dispatch without paid calls."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ClaudeDispatchTest(unittest.TestCase):
    def test_pin_checks_precede_authentication_and_review(self):
        with tempfile.TemporaryDirectory(dir=ROOT, prefix='.claude-dispatch-test-') as directory:
            root = Path(directory)
            clone = root / 'clone'
            clone.mkdir()
            for args in [['init', '-q'], ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '-qm', 'fixture']]:
                subprocess.run(['git', '-C', str(clone), *args], check=True)
            source = root / 'source'
            (source / '.claude').mkdir(parents=True)
            (source / '.claude/.credentials.json').write_text('{}')
            (source / '.claude.json').write_text('{}')
            packet = root / 'packet.md'
            packet.write_text('Review fixture.')
            executable = root / 'pinned-claude'
            executable.write_text("#!/usr/bin/env python3\nimport os,sys,json\nif sys.argv[1:] == ['--version']:\n print('2.1.284 (Claude Code)');sys.exit(0)\nwith open(os.environ['CAPTURE'], 'w') as f: json.dump({'args':sys.argv[1:],'home':os.environ['HOME']}, f)\nsys.exit(23)\n")
            executable.chmod(0o755)
            capture = root / 'capture.json'
            env = dict(os.environ, HOME=str(source), CAPTURE=str(capture), BENCH_CLAUDE=str(executable),
                       BENCH_CLAUDE_SHA256=hashlib.sha256(executable.read_bytes()).hexdigest(),
                       BENCH_CLAUDE_VERSION='2.1.284 (Claude Code)')
            def invoke(name):
                return subprocess.run([str(ROOT / 'bench/tools/dispatch.sh'), 'claude-builtin', str(root / name), str(clone), 'main', str(packet), 'claude-fable-5-1', 'high'], env=env, capture_output=True, text=True)
            result = invoke('valid')
            self.assertEqual(result.returncode, 23, result.stderr)
            recorded = json.loads(capture.read_text())
            self.assertIn('--safe-mode', recorded['args'])
            self.assertEqual(recorded['args'][recorded['args'].index('--model') + 1], 'claude-fable-5-1')
            self.assertEqual(recorded['args'][recorded['args'].index('--effort') + 1], 'high')
            self.assertEqual(recorded['home'], str(root / 'valid/home'))
            self.assertFalse((root / 'valid/home/.claude/.credentials.json').exists())
            capture.unlink()
            for name, key, wrong in [('hash', 'BENCH_CLAUDE_SHA256', '0' * 64), ('version', 'BENCH_CLAUDE_VERSION', 'wrong')]:
                previous = env[key]
                env[key] = wrong
                result = invoke(name)
                env[key] = previous
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn('mismatch', result.stderr)
                self.assertFalse(capture.exists())
                self.assertFalse((root / name / 'home/.claude/.credentials.json').exists())


if __name__ == '__main__':
    unittest.main()
