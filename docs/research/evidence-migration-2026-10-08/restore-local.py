#!/usr/bin/env python3
"""Restore one captured original path without writing to its original workspace.

Run in the repository: restore-local.py ROOT_NAME ORIGINAL_RELATIVE_PATH NEW_FILE
"""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path.cwd()
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store as store

label, original, output_name = sys.argv[1:]
summary = store.read(ROOT / 'docs/research/evidence-migration-2026-10-08/capture-summary.json')
roots = [row for row in summary['roots'] if Path(row['root']).name == label]
if len(roots) != 1:
    raise SystemExit('Choose exactly one root name from capture-summary.json')
root = roots[0]
mapping = store.resolve(ROOT, root['mapping'], root['sha256'])
with gzip.open(mapping, 'rt') as handle:
    rows = [row for row in json.load(handle)['entries'] if row['path'] == original]
if len(rows) != 1:
    raise SystemExit('Path was not selected; consult the migration inventory')
row = rows[0]
if row.get('blocked'):
    raise SystemExit('Not published: ' + row['blocked'])
reference = row['storage']
if reference.get('git_commit'):
    name = Path(reference['path'])
    if name.is_absolute() or '..' in name.parts or '.git' in name.parts or name.as_posix() != reference['path']:
        raise SystemExit('Unsafe Git reference path')
    local = ROOT / name
    if local.is_file() and not local.is_symlink() and store.digest(local) == row['sha256']:
        data = local.read_bytes()
    else:
        data = subprocess.check_output(['git', 'show', f"{reference['git_commit']}:{reference['path']}"])
else:
    data = store.resolve(ROOT, reference['path'], row['sha256']).read_bytes()
if len(data) != row['bytes'] or hashlib.sha256(data).hexdigest() != row['sha256']:
    raise SystemExit('Restored bytes differ from original inventory')
output = Path(output_name).absolute()
if any(path.is_symlink() for path in (output, *output.parents)):
    raise SystemExit('Output must not use symlinks')
output.parent.mkdir(parents=True, exist_ok=True)
with output.open('xb') as handle:
    handle.write(data)
output.chmod(row['mode'])
print(f'Verified {row["sha256"]}; restored {row["bytes"]} bytes to {output}')
