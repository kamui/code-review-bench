#!/usr/bin/env python3
"""Restore an exact package copy from the recorded recovery release."""
import argparse
import copy
from pathlib import Path

import evidence_store as store

INDEX = 'bench/evidence/recovery/2026-10-08-v1.json'


def manifest(root, name):
    index = store.read(root / INDEX)
    if index.get('schema_version') != 1:
        raise store.EvidenceError('unsupported recovery index')
    records = [entry for entry in index['sources'] if entry['name'] == name]
    if len(records) != 1:
        raise store.EvidenceError('choose exactly one source name from the recovery index')
    record = records[0]
    source = store.confined(root, record['manifest'])
    if store.digest(source) != record['manifest_sha256']:
        raise store.EvidenceError('original discovery manifest changed')
    result = copy.deepcopy(store.validate(store.read(source), published=True))
    if result['repository'] != index['repository']:
        raise store.EvidenceError('recovery storage repository differs')
    if set(record['publications']) != {package['sha256'] for package in result['packages']}:
        raise store.EvidenceError('recovery index does not cover exactly the original packages')
    result['tag'] = index['tag']
    for package in result['packages']:
        package['publication'] = record['publications'][package['sha256']]
    return store.validate(result, published=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, help='source name from ' + INDEX)
    parser.add_argument('--root', type=Path, default=Path.cwd())
    parser.add_argument('--path', action='append')
    parser.add_argument('--offline', action='store_true')
    args = parser.parse_args()
    store.materialize(manifest(args.root, args.source), args.root, args.path, args.offline)
    print('Recovery copy verified and restored.')


if __name__ == '__main__':
    main()
