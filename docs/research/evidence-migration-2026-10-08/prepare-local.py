#!/usr/bin/env python3
"""Prepare an immutable local selection; never upload or delete source evidence.

Usage: prepare-local.py CANDIDATES_JSON INVENTORY_DIRECTORY NEW_OUTPUT_DIRECTORY
Run in the repository whose current tracked-file inventory is supplied.
"""
import collections
import gzip
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys
import tarfile
import zipfile

sys.path.insert(0, str(Path.cwd() / 'bench/tools'))
import evidence_store as store

candidates, inventories, output = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
output.mkdir()
staging = output / 'objects'
staging.mkdir()
known = set()

def strings(value):
    if isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)
    elif isinstance(value, str) and len(value) >= 24:
        yield value.encode()

public = {}
for path in sorted(inventories.glob('*.json.gz')):
    with gzip.open(path, 'rt') as handle:
        inventory = json.load(handle)
    root = Path(inventory['root'])
    for entry in inventory['entries']:
        if entry['class'] == 'account-state':
            try:
                known.update(strings(json.loads((root / entry['path']).read_text())))
            except (OSError, ValueError):
                pass
        if root == Path.cwd() and entry['class'] == 'tracked-identical':
            public.setdefault((entry['sha256'], entry['mode']), entry['path'])

patterns = [rb'sk-(?:proj-|ant-)[A-Za-z0-9_-]{25,}', rb'gh[pousr]_[A-Za-z0-9]{30,}',
            rb'github_pat_[A-Za-z0-9_]{30,}', rb'-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----',
            rb'eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}']
patterns.extend(re.escape(value) for value in known)
secret = re.compile(b'|'.join(patterns))
overlap = max([4096] + [len(value) for value in known])
sensitive_names = {'auth.json', '.credentials.json', '.netrc', '.npmrc', '.pypirc', 'credentials', 'id_rsa', 'id_ed25519'}

def scan_stream(handle):
    tail = b''
    while block := handle.read(1024 * 1024):
        data = tail + block
        if secret.search(data):
            return 'credential-signature-or-known-account-value'
        tail = data[-overlap:]
    return None

def scan(path, name):
    with path.open('rb') as handle:
        if reason := scan_stream(handle):
            return reason
    if name.endswith(('.tar.gz', '.tgz', '.tar')):
        try:
            with tarfile.open(path) as bundle:
                for member in bundle:
                    if Path(member.name).name in sensitive_names:
                        return 'credential-filename-in-archive'
                    if member.isfile():
                        with bundle.extractfile(member) as handle:
                            if reason := scan_stream(handle):
                                return reason + '-in-archive'
                        if member.name.endswith(('.tar.gz', '.tgz', '.zip', '.tar', '.gz')):
                            return 'nested-archive-needs-content-review'
        except (tarfile.TarError, OSError):
            return 'unreadable-archive'
    elif name.endswith('.zip'):
        try:
            with zipfile.ZipFile(path) as bundle:
                for member in bundle.infolist():
                    if Path(member.filename).name in sensitive_names:
                        return 'credential-filename-in-archive'
                    with bundle.open(member) as handle:
                        if reason := scan_stream(handle):
                            return reason + '-in-archive'
                    if member.filename.endswith(('.tar.gz', '.tgz', '.zip', '.tar', '.gz')):
                        return 'nested-archive-needs-content-review'
        except (zipfile.BadZipFile, OSError):
            return 'unreadable-archive'
    elif name.endswith('.gz'):
        try:
            with gzip.open(path, 'rb') as handle:
                if reason := scan_stream(handle):
                    return reason + '-in-gzip'
        except (OSError, EOFError):
            return 'unreadable-gzip'
    return None

selection = {'repository': 'kamui/code-review-bench', 'tag': 'evidence-local-bench-2026-10-08-v1',
             'subjects': [], 'files': []}
results, references, included, blocked = {}, {}, {}, []
rows = json.loads(candidates.read_text())
for index, row in enumerate(rows):
    key = (row['sha256'], row['mode'])
    root = row['root']
    references.setdefault(root, [])
    destination = public.get(key)
    if destination:
        references[root].append({**row, 'storage': {'git_commit': '92aa9400e0f5d8ca360320fa6207794e5130299a', 'path': destination}})
        continue
    identity = f'{key[0]}.{key[1]:03o}'
    if key not in results:
        source = Path(root) / row['path']
        path = staging / identity
        if any(p.is_symlink() for p in (source, *source.parents)):
            reason = 'source-link'
        else:
            try:
                shutil.copy2(source, path)
                if store.digest(path) != key[0] or path.stat().st_size != row['bytes'] or path.stat().st_mode & 0o777 != key[1]:
                    reason = 'changed-since-inventory'
                else:
                    reason = scan(path, row['path'])
            except OSError:
                reason = 'source-unavailable'
        results[key] = reason
        if reason:
            path.unlink(missing_ok=True)
        else:
            logical = f'artifacts/local-evidence/2026-10-08/objects/{identity}'
            included[key] = {'source': str(path), 'path': logical,
                             'kind': 'reproducibility' if row['inventory_class'] == 'reproducibility' else 'evidence'}
    if results[key]:
        blocked.append({**row, 'reason': results[key]})
        references[root].append({**row, 'storage': None, 'blocked': results[key]})
    else:
        references[root].append({**row, 'storage': {'path': included[key]['path']}})
    if index % 5000 == 0:
        print(f'checked {index}/{len(rows)} candidates; {len(included)} unique selected', flush=True)

selection['files'] = list(included.values())
summary = []
for root, entries in references.items():
    label = Path(root).name
    if label == 'finish-it-7':
        label = 'nested-finish-it-7'
    name = f'{label}.json.gz'
    mapping = output / name
    with gzip.GzipFile(filename=str(mapping), mode='wb', mtime=0) as handle:
        handle.write(json.dumps({'schema_version':1, 'root':root, 'entries':entries, 'retirement_authorized':False},separators=(',',':')).encode())
    logical = f'artifacts/local-evidence/2026-10-08/mappings/{name}'
    selection['files'].append({'source':str(mapping),'path':logical,'kind':'evidence'})
    selection['subjects'].append(root.replace('/home/jack/.t3/', 't3/'))
    counts = collections.Counter('blocked' if row.get('blocked') else 'already-in-git' if row['storage'].get('git_commit') else 'selected' for row in entries)
    summary.append({'root': root, 'mapping':logical, 'sha256':store.digest(mapping), 'counts':dict(counts)})
store.write_new(output/'selection.json',selection)
store.write_new(output/'blocked.json',blocked)
store.write_new(output/'summary.json',{'roots':summary,'unique_selected':len(included),
    'unique_bytes':sum(Path(item['source']).stat().st_size for item in included.values()),
    'blocked_paths':len(blocked),'blocked_reasons':dict(collections.Counter(row['reason'] for row in blocked)),
    'retirement_authorized':False})
print(json.dumps(store.read(output/'summary.json'),indent=2),flush=True)
store.pack(selection,output/'packed')
print('Local package prepared; no upload or source deletion performed.',flush=True)
