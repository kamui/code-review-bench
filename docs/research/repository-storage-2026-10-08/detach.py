#!/usr/bin/env python3
"""Verify published payloads against Git, then untrack them without removing local bytes.

Run once from the migration base after remote verification has written the receipt.
"""
import hashlib
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store as store

BASE = '4c72200c5fd3b59867d41491c65c2ba30c517ebb'
MANIFEST = ROOT / 'bench/evidence/manifests/repository-payloads-2026-10-08-v1.json'
RECEIPT = ROOT / 'bench/evidence/receipts/repository-payloads-2026-10-08-v1-restoration.json'
manifest = store.validate(store.read(MANIFEST), published=True)
receipt = store.read(RECEIPT)
assert receipt['status'] == 'verified' and receipt['manifest_sha256'] == store.digest(MANIFEST)
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == BASE
indexed = {}
for row in subprocess.check_output(['git', 'ls-files', '--stage', '-z']).split(b'\0'):
    if row:
        info, name = row.split(b'\t', 1)
        mode, blob, stage = info.decode().split()
        assert stage == '0'
        indexed[name.decode()] = (mode, blob)
names = []
for package in manifest['packages']:
    for member in package['members']:
        name = member['path']
        path = store.confined(ROOT, name)
        store.checked_file(path, member)
        data = path.read_bytes()
        blob = hashlib.sha1(f'blob {len(data)}\0'.encode() + data).hexdigest()
        assert indexed[name] == (f'100{member["mode"]:03o}', blob)
        original = subprocess.check_output(['git', 'rev-parse', f'{BASE}:{name}'], text=True).strip()
        assert original == blob
        names.append(name)
assert len(names) == len(set(names)) == 3508
ignore = ROOT / '.gitignore'
lines = ['\n# Verified release payloads; restore with bun run evidence:fetch.\n']
for name in sorted(names):
    assert '\n' not in name and not name.endswith(' ')
    lines.append('/' + ''.join('\\' + c if c in '\\*?[]' else c for c in name) + '\n')
with ignore.open('a') as target:
    target.writelines(lines)
subprocess.run(['git', '--literal-pathspecs', 'rm', '--cached', '--quiet', '--pathspec-from-file=-', '--pathspec-file-nul'],
               input=b'\0'.join(name.encode() for name in names) + b'\0', check=True)
print(f'Untracked {len(names)} verified files; existing local copies remain materialized.')
