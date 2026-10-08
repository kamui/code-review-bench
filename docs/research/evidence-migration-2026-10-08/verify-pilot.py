#!/usr/bin/env python3
"""Run from a fresh checkout after removing the four pilot payload copies."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile

ROOT = Path.cwd()
sys.path[:0] = [str(ROOT / 'bench/tools'), str(ROOT / 'tools')]
import evidence_store as store
import collect_run
import import_benchmark
import export_explorer

manifest_path = ROOT / 'bench/evidence/manifests/local-pilot-2026-10-08-v1.json'
inputs = store.read(ROOT / 'docs/research/evidence-migration-2026-10-08/pilot-inputs.json')
manifest = store.read(manifest_path)
assert not Path('/home/jack/.t3/bench-runs').exists()
assert not Path('/home/jack/.t3/bench-cache').exists()
assert not Path('/home/jack/.t3/worktrees').exists()
assert not (ROOT / '.cache/evidence').exists()
for archive in inputs['archives']:
    assert not (ROOT / archive['path']).exists(), archive['path']
records = {row['attempt']: row['sha256'] for row in inputs['attempts']}
records[inputs['grading_receipt']] = inputs['grading_receipt_sha256']
for path, sha in records.items():
    assert store.digest(ROOT / path) == sha
first = inputs['archives'][0]['path']
store.materialize(manifest, ROOT, [first])
assert all(not (ROOT / row['path']).exists() for row in inputs['archives'][1:])
for run in sorted({str(Path(row['attempt']).parents[2]) for row in inputs['attempts']}):
    collect_run.collect(ROOT / run)
store.materialize(manifest, ROOT)
verified = 0
for archive in inputs['archives']:
    path = ROOT / archive['path']
    assert store.digest(path) == archive['sha256']
    with tarfile.open(path) as bundle:
        members = {m.name: m for m in bundle}
        assert set(members) == {row['path'] for row in archive['members']}
        for row in archive['members']:
            member = members[row['path']]
            assert member.isfile() and member.size == row['bytes'] and member.mode == row['mode']
            assert hashlib.sha256(bundle.extractfile(member).read()).hexdigest() == row['sha256']
            verified += 1
    if archive['path'].startswith('artifacts/transcripts/'):
        path.unlink()
        url = export_explorer.evidence(path, ROOT / 'public', archive['sha256'])
        assert store.digest(ROOT / 'public/evidence' / archive['path']) == archive['sha256']
        assert url.endswith(archive['path'])
import_benchmark.verify()
store.materialize(manifest, ROOT, offline=True)
for path, sha in records.items():
    assert store.digest(ROOT / path) == sha
store.restoration_receipt(manifest_path, ROOT / 'bench/evidence/receipts/local-pilot-2026-10-08-v1-restoration.json')
store.write_new(ROOT / 'bench/evidence/receipts/local-pilot-2026-10-08-v1-consumers.json', {
    'schema_version': 1,
    'checkout_commit': subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
    'manifest_sha256': store.digest(manifest_path),
    'origin_roots_hidden': True,
    'empty_cache_start': True,
    'selective_fetch': 'passed',
    'collector': 'passed',
    'legacy_import': 'passed; historical limitations retained',
    'explorer_downloads': 'passed',
    'offline_fetch': 'passed',
    'archive_members_verified': verified,
    'frozen_records_unchanged': records,
    'retirement_authorized': False,
})
print(f'PASS: {len(inputs["archives"])} real archives, {verified} members, frozen records unchanged; origins hidden')
