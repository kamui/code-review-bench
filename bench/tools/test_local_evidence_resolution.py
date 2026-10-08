import copy
import gzip
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import evidence_store as store
import local_evidence_resolution as resolution


class LocalResolution(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.row = {'root': '/original/workspace', 'path': 'home/session.jsonl', 'sha256': 'a' * 64,
                    'bytes': 123, 'mode': 0o600, 'blocked': 'credential-pattern', 'storage': None}
        self.entry = {**self.row, 'storage': {'path': 'artifacts/capture/session'}}
        self.mapping = self.root / 'mapping.json.gz'
        summary = self.root / resolution.SUMMARY
        summary.parent.mkdir(parents=True)
        summary.write_text(json.dumps({'mapping': {'path': 'artifacts/capture/mapping.json.gz', 'sha256': 'b' * 64}}))
        self.resolve = patch.object(store, 'resolve', return_value=self.mapping).start()
        self.addCleanup(patch.stopall)

    def save(self, entries):
        with gzip.open(self.mapping, 'wt') as handle:
            json.dump({'schema_version': 1, 'entries': entries, 'retirement_authorized': False}, handle)

    def test_exact_versioned_resolution_preserves_original_record(self):
        original = copy.deepcopy(self.row)
        self.save([self.entry])
        self.assertEqual(resolution.resolve(self.root, self.row), self.entry['storage'])
        self.assertEqual(self.row, original)
        self.resolve.assert_called_once_with(self.root, 'artifacts/capture/mapping.json.gz', 'b' * 64)

    def test_missing_duplicate_or_changed_identity_is_refused(self):
        variants = [[], [self.entry, self.entry]]
        for key, value in [('root', '/other'), ('path', 'other'), ('sha256', 'c' * 64), ('mode', 0o644), ('bytes', 124)]:
            variants.append([{**self.entry, key: value}])
        for entries in variants:
            self.save(entries)
            with self.assertRaises(store.EvidenceError):
                resolution.resolve(self.root, self.row)

    def test_missing_summary_and_unsafe_member_remain_blocked(self):
        self.save([{**self.entry, 'storage': {'path': '../private'}}])
        with self.assertRaises(store.EvidenceError):
            resolution.resolve(self.root, self.row)
        (self.root / resolution.SUMMARY).unlink()
        with self.assertRaisesRegex(store.EvidenceError, 'Not published'):
            resolution.resolve(self.root, self.row)


if __name__ == '__main__':
    unittest.main()
