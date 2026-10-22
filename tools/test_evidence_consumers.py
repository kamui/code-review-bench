"""Cold retrieval preserves frozen records through the existing evidence consumers."""
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'bench/tools'))
import rust_runtime


# The release routes of the corpora's gh stub, answering from release.json and the assets stored beside it, and
# appending each argument list to calls.jsonl.
GH_STUB = r"""import json, os, sys
from pathlib import Path
from urllib.parse import quote
state = Path(os.environ["CONTRACT_GH_STATE"])
args = sys.argv[1:]
state.mkdir(parents=True, exist_ok=True)
with (state / "calls.jsonl").open("a", encoding="utf-8") as log:
    log.write(json.dumps(args) + "\n")
def fail(message, code=1):
    sys.stderr.write(message + "\n")
    sys.exit(code)
try:
    release = json.loads((state / "release.json").read_text(encoding="utf-8"))
except FileNotFoundError:
    fail("HTTP 404: Not Found")
routes = f"repos/{release['repository']}/releases/"
stored = state / "assets"
def assets():
    found = []
    if stored.is_dir():
        for directory in sorted(stored.iterdir(), key=lambda path: int(path.name)):
            for file in sorted(directory.iterdir()):
                found.append({"id": int(directory.name), "name": file.name, "size": file.stat().st_size})
    return found
if args == ["api", routes + "tags/" + quote(release["tag"], safe="")]:
    print(json.dumps({"id": release["id"], "tag_name": release["tag"], "draft": release.get("draft", False)}))
elif args == ["api", "--paginate", "--slurp", routes + f"{release['id']}/assets?per_page=100"]:
    listed = assets()
    print(json.dumps([listed[at:at + 100] for at in range(0, len(listed), 100)] or [[]]))
elif args[:2] == ["release", "upload"] and len(args) == 6 and args[2] == release["tag"] and args[4:] == ["--repo", release["repository"]]:
    if any(asset["name"] == Path(args[3]).name for asset in assets()):
        fail("asset under the same name already exists")
    target = stored / str(max([100] + [asset["id"] for asset in assets()]) + 1) / Path(args[3]).name
    target.parent.mkdir(parents=True)
    target.write_bytes(Path(args[3]).read_bytes())
elif len(args) == 4 and args[0] == "api" and args[1].startswith(routes + "assets/") and args[2:] == ["-H", "Accept: application/octet-stream"]:
    matches = [stored / str(asset["id"]) / asset["name"] for asset in assets() if routes + f"assets/{asset['id']}" == args[1]]
    if not matches:
        fail("HTTP 404: Not Found")
    sys.stdout.buffer.write(matches[0].read_bytes())
else:
    fail("contract gh stub: unsupported arguments", 2)
"""


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def write(path, value):
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


class Release:
    """The corpora's stub gh release, kept in one state directory across bench-tools calls."""

    def __init__(self, base):
        self.state = base / 'gh-state'
        write(self.state / 'release.json', {'repository': 'owner/canonical', 'tag': 'evidence-fixture', 'id': 42})

    def assets(self):
        return sorted((self.state / 'assets').glob('*/*'), key=lambda path: int(path.parent.name))

    def calls(self):
        return [json.loads(line) for line in (self.state / 'calls.jsonl').read_text().splitlines()]


def rust_evidence(release, *arguments, cwd=None):
    return rust_bench_tools(release, cwd, 'evidence', *arguments)


def rust_bench_tools(release, cwd, *arguments):
    """Run `bench-tools` in `cwd` with the corpora's stub gh answering from the release, or from no release."""
    with tempfile.TemporaryDirectory() as tools:
        tools = Path(tools)
        (tools / 'gh.py').write_text(GH_STUB)
        (tools / 'gh').write_text(f'#!/bin/sh\nexec {sys.executable} {tools / "gh.py"} "$@"\n')
        (tools / 'gh').chmod(0o755)
        state = release.state if release is not None else tools / 'state'
        environment = {**os.environ, 'PATH': f'{tools}{os.pathsep}{os.environ["PATH"]}', 'CONTRACT_GH_STATE': str(state)}
        return subprocess.run([str(rust_runtime.resolve('bench-tools')), *map(str, arguments)],
                              capture_output=True, text=True, env=environment, cwd=cwd)


def publish(test, release, selection, base, manifest):
    """Pack the selection and publish it to the release, writing its manifest."""
    write(base / 'selection.json', selection)
    for arguments in (['pack', '--selection', base / 'selection.json', '--out', base / 'packed'],
                      ['publish', '--manifest', base / 'packed/manifest.json', '--out', manifest]):
        done = rust_evidence(release, *arguments, cwd=base)
        test.assertEqual(done.returncode, 0, done.stdout + done.stderr)
    return read(manifest)


def collect(release, root, run):
    """`bench-tools evidence collect` for a run of the repository at root."""
    return rust_evidence(release, 'collect', '--run', str(run), cwd=root)


def native_tools(*arguments):
    return subprocess.run([str(rust_runtime.resolve('bench-tools')), 'native-artifacts', *map(str, arguments)],
                          capture_output=True, text=True)


class ColdConsumersTest(unittest.TestCase):
    def test_success_failed_replacement_and_grading_survive_origin_removal(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            origin, cold = [base / name for name in ('origin', 'cold')]
            origin.mkdir()
            cold.mkdir()
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
                write(attempt / 'attempt.json', {'attempt_id': name, 'disposition': disposition,
                    'replaces': 'att-002' if name == 'att-003' else None, 'usage': {'usd': 1.25},
                    'transcript_archive': {'path': str(archive), 'sha256': digest(archive), 'restoration_check': 'passed'}})
                write(attempt / 'normalized.json', {'findings': []})
            done = collect(None, origin, run)
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            transcript_entries = read(run / 'transcripts.json')
            native_root = base / 'native'
            native_root.mkdir()
            (native_root / 'review.json').write_text('{}')
            (native_root / 'run/scratch').mkdir(parents=True)
            probe = native_root / 'run/scratch/probe.sh'
            probe.write_text('#!/bin/sh\nexit 0\n')
            probe.chmod(0o755)
            index = base / 'native-artifacts.json'
            write(index, {'root': str(native_root), 'files': [
                {'path': path.relative_to(native_root).as_posix(), 'bytes': path.stat().st_size, 'sha256': digest(path)}
                for path in sorted(native_root.rglob('*')) if path.is_file()]})
            native_out = run / 'attempts/att-001'
            record_path = native_out / 'attempt.json'
            record = read(record_path)
            record_path.unlink()
            filed = native_tools('file', native_root, index, native_out, '--native-relative', 'review.json')
            self.assertEqual(filed.returncode, 0, filed.stdout + filed.stderr)
            pin = json.loads(filed.stdout)['pin']
            record_path.write_text(json.dumps({**record, 'native_artifact_storage': pin}))
            grading = base / 'queue/run/target/attempt-1'
            work = grading / 'work'
            (work / 'home/.codex').mkdir(parents=True)
            (work / 'home/.codex/config.toml').write_text('model = "fixture"\n')
            write(work / 'dispatch.json', {'exit_code': 0, 'usage': {'high': 2.0}})
            write(work / 'verdicts.json', {'verdicts': []})
            write(grading / 'reservation.json', {'maxBudgetUsd': 6})
            archived = subprocess.run([str(rust_runtime.resolve('bench-tools')), 'regrade', 'archive-attempt', str(grading),
                                       str(origin / 'bench/regrading/fixture'), '--root', str(origin)],
                                      cwd=origin, capture_output=True, text=True, stdin=subprocess.DEVNULL)
            self.assertEqual(archived.returncode, 0, archived.stdout + archived.stderr)
            grading_ref = json.loads(archived.stdout)
            grading_archive = read(origin / grading_ref['path'])['archive']['path']
            files = [path for path in origin.rglob('*') if path.is_file()]
            frozen = {path.relative_to(origin).as_posix(): path.read_bytes() for path in files}
            imported = {'files': [{'path': name, 'sha256': digest(origin / name)} for name in frozen
                                  if not name.startswith('artifacts/transcripts/')],
                        'transcripts': transcript_entries + [{'path': 'artifacts/transcripts/absent.tar.gz', 'status': 'missing'}]}
            write(origin / 'bench/import-manifest.json', imported)
            archives = [path for path in files if path.suffix in {'.gz', '.zip'}]
            selection = {'repository': 'owner/canonical', 'tag': 'evidence-fixture',
                         'subjects': ['att-001', 'att-002', 'att-003', 'grading'], 'files': [
                {'source': str(path), 'path': path.relative_to(origin).as_posix(), 'kind': 'evidence'} for path in archives]}
            remote = Release(base)
            manifest_path = origin / 'bench/evidence/manifests/fixture.json'
            publish(self, remote, selection, base, manifest_path)
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
            done = collect(remote, cold, cold / 'bench/runs/example')
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertFalse((cold / grading_archive).exists())
            checked = rust_bench_tools(remote, cold, 'import-benchmark', '--check')
            self.assertEqual(checked.returncode, 0, checked.stdout + checked.stderr)
            native_cold = cold / native_out.relative_to(origin)
            for command in (['verify', native_cold], ['restore', native_cold, '--out', base / 'restored-native']):
                done = native_tools(*command)
                self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual((base / 'restored-native/run/scratch/probe.sh').stat().st_mode & 0o777, 0o755)
            self.assertEqual((base / 'restored-native/review.json').read_text(), '{}')
            archive = transcript_entries[0]
            (cold / archive['path']).unlink()
            done = rust_evidence(remote, 'resolve', archive['path'], '--sha256', archive['sha256'], '--root', str(cold))
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual(digest(cold / archive['path']), archive['sha256'])
            done = rust_evidence(remote, 'fetch', '--manifest', str(cold / manifest_path.relative_to(origin)),
                                 '--root', str(cold), '--path', grading_archive)
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            with tarfile.open(cold / grading_archive) as bundle:
                self.assertEqual(bundle.extractfile('work/home/.codex/config.toml').read(), b'model = "fixture"\n')
                self.assertEqual(json.load(bundle.extractfile('work/dispatch.json'))['usage']['high'], 2.0)
            for name, data in frozen.items():
                self.assertEqual((cold / name).read_bytes(), data, name)
            self.assertEqual(read(cold / 'bench/import-manifest.json')['transcripts'][-1]['status'], 'missing')
            self.assertTrue(all('owner/canonical' in ' '.join(call) for call in remote.calls()))

    def test_records_naming_their_origin_checkout_restore_in_a_checkout_at_another_path(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            origin, cold = [base / name for name in ('origin', 'cold')]
            run = origin / 'bench/runs/example'
            records = {'attempts/att-001': 'valid completed', 'attempts/att-002': 'failed: fixture',
                       'attempts/att-003': 'valid completed', 'probes/att-001': 'valid completed'}
            for name, disposition in records.items():
                layer, attempt_id = name.split('/')
                archive = origin / 'artifacts/transcripts' / ('example' if layer == 'attempts' else 'example-probe') / (attempt_id + '.tar.gz')
                archive.parent.mkdir(parents=True, exist_ok=True)
                archive.write_bytes(('transcript of ' + name).encode())
                write(run / name / 'attempt.json', {
                    'run_id': 'example', 'attempt_id': attempt_id, 'disposition': disposition, 'cell': {'target': 'task'},
                    'predecessor': 'att-002' if name == 'attempts/att-003' else None, 'usage': {'metering_status': 'complete'},
                    'observed': {'tree_identity_before': 'same', 'tree_identity_after': 'same'},
                    'transcript_archive': {'path': str(archive), 'sha256': digest(archive), 'restoration_check': 'passed'}})
                (run / name / 'normalized.json').write_text('{}')
                (run / name / 'usage-requests.jsonl').write_text('{}\n')
            done = collect(None, origin, run)
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            mapping = (run / 'transcripts.json').read_bytes()
            entries = read(run / 'transcripts.json')
            self.assertEqual([entry['path'] for entry in entries], [
                'artifacts/transcripts/example/att-001.tar.gz', 'artifacts/transcripts/example/att-002.tar.gz',
                'artifacts/transcripts/example/att-003.tar.gz', 'artifacts/transcripts/example-probe/att-001.tar.gz'])
            remote = Release(base)
            publish(self, remote, {'repository': 'owner/canonical', 'tag': 'evidence-fixture', 'subjects': sorted(records), 'files': [
                {'source': str(origin / entry['path']), 'path': entry['path'], 'kind': 'evidence'} for entry in entries]},
                    base, origin / 'bench/evidence/manifests/fixture.json')
            shutil.copytree(origin / 'bench', cold / 'bench')
            shutil.rmtree(origin)
            shutil.rmtree(base / 'packed')

            def forget_local_copies():
                for name in ('artifacts', '.cache'):
                    shutil.rmtree(cold / name, ignore_errors=True)

            done = collect(remote, cold, cold / 'bench/runs/example')
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual((cold / 'bench/runs/example/transcripts.json').read_bytes(), mapping)
            for entry in entries:
                self.assertEqual(digest(cold / entry['path']), entry['sha256'])
            forget_local_copies()
            done = rust_evidence(remote, 'resolve', entries[1]['path'], '--sha256', entries[1]['sha256'], '--root', str(cold))
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual(digest(cold / entries[1]['path']), entries[1]['sha256'])

            forget_local_copies()
            asset = remote.assets()[0]
            published = asset.read_bytes()
            asset.write_bytes(bytes(len(published)))
            done = collect(remote, cold, cold / 'bench/runs/example')
            self.assertEqual(done.returncode, 1)
            self.assertRegex(done.stderr, '^collect_run.py: missing or changed evidence: ')
            done = rust_evidence(remote, 'resolve', entries[1]['path'], '--sha256', entries[1]['sha256'], '--root', str(cold))
            self.assertEqual((done.returncode, done.stderr), (1, ''))
            self.assertRegex(done.stdout, '^evidence: missing or changed evidence: ')
            asset.write_bytes(published)
            shutil.rmtree(remote.state / 'assets')
            done = collect(remote, cold, cold / 'bench/runs/example')
            self.assertEqual((done.returncode, done.stderr),
                             (1, 'collect_run.py: published release or asset identity changed\n'))
            done = rust_evidence(remote, 'resolve', entries[1]['path'], '--root', str(cold))
            self.assertEqual(done.stdout, 'evidence: published release or asset identity changed\n')
            self.assertEqual((cold / 'bench/runs/example/transcripts.json').read_bytes(), mapping)
            self.assertFalse((cold / 'artifacts').exists())

    def test_archive_stored_under_a_path_no_record_names_is_mapped_by_its_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / 'checkout'
            payloads = {'artifacts/transcripts/example-probes/att-009.tar.gz': b'stored once',
                        'artifacts/transcripts/other/first.tar.gz': b'stored twice',
                        'artifacts/transcripts/other/second.tar.gz': b'stored twice'}
            for index, data in enumerate(payloads.values()):
                (base / str(index)).write_bytes(data)
            remote = Release(base)
            publish(self, remote, {'repository': 'owner/canonical', 'tag': 'evidence-fixture', 'subjects': ['example'], 'files': [
                {'source': str(base / str(index)), 'path': name, 'kind': 'evidence'} for index, name in enumerate(payloads)]},
                    base, root / 'bench/evidence/manifests/fixture.json')
            run = root / 'bench/runs/example'

            def record(name, data, recorded='~/gone/cache/example-probe/att-001.tar.gz'):
                write(run / name / 'attempt.json', {'transcript_archive': {
                    'path': recorded, 'sha256': digest(base / str(list(payloads.values()).index(data)))}})

            record('probes/att-001', b'stored once')
            held = 'artifacts/transcripts/held/att-002.tar.gz'
            (root / held).parent.mkdir(parents=True)
            (root / held).write_bytes(b'stored once')
            record('probes/att-002', b'stored once', '/gone/checkout/' + held)
            done = collect(remote, root, run)
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual([entry['path'] for entry in read(run / 'transcripts.json')],
                             ['artifacts/transcripts/example-probes/att-009.tar.gz', held])
            self.assertEqual((root / 'artifacts/transcripts/example-probes/att-009.tar.gz').read_bytes(), b'stored once')
            record('attempts/att-001', b'stored twice')
            done = collect(remote, root, run)
            self.assertEqual(done.returncode, 1)
            self.assertIn('collect_run.py: Transcript archive unavailable: artifacts/transcripts/example/att-001.tar.gz',
                          done.stderr)


if __name__ == '__main__':
    unittest.main()
