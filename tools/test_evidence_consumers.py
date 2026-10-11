"""Cold retrieval preserves frozen records through the existing evidence consumers."""
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'bench/tools'))
import evidence_store as store
import native_artifacts
import prune_workspace
import regrade
from test_evidence_store import FakeGitHub
import collect_run
import export_explorer
import import_benchmark


class ColdConsumersTest(unittest.TestCase):
    def test_success_failed_replacement_and_grading_survive_origin_removal(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            origin, cold, remote_path = [base / name for name in ('origin', 'cold', 'remote')]
            origin.mkdir()
            cold.mkdir()
            remote_path.mkdir()
            run = origin / 'bench/runs/example'
            dispositions = {'att-001': 'valid completed', 'att-002': 'failed: fixture', 'att-003': 'valid completed'}
            for name, disposition in dispositions.items():
                attempt = run / 'attempts' / name
                attempt.mkdir(parents=True)
                archive = base / (name + '.tar.gz')
                with tarfile.open(archive, 'w:gz') as bundle:
                    data = ('session ' + name).encode()
                    info = tarfile.TarInfo('home/.codex/sessions/session.jsonl')
                    info.size = len(data)
                    bundle.addfile(info, io.BytesIO(data))
                store.write_new(attempt / 'attempt.json', {'attempt_id': name, 'disposition': disposition,
                    'replaces': 'att-002' if name == 'att-003' else None, 'usage': {'usd': 1.25},
                    'transcript_archive': {'path': str(archive), 'sha256': store.digest(archive), 'restoration_check': 'passed'}})
                store.write_new(attempt / 'normalized.json', {'findings': []})
            with patch.object(collect_run, 'ROOT', origin):
                collect_run.collect(run)
            transcript_entries = store.read(run / 'transcripts.json')
            native_root = base / 'native'
            native_root.mkdir()
            (native_root / 'review.json').write_text('{}')
            (native_root / 'run/scratch').mkdir(parents=True)
            probe = native_root / 'run/scratch/probe.sh'
            probe.write_text('#!/bin/sh\nexit 0\n')
            probe.chmod(0o755)
            index = base / native_artifacts.INDEX
            store.write_new(index, {'root': str(native_root), 'files': [
                {'path': path.relative_to(native_root).as_posix(), 'bytes': path.stat().st_size, 'sha256': store.digest(path)}
                for path in sorted(native_root.rglob('*')) if path.is_file()]})
            native_out = run / 'attempts/att-001'
            record_path = native_out / 'attempt.json'
            record = store.read(record_path)
            record_path.unlink()
            pin = native_artifacts.file_artifacts(native_root, index, native_out, 'review.json')
            record_path.write_text(json.dumps({**record, 'native_artifact_storage': pin}))
            grading = base / 'queue/run/target/attempt-1'
            work = grading / 'work'
            (work / 'home/.codex').mkdir(parents=True)
            (work / 'home/.codex/config.toml').write_text('model = "fixture"\n')
            store.write_new(work / 'dispatch.json', {'exit_code': 0, 'usage': {'high': 2.0}})
            store.write_new(work / 'verdicts.json', {'verdicts': []})
            store.write_new(grading / 'reservation.json', {'maxBudgetUsd': 6})
            with patch.object(regrade, 'ROOT', origin):
                grading_ref = regrade.archive_attempt(grading, origin / 'bench/regrading/fixture')
            grading_archive = store.read(origin / grading_ref['path'])['archive']['path']
            files = [path for path in origin.rglob('*') if path.is_file()]
            frozen = {path.relative_to(origin).as_posix(): path.read_bytes() for path in files}
            imported = {'files': [{'path': name, 'sha256': store.digest(origin / name)} for name in frozen
                                  if not name.startswith('artifacts/transcripts/')],
                        'transcripts': transcript_entries + [{'path': 'artifacts/transcripts/absent.tar.gz', 'status': 'missing'}]}
            store.write_new(origin / 'bench/import-manifest.json', imported)
            archives = [path for path in files if path.suffix in {'.gz', '.zip'}]
            selection = {'repository': 'owner/canonical', 'tag': 'evidence-fixture',
                         'subjects': ['att-001', 'att-002', 'att-003', 'grading'], 'files': [
                {'source': str(path), 'path': path.relative_to(origin).as_posix(), 'kind': 'evidence'} for path in archives]}
            store.pack(selection, base / 'packed')
            remote = FakeGitHub(remote_path)
            manifest_path = origin / 'bench/evidence/manifests/fixture.json'
            manifest = store.publish(base / 'packed/manifest.json', manifest_path, remote)
            for path in origin.rglob('*'):
                if path.is_file() and path not in archives:
                    target = cold / path.relative_to(origin)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(path, target)
            shutil.rmtree(origin)
            shutil.rmtree(grading.parents[2])
            shutil.rmtree(native_root)
            shutil.rmtree(base / 'packed')
            for path in base.glob('att-*.tar.gz'):
                path.unlink()
            with patch.object(store, 'GitHub', return_value=remote):
                with patch.object(collect_run, 'ROOT', cold):
                    collect_run.collect(cold / 'bench/runs/example')
                self.assertFalse((cold / grading_archive).exists())
                with patch.object(import_benchmark, 'ROOT', cold), patch.object(import_benchmark, 'MANIFEST', cold / 'bench/import-manifest.json'):
                    import_benchmark.verify()
                native_cold = cold / native_out.relative_to(origin)
                native_artifacts.verify(native_cold)
                native_artifacts.restore(native_cold, base / 'restored-native')
                self.assertEqual((base / 'restored-native/run/scratch/probe.sh').stat().st_mode & 0o777, 0o755)
                archive = transcript_entries[0]
                (cold / archive['path']).unlink()
                with patch.object(export_explorer, 'ROOT', cold):
                    url = export_explorer.evidence(cold / archive['path'], cold / 'public', archive['sha256'])
                self.assertIn('/evidence/artifacts/transcripts/', url)
                self.assertEqual(store.digest(cold / 'public/evidence' / archive['path']), archive['sha256'])
                store.materialize(manifest, cold, [grading_archive], remote=remote)
            with tarfile.open(cold / grading_archive) as bundle:
                self.assertEqual(bundle.extractfile('work/home/.codex/config.toml').read(), b'model = "fixture"\n')
                self.assertEqual(json.load(bundle.extractfile('work/dispatch.json'))['usage']['high'], 2.0)
            for name, data in frozen.items():
                self.assertEqual((cold / name).read_bytes(), data, name)
            self.assertEqual(store.read(cold / 'bench/import-manifest.json')['transcripts'][-1]['status'], 'missing')
            self.assertTrue(all(repository == 'owner/canonical' for repository, _ in remote.requests))

    def test_records_naming_their_origin_checkout_restore_in_a_checkout_at_another_path(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            origin, cold, remote_path = [base / name for name in ('origin', 'cold', 'remote')]
            remote_path.mkdir()
            run = origin / 'bench/runs/example'
            records = {'attempts/att-001': 'valid completed', 'attempts/att-002': 'failed: fixture',
                       'attempts/att-003': 'valid completed', 'probes/att-001': 'valid completed'}
            for name, disposition in records.items():
                layer, attempt_id = name.split('/')
                archive = origin / 'artifacts/transcripts' / ('example' if layer == 'attempts' else 'example-probe') / (attempt_id + '.tar.gz')
                archive.parent.mkdir(parents=True, exist_ok=True)
                archive.write_bytes(('transcript of ' + name).encode())
                store.write_new(run / name / 'attempt.json', {
                    'run_id': 'example', 'attempt_id': attempt_id, 'disposition': disposition, 'cell': {'target': 'task'},
                    'predecessor': 'att-002' if name == 'attempts/att-003' else None, 'usage': {'metering_status': 'complete'},
                    'observed': {'tree_identity_before': 'same', 'tree_identity_after': 'same'},
                    'transcript_archive': {'path': str(archive), 'sha256': store.digest(archive), 'restoration_check': 'passed'}})
                (run / name / 'normalized.json').write_text('{}')
                (run / name / 'usage-requests.jsonl').write_text('{}\n')
            with patch.object(collect_run, 'ROOT', origin):
                collect_run.collect(run)
            mapping = (run / 'transcripts.json').read_bytes()
            entries = store.read(run / 'transcripts.json')
            self.assertEqual([entry['path'] for entry in entries], [
                'artifacts/transcripts/example/att-001.tar.gz', 'artifacts/transcripts/example/att-002.tar.gz',
                'artifacts/transcripts/example/att-003.tar.gz', 'artifacts/transcripts/example-probe/att-001.tar.gz'])
            store.pack({'repository': 'owner/canonical', 'tag': 'evidence-fixture', 'subjects': sorted(records), 'files': [
                {'source': str(origin / entry['path']), 'path': entry['path'], 'kind': 'evidence'} for entry in entries]}, base / 'packed')
            remote = FakeGitHub(remote_path)
            store.publish(base / 'packed/manifest.json', origin / 'bench/evidence/manifests/fixture.json', remote)
            shutil.copytree(origin / 'bench', cold / 'bench')
            shutil.rmtree(origin)
            shutil.rmtree(base / 'packed')
            workspace = base / 'work/example/att-001'
            clone = workspace / 'clone'
            clone.mkdir(parents=True)
            for args in (['init', '-q'], ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                                         'commit', '--allow-empty', '-qm', 'fixture']):
                subprocess.run(['git', '-C', str(clone), *args], check=True)
            head = subprocess.check_output(['git', '-C', str(clone), 'rev-parse', 'HEAD'], text=True).strip()
            store.write_new(cold / 'bench/targets/task/target.json', {'head': head})
            valid, failed = [cold / 'bench/runs/example/attempts' / name for name in ('att-001', 'att-002')]

            def forget_local_copies():
                for name in ('artifacts', '.cache'):
                    shutil.rmtree(cold / name, ignore_errors=True)

            with patch.object(store, 'GitHub', return_value=remote), patch.object(collect_run, 'ROOT', cold):
                collect_run.collect(cold / 'bench/runs/example')
                self.assertEqual((cold / 'bench/runs/example/transcripts.json').read_bytes(), mapping)
                for entry in entries:
                    self.assertEqual(store.digest(cold / entry['path']), entry['sha256'])
                forget_local_copies()
                self.assertEqual(prune_workspace.prune(valid, workspace)['archive'], str(cold / entries[0]['path']))
                with self.assertRaisesRegex(prune_workspace.Refused, 'did not complete validly'):
                    prune_workspace.prune(failed, workspace.with_name('att-002'))
                with patch.object(export_explorer, 'ROOT', cold):
                    export_explorer.evidence(cold / entries[1]['path'], cold / 'public', entries[1]['sha256'])
                self.assertEqual(store.digest(cold / 'public/evidence' / entries[1]['path']), entries[1]['sha256'])

                forget_local_copies()
                asset = remote_path / remote.assets[0]['name']
                published = asset.read_bytes()
                asset.write_bytes(bytes(len(published)))
                with self.assertRaisesRegex(store.EvidenceError, 'missing or changed evidence'):
                    collect_run.collect(cold / 'bench/runs/example')
                with self.assertRaisesRegex(prune_workspace.Refused, 'missing or changed evidence'):
                    prune_workspace.prune(valid, workspace)
                with patch.object(export_explorer, 'ROOT', cold), self.assertRaisesRegex(store.EvidenceError, 'missing or changed evidence'):
                    export_explorer.evidence(cold / entries[1]['path'], cold / 'public-changed', entries[1]['sha256'])
                asset.write_bytes(published)
                remote.assets.clear()
                with self.assertRaisesRegex(store.EvidenceError, 'asset identity changed'):
                    collect_run.collect(cold / 'bench/runs/example')
                with self.assertRaisesRegex(prune_workspace.Refused, 'asset identity changed'):
                    prune_workspace.prune(valid, workspace)
            self.assertTrue(clone.is_dir())
            self.assertEqual((cold / 'bench/runs/example/transcripts.json').read_bytes(), mapping)
            self.assertFalse((cold / 'artifacts').exists())

    def test_archive_stored_under_a_path_no_record_names_is_mapped_by_its_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root, remote_path = base / 'checkout', base / 'remote'
            remote_path.mkdir()
            payloads = {'artifacts/transcripts/example-probes/att-009.tar.gz': b'stored once',
                        'artifacts/transcripts/other/first.tar.gz': b'stored twice',
                        'artifacts/transcripts/other/second.tar.gz': b'stored twice'}
            for index, data in enumerate(payloads.values()):
                (base / str(index)).write_bytes(data)
            store.pack({'repository': 'owner/canonical', 'tag': 'evidence-fixture', 'subjects': ['example'], 'files': [
                {'source': str(base / str(index)), 'path': name, 'kind': 'evidence'} for index, name in enumerate(payloads)]}, base / 'packed')
            remote = FakeGitHub(remote_path)
            store.publish(base / 'packed/manifest.json', root / 'bench/evidence/manifests/fixture.json', remote)
            run = root / 'bench/runs/example'

            def record(name, data, recorded='~/gone/cache/example-probe/att-001.tar.gz'):
                store.write_new(run / name / 'attempt.json', {'transcript_archive': {
                    'path': recorded, 'sha256': store.digest(base / str(list(payloads.values()).index(data)))}})

            record('probes/att-001', b'stored once')
            held = 'artifacts/transcripts/held/att-002.tar.gz'
            (root / held).parent.mkdir(parents=True)
            (root / held).write_bytes(b'stored once')
            record('probes/att-002', b'stored once', '/gone/checkout/' + held)
            with patch.object(store, 'GitHub', return_value=remote), patch.object(collect_run, 'ROOT', root):
                collect_run.collect(run)
                self.assertEqual([entry['path'] for entry in store.read(run / 'transcripts.json')],
                                 ['artifacts/transcripts/example-probes/att-009.tar.gz', held])
                self.assertEqual((root / 'artifacts/transcripts/example-probes/att-009.tar.gz').read_bytes(), b'stored once')
                record('attempts/att-001', b'stored twice')
                with self.assertRaisesRegex(ValueError, 'Transcript archive unavailable: artifacts/transcripts/example/att-001.tar.gz'):
                    collect_run.collect(run)


if __name__ == '__main__':
    unittest.main()
