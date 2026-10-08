#!/usr/bin/env python3
"""Inventory execution storage without deleting or changing it.

Usage: evidence_inventory.py --root PATH [--root PATH ...] --out inventory.json
Every non-directory entry is listed, including ignored files and symlinks. Classification
is advisory, never deletion authorization. Exit 0: inventoried; 1: invalid input; 2: I/O error.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys

import evidence_store as store

CREDENTIALS = {'home/.codex/auth.json', 'home/.claude/.credentials.json'}
GENERATED = {'node_modules', 'dist', '.output', '.vite', '__pycache__', 'clone-cache'}


def git(path, *args):
    return subprocess.check_output(['git', '-C', str(path), *args], text=True, encoding='utf-8', stderr=subprocess.PIPE).strip()


def tracked(root):
    try:
        if Path(git(root, 'rev-parse', '--show-toplevel')).resolve() != root:
            return {}, None
        entries = subprocess.check_output(['git', '-C', str(root), 'ls-files', '--stage', '-z']).split(b'\0')
        index = {}
        for entry in filter(None, entries):
            metadata, name = entry.split(b'\t', 1)
            mode, sha, stage = metadata.decode().split()
            if stage != '0':
                raise store.EvidenceError('unmerged index')
            index[os.fsdecode(name)] = (mode, sha)
        return index, git(root, 'rev-parse', 'HEAD')
    except subprocess.CalledProcessError:
        return {}, None


def identities(path, size):
    sha, blob = hashlib.sha256(), hashlib.sha1()
    blob.update(f'blob {size}\0'.encode())
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            sha.update(block)
            blob.update(block)
    return sha.hexdigest(), blob.hexdigest()


def classification(name, indexed, blob):
    parts = Path(name).parts
    if name in indexed:
        return ('tracked-identical', 'matches Git index') if blob == indexed[name][1] else ('development', 'differs from Git index')
    if name in CREDENTIALS or any(name.endswith('/' + entry) for entry in CREDENTIALS):
        return 'account-state', 'authentication; exclude from publication'
    if '.git' in parts:
        return 'rebuildable', 'Git execution metadata; verify reachability before removal'
    if any(part in GENERATED for part in parts):
        return 'rebuildable', 'generated storage; not evidence of safe deletion'
    if 'archives' in parts and name.endswith(('.tar.gz', '.tgz', '.tar.zst')):
        return 'reproducibility', 'dependency archive may be hash-pinned'
    if any(part in {'clone-work', 'home', 'attempts', 'grading', 'regrading'} for part in parts):
        return 'local-evidence', 'execution or grading material requires capture'
    if Path(name).suffix in {'.json', '.jsonl', '.log'}:
        return 'local-evidence', 'possible records or diagnostics require inspection'
    return 'unknown', 'no evidence or rebuildability contract establishes its role'


def inventory(root, hash_all=False):
    root = Path(root).absolute()
    if root != root.resolve():
        raise store.EvidenceError(f'root must be canonical, without aliases or parent traversals: {root}')
    if any(path.is_symlink() for path in (root, *root.parents)) or not root.is_dir():
        raise store.EvidenceError(f'root must be an existing directory without symlink ancestors: {root}')
    indexed, head = tracked(root)
    entries, totals = [], Counter()
    for directory, dirs, files in os.walk(root, followlinks=False):
        current = Path(directory)
        links = [name for name in dirs if (current / name).is_symlink()]
        dirs[:] = sorted(set(dirs) - set(links))
        for name in sorted(files + links):
            path = current / name
            relative = path.relative_to(root).as_posix()
            info = path.lstat()
            kind = 'file' if stat.S_ISREG(info.st_mode) else 'symlink' if stat.S_ISLNK(info.st_mode) else 'special'
            classification_name, reason = classification(relative, {}, None)
            sha = blob = None
            if kind == 'file' and (hash_all or relative in indexed or classification_name not in {'rebuildable', 'account-state'}):
                sha, blob = identities(path, info.st_size)
            if kind != 'file':
                classification_name, reason = 'unknown', f'{kind}: must be inspected; never followed'
            else:
                classification_name, reason = classification(relative, indexed, blob)
            entry = {'path': relative, 'type': kind, 'bytes': info.st_size, 'allocated_bytes': info.st_blocks * 512,
                     'mode': stat.S_IMODE(info.st_mode), 'sha256': sha, 'class': classification_name, 'reason': reason}
            if kind == 'symlink':
                entry['target'] = os.readlink(path)
            entries.append(entry)
            totals[classification_name] += info.st_blocks * 512
    present = {entry['path'] for entry in entries}
    missing = sorted(set(indexed) - present)
    return {'schema_version': 1, 'root': str(root), 'head': head, 'entries': entries,
            'missing_tracked': missing, 'allocated_bytes_by_class': dict(sorted(totals.items())),
            'retirement_authorized': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, action='append', required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--hash-all', action='store_true', help='also hash rebuildable and account-state files locally')
    args = parser.parse_args()
    try:
        roots = [path.absolute() for path in args.root]
        if any(args.out.absolute().is_relative_to(root) for root in roots):
            raise store.EvidenceError('write the inventory outside the inspected roots')
        result = {'schema_version': 1, 'roots': [inventory(root, args.hash_all) for root in roots]}
        store.write_new(args.out, result)
        print(json.dumps([{'root': item['root'], 'files': len(item['entries']),
                           'allocated_bytes_by_class': item['allocated_bytes_by_class'],
                           'missing_tracked': len(item['missing_tracked'])} for item in result['roots']], indent=2))
        return 0
    except ValueError as error:
        print(f'inventory: {error}')
        return 1
    except (OSError, subprocess.SubprocessError) as error:
        print(f'inventory: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
