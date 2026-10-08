#!/usr/bin/env python3
"""Reconcile a saved host inventory with evidence identities; never authorize deletion."""
from __future__ import annotations

import argparse
from collections import Counter
import gzip
import json
from pathlib import Path

import check_manifest
import evidence_store as store

SCHEMA = Path(__file__).resolve().parents[1] / 'schema/evidence-reconciliation.schema.json'


def member_index(manifests):
    index = {}
    for manifest in manifests:
        store.validate(manifest, published=True)
        for package in manifest['packages']:
            for member in package['members']:
                key = (member['sha256'], member['bytes'], member['mode'])
                index.setdefault(key, member['path'])
    return index


def classify(entry, shared):
    row = dict(entry, inventory_class=entry['class'], storage=None)
    parts = Path(entry['path']).parts
    if entry['type'] != 'file':
        row.update(status='retained', reason='Link or special file; inspection and retirement remain blocked.')
    elif entry['class'] == 'account-state':
        row.update(status='excluded', reason='Authentication stays local and is excluded from publication.')
    elif (key := (entry['sha256'], entry['bytes'], entry['mode'])) in shared:
        row.update(status='shared', storage=shared[key])
        if row['class'] == 'unknown':
            row['class'] = 'local-evidence'
        row['reason'] = 'Exact bytes, size and permissions are indexed by a published evidence manifest.'
    else:
        row['status'] = 'retained'
        if '.git' in parts or any(part.endswith('.git') for part in parts) or 'mirrors' in parts:
            row.update({'class': 'reproducibility', 'reason': 'Source/Git metadata retained; independent reachability and retirement are not established.'})
        elif parts[:2] in {('public', 'evidence'), ('public', 'data')} or entry['path'] == 'src/routeTree.gen.ts':
            row.update({'class': 'rebuildable', 'reason': 'Generated explorer/router output; this inventory does not verify safe deletion.'})
        elif any(part in {'clone', 'clone-work', 'base', 'head'} for part in parts) and row['class'] == 'unknown':
            row.update({'class': 'development', 'reason': 'Source or investigation workspace; unique changes have not been ruled out.'})
        elif 'caches' in parts and row['class'] == 'unknown':
            row.update({'class': 'reproducibility', 'reason': 'Preparation cache material; retained until pinned dependencies and layout are independently verified.'})
    return row


def reconcile(inventory, shared):
    rows = [classify(entry, shared) for entry in inventory['entries']]
    totals = Counter()
    for row in rows:
        totals[row['class']] += row['allocated_bytes']
    result = {**inventory, 'entries': rows, 'allocated_bytes_by_class': dict(totals), 'retirement_authorized': False}
    validate(result)
    return result


def validate(record):
    problems = check_manifest.validate(store.read(SCHEMA), record)
    if problems:
        raise store.EvidenceError('; '.join(problems[:10]))
    names = set()
    for entry in record['entries']:
        path = Path(entry['path'])
        if path.is_absolute() or not path.parts or '..' in path.parts or path.as_posix() != entry['path'] or entry['path'] in names:
            raise store.EvidenceError('duplicate or unsafe inventory path')
        names.add(entry['path'])
        if entry['status'] == 'shared':
            if entry['type'] != 'file' or not entry['sha256'] or not entry['storage'] or entry['class'] == 'account-state':
                raise store.EvidenceError('shared entry must identify a regular noncredential payload')
            store.logical_path(entry['storage'])
        elif entry['storage'] is not None:
            raise store.EvidenceError('retained/excluded entry cannot claim shared storage')
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--root', type=Path, default=Path.cwd())
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    opener = gzip.open if args.inventory.suffix == '.gz' else open
    with opener(args.inventory, 'rt') as handle:
        inventory = json.load(handle)
    shared = member_index([manifest for _, manifest in store.manifests(args.root)])
    result = reconcile(inventory, shared)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open('xb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as output:
        output.write(json.dumps(result, separators=(',', ':')).encode())
    print(json.dumps({'root': result['root'], 'entries': len(result['entries']),
                      'statuses': dict(Counter(row['status'] for row in result['entries'])),
                      'retirement_authorized': False}))


if __name__ == '__main__':
    main()
