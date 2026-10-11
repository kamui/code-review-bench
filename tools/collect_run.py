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


def verified_path(run, path, archive, stored, stored_paths):
    """Verify the record's archive, place it at its repository path and return that path."""
    logical = logical_path(run, path, archive['path'])
    same_bytes = stored.get(archive['sha256'], set())
    if not (ROOT / logical).is_file() and logical not in stored_paths and len(same_bytes) == 1:
        logical, = same_bytes
    destination = ROOT / logical
    source = destination if destination.is_file() else (
        evidence_store.resolve(ROOT, logical, archive['sha256']) if logical in stored_paths else ROOT / Path(archive['path']).expanduser())
    if not source.is_file():
        raise ValueError(f'Transcript archive unavailable: {logical} for {path}')
    if hashlib.sha256(source.read_bytes()).hexdigest() != archive['sha256']:
        raise ValueError(f'Transcript checksum mismatch: {path}')
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source.resolve() != destination.resolve():
        shutil.copyfile(source, destination)
    return logical


def collect(run, missing=()):
    stored = {}
    for _, manifest in evidence_store.manifests(ROOT):
        for package in manifest['packages']:
            for member in package['members']:
                stored.setdefault(member['sha256'], set()).add(member['path'])
    stored_paths = set().union(*stored.values())
    records = sorted(record for layer in ('attempts', 'probes') for record in (run / layer).glob('*/attempt.json'))
    mapping = run / 'transcripts.json'
    lost = {run / name / 'attempt.json' for name in missing}
    if mapping.is_file():
        lost |= {ROOT / entry['attempt'] for entry in json.loads(mapping.read_text()) if entry['status'] == 'missing'}
    if not lost <= set(records):
        raise ValueError(f'No such record to mark missing: {sorted(map(str, lost - set(records)))}')
    entries = []
    for path in records:
        archive = json.loads(path.read_text()).get('transcript_archive')
        if not archive:
            continue
        entry = {'attempt': path.relative_to(ROOT).as_posix(), 'sha256': archive['sha256']}
        try:
            entries.append({**entry, 'path': verified_path(run, path, archive, stored, stored_paths), 'status': 'verified'})
        except ValueError:
            if path not in lost:
                raise
            entries.append({**entry, 'path': logical_path(run, path, archive['path']), 'status': 'missing'})
    mapping.write_text(json.dumps([{key: entry[key] for key in ('attempt', 'path', 'sha256', 'status')} for entry in entries], indent=2) + '\n')
    verified = sum(entry['status'] == 'verified' for entry in entries)
    print(f'Collected {verified} verified transcripts for {run.name}' + (f'; {len(entries) - verified} recorded as missing' if verified < len(entries) else ''))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--missing', action='append', default=[], metavar='RECORD',
                        help='a record directory of the run, such as probes/att-002, whose archive no copy holds; '
                             'its entry keeps the frozen hash with status missing')
    args = parser.parse_args()
    collect(args.run.resolve(), args.missing)
