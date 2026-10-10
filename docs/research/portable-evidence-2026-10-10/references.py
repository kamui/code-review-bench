#!/usr/bin/env python3
"""Inventory every transcript archive reference under bench/runs and how another checkout resolves it.

Usage: references.py [--check]. Reads attempt records, mappings and storage manifests; no network.
Exit 0: the record was written, or --check reproduced it; 1: the saved record differs.
"""
import argparse
from collections import Counter
import json
from pathlib import Path, PurePosixPath
import sys

ROOT = Path(__file__).resolve().parents[3]
RECORD = Path(__file__).with_name('archive-references.v1.json')
sys.path.insert(0, str(ROOT / 'tools'))
import collect_run


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def inventory():
    stored = {}
    for path in sorted((ROOT / 'bench/evidence/manifests').glob('*.json')):
        for package in read(path)['packages']:
            for member in package['members']:
                stored.setdefault(member['sha256'], set()).add(member['path'])
    mappings = {entry['attempt']: ('bench/import-manifest.json', entry)
                for entry in read(ROOT / 'bench/import-manifest.json')['transcripts']}
    for path in sorted((ROOT / 'bench/runs').glob('*/transcripts.json')):
        for entry in read(path):
            mappings[entry['attempt']] = (path.relative_to(ROOT).as_posix(), entry)
    states, runs, unresolved = Counter(), {}, []
    for path in sorted((ROOT / 'bench/runs').glob('**/attempt.json')):
        archive = read(path).get('transcript_archive')
        if not archive:
            continue
        record = path.relative_to(ROOT).as_posix()
        recorded = PurePosixPath(archive['path'])
        portable = not recorded.is_absolute() and recorded.parts[0] != '~'
        source, entry = mappings.get(record, (None, None))
        if entry and entry['status'] != 'verified':
            state = 'mapped with a recorded hash mismatch'
        elif entry:
            state = 'mapped' if entry['path'] in stored.get(archive['sha256'], ()) else 'mapped to a path outside release storage'
        elif portable:
            state = 'portable path' if archive['path'] in stored.get(archive['sha256'], ()) else 'portable path outside release storage'
        else:
            state = 'unresolved'
            names = collect_run.logical_path(path.parents[2], path, archive['path'])
            unresolved.append({'record': record, 'recorded': archive['path'], 'sha256': archive['sha256'],
                               'run_mapping': (path.parents[2] / 'transcripts.json').is_file(),
                               'stored_at': sorted(stored.get(archive['sha256'], ())),
                               'collector_path': names})
        states[state] += 1
        if source and source != 'bench/import-manifest.json':
            runs.setdefault(source, Counter())['portable' if portable else 'checkout or home path'] += 1
    return {'schema_version': 1, 'records': sum(states.values()), 'states': dict(sorted(states.items())),
            'run_mappings': {name: dict(sorted(counts.items())) for name, counts in sorted(runs.items())},
            'unresolved': unresolved}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    encoded = json.dumps(inventory(), indent=2) + '\n'
    if args.check:
        if RECORD.read_text(encoding='utf-8') != encoded:
            raise SystemExit('archive references differ from the saved record')
        print('archive references: reproduced')
    else:
        RECORD.write_text(encoded, encoding="utf-8")
        print(f'wrote {RECORD.relative_to(ROOT)}')
