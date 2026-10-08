#!/usr/bin/env python3
"""Verify the published capture in a fresh checkout with original .t3 paths hidden."""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store as store

assert not Path('/home/jack/.t3/bench-runs').exists()
assert not Path('/home/jack/.t3/bench-cache').exists()
assert not Path('/home/jack/.t3/worktrees').exists()
manifest_path = ROOT / 'bench/evidence/manifests/local-capture-2026-10-08-v1.json'
manifest = store.read(manifest_path)
summary = store.read(ROOT / 'docs/research/evidence-migration-2026-10-08/capture-summary.json')
assert not (ROOT / '.cache/evidence').exists()
assert all(not (ROOT / m['path']).exists() for p in manifest['packages'] for m in p['members'])
store.materialize(manifest, ROOT)
members = {m['path']: m for p in manifest['packages'] for m in p['members']}
counts = {'captured_paths': 0, 'git_paths': 0, 'blocked_paths': 0}
public = {}
samples = {}
for root in summary['roots']:
    mapping = store.resolve(ROOT, root['mapping'], root['sha256'])
    with gzip.open(mapping, 'rt') as handle:
        entries = json.load(handle)['entries']
    for row in entries:
        if row.get('blocked'):
            assert row['storage'] is None
            counts['blocked_paths'] += 1
            samples.setdefault('blocked', (root, row))
            continue
        ref = row['storage']
        if ref.get('git_commit'):
            key = (ref['path'], row['sha256'])
            if key not in public:
                source = ROOT / ref['path']
                data = source.read_bytes()
                assert hashlib.sha256(data).hexdigest() == row['sha256'] and len(data) == row['bytes']
                public[key] = True
            counts['git_paths'] += 1
            samples.setdefault('git', (root, row))
        else:
            member = members[ref['path']]
            assert all(member[key] == row[key] for key in ('sha256', 'bytes', 'mode'))
            counts['captured_paths'] += 1
            if row['path'].endswith('.jsonl') and row['bytes'] > 0:
                samples.setdefault('transcript', (root, row))
            if row['inventory_class'] == 'reproducibility':
                samples.setdefault('reproducibility', (root, row))
            if row['mode'] & 0o111 and row['bytes'] > 0:
                samples.setdefault('executable', (root, row))
for category, (root, row) in samples.items():
    output = ROOT / 'restored-local-samples' / category
    command = [sys.executable, 'docs/research/evidence-migration-2026-10-08/restore-local.py',
               Path(root['root']).name, row['path'], str(output)]
    result = subprocess.run(command, capture_output=True, text=True)
    if category == 'blocked':
        assert result.returncode != 0 and not output.exists()
    else:
        assert result.returncode == 0, result.stderr
        assert store.digest(output) == row['sha256'] and output.stat().st_mode & 0o777 == row['mode']
        retry = subprocess.run(command, capture_output=True, text=True)
        assert retry.returncode != 0 and store.digest(output) == row['sha256']
assert counts['blocked_paths'] == summary['blocked_paths']
store.materialize(manifest, ROOT, offline=True)
store.write_new(ROOT / 'bench/evidence/receipts/local-capture-2026-10-08-v1-consumers.json', {
    'schema_version': 1,
    'checkout_commit': subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
    'manifest_sha256': store.digest(manifest_path),
    'origin_roots_hidden': True,
    'empty_cache_start': True,
    'unique_payloads_verified': len(members),
    'mappings_verified': len(summary['roots']),
    'counts': counts,
    'restore_samples': sorted(samples),
    'changed_destination_refusal': 'passed',
    'blocked_path_refusal': 'passed',
    'offline_fetch': 'passed',
    'retirement_authorized': False,
})
print(json.dumps({'status':'passed', **counts, 'samples':sorted(samples)}))
