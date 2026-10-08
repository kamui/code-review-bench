#!/usr/bin/env python3
"""Snapshot selected public Git payloads for relocation; never remove source files.

Run from the pinned checkout: prepare.py PUBLIC_TREE_JSON NEW_OUTPUT_DIRECTORY
The public tree is GitHub's recursive tree response for the checkout's commit.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store as store

public = store.read(sys.argv[1])
assert not public['truncated']
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
assert public['sha'] in {head, subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], text=True).strip()}
published = {row['path']: row for row in public['tree'] if row['type'] == 'blob'}
output = Path(sys.argv[2]).absolute()
output.mkdir()
files = output / 'files'
selection = {'repository': 'kamui/code-review-bench', 'tag': 'evidence-repository-payloads-2026-10-08-v1',
             'subjects': ['tracked-repository-payloads/' + head], 'files': []}
rows = []
for line in subprocess.check_output(['git', 'ls-tree', '-r', '-l', 'HEAD'], text=True).splitlines():
    info, name = line.split('\t', 1)
    mode, kind, blob, size = info.split()
    if kind != 'blob':
        continue
    path = Path(name)
    category = ('archive' if path.suffix in {'.gz', '.zip', '.xz', '.zst', '.tgz'} else
                'run-stdout' if name.startswith('bench/runs/') and path.name == 'stdout.jsonl' else
                'upstream-snapshot' if name.startswith('docs/research/') and '/upstream/' in name and path.suffix == '.json' else None)
    if category is None:
        continue
    assert not (ROOT / name).is_symlink()
    data = (ROOT / name).read_bytes()
    actual = hashlib.sha1(f'blob {len(data)}\0'.encode() + data).hexdigest()
    assert actual == blob == published[name]['sha'] and len(data) == int(size) == published[name]['size']
    target = files / name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / name, target)
    assert store.digest(target) == hashlib.sha256(data).hexdigest()
    selection['files'].append({'source': str(target), 'path': name, 'kind': 'evidence'})
    rows.append({'path': name, 'bytes': len(data), 'sha256': store.digest(target),
                 'mode': int(mode, 8) & 0o777, 'public_git_blob': blob, 'category': category})
store.write_new(output / 'selection.json', selection)
store.write_new(output / 'selected.json', {'source_commit': head, 'public_tree': public['sha'], 'files': rows})
store.pack(selection, output / 'packed', part_limit=64 * 1024 * 1024)
print(json.dumps({'files': len(rows), 'original_bytes': sum(row['bytes'] for row in rows),
                  'packed_bytes': sum(p['bytes'] for p in store.read(output / 'packed/manifest.json')['packages'])}))
