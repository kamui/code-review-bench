import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import export_explorer as exporter
import current_grading as current
from test_current_grading import fixture, save_current, write


class ExportTest(unittest.TestCase):
    def build_fixture(self, root):
        with patch.object(exporter, 'ROOT', root), patch.object(exporter, 'BENCH', root / 'bench'), \
                patch.object(exporter, 'PUBLIC', root / 'public'), patch.object(exporter, 'BASE_PATH', '/bench'):
            exporter.build()
        return current.read_json(root / 'public/data/benchmark.json')

    def test_current_preview_never_opens_grade_releases_or_results(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            (root / 'bench/runs/run/results.v99.json').write_text('invalid old results', encoding='utf-8')
            before = (root / 'bench/runs/run/attempts/att-001/attempt.json').read_bytes()
            result = self.build_fixture(root)
            self.assertEqual((len(result['tasks']), len(result['configurations']), len(result['attempts'])), (1, 1, 1))
            self.assertEqual(result['grading']['kind'], 'ungraded')
            attempt = result['attempts'][0]
            self.assertTrue(attempt['admitted'])
            self.assertTrue(attempt['complete'])
            self.assertIsNone(attempt['falseFindings'])
            self.assertIsNone(attempt['noise'])
            self.assertEqual(attempt['feedback'], {'kind': 'unavailable', 'observedItems': 1})
            self.assertEqual(result['outcomes'][0]['trials'][1]['status'], 'pending')
            self.assertEqual((root / 'bench/runs/run/attempts/att-001/attempt.json').read_bytes(), before)
            detail = current.read_json(root / 'public/data/attempts/run/att-001.json')
            self.assertEqual(detail['items'][0]['assignment'], 'unassessed')
            self.assertEqual(detail['normalizedUrl'], '/bench/evidence/bench/runs/run/attempts/att-001/normalized.json')

    def test_billing_receipt_is_verified_and_exported_without_repricing(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            selected, documents = fixture(root)
            receipt = write(root, 'bench/billing.json', {'schema_version': 1, 'billing': 'api-dollars',
                            'sources': [{'run': 'runs/run', 'arm': 'high'}]})
            registry = current.read_json(root / 'bench/scoreboard.current.json')
            pin = current.pin_file(receipt, root)
            registry['sources'][0]['billing_correction'] = {**pin, 'path': 'billing.json'}
            write(root, 'bench/scoreboard.current.json', registry)
            save_current(root, current.inventory(root), documents)
            result = self.build_fixture(root)
            self.assertEqual(result['attempts'][0]['billing'], 'api-dollars')
            detail = current.read_json(root / 'public/data/attempts/run/att-001.json')
            self.assertEqual(detail['billingCorrectionUrl'], '/bench/evidence/bench/billing.json')
            receipt.write_text('{}', encoding='utf-8')
            with self.assertRaisesRegex(current.Inconsistent, 'source hash changed'):
                self.build_fixture(root)

    def test_missing_current_inventory_never_falls_back_to_an_archive(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            (root / 'bench/grading/current/inventory.json').unlink()
            write(root, 'bench/scoreboard.json', {'suites': []})
            with self.assertRaises(current.InputError):
                self.build_fixture(root)

    def test_review_duration_uses_the_filed_end_event_and_excludes_retry_gaps(self):
        record = {'disposition': 'valid completed', 'timing': {
            'dispatched_at': '2026-09-29T10:00:00Z', 'completed_at': '2026-09-29T10:02:00Z',
            'payload_validated_at': '2026-09-29T11:00:00Z', 'stopped_at': None}}
        self.assertEqual(exporter.duration_seconds(record), 120)
        record['disposition'] = 'harness-invalid'
        record['timing']['stopped_at'] = '2026-09-29T10:00:30Z'
        self.assertEqual(exporter.duration_seconds(record), 30)
        replacement = {'disposition': 'valid completed', 'timing': {
            'dispatched_at': '2026-09-29T12:00:00Z', 'completed_at': '2026-09-29T12:01:00Z'}}
        self.assertEqual(exporter.duration_seconds(record) + exporter.duration_seconds(replacement), 90)

    def test_missing_or_invalid_timing_is_unavailable(self):
        for timing in ({}, {'dispatched_at': 'invalid', 'completed_at': '2026-09-29T10:00:00Z'},
                       {'dispatched_at': '2026-09-29T10:01:00Z', 'completed_at': '2026-09-29T10:00:00Z'},
                       {'dispatched_at': '2026-09-29T10:00:00', 'completed_at': '2026-09-29T10:01:00Z'}):
            with self.subTest(timing=timing):
                self.assertIsNone(exporter.duration_seconds({'disposition': 'valid completed', 'timing': timing}))
        instant = '2026-09-29T10:00:00Z'
        self.assertEqual(exporter.duration_seconds({'disposition': 'valid completed', 'timing': {
            'dispatched_at': instant, 'completed_at': instant}}), 0)

    def test_evidence_urls_respect_site_base_path(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'record.json'
            source.write_text('{}\n')
            for base_path in ('', '/code-review-bench'):
                with self.subTest(base_path=base_path), patch.object(exporter, 'ROOT', root), \
                        patch.object(exporter, 'PUBLIC', root / 'public'), patch.object(exporter, 'BASE_PATH', base_path):
                    url = exporter.evidence(source)
                    self.assertEqual(url, base_path + '/evidence/record.json')
                    self.assertEqual((root / 'public' / url.removeprefix(base_path + '/')).read_text(), source.read_text())

    def test_missing_metering_is_not_zero(self):
        with TemporaryDirectory() as directory:
            path = Path(directory) / 'usage.jsonl'
            complete = {'usage': {'metering_status': 'complete'}}
            self.assertIsNone(exporter.usage_tokens(path, complete))
            path.write_text('{"output_tokens": 7}\n{"output_tokens": 3}\n')
            self.assertEqual(exporter.usage_tokens(path, complete), 10)
            self.assertIsNone(exporter.usage_tokens(path, {}))
            path.write_text('{"output_tokens": 7}\n{}\n')
            self.assertIsNone(exporter.usage_tokens(path, complete))



if __name__ == '__main__':
    unittest.main()
