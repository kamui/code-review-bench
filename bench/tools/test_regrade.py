import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import tarfile

import regrade


class RegradingBudget(unittest.TestCase):
    def test_dispatch_arguments_parse_with_the_pinned_key_and_client(self):
        import grade
        import sys
        arguments = regrade.dispatch_arguments(Path("work"), Path("key.json"),
                                              {"model": "claude-opus-5-5", "effort": "high"}, 2, "2.1.286")
        with patch.object(sys, "argv", ["grade.py", *map(str, arguments)]), patch.object(grade, "dispatch", return_value=[]) as dispatch:
            self.assertEqual(grade.main(), 0)
        self.assertEqual(dispatch.call_args.args[0].key, "key.json")
        self.assertEqual(dispatch.call_args.args[0].expected_cli_version, "2.1.286")
        with self.assertRaisesRegex(ValueError, "pinned"):
            regrade.dispatch_arguments(Path("work"), Path("key.json"), {"model": "m", "effort": "high"}, 2, None)

    def test_neutral_workspace_preserves_receipts_and_portable_evidence(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(regrade, "ROOT", Path(temp)):
            directory = Path(temp) / '.local/queue-4'
            attempt = directory / 'batches/reviewer-model-high/target/attempt-1'
            attempt.mkdir(parents=True)
            work = regrade.grading_workspace(attempt)
            self.assertNotIn('reviewer-model-high', str(work))
            self.assertEqual(regrade.grading_workspace(attempt), work)
            work.mkdir(parents=True)
            (attempt / 'reservation.json').write_text('{}')
            (work / 'dispatch.json').write_text(json.dumps({'usage': {'high': 0.5}}))
            self.assertEqual(regrade.spent(directory), regrade.money('0.5'))
            receipt = regrade.read(Path(temp) / regrade.archive_attempt(attempt)['path'])
            with tarfile.open(Path(temp) / receipt['archive']['path']) as bundle:
                member = bundle.getmember('work/dispatch.json')
                self.assertTrue(member.isfile())
                self.assertEqual(json.load(bundle.extractfile(member))['usage']['high'], 0.5)

    def test_existing_workspace_is_preserved_for_mapping_paid_attempts(self):
        with tempfile.TemporaryDirectory() as temp:
            attempt = Path(temp) / 'attempt-1'
            work = attempt / 'work'
            work.mkdir(parents=True)
            self.assertEqual(regrade.grading_workspace(attempt), work)
            self.assertFalse(work.is_symlink())

    def test_reservations_fit_remaining_total_with_headroom(self):
        for used in (0, 10, 27, 27.995, 28, 28.5, 29, 30):
            for items in (0, 1, 10, 100, 1000):
                amount = regrade.allowance(30, used, items)
                if amount is not None:
                    self.assertGreaterEqual(amount, 1)
                    self.assertLessEqual(regrade.money(used) + amount + 1, 30)
        self.assertIsNone(regrade.allowance(30, 29, 1))
        self.assertIsNone(regrade.allowance(30, 30, 100))

    def test_prior_budget_failure_gets_a_larger_replacement_allowance(self):
        self.assertGreater(regrade.allowance(33, 3.706681, 28), regrade.money("1.350928"))

    def test_invalid_amounts_are_refused(self):
        for amount in (-1, 'NaN', 'Infinity'):
            with self.assertRaises(ValueError):
                regrade.money(amount)

    def test_missing_or_unpriced_dispatch_blocks_further_spending(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            attempt = directory / 'batches/run/target/attempt-1'
            attempt.mkdir(parents=True)
            (attempt / 'reservation.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'unsettled'):
                regrade.spent(directory)
            work = attempt / 'work'
            work.mkdir()
            (work / 'dispatch.json').write_text(json.dumps({'usage': {'high': None}}))
            with self.assertRaisesRegex(ValueError, 'unpriced'):
                regrade.spent(directory)

    def test_failed_attempt_charges_remain_in_total(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            for index, amount in enumerate((1.25, 0.375), 1):
                attempt = directory / f'batches/run/target/attempt-{index}'
                (attempt / 'work').mkdir(parents=True)
                (attempt / 'reservation.json').write_text('{}')
                (attempt / 'work/dispatch.json').write_text(json.dumps({'exit_code': index - 1, 'usage': {'high': amount}}))
            self.assertEqual(regrade.spent(directory), regrade.money('1.625'))


if __name__ == '__main__':
    unittest.main()
