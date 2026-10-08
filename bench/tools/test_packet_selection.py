#!/usr/bin/env python3
"""Exercise frozen packet selection and dispatch without calling a model.

Usage: python3 bench/tools/test_packet_selection.py
Inputs: disposable git repositories. Exit: 0 on success, 1 on test failure.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
import codex_skill_runner as skill
import packet_selection
import run_cell


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
        shutil.copytree(run_cell.BENCH / 'schema', self.bench / 'schema')
        self.original = self.target / 'packet.md'
        self.original.write_bytes(b'# Original packet\n\n')
        self.packet = self.target / 'packet.v2.md'
        self.packet.write_bytes(b'# Re-cut packet\r\n\n\n')
        self.target_doc = {'id': 't1', 'packet_sha256': packet_selection.digest(self.original),
                           'diff_manifest_sha256': 'd' * 64, 'merge_base': 'a' * 40, 'head': 'b' * 40,
                           'provisioning': {'allowance': 'Run focused tests.', 'unavailable': 'network'}}
        self.write(self.target / 'target.json', self.target_doc)
        self.replacements = self.root / 'docs/packet-replacements.v1.json'
        self.replacement_doc = {'schema_version': 1, 'targets': [{
            'target': 't1', 'target_sha256': packet_selection.digest(self.target / 'target.json'),
            'original': {'path': str(self.original.relative_to(self.root)), 'sha256': packet_selection.digest(self.original)},
            'replacement': {'path': str(self.packet.relative_to(self.root)), 'sha256': packet_selection.digest(self.packet)}}]}
        self.write(self.replacements, self.replacement_doc)
        self.arm_path = self.bench / 'arms/test.json'
        self.arm = {'id': 'test', 'kind': 'codex', 'model': 'test-model', 'effort': 'high'}
        self.write(self.arm_path, self.arm)
        self.manifest = {
            'schema_version': 1, 'run_id': self.run.name, 'created_at': '2026-01-01T00:00:00Z',
            'method_revision': 'm', 'rubric_version': 1, 'metric_code_revision': 'c',
            'arms': [{'id': 'test', 'arm_file_sha256': packet_selection.digest(self.arm_path),
                      'resolved_skill_tree': None, 'expected_cli_version': 'test', 'expected_prompt_hashes': [],
                      'billing_mode': 'subscription'}],
            'cohort': [{'target': 't1', 'packet_sha256': packet_selection.digest(self.packet),
                        'diff_manifest_sha256': 'd' * 64, 'register_version': 1, 'cohort_group': 'fixture'}],
            'packet_replacements': {'path': str(self.replacements.relative_to(self.root)),
                                    'sha256': packet_selection.digest(self.replacements)},
            'planned_cells': [{'target': 't1', 'arm': 'test', 'replicate': 1}],
            'caps': {'max_attempts': 1, 'replacements': 0, 'spend_usd': 2, 'closeout_reserve_usd': 0,
                     'max_in_flight': 1, 'attempt_usd': 1},
            'sealed_order': ['t1/test/1'], 'rates': [], 'deviations': [],
            'execution_policy': {'allowance': 'Five minutes.', 'branch_layout': '`main` is the merge-base.'}}
        self.git('init', '-q')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'user.email', 'fixture@example.test')
        self.freeze()
        for module, name, value in ((run_cell, 'BENCH', self.bench), (run_cell, 'REPO', self.root),
                                     (skill, 'BENCH', self.bench)):
            self.enterContext(patch.object(module, name, value))

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

    def select(self):
        return packet_selection.select(self.run, self.manifest, self.target, self.root)

    def test_recut_dispatch_records_selection_and_preserves_packet_bytes(self):
        args = argparse.Namespace(next=True, cell=None, replace=None, dry_run=False, quota=None)
        calls = []

        def tool(argv, env=None):
            calls.append(argv)
            if Path(argv[0]).name == 'dispatch.sh':
                Path(argv[2], 'timing.json').write_text('{}', encoding='utf-8')
            return subprocess.CompletedProcess(argv, 0, '{}', '')

        with patch.object(run_cell, 'tool', side_effect=tool), \
                patch.object(run_cell, 'check_dispatch_rates', return_value=({}, {})), \
                patch.object(run_cell, 'file', return_value={'disposition': 'valid completed', 'usage': {'priced_total_usd': 0}}):
            run_cell.claim_and_run(self.run, self.work, args)
        attempt = self.work / 'att-001'
        self.assertTrue((attempt / 'input.md').read_bytes().startswith(self.packet.read_bytes()))
        claim = json.loads((attempt / 'cell.json').read_bytes())
        self.assertEqual(claim['packet_replacements'], self.manifest['packet_replacements'])
        self.assertEqual(claim['packet'], self.replacement_doc['targets'][0]['replacement'])
        self.assertEqual(len(calls), 2)

    def assert_refused_before_claim(self, message):
        self.write(self.run / 'manifest.json', self.manifest)
        args = argparse.Namespace(next=True, cell=None, replace=None, dry_run=False, quota=None)
        with patch.object(run_cell, 'check_dispatch_rates') as prices, patch.object(run_cell, 'tool') as tool:
            with self.assertRaisesRegex(run_cell.Refused, message):
                run_cell.claim_and_run(self.run, self.work, args)
        self.assertFalse((self.work / 'att-001').exists())
        prices.assert_not_called()
        tool.assert_not_called()

    def test_changed_target_refuses_before_claim(self):
        self.target_doc['head'] = 'c' * 40
        self.write(self.target / 'target.json', self.target_doc)
        self.assert_refused_before_claim('frozen target.json changed')

    def test_changed_original_refuses_before_claim(self):
        self.original.write_text('changed', encoding='utf-8')
        self.assert_refused_before_claim('original packet.md differs')

    def test_changed_replacement_refuses_before_claim(self):
        self.packet.write_text('changed', encoding='utf-8')
        self.assert_refused_before_claim('replacement packet differs')

    def test_changed_manifest_refuses_before_claim(self):
        self.replacements.write_text('{}', encoding='utf-8')
        self.assert_refused_before_claim('replacement manifest differs')

    def test_replacement_must_match_cohort(self):
        self.manifest['cohort'][0]['packet_sha256'] = '0' * 64
        self.freeze()
        self.assert_refused_before_claim('selected packet differs')

    def test_manifest_original_must_match_target_pin(self):
        self.replacement_doc['targets'][0]['original']['sha256'] = '0' * 64
        self.repin()
        self.assert_refused_before_claim('original packet identity differs')

    def repin(self):
        self.write(self.replacements, self.replacement_doc)
        self.manifest['packet_replacements']['sha256'] = packet_selection.digest(self.replacements)
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
                self.assert_refused_before_claim('differs from freeze_commit')

    def test_removing_frozen_pin_refuses_original_packet_before_claim(self):
        del self.manifest['packet_replacements']
        self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
        self.assert_refused_before_claim('differs from freeze_commit')

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
        self.assert_refused_before_claim('duplicate packet replacement target')

    def test_missing_and_escaping_paths_refuse(self):
        for name in ('missing.md', '../outside.md', '/tmp/outside.md'):
            with self.subTest(name=name):
                self.replacement_doc['targets'][0]['replacement']['path'] = name
                self.repin()
                self.assert_refused_before_claim('No such file|must stay inside')

    def test_legacy_prompt_hash_and_environment_override(self):
        del self.manifest['packet_replacements']
        self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
        self.freeze()
        with patch.dict('os.environ', {'BENCH_PACKET_REPLACEMENTS': str(self.replacements)}):
            self.assertEqual(self.select(), self.original)
        clone = Path('/fixture/clone')
        hashes = run_cell.write_input(self.work, self.target, 'Policy\n', clone)
        expected = ('# Original packet\n\nPolicy\n'
                    '\nPaths for this attempt: `<clone>` is `/fixture/clone`, `<cache>` is `/fixture/clone-cache`, '
                    'and the work directory is `/fixture/clone-work`.\n').encode()
        self.assertEqual((self.work / 'input.md').read_bytes(), expected)
        self.assertEqual(hashes['input_sha256'], hashlib.sha256(expected).hexdigest())

    def test_unlisted_target_uses_original_pin(self):
        self.replacement_doc['targets'] = []
        self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
        self.repin()
        self.assertEqual(self.select(), self.original)

    def prepare_skill(self, kind, invocation='Review {PACKET}\n'):
        self.arm['kind'] = kind
        self.write(self.arm_path, self.arm)
        entry = self.manifest['arms'][0]
        entry['arm_file_sha256'] = packet_selection.digest(self.arm_path)
        inputs = self.run / 'inputs'
        (inputs / 'skill').mkdir(parents=True, exist_ok=True)
        (inputs / 'skill/SKILL.md').write_text('Review the code.\n', encoding='utf-8')
        tree, files = skill.skill_tree_hash(inputs / 'skill')
        entry['resolved_skill_tree'] = tree
        self.write(inputs / 'skill-pin.json', {'tree_hash': tree, 'files': files, 'skill_id': 'fixture'})
        (inputs / 'invocation.md').write_text(invocation, encoding='utf-8')
        (inputs / 'shared-policy.md').write_text('Policy\n', encoding='utf-8')
        self.write(inputs / 'runner.json', {'artifact_root': 'report'})
        self.write(inputs / 'input-pin.json', {'files': [
            {'path': name, 'bytes': (inputs / name).stat().st_size, 'sha256': packet_selection.digest(inputs / name)}
            for name in ('invocation.md', 'runner.json', 'shared-policy.md')]})
        self.freeze()
        attempt = self.work / kind
        clone = attempt / 'clone'
        clone.mkdir(parents=True, exist_ok=True)
        return argparse.Namespace(run=str(self.run), attempt_dir=str(attempt), clone=str(clone),
                                  packet=str(self.packet), arm=str(self.arm_path), target='t1')

    def skill_prompt(self, args, kind):
        def git(clone, *command):
            return self.target_doc['merge_base'] if command == ('rev-parse', 'main') else self.target_doc['head']
        with patch.object(skill, 'git', side_effect=git):
            return skill.prepare_attempt(args, kind)['attempt_input'].read_bytes().decode('utf-8')

    def test_skill_prompt_with_recut_differs_from_saved_prompt_only_in_packet_text(self):
        pinned = copy.deepcopy(self.manifest)
        for kind in ('codex-skill', 'claude-skill'):
            for invocation in ('Review {PACKET}\n', 'The review task section above holds the packet.\n'):
                with self.subTest(kind=kind, invocation=invocation):
                    self.manifest = copy.deepcopy(pinned)
                    recut = self.skill_prompt(self.prepare_skill(kind, invocation), kind)
                    del self.manifest['packet_replacements']
                    self.manifest['cohort'][0]['packet_sha256'] = self.target_doc['packet_sha256']
                    args = self.prepare_skill(kind, invocation)
                    args.packet = str(self.original)
                    before, after = self.skill_prompt(args, kind).split('# Original packet')
                    self.assertEqual(recut, before + '# Re-cut packet' + after)
                    if '{PACKET}' not in invocation:
                        self.assertTrue(recut.startswith('Policy\n\n## Review task\n\n# Re-cut packet\n\n'))

    def test_both_skill_runners_reject_original_packet_for_recut_run(self):
        for kind in ('codex-skill', 'claude-skill'):
            with self.subTest(kind=kind):
                args = self.prepare_skill(kind)
                args.packet = str(self.original)
                with self.assertRaisesRegex(skill.RunnerError, 'PR packet differs'):
                    self.skill_prompt(args, kind)

    def test_dispatch_passes_recut_to_each_skill_runner(self):
        for kind in ('codex-skill', 'claude-skill'):
            with self.subTest(kind=kind):
                self.arm['kind'] = kind
                self.write(self.arm_path, self.arm)
                attempt = self.work / kind
                attempt.mkdir()
                commands = []
                def tool(argv, env=None):
                    commands.append(argv)
                    if Path(argv[1]).name == run_cell.SKILL_RUNNERS[kind]:
                        (attempt / 'timing.json').write_text('{}', encoding='utf-8')
                    return subprocess.CompletedProcess(argv, 0, '{}', '')
                claim = {'cell': {'target': 't1', 'arm': 'test', 'replicate': 1}}
                with patch.object(run_cell, 'tool', side_effect=tool):
                    run_cell.dispatch(run_cell.Run(self.run, self.work), kind, claim)
                command = commands[-1]
                self.assertEqual(command[command.index('--packet') + 1], str(self.packet))


if __name__ == '__main__':
    unittest.main(verbosity=2)
