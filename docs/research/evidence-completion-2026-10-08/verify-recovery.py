#!/usr/bin/env python3
"""Exercise the retained recovery release while refusing original-release access."""
from pathlib import Path
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store as store
import recover_evidence as recovery

assert not Path('/home/jack/.t3/bench-runs').exists()
assert not Path('/home/jack/.t3/worktrees').exists()
assert not (ROOT / '.cache/evidence').exists()
index = store.read(ROOT / recovery.INDEX)


class RecoveryOnly(store.GitHub):
    def release(self, repository, tag):
        if tag != index['tag']:
            raise store.EvidenceError('original release deliberately unavailable for this exercise')
        return super().release(repository, tag)


remote = RecoveryOnly()
rows = []
for source in index['sources']:
    original = store.read(ROOT / source['manifest'])
    members = [m for p in original['packages'] for m in p['members']]
    assert all(not (ROOT / m['path']).exists() for m in members)
    try:
        store.materialize(original, ROOT, [members[0]['path']], remote=remote)
    except store.EvidenceError as error:
        assert 'deliberately unavailable' in str(error)
    else:
        raise AssertionError('original release was unexpectedly available')
    manifest = recovery.manifest(ROOT, source['name'])
    store.materialize(manifest, ROOT, remote=remote)
    for member in members:
        store.checked_file(ROOT / member['path'], member)
    sample = members[0]
    (ROOT / sample['path']).unlink()
    store.materialize(manifest, ROOT, [sample['path']], offline=True, remote=remote)
    store.checked_file(ROOT / sample['path'], sample)
    rows.append({'source': source['name'], 'members': len(members),
                 'packages': [p['sha256'] for p in manifest['packages']],
                 'original_release_refused': True, 'offline_sample_restored': True})
    print(source['name'] + ': complete recovery verified', flush=True)
store.write_new(sys.argv[1], {'schema_version': 1, 'status': 'verified', 'sources': rows,
    'origin_roots_hidden': True, 'empty_evidence_cache_start': True,
    'index_sha256': store.digest(ROOT / recovery.INDEX),
    'limitation': 'A separate release in the same repository protects against individual asset/release loss, not loss of the GitHub repository, account or provider.'})
