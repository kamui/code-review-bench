#!/usr/bin/env python3
"""Relocate the tracked last-push transcript archives to release storage.

Run from the checkout at the pushed source commit:
  relocate_transcripts.py prepare PUBLIC_TREE_JSON NEW_OUTPUT_DIRECTORY
  relocate_transcripts.py detach
PUBLIC_TREE_JSON is GitHub's recursive tree response for the commit's artifacts/transcripts tree.
prepare snapshots and packs the archives and never removes a source file. detach runs once, after
remote verification has written the receipt, and untracks them without removing local bytes.
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

SOURCE = '297edaaf64d5ce569033165462363befd8f97314'
TREE = 'artifacts/transcripts'
PREFIX = TREE + '/2026-10-08-last-push-'
NAME = 'last-push-transcripts-2026-10-09-v1'
MANIFEST = ROOT / f'bench/evidence/manifests/{NAME}.json'
RECEIPT = ROOT / f'bench/evidence/receipts/{NAME}-restoration.json'
COUNT = 184


def git(*arguments):
    return subprocess.check_output(['git', *arguments], text=True).strip()


def blob(data):
    return hashlib.sha1(f'blob {len(data)}\0'.encode() + data).hexdigest()


def record(name):
    run, attempt = Path(name).parent.name, Path(name).name.removesuffix('.tar.gz')
    if run.endswith('-probe'):
        return ROOT / 'bench/runs' / run.removesuffix('-probe') / 'probes' / attempt / 'attempt.json'
    return ROOT / 'bench/runs' / run / 'attempts' / attempt / 'attempt.json'


def prepare(public_tree, output):
    public = store.read(public_tree)
    assert not public['truncated'] and public['sha'] == git('rev-parse', f'{SOURCE}:{TREE}')
    published = {f'{TREE}/{row["path"]}': row for row in public['tree'] if row['type'] == 'blob'}
    output = Path(output).absolute()
    output.mkdir()
    selection = {'repository': 'kamui/code-review-bench', 'tag': 'evidence-' + NAME, 'subjects': [], 'files': []}
    for line in git('ls-tree', '-r', '-l', SOURCE, '--', TREE).splitlines():
        info, name = line.split('\t', 1)
        mode, kind, identity, size = info.split()
        if kind != 'blob' or not name.startswith(PREFIX):
            continue
        assert not (ROOT / name).is_symlink()
        data = (ROOT / name).read_bytes()
        assert blob(data) == identity == published[name]['sha'] and len(data) == int(size) == published[name]['size']
        assert hashlib.sha256(data).hexdigest() == store.read(record(name))['transcript_archive']['sha256']
        target = output / 'files' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / name, target)
        target.chmod(int(mode, 8) & 0o777)
        selection['subjects'].append(f'{Path(name).parent.name}/{Path(name).name.removesuffix(".tar.gz")}')
        selection['files'].append({'source': str(target), 'path': name, 'kind': 'evidence'})
    assert len(selection['files']) == COUNT
    store.write_new(output / 'selection.json', selection)
    packed = store.pack(selection, output / 'packed')
    print(json.dumps({'files': COUNT, 'original_bytes': sum(m['bytes'] for p in packed['packages'] for m in p['members']),
                      'packed_bytes': sum(p['bytes'] for p in packed['packages'])}))


def detach():
    manifest = store.validate(store.read(MANIFEST), published=True)
    receipt = store.read(RECEIPT)
    assert receipt['status'] == 'verified' and receipt['manifest_sha256'] == store.digest(MANIFEST)
    assert git('rev-parse', 'HEAD') == SOURCE
    indexed = {}
    for row in subprocess.check_output(['git', 'ls-files', '--stage', '-z', '--', TREE]).split(b'\0'):
        if row:
            info, name = row.split(b'\t', 1)
            mode, identity, stage = info.decode().split()
            assert stage == '0'
            indexed[name.decode()] = (mode, identity)
    names = []
    for package in manifest['packages']:
        for member in package['members']:
            name = member['path']
            path = store.confined(ROOT, name)
            store.checked_file(path, member)
            identity = blob(path.read_bytes())
            assert indexed[name] == (f'100{member["mode"]:03o}', identity) == ('100644', git('rev-parse', f'{SOURCE}:{name}'))
            names.append(name)
    assert len(names) == len(set(names)) == COUNT == sum(name.startswith(PREFIX) for name in indexed)
    lines = [f'\n# Verified release payloads; restore with evidence_store.py fetch --manifest {MANIFEST.relative_to(ROOT)}.\n']
    for name in sorted(names):
        assert '\n' not in name and not name.endswith(' ')
        lines.append('/' + ''.join('\\' + c if c in '\\*?[]' else c for c in name) + '\n')
    with (ROOT / '.gitignore').open('a') as target:
        target.writelines(lines)
    subprocess.run(['git', '--literal-pathspecs', 'rm', '--cached', '--quiet', '--pathspec-from-file=-', '--pathspec-file-nul'],
                   input=b'\0'.join(name.encode() for name in names) + b'\0', check=True)
    print(f'Untracked {len(names)} verified files; existing local copies remain materialized.')


if __name__ == '__main__':
    if sys.argv[1:2] == ['prepare'] and len(sys.argv) == 4:
        prepare(sys.argv[2], sys.argv[3])
    elif sys.argv[1:] == ['detach']:
        detach()
    else:
        raise SystemExit(__doc__)
