"""Prove workspace cleanup preserves evidence and refuses unsafe attempts."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

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


if __name__ == '__main__':
    unittest.main()
