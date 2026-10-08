"""Resolve previously withheld paths through a new hash-pinned mapping."""
import gzip
import json

import evidence_store as store

SUMMARY = 'docs/research/evidence-completion-2026-10-08/withheld-summary.json'


def resolve(root, row):
    if not (root / SUMMARY).is_file():
        raise store.EvidenceError('Not published: ' + row['blocked'])
    reference = store.read(root / SUMMARY)['mapping']
    mapping = store.resolve(root, reference['path'], reference['sha256'])
    with gzip.open(mapping, 'rt') as handle:
        document = json.load(handle)
    if document.get('schema_version') != 1 or document.get('retirement_authorized') is not False:
        raise store.EvidenceError('unsupported local-evidence resolution mapping')
    matches = [entry for entry in document['entries'] if entry['root'] == row['root'] and entry['path'] == row['path']]
    if len(matches) != 1 or any(matches[0][key] != row[key] for key in ['sha256', 'bytes', 'mode']):
        raise store.EvidenceError('resolution must match exactly one original path, hash, size and mode')
    reference = matches[0]['storage']
    if set(reference) != {'path'}:
        raise store.EvidenceError('resolution must name one published member')
    store.logical_path(reference['path'])
    return reference
