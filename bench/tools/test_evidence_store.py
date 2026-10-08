"""Exercise portable publication and restoration without a GitHub write."""
import copy
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

import evidence_store as store


class FakeGitHub:
    def __init__(self, root):
        self.root = Path(root)
        self.assets = []
        self.uploads = 0
        self.requests = []
        self.draft = False

    def release(self, repository, tag):
        self.requests.append((repository, tag))
        return {'id': 42, 'draft': self.draft, 'assets': copy.deepcopy(self.assets)}

    def upload(self, repository, tag, archive):
        self.uploads += 1
        if any(a['name'] == archive.name for a in self.assets):
            raise AssertionError('attempted overwrite')
        shutil.copyfile(archive, self.root / archive.name)
        self.assets.append({'id': 100 + len(self.assets), 'name': archive.name, 'size': archive.stat().st_size})

    def download(self, repository, asset_id, destination, limit):
        asset = next(a for a in self.assets if a['id'] == asset_id)
        shutil.copyfile(self.root / asset['name'], destination)


class EvidenceStoreTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.remote_root = self.root / 'remote'
        self.remote_root.mkdir()
        self.remote = FakeGitHub(self.remote_root)
        self.source = self.root / 'source'
        self.source.mkdir()
        self.payloads = {'artifacts/transcripts/run/att-001.tar.gz': b'original compressed transcript',
                         'artifacts/workspaces/run/att-001/probe.py': b'print("observed")\n',
                         'artifacts/reproducibility/task.tar.gz': b'original frozen dependency archive'}
        files = []
        for i, (path, content) in enumerate(self.payloads.items()):
            source = self.source / str(i)
            source.write_bytes(content)
            source.chmod(0o755 if path.endswith('.py') else 0o644)
            files.append({'source': str(source), 'path': path, 'kind': 'reproducibility' if '/reproducibility/' in path else 'evidence'})
        self.selection = {'repository': 'canonical/bench', 'tag': 'evidence-test', 'subjects': ['run/att-001'], 'files': files}
        self.packed = self.root / 'packed'
        self.manifest_path = self.root / 'published.json'
        store.pack(self.selection, self.packed)

    def publish(self):
        return store.publish(self.packed / 'manifest.json', self.manifest_path, self.remote)

    def test_published_bytes_restore_in_fresh_checkout_without_sources(self):
        manifest = self.publish()
        shutil.rmtree(self.source)
        shutil.rmtree(self.packed)
        clone = self.root / 'fork-checkout'
        store.materialize(manifest, clone, remote=self.remote)
        for path, content in self.payloads.items():
            self.assertEqual((clone / path).read_bytes(), content)
        self.assertEqual((clone / 'artifacts/workspaces/run/att-001/probe.py').stat().st_mode & 0o777, 0o755)
        self.assertTrue(all(repo == 'canonical/bench' for repo, _ in self.remote.requests))
        self.assertEqual(self.remote.uploads, 1)
        shutil.rmtree(clone / 'artifacts')
        self.remote.assets.clear()
        store.materialize(manifest, clone, offline=True)
        self.assertEqual((clone / next(iter(self.payloads))).read_bytes(), next(iter(self.payloads.values())))

    def test_selective_fetch_and_existing_changed_file(self):
        manifest = self.publish()
        name = next(iter(self.payloads))
        clone = self.root / 'clone'
        store.materialize(manifest, clone, [name], remote=self.remote)
        self.assertFalse((clone / 'artifacts/workspaces').exists())
        (clone / name).write_bytes(b'local modification')
        with self.assertRaisesRegex(store.EvidenceError, 'changed evidence'):
            store.materialize(manifest, clone, remote=self.remote)
        self.assertEqual((clone / name).read_bytes(), b'local modification')

    def test_retry_does_not_upload_or_replace_published_identity(self):
        first = self.publish()
        self.assertEqual(first, self.publish())
        self.assertEqual(self.remote.uploads, 1)
        (self.remote_root / self.remote.assets[0]['name']).write_bytes(b'changed')
        with self.assertRaises(store.EvidenceError):
            self.publish()
        self.assertEqual(store.read(self.manifest_path), first)

    def test_failed_remote_readback_never_creates_manifest(self):
        original = self.remote.download
        def corrupt(*args):
            original(*args)
            Path(args[2]).write_bytes(b'corrupt remote bytes')
        self.remote.download = corrupt
        with self.assertRaises(store.EvidenceError):
            self.publish()
        self.assertFalse(self.manifest_path.exists())

    def test_missing_and_replaced_assets_refuse_verification(self):
        manifest = self.publish()
        self.remote.assets[0]['id'] += 1
        with self.assertRaisesRegex(store.EvidenceError, 'identity changed'):
            store.verify_remote(manifest, self.remote)
        self.remote.assets.clear()
        with self.assertRaises(store.EvidenceError):
            store.materialize(manifest, self.root / 'empty', remote=self.remote)

    def test_archive_member_attacks_are_rejected_even_with_matching_outer_hash(self):
        manifest = self.publish()
        for name, kind in [('../../escape', tarfile.REGTYPE), ('artifacts/link', tarfile.SYMTYPE),
                           (next(iter(self.payloads)), tarfile.REGTYPE)]:
            with self.subTest(name=name, kind=kind):
                archive = self.root / 'attack.tar.gz'
                with tarfile.open(archive, 'w:gz') as bundle:
                    info = tarfile.TarInfo(name)
                    info.type = kind
                    info.linkname = '/tmp'
                    info.size = 3 if kind == tarfile.REGTYPE else 0
                    bundle.addfile(info, io.BytesIO(b'bad') if info.size else None)
                package = {**manifest['packages'][0], 'sha256': store.digest(archive), 'bytes': archive.stat().st_size}
                with self.assertRaises(store.EvidenceError):
                    store.unpack(archive, package, self.root / 'attacked')
        self.assertFalse((self.root / 'escape').exists())

    def test_symlink_destination_and_invalid_manifest_are_rejected(self):
        manifest = self.publish()
        clone = self.root / 'clone'
        clone.mkdir()
        (clone / 'artifacts').symlink_to(self.remote_root, target_is_directory=True)
        with self.assertRaisesRegex(store.EvidenceError, 'symlink'):
            store.materialize(manifest, clone, remote=self.remote)
        broken = copy.deepcopy(manifest)
        broken['packages'][0]['members'][0]['path'] = 'bench/../../.git/config'
        with self.assertRaisesRegex(store.EvidenceError, 'unsafe'):
            store.validate(broken)
        broken = copy.deepcopy(manifest)
        broken['packages'][0]['members'].append(broken['packages'][0]['members'][0])
        with self.assertRaisesRegex(store.EvidenceError, 'duplicate'):
            store.validate(broken)

    def test_packing_preserves_inputs_and_splits_packages(self):
        selection = copy.deepcopy(self.selection)
        output = self.root / 'split'
        manifest = store.pack(selection, output, part_limit=40)
        self.assertEqual(len(manifest['packages']), 3)
        for source in self.source.iterdir():
            self.assertTrue(source.is_file())
        again = store.pack(selection, self.root / 'again', part_limit=40)
        self.assertEqual(manifest, again)

    def test_draft_release_refused_and_cli_reports_invalid_selection(self):
        self.remote.draft = True
        with self.assertRaisesRegex(store.EvidenceError, 'draft'):
            self.publish()
        self.assertEqual(self.remote.uploads, 0)
        selection = copy.deepcopy(self.selection)
        selection['files'][0]['path'] = '/tmp/escape'
        path = self.root / 'selection.json'
        path.write_text(json.dumps(selection), encoding='utf-8')
        result = subprocess.run([sys.executable, store.__file__, 'pack', '--selection', str(path), '--out', str(self.root / 'bad')],
                                capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('unsafe evidence path', result.stdout)


if __name__ == '__main__':
    unittest.main()
