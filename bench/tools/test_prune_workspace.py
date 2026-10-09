"""Prove workspace cleanup preserves evidence and refuses unsafe attempts."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import native_artifacts
import prune_workspace


class CleanupTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.attempt = self.root / 'bench/runs/run/attempts/att-001'
        self.attempt.mkdir(parents=True)
        self.workspace = self.root / 'work/run/att-001'
        clone = self.workspace / 'clone'
        clone.mkdir(parents=True)
        for args in [['init', '-q'], ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '-qm', 'fixture']]:
            subprocess.run(['git', '-C', str(clone), *args], check=True)
        head = subprocess.check_output(['git', '-C', str(clone), 'rev-parse', 'HEAD'], text=True).strip()
        target = self.root / 'bench/targets/task'
        target.mkdir(parents=True)
        (target / 'target.json').write_text(json.dumps({'head': head}))
        for name in ['clone-cache', 'clone-work', 'home']:
            (self.workspace / name).mkdir()
            (self.workspace / name / 'keep').write_text(name)
        archive = self.root / 'transcript.tar.gz'
        archive.write_bytes(b'preserved transcript fixture')
        self.record = {'run_id': 'run', 'attempt_id': 'att-001', 'disposition': 'valid completed',
            'cell': {'target': 'task'}, 'usage': {'metering_status': 'complete'},
            'observed': {'tree_identity_before': 'same', 'tree_identity_after': 'same'},
            'transcript_archive': {'path': str(archive), 'sha256': hashlib.sha256(archive.read_bytes()).hexdigest(), 'restoration_check': 'passed'}}
        (self.attempt / 'normalized.json').write_text('{}')
        (self.attempt / 'usage-requests.jsonl').write_text('{}\n')
        self.save()

    def save(self):
        (self.attempt / 'attempt.json').write_text(json.dumps(self.record))

    def test_verified_cleanup_retains_evidence_and_is_idempotent(self):
        preview = prune_workspace.prune(self.attempt, self.workspace)
        self.assertFalse(preview['applied'])
        self.assertTrue((self.workspace / 'clone').exists())
        readonly = self.workspace / 'clone-cache/readonly'
        readonly.mkdir()
        (readonly / 'cached').write_text('rebuildable')
        readonly.chmod(0o555)
        first = prune_workspace.prune(self.attempt, self.workspace, apply=True)
        self.assertFalse((self.workspace / 'clone').exists())
        self.assertFalse((self.workspace / 'clone-cache').exists())
        self.assertEqual((self.workspace / 'clone-work/keep').read_text(), 'clone-work')
        self.assertEqual((self.workspace / 'home/keep').read_text(), 'home')
        self.assertTrue((self.root / 'transcript.tar.gz').exists())
        self.assertTrue((self.attempt / 'normalized.json').exists())
        self.assertEqual(prune_workspace.prune(self.attempt, self.workspace, apply=True), first)

    def test_archive_corruption_prevents_deletion(self):
        (self.root / 'transcript.tar.gz').write_bytes(b'changed')
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune(self.attempt, self.workspace, apply=True)
        self.assertTrue((self.workspace / 'clone-cache').exists())

    def test_fixture_cleanup_uses_explicit_target_and_checks_identity(self):
        target = self.root / 'bench/runs/toy/fixture'
        target.parent.mkdir(parents=True)
        (self.root / 'bench/targets/task').rename(target)
        definition = json.loads((target / 'target.json').read_text())
        definition['id'] = 'other-task'
        (target / 'target.json').write_text(json.dumps(definition))
        with self.assertRaisesRegex(prune_workspace.Refused, 'target identity'):
            prune_workspace.prune(self.attempt, self.workspace, apply=True, target_dir=target)
        self.assertTrue((self.workspace / 'clone').exists())
        definition['id'] = 'task'
        (target / 'target.json').write_text(json.dumps(definition))
        receipt = prune_workspace.prune(self.attempt, self.workspace, apply=True, target_dir=target)
        self.assertTrue(receipt['applied'])
        self.assertFalse((self.workspace / 'clone').exists())

    def test_corrupt_native_scratch_archive_prevents_cleanup(self):
        native = self.workspace / 'reports'
        scratch = native / 'scratch'
        scratch.mkdir(parents=True)
        (native / 'review.json').write_text('{}')
        (scratch / 'probe.py').write_text('print("evidence")')
        index = self.workspace / 'native-artifacts.json'
        index.write_text(json.dumps({'root': str(native), 'files': [
            {'path': p.relative_to(native).as_posix(), 'bytes': p.stat().st_size,
             'sha256': native_artifacts.digest(p)} for p in native.rglob('*') if p.is_file()]}))
        (self.attempt / 'attempt.json').unlink()
        self.record['native_artifact_storage'] = native_artifacts.file_artifacts(
            native, index, self.attempt, 'review.json')
        self.save()
        self.assertFalse(prune_workspace.prune(self.attempt, self.workspace)['applied'])
        (self.attempt / native_artifacts.ARCHIVE).write_bytes(b'corrupt')
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune(self.attempt, self.workspace, apply=True)
        self.assertTrue((self.workspace / 'clone').exists())
        self.assertTrue((self.workspace / 'clone-cache').exists())

    def test_failed_attempt_is_retained(self):
        self.record['disposition'] = 'harness-invalid'
        self.save()
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune(self.attempt, self.workspace, apply=True)
        self.assertTrue((self.workspace / 'clone').exists())

    def test_modified_clone_is_retained(self):
        (self.workspace / 'clone/new.txt').write_text('diagnostic evidence')
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune(self.attempt, self.workspace, apply=True)
        self.assertTrue((self.workspace / 'clone/new.txt').exists())

    def test_linked_cache_is_retained(self):
        import shutil
        shutil.rmtree(self.workspace / 'clone-cache')
        (self.workspace / 'clone-cache').symlink_to(self.workspace / 'clone-work', target_is_directory=True)
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune(self.attempt, self.workspace, apply=True)
        self.assertTrue((self.workspace / 'clone-work/keep').exists())


class GradingCleanupTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name).resolve() / 'work'
        clone = self.work / 'clone'
        clone.mkdir(parents=True)
        for args in [['init', '-q'], ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '-qm', 'fixture']]:
            subprocess.run(['git', '-C', str(clone), *args], check=True)
        self.head = subprocess.check_output(['git', '-C', str(clone), 'rev-parse', 'HEAD'], text=True).strip()
        for name in ['clone-cache', 'clone-work', 'home']:
            (self.work / name).mkdir()
            (self.work / name / 'keep').write_text(name)
        self.record = {'session_id': 'session-a', 'exit_code': 0, 'verdicts_present': True, 'audit_violations': [],
                       'usage': {'priced_total_usd': 1.5}}
        (self.work / 'verdicts.json').write_text('{"reviews": {}}')
        self.mapped = {'provenance': {'kind': 'dispatch', 'session_id': 'session-a'},
                       'verdicts_sha256': hashlib.sha256(b'{"reviews": {}}').hexdigest()}
        self.save()

    def save(self):
        (self.work / 'dispatch.json').write_text(json.dumps(self.record))

    def kept(self):
        return [name for name in ('clone', 'clone-cache') if (self.work / name).exists()]

    def test_completed_grading_is_pruned_with_a_receipt_and_is_idempotent(self):
        evidence = {name: (self.work / name).read_bytes() for name in ('dispatch.json', 'verdicts.json')}
        preview = prune_workspace.prune_grading(self.work, self.head, self.mapped)
        self.assertFalse(preview['applied'])
        self.assertEqual(self.kept(), ['clone', 'clone-cache'])
        self.assertFalse((self.work / 'workspace-pruned.json').exists())
        readonly = self.work / 'clone-cache/readonly'
        readonly.mkdir()
        (readonly / 'cached').write_text('rebuildable')
        readonly.chmod(0o555)
        first = prune_workspace.prune_grading(self.work, self.head, self.mapped, apply=True)
        self.assertEqual(self.kept(), [])
        self.assertEqual(first['paths'], [str(self.work / 'clone'), str(self.work / 'clone-cache')])
        self.assertEqual(first['verdicts_sha256'], hashlib.sha256(evidence['verdicts.json']).hexdigest())
        self.assertEqual(first['dispatch_sha256'], hashlib.sha256(evidence['dispatch.json']).hexdigest())
        self.assertEqual(first['session_id'], 'session-a')
        self.assertTrue(all(type(first[name]) is int for name in ('free_bytes_before', 'free_bytes_after')))
        self.assertEqual(json.loads((self.work / 'workspace-pruned.json').read_text()), first)
        self.assertEqual({name: (self.work / name).read_bytes() for name in evidence}, evidence)
        self.assertEqual((self.work / 'clone-work/keep').read_text(), 'clone-work')
        self.assertEqual((self.work / 'home/keep').read_text(), 'home')
        self.assertEqual(prune_workspace.prune_grading(self.work, self.head, self.mapped, apply=True), first)

    def test_active_failed_and_unverified_gradings_are_retained(self):
        cases = {'active': None, 'failed session': {'exit_code': 1}, 'timed out': {'exit_code': None},
                 'no verdicts': {'verdicts_present': False}, 'access violation': {'audit_violations': ['read outside']},
                 'unpriced': {'usage': {'priced_total_usd': None}}}
        for name, change in cases.items():
            with self.subTest(name):
                (self.work / 'dispatch.json').unlink(missing_ok=True)
                if change is not None:
                    (self.work / 'dispatch.json').write_text(json.dumps({**self.record, **change}))
                with self.assertRaises(prune_workspace.Refused):
                    prune_workspace.prune_grading(self.work, self.head, self.mapped, apply=True)
                self.assertEqual(self.kept(), ['clone', 'clone-cache'])
        self.save()
        (self.work / 'verdicts.json').unlink()
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune_grading(self.work, self.head, self.mapped, apply=True)
        self.assertEqual(self.kept(), ['clone', 'clone-cache'])
        self.assertFalse((self.work / 'workspace-pruned.json').exists())

    def test_unmapped_or_rejected_gradings_are_retained(self):
        manual = {**self.mapped, 'provenance': {'kind': 'manual', 'identity': 'someone'}}
        for name, assessment in {'another session': {**self.mapped, 'provenance': {'kind': 'dispatch', 'session_id': 'session-b'}},
                                 'other verdicts': {**self.mapped, 'verdicts_sha256': '0' * 64},
                                 'a manual receipt for a dispatched session': manual}.items():
            with self.subTest(name):
                with self.assertRaises(prune_workspace.Refused):
                    prune_workspace.prune_grading(self.work, self.head, assessment, apply=True)
                self.assertEqual(self.kept(), ['clone', 'clone-cache'])

    def test_manually_assessed_workspace_is_pruned_without_a_dispatch_record(self):
        import shutil
        (self.work / 'dispatch.json').unlink()
        shutil.rmtree(self.work / 'home')
        manual = {**self.mapped, 'provenance': {'kind': 'manual', 'identity': 'someone'}}
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune_grading(self.work, self.head, {**manual, 'verdicts_sha256': '0' * 64}, apply=True)
        receipt = prune_workspace.prune_grading(self.work, self.head, manual, apply=True)
        self.assertEqual(self.kept(), [])
        self.assertEqual((receipt['applied'], receipt['verdicts_sha256']), (True, manual['verdicts_sha256']))
        self.assertNotIn('session_id', receipt)

    def test_modified_or_moved_clone_is_retained(self):
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune_grading(self.work, '0' * 40, self.mapped, apply=True)
        (self.work / 'clone/new.txt').write_text('diagnostic evidence')
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune_grading(self.work, self.head, self.mapped, apply=True)
        self.assertTrue((self.work / 'clone/new.txt').exists())
        self.assertEqual(self.kept(), ['clone', 'clone-cache'])

    def test_linked_cache_is_retained(self):
        import shutil
        shutil.rmtree(self.work / 'clone-cache')
        (self.work / 'clone-cache').symlink_to(self.work / 'clone-work', target_is_directory=True)
        with self.assertRaises(prune_workspace.Refused):
            prune_workspace.prune_grading(self.work, self.head, self.mapped, apply=True)
        self.assertTrue((self.work / 'clone-work/keep').exists())
        self.assertTrue((self.work / 'clone').exists())

    def test_workspace_without_rebuildable_directories_is_left_alone(self):
        import shutil
        for name in ('clone', 'clone-cache'):
            shutil.rmtree(self.work / name)
        (self.work / 'dispatch.json').unlink()
        self.assertIsNone(prune_workspace.prune_grading(self.work, self.head, self.mapped, apply=True))
        self.assertFalse((self.work / 'workspace-pruned.json').exists())

    def test_command_line_previews_applies_and_refuses(self):
        target = Path(self.temp.name) / 'target'
        target.mkdir()
        (target / 'target.json').write_text(json.dumps({'head': self.head}))
        receipt = Path(self.temp.name) / 'receipt.json'
        receipt.write_text(json.dumps(self.mapped))
        located = ['--target', str(target), '--receipt', str(receipt)]

        def cli(*extra):
            return subprocess.run([sys.executable, prune_workspace.__file__, '--grading-work', str(self.work), *extra],
                                  capture_output=True, text=True)

        done = cli(*located)
        self.assertEqual((done.returncode, json.loads(done.stdout)['applied']), (0, False))
        self.assertEqual(self.kept(), ['clone', 'clone-cache'])
        self.assertEqual(cli('--target', str(target)).returncode, 2)
        self.record['exit_code'] = 1
        self.save()
        done = cli(*located, '--apply')
        self.assertEqual((done.returncode, done.stdout.strip()), (1, 'cleanup refused: grading session did not complete validly'))
        self.assertEqual(self.kept(), ['clone', 'clone-cache'])
        self.record['exit_code'] = 0
        self.save()
        done = cli(*located, '--apply')
        self.assertEqual((done.returncode, json.loads(done.stdout)['applied']), (0, True))
        self.assertEqual(self.kept(), [])


if __name__ == '__main__':
    unittest.main()
