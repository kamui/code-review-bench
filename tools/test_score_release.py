import json
import unittest

import export_explorer
import score


class ScoreReleaseTest(unittest.TestCase):
    def test_approved_recoveries_retain_failed_attempts_and_their_usage(self):
        registry = json.loads((export_explorer.BENCH / 'scoreboard.current.json').read_text())
        for entry_id, predecessor, recovered in [('codex-ce-luna-high', 'att-010', 'att-014'),
                                                  ('codex-thermo-luna-high', 'att-026', 'att-038')]:
            with self.subTest(entry=entry_id):
                source = next(e for e in registry['suites'][0]['entries'] if e['id'] == entry_id)['sources'][0]
                run = export_explorer.BENCH / source['run']
                published = json.loads((run / source['results']).read_text())
                mappings = {i['target']: i['mapping_version'] for i in published['inputs']}
                computed = score.compute(run, mappings, None, None, 'test', rubric_version=2)
                cell = next(c for c in computed['cells'] if predecessor in c['attempts'])
                self.assertEqual(cell['attempts'], [predecessor, recovered])
                self.assertEqual(cell['status'], 'valid completed')
                before = json.loads((run / 'attempts' / predecessor / 'attempt.json').read_text())
                after = json.loads((run / 'attempts' / recovered / 'attempt.json').read_text())
                self.assertNotEqual(before['disposition'], 'valid completed')
                self.assertEqual(after['disposition'], 'valid completed')
                self.assertEqual(after['predecessor'], predecessor)
                for key in ('cells', 'inputs', 'by_arm', 'by_target_arm', 'by_shape', 'by_cohort'):
                    self.assertEqual(computed[key], published[key])
                mapping = json.loads((run / 'scoring' / cell['target'] / f"mapping.v{mappings[cell['target']]}.json").read_text())
                grade = next(a for a in mapping['attempts'] if a['attempt_id'] == predecessor)
                register = json.loads((export_explorer.BENCH / 'targets' / cell['target'] / f"register.v{mapping['register']['version']}.json").read_text())
                failed = score.score_attempt(before, grade, register, before['usage']['priced_total_usd'])
                self.assertEqual(failed['recall'], 0)
                self.assertEqual(failed['cost'], before['usage']['priced_total_usd'])


if __name__ == '__main__':
    unittest.main()
