import copy
import json
from pathlib import Path
import tempfile
import unittest

import evidence_store as store
import recover_evidence as recovery
from test_evidence_store import FakeGitHub


class Recovery(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        source = self.root / 'source'
        source.write_bytes(b'original observation')
        source.chmod(0o600)
        packed = self.root / 'packed'
        store.pack({'repository': 'owner/bench', 'tag': 'evidence-original', 'subjects': ['attempt'], 'files': [
            {'source': str(source), 'path': 'artifacts/observed.txt', 'kind': 'evidence'}]}, packed)
        remote_dir = self.root / 'remote'
        remote_dir.mkdir()
        self.remote = FakeGitHub(remote_dir)
        self.original_path = self.root / 'bench/evidence/manifests/original.json'
        self.original = store.publish(packed / 'manifest.json', self.original_path, self.remote)
        self.index = {'schema_version': 1, 'repository': 'owner/bench', 'tag': 'evidence-recovery', 'sources': [{
            'name': 'original', 'manifest': 'bench/evidence/manifests/original.json',
            'manifest_sha256': store.digest(self.original_path),
            'publications': {p['sha256']: p['publication'] for p in self.original['packages']}}]}
        self.save()

    def save(self):
        path = self.root / recovery.INDEX
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.index))

    def test_restores_when_original_release_is_unavailable_without_changing_bytes(self):
        read_release = self.remote.release
        def recovery_only(repository, tag):
            if tag != 'evidence-recovery':
                raise RuntimeError('original release unavailable')
            return read_release(repository, tag)
        self.remote.release = recovery_only
        before = self.original_path.read_bytes()
        with self.assertRaisesRegex(RuntimeError, 'unavailable'):
            store.materialize(self.original, self.root, remote=self.remote)
        store.materialize(recovery.manifest(self.root, 'original'), self.root, remote=self.remote)
        payload = self.root / 'artifacts/observed.txt'
        self.assertEqual(payload.read_bytes(), b'original observation')
        self.assertEqual(payload.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.original_path.read_bytes(), before)

    def test_changed_original_and_incomplete_package_mapping_are_refused(self):
        before = self.original_path.read_bytes()
        self.original_path.write_bytes(before + b'\n')
        with self.assertRaisesRegex(store.EvidenceError, 'manifest changed'):
            recovery.manifest(self.root, 'original')
        self.original_path.write_bytes(before)
        self.index['sources'][0]['publications'] = {}
        self.save()
        with self.assertRaisesRegex(store.EvidenceError, 'exactly the original packages'):
            recovery.manifest(self.root, 'original')

    def test_wrong_repository_unsafe_source_and_duplicate_selection_are_refused(self):
        original = copy.deepcopy(self.index)
        self.index['repository'] = 'other/storage'
        self.save()
        with self.assertRaises(store.EvidenceError):
            recovery.manifest(self.root, 'original')
        self.index = copy.deepcopy(original)
        self.index['sources'][0]['manifest'] = '../outside'
        self.save()
        with self.assertRaises(store.EvidenceError):
            recovery.manifest(self.root, 'original')
        self.index = copy.deepcopy(original)
        self.index['sources'].append(copy.deepcopy(self.index['sources'][0]))
        self.save()
        with self.assertRaises(store.EvidenceError):
            recovery.manifest(self.root, 'original')


if __name__ == '__main__':
    unittest.main()
