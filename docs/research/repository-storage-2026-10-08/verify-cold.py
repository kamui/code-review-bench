#!/usr/bin/env python3
"""Verify relocation in a disposable checkout with originating storage hidden."""
import gzip
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store as store

assert not Path('/home/jack/.t3/bench-runs').exists()
assert not Path('/home/jack/.t3/worktrees').exists()
assert not (ROOT / '.cache/evidence').exists()
manifest_path = ROOT / 'bench/evidence/manifests/repository-payloads-2026-10-08-v1.json'
manifest = store.read(manifest_path)
members = [m for p in manifest['packages'] for m in p['members']]
assert all(not (ROOT / m['path']).exists() for m in members)
store.materialize(manifest, ROOT)
for member in members:
    store.checked_file(ROOT / member['path'], member)
store.materialize(manifest, ROOT, offline=True)
fallback = store.read(ROOT / 'bench/evidence/manifests/local-git-fallbacks-2026-10-08-v1.json')
store.materialize(fallback, ROOT)
summary = store.read(ROOT / 'docs/research/evidence-migration-2026-10-08/capture-summary.json')
count = 0
sample = None
for root in summary['roots']:
    mapping = store.resolve(ROOT, root['mapping'], root['sha256'])
    with gzip.open(mapping, 'rt') as handle:
        rows = json.load(handle)['entries']
    for row in rows:
        if (row.get('storage') or {}).get('git_commit'):
            data = ROOT / 'artifacts/local-capture/git-objects' / row['sha256']
            assert data.stat().st_size == row['bytes'] and store.digest(data) == row['sha256']
            count += 1
            if sample is None and row['bytes']:
                sample = (root, row)
assert count == 42255
root, row = sample
original = ROOT / row['storage']['path']
saved = original.read_bytes() if original.exists() else None
if saved is not None:
    original.unlink()
try:
    output = ROOT / '.cache/restored-git-fallback'
    subprocess.run([sys.executable, 'docs/research/evidence-migration-2026-10-08/restore-local.py',
                    Path(root['root']).name, row['path'], str(output)], check=True)
    assert store.digest(output) == row['sha256'] and output.stat().st_mode & 0o777 == row['mode']
finally:
    if saved is not None:
        original.write_bytes(saved)
checks = [
    [sys.executable, 'tools/import_benchmark.py', '--check'],
    [sys.executable, 'bench/tools/current_grading.py', 'check'],
    [sys.executable, 'bench/tools/calibration.py', 'check'],
    [sys.executable, 'bench/tools/claims.py', 'check'],
    [sys.executable, 'tools/export_explorer.py'],
]
for command in checks:
    subprocess.run(command, check=True)
store.write_new(sys.argv[1], {
    'status': 'verified', 'checkout_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'origin_roots_hidden': True, 'empty_cache_start': True, 'repository_files': len(members),
    'manifest_sha256': store.digest(manifest_path), 'git_backed_paths': count,
    'git_fallback_unique_objects': sum(len(p['members']) for p in fallback['packages']),
    'fallback_restoration_permissions': 'passed', 'offline_fetch': 'passed', 'consumer_checks': checks,
})
