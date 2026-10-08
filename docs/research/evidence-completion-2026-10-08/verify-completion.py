#!/usr/bin/env python3
"""Verify the shared completion packages from an empty checkout and evidence cache."""
import argparse
from collections import Counter
import gzip
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store as store
import reconcile_evidence

REPORT = ROOT / 'docs/research/evidence-completion-2026-10-08'


def compressed(path):
    with gzip.open(path, 'rt') as handle:
        return json.load(handle)


def verify():
    assert not Path('/home/jack/.t3/bench-cache').exists()
    assert not Path('/home/jack/.t3/bench-runs').exists()
    assert not Path('/home/jack/.t3/worktrees').exists()
    assert not (ROOT / '.cache/evidence').exists()
    manifests = []
    for name in ['dependencies', 'withheld', 'host-inventory']:
        path = ROOT / f'bench/evidence/manifests/{name}-2026-10-08-v1.json'
        manifest = store.read(path)
        members = [member for package in manifest['packages'] for member in package['members']]
        assert all(not (ROOT / member['path']).exists() for member in members)
        store.materialize(manifest, ROOT)
        for member in members:
            store.checked_file(ROOT / member['path'], member)
        manifests.append({'path': str(path.relative_to(ROOT)), 'sha256': store.digest(path),
                          'members': len(members), 'packages': [p['sha256'] for p in manifest['packages']]})
        print(name + ': remote members verified', flush=True)
    mapping = store.read(REPORT / 'withheld-summary.json')['mapping']
    assert store.digest(ROOT / mapping['path']) == mapping['sha256']
    resolutions = compressed(ROOT / mapping['path'])['entries']
    for row in resolutions:
        store.checked_file(ROOT / row['storage']['path'], row)
    inventory = store.read(REPORT / 'inventory-index.json')
    entries = 0
    for row in inventory['roots']:
        path = ROOT / row['path']
        assert store.digest(path) == row['sha256'] and path.stat().st_size == row['bytes']
        record = reconcile_evidence.validate(compressed(path))
        assert record['root'] == row['root'] and len(record['entries']) == row['entries']
        assert dict(Counter(item['status'] for item in record['entries'])) == row['statuses']
        assert dict(Counter(item['class'] for item in record['entries'])) == row['classes']
        snapshot = path.parent.parent / 'snapshots' / path.name
        assert store.digest(snapshot) == row['snapshot_sha256']
        entries += len(record['entries'])
    print(f"{len(inventory['roots'])} inventories and {entries} classified rows verified", flush=True)
    output = ROOT / 'restored-withheld-sample.tar.gz'
    subprocess.run([sys.executable, 'docs/research/evidence-migration-2026-10-08/restore-local.py',
                    'bench-cache', 'archives/i-requests-6667.tar.gz', str(output)], check=True)
    target = next(row for row in store.read(REPORT / 'dependency-catalog.json')['targets']
                  if row['target'] == 'i-requests-6667')
    store.checked_file(output, target)
    return {'schema_version': 1, 'status': 'verified', 'manifests': manifests,
            'origin_roots_hidden': True, 'empty_evidence_cache_start': True,
            'withheld_paths': len(resolutions), 'inventory_roots': len(inventory['roots']),
            'inventory_rows': entries, 'restore_local_sample': 'i-requests-6667',
            'retirement_authorized': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    store.write_new(args.out, verify())
