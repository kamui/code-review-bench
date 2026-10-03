import json
from pathlib import Path
import tempfile
import unittest

import current_grading as current
import methodology
from test_current_grading import fixture


class ReconciliationPlan(unittest.TestCase):
    def test_queue_is_selected_and_keeps_quiet_ungraded_reviews(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fixture(root)
            result = methodology.plan(root)
            self.assertEqual(result['counts']['selected_attempts'], 1)
            self.assertEqual(len(result['reviews']), 1)
            self.assertEqual(len(result['batches']), 1)
            self.assertEqual(result['reviews'][0]['action'], 'current-claim-assessment')
            self.assertNotIn('priorMapping', result['reviews'][0])
            self.assertEqual(len(result['batches'][0]['inputFingerprint']), 64)
            self.assertEqual(result['batches'][0]['state'], 'missing')
            self.assertEqual(result['pendingCandidates'], [])
            self.assertEqual(result['targets'][0]['control']['status'], 'known-problems')

    def test_actual_queue_includes_exactly_the_selected_source_attempts(self):
        result = methodology.plan()
        selected = current.inventory()
        self.assertEqual({r['attempt'] for r in result['reviews']}, {a['id'] for a in selected['attempts'] if a['review']})
        self.assertEqual(len(result['batches']), selected['counts']['batches'])
        self.assertTrue(any(r['items'] == 0 for r in result['reviews']))
        self.assertEqual(len(result['targets']), selected['counts']['tasks'])
        self.assertTrue(all('priorAssignments' not in r for r in result['reviews']))


if __name__ == '__main__':
    unittest.main()
