#!/usr/bin/env python3
"""Exercise frozen packet selection through ``bench-tools packet-selection``.

Usage: python3 bench/tools/test_packet_selection.py
Inputs: disposable git repositories. Exit: 0 on success, 1 on test failure.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parent))
import rust_runtime

TOOLS = str(rust_runtime.resolve("bench-tools"))


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class PacketSelectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bench = self.root / 'bench'
        self.target = self.bench / 'targets/t1'
        self.run = self.bench / 'runs/2026-01-01-test'
        self.work = self.root / 'work'
        self.target.mkdir(parents=True)
        self.run.mkdir(parents=True)
        self.work.mkdir()
        self.original = self.target / 'packet.md'
        self.original.write_bytes(b'# Original packet\n\n')
        self.packet = self.target / 'packet.v2.md'
        self.packet.write_bytes(b'# Re-cut packet\r\n\n\n')
        self.target_doc = {'id': 't1', 'packet_sha256': digest(self.original),
                           'diff_manifest_sha256': 'd' * 64, 'merge_base': 'a' * 40, 'head': 'b' * 40,
                           'provisioning': {'allowance': 'Run focused tests.', 'unavailable': 'network'}}
        self.write(self.target / 'target.json', self.target_doc)
        self.replacements = self.root / 'docs/packet-replacements.v1.json'
        self.replacement_doc = {'schema_version': 1, 'targets': [{
            'target': 't1', 'target_sha256': digest(self.target / 'target.json'),
            'original': {'path': str(self.original.relative_to(self.root)), 'sha256': digest(self.original)},
            'replacement': {'path': str(self.packet.relative_to(self.root)), 'sha256': digest(self.packet)}}]}
        self.write(self.replacements, self.replacement_doc)
        self.arm_path = self.bench / 'arms/test.json'
        self.arm = {'id': 'test', 'kind': 'codex', 'model': 'test-model', 'effort': 'high'}
        self.write(self.arm_path, self.arm)
        self.manifest = {
            'schema_version': 1, 'run_id': self.run.name, 'created_at': '2026-01-01T00:00:00Z',
            'method_revision': 'm', 'rubric_version': 1, 'metric_code_revision': 'c',
            'arms': [{'id': 'test', 'arm_file_sha256': digest(self.arm_path),
                      'resolved_skill_tree': None, 'expected_cli_version': 'test', 'expected_prompt_hashes': [],
                      'billing_mode': 'subscription'}],
            'cohort': [{'target': 't1', 'packet_sha256': digest(self.packet),
                        'diff_manifest_sha256': 'd' * 64, 'register_version': 1, 'cohort_group': 'fixture'}],
            'packet_replacements': {'path': str(self.replacements.relative_to(self.root)),
                                    'sha256': digest(self.replacements)},
            'planned_cells': [{'target': 't1', 'arm': 'test', 'replicate': 1}],
            'caps': {'max_attempts': 1, 'replacements': 0, 'spend_usd': 2, 'closeout_reserve_usd': 0,
                     'max_in_flight': 1, 'attempt_usd': 1},
            'sealed_order': ['t1/test/1'], 'rates': [], 'deviations': [],
            'execution_policy': {'allowance': 'Five minutes.', 'branch_layout': '`main` is the merge-base.'}}
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.test')
        self.freeze()

    def write(self, path, doc):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(doc), encoding='utf-8')

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.root), *args], text=True, encoding='utf-8').strip()

    def freeze(self):
        self.write(self.run / 'manifest.json', self.manifest)
        self.git('add', '.')
        self.git('commit', '-qm', 'Freeze fixture')
        self.manifest.update(frozen_at='2026-01-01T00:00:00Z', freeze_commit=self.git('rev-parse', 'HEAD'))
        self.write(self.run / 'manifest.json', self.manifest)

    def selection(self, env=None):
        self.write(self.run / 'manifest.json', self.manifest)
        return subprocess.run([TOOLS, 'packet-selection', '--run', str(self.run), '--target-dir', str(self.target),
                               '--root', str(self.root)], capture_output=True, text=True, encoding='utf-8', env=env)

    def select(self, env=None):
        result = self.selection(env)
        self.assertEqual(result.returncode, 0, result.stderr)
        return Path(result.stdout.rstrip('\n'))

    def assert_refused(self, message):
        result = self.selection()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, '')
        self.assertRegex(result.stderr, message)

    def test_frozen_replacement_is_selected(self):
        self.assertEqual(self.select(), self.packet)

    def test_changed_target_is_refused(self):
        self.target_doc['head'] = 'c' * 40
        self.write(self.target / 'target.json', self.target_doc)
        self.assert_refused('frozen target.json changed')

    def test_changed_original_is_refused(self):
        self.original.write_text('changed', encoding='utf-8')
        self.assert_refused('original packet.md differs')

    def test_changed_replacement_is_refused(self):
        self.packet.write_text('changed', encoding='utf-8')
        self.assert_refused('replacement packet differs')

    def test_changed_manifest_is_refused(self):
        self.replacements.write_text('{}', encoding='utf-8')
        self.assert_refused('replacement manifest differs')

    def test_replacement_must_match_cohort(self):
        self.manifest['cohort'][0]['packet_sha256'] = '0' * 64
        self.freeze()
        self.assert_refused('selected packet differs')

    def test_manifest_original_must_match_target_pin(self):
        self.replacement_doc['targets'][0]['original']['sha256'] = '0' * 64
        self.repin()
        self.assert_refused('original packet identity differs')

    def repin(self):
        self.write(self.replacements, self.replacement_doc)
        self.manifest['packet_replacements']['sha256'] = digest(self.replacements)
        self.freeze()

    def test_pin_and_cohort_cannot_change_after_freeze(self):
        original = copy.deepcopy(self.manifest)
        for key in ('packet_replacements', 'cohort'):
            with self.subTest(key=key):
                self.manifest = copy.deepcopy(original)
                if key == 'packet_replacements':
                    self.manifest[key]['sha256'] = '0' * 64
                else:
                    self.manifest[key][0]['packet_sha256'] = '0' * 64
                self.assert_refused('differs from freeze_commit')

    def test_removing_frozen_pin_refuses_original_packet(self):
        del self.manifest['packet_replacements']
        self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
        self.assert_refused('differs from freeze_commit')

    def test_legacy_run_without_readable_freeze_remains_supported(self):
        del self.manifest['packet_replacements']
        self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
        for freeze in (None, '0' * 40):
            with self.subTest(freeze=freeze):
                self.manifest['freeze_commit'] = freeze
                self.assertEqual(self.select(), self.original)

    def test_duplicate_replacement_is_refused(self):
        self.replacement_doc['targets'] *= 2
        self.repin()
        self.assert_refused('duplicate packet replacement target')

    def test_missing_and_escaping_paths_refuse(self):
        for name in ('missing.md', '../outside.md', '/tmp/outside.md'):
            with self.subTest(name=name):
                self.replacement_doc['targets'][0]['replacement']['path'] = name
                self.repin()
                self.assert_refused('No such file|must stay inside')

    def test_environment_does_not_override_the_frozen_pin(self):
        del self.manifest['packet_replacements']
        self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
        self.freeze()
        env = {**os.environ, 'BENCH_PACKET_REPLACEMENTS': str(self.replacements)}
        self.assertEqual(self.select(env), self.original)

    def test_unlisted_target_uses_original_pin(self):
        self.replacement_doc['targets'] = []
        self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
        self.repin()
        self.assertEqual(self.select(), self.original)


if __name__ == '__main__':
    unittest.main(verbosity=2)
