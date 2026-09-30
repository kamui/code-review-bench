import json
from pathlib import Path
import tempfile
import unittest

import regrade


class RegradingBudget(unittest.TestCase):
    def test_reservations_fit_remaining_total_with_headroom(self):
        for used in (0, 10, 27, 27.995, 28, 28.5, 29, 30):
            for items in (0, 1, 10, 100, 1000):
                amount = regrade.allowance(30, used, items)
                if amount is not None:
                    self.assertGreaterEqual(amount, 1)
                    self.assertLessEqual(regrade.money(used) + amount + 1, 30)
        self.assertIsNone(regrade.allowance(30, 29, 1))
        self.assertIsNone(regrade.allowance(30, 30, 100))

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
