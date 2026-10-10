"""Collect portable, checksum-verified transcript references for a completed run."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store

ARCHIVES = PurePosixPath('artifacts/transcripts')


def logical_path(run, record, recorded):
    """The archive's repository path: the one its record holds or names inside any checkout, else the collector's own."""
    path = PurePosixPath(recorded)
    if not path.is_absolute() and path.parts[0] != '~':
        try:
            return evidence_store.logical_path(recorded).as_posix()
        except evidence_store.EvidenceError:
            pass
    parts = path.parts
    for start in range(len(parts) - len(ARCHIVES.parts), -1, -1):
        if parts[start:start + len(ARCHIVES.parts)] == ARCHIVES.parts:
            return evidence_store.logical_path(PurePosixPath(*parts[start:]).as_posix()).as_posix()
    layer = record.parent.parent.name
    return (ARCHIVES / run.name / ('' if layer == 'attempts' else layer) / f'{record.parent.name}.tar.gz').as_posix()


def collect(run):
    stored_paths = {m['path'] for _, manifest in evidence_store.manifests(ROOT)
                    for package in manifest['packages'] for m in package['members']}
    entries = []
    for path in sorted(record for layer in ('attempts', 'probes') for record in (run / layer).glob('*/attempt.json')):
        record = json.loads(path.read_text())
        archive = record.get('transcript_archive')
        if not archive:
            continue
        logical = logical_path(run, path, archive['path'])
        destination = ROOT / logical
        source = destination if destination.is_file() else (
            evidence_store.resolve(ROOT, logical, archive['sha256']) if logical in stored_paths else ROOT / Path(archive['path']).expanduser())
        if not source.is_file():
            raise ValueError(f'Transcript archive unavailable: {logical} for {path}')
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        if digest != archive['sha256']:
            raise ValueError(f'Transcript checksum mismatch: {path}')
        destination.parent.mkdir(parents=True, exist_ok=True)
        if source.resolve() != destination.resolve():
            shutil.copyfile(source, destination)
        entries.append({'attempt': path.relative_to(ROOT).as_posix(), 'path': logical,
                        'sha256': digest, 'status': 'verified'})
    (run / 'transcripts.json').write_text(json.dumps(entries, indent=2) + '\n')
    print(f'Collected {len(entries)} verified transcripts for {run.name}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, required=True)
    args = parser.parse_args()
    collect(args.run.resolve())
