"""Policy v2 authority, historical routing and both current filing paths."""
import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import delegation
import ruling_record as record
import ruling_summary as summary
from test_ruling_record import ROOT, sound


class PolicyTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.policy = record.load(ROOT / delegation.POLICY)
        receipt = self.root / 'bench/grading/rulings/delegation.v2.md'
        receipt.parent.mkdir(parents=True)
        receipt.write_text('Synthetic test only. Fixture owner adopts policy v2.')
        self.policy.update(status='adopted', receipt=self.pin(receipt), adoption_scope='Fixture owner adopts policy v2.')
        self.write(self.root / delegation.POLICY, self.policy)
        for kind in ('recovery', 'candidate', 'band', 'grouping', 'control', 'reconciliation'):
            before = sound(kind)
            paths = [before['rule']['path'], before['case']['path'], before['rulings'], before['clauses']]
            paths += [a['brief']['path'] for a in before['answers'] if a['brief']]
            for name in filter(None, paths):
                path = self.root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, path)
            if before['dossier']:
                shutil.copytree(ROOT / before['dossier'], self.root / before['dossier'], dirs_exist_ok=True)

    def write(self, path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value))
        return path

    def pin(self, path):
        return {'path': str(path.relative_to(self.root)), 'sha256': record.digest(path)}

    def add(self, number, kind='recovery', round_name='first', day='01', closed=True, agents=False, pending=False):
        before = sound(kind)
        before.update(ruling=f'case-{number}', target=f'pr-{number % 5}', group='N1' if kind == 'candidate' else f'Q{number}')
        directory = self.root / f'docs/research/{round_name}/rulings'
        opening_path = directory.parent / 'opening.json'
        if not opening_path.exists():
            closed_rounds = [self.pin(item) for item in sorted((self.root / 'docs/research').rglob('round.json'))
                             if record.load(item)['closed']]
            self.write(opening_path, {'round': round_name, 'source_commit': 'a' * 40, 'closed_rounds': closed_rounds})
        before['round_opening'] = self.pin(opening_path)
        self.write(directory / 'round.json', {'round': round_name, 'opened': f'2026-10-{day}', 'closed': closed, 'no_record': {},
                                             'opening': self.pin(opening_path)})
        path = self.write(directory / f'{number:03}.before.json', before)
        after = {'contract': 2, 'ruling': before['ruling'], 'before': self.pin(path), 'settled_by': 'agents' if agents else 'owner',
                 'policy': self.pin(self.root / delegation.POLICY) if agents else None, 'asked': 0 if agents else 1,
                 'outcome': before['answers'][0]['outcome'], 'by_default': False, 'ground': None, 'miss': None, 'lesson': None}
        for field in ('facts', 'same_fault_as'):
            if field in before['answers'][0]:
                after[field] = before['answers'][0][field]
        if not pending:
            self.write(path.with_name(path.name.replace('.before.', '.after.')), after)
        return before, after, path

    def ready(self, count=15, kind='recovery'):
        for number in range(count):
            self.add(number, kind)
        return self.add(count, kind, 'second', '02', closed=False, pending=True)

    def route(self, before, path):
        return delegation.route(before, self.root, before_path=path)

    def save(self, before, after, path):
        self.write(path, before)
        after['before'] = self.pin(path)
        self.write(path.with_name(path.name.replace('.before.', '.after.')), after)

    def test_proposal_has_no_authority_and_receipt_is_required(self):
        before, _, path = self.ready()
        self.write(self.root / delegation.POLICY, {**self.policy, 'status': 'proposed', 'receipt': None})
        self.assertIn('no policy is adopted', self.route(before, path)['reasons'][0])
        self.write(self.root / delegation.POLICY, {**self.policy, 'receipt': None})
        with self.assertRaisesRegex(ValueError, 'adoption receipt'):
            self.route(before, path)
        self.policy['levels'][0]['decisions'] = 1
        self.write(self.root / delegation.POLICY, self.policy)
        with self.assertRaisesRegex(ValueError, 'new version'):
            self.route(before, path)

    def test_thresholds_and_no_promotion_inside_round(self):
        before, after, path = self.ready(14)
        self.assertTrue(self.route(before, path)['reasons'])
        self.save(before, after, path)
        next_before, _, next_path = self.add(15, round_name='second', day='02', closed=False, pending=True)
        self.assertTrue(self.route(next_before, next_path)['reasons'])
        sheet = record.load(path.parent / 'round.json')
        sheet.update(closed=True, closed_at='2026-10-02T23:00:00Z')
        self.write(path.parent / 'round.json', sheet)
        _, last, _ = self.add(15, round_name='second', day='02')
        third, _, third_path = self.add(16, round_name='third', day='03', closed=False, pending=True)
        self.assertEqual(self.route(third, third_path), {'level': 1, 'reasons': []})

    def test_exactly_fifteen_matches_on_four_pull_requests_settle(self):
        before, _, path = self.ready()
        for item in (self.root / 'docs/research/first/rulings').glob('*.before.json'):
            value = record.load(item)
            value['target'] = f'pr-{int(item.name[:3]) % 4}'
            self.save(value, record.load(item.with_name(item.name.replace('.before.', '.after.'))), item)
        self.assertEqual(self.route(before, path), {'level': 1, 'reasons': []})

    def test_reasking_with_unchanged_answer_records_a_miss_without_resetting_trust(self):
        before, after, path = self.ready()
        after.update(asked=2, miss={'cause': 'fact-not-fetched', 'note': 'The second question supplied the missing fact.'})
        self.save(before, after, path)
        next_before, _, next_path = self.add(16, round_name='second', day='02', closed=False, pending=True)
        self.assertEqual(summary.earned(summary.records(self.root), 'recovery', self.root)['count'], 16)
        self.assertEqual(self.route(next_before, next_path), {'level': 1, 'reasons': []})

    def test_level_two_requires_thirty_owner_matches_on_five_pull_requests(self):
        before, _, path = self.ready(30)
        self.assertEqual(self.route(before, path), {'level': 2, 'reasons': []})
        for item in path.parents[2].joinpath('first/rulings').glob('*.before.json'):
            value = record.load(item)
            value['target'] = 'same-pr'
            after_path = item.with_name(item.name.replace('.before.', '.after.'))
            after = record.load(after_path)
            self.save(value, after, item)
        self.assertEqual(self.route(before, path)['level'], 0)

    def test_three_pull_requests_are_not_enough(self):
        before, _, path = self.ready()
        for item in (self.root / 'docs/research/first/rulings').glob('*.before.json'):
            value = record.load(item)
            value['target'] = f'pr-{int(item.name[:3]) % 3}'
            self.save(value, record.load(item.with_name(item.name.replace('.before.', '.after.'))), item)
        self.assertEqual(self.route(before, path)['level'], 0)

    def test_each_gate_and_saved_ruling_reviews_ask(self):
        before, _, path = self.ready()
        self.assertEqual(self.route(before, path), {'level': 1, 'reasons': []})
        for change in ('medium', 'low', 'gap', 'conflict', 'cannot-tell', 'review', 'model'):
            with self.subTest(change):
                case = copy.deepcopy(before)
                if change == 'medium':
                    case['answers'][1].update(confidence='medium', short_of_high=['fact-reported'], would_settle=False)
                elif change == 'low':
                    case['answers'][0].update(confidence='low', short_of_high=['fact-reported', 'flip-fact-open'], would_settle=False)
                elif change in ('gap', 'conflict'):
                    case['answers'][0].update(confidence='medium', short_of_high=[change], would_settle=False)
                    case['answers'][0]['rule_gap' if change == 'gap' else 'conflict'] = 'Unresolved rule.'
                elif change == 'cannot-tell':
                    case['answers'][0]['outcome'] = 'cannot-tell'
                elif change == 'review':
                    case['reviews'] = {'ruling': 'second-04', 'saved_outcome': 'recovers'}
                else:
                    case['answers'][1]['model'] = 'replacement'
                self.write(path, case)
                self.assertTrue(self.route(case, path)['reasons'])
        case = copy.deepcopy(before)
        case['answers'][0].update(confidence='medium', short_of_high=['fact-reported'], would_settle=False)
        self.write(path, case)
        self.assertFalse(self.route(case, path)['reasons'])

    def test_agent_records_do_not_earn_trust_and_history_does_not_recurse(self):
        before, after, path = self.ready()
        after.update(settled_by='agents', policy=self.pin(self.root / delegation.POLICY), asked=0)
        self.save(before, after, path)
        for number in range(16, 32):
            self.add(number, round_name='second', day='02', closed=False, agents=True)
        pairs = summary.records(self.root)
        self.assertEqual(summary.earned(pairs, 'recovery', self.root)['count'], 15)
        self.assertEqual(len(pairs), 32)
        forged = copy.deepcopy(after)
        forged['facts']['says_what'] = 'no'
        self.assertTrue(record.after_faults(before, forged, path, self.root))

    def test_miss_resets_now_without_invalidating_earlier_settlement(self):
        before, after, path = self.ready()
        after.update(settled_by='agents', policy=self.pin(self.root / delegation.POLICY), asked=0)
        self.save(before, after, path)
        miss, answer, miss_path = self.add(16, round_name='second', day='02', closed=False)
        answer.update(outcome='does-not-recover', miss={'cause': 'owner-weighs-differently', 'note': 'Owner disagreed.'})
        self.save(miss, answer, miss_path)
        next_before, _, next_path = self.add(17, round_name='second', day='02', closed=False, pending=True)
        self.assertEqual(len(summary.records(self.root)), 18)
        self.assertIn('reset', ' '.join(self.route(next_before, next_path)['reasons']))

    def test_later_owner_reversal_of_agent_settlement_resets_without_rejecting_history(self):
        before, after, path = self.ready()
        before['ruling'] = 'second-26'
        after.update(ruling='second-26', settled_by='agents', policy=self.pin(self.root / delegation.POLICY), asked=0)
        self.save(before, after, path)
        review, answer, review_path = self.add(16, round_name='second', day='02', closed=False)
        review.update(reviews={'ruling': 'second-26', 'saved_outcome': 'recovers'}, group=before['group'])
        review['answers'][1].update(confidence='medium', short_of_high=['fact-reported'], would_settle=False)
        answer.update(outcome='does-not-recover', miss={'cause': 'owner-weighs-differently', 'note': 'Owner reversed the settlement.'})
        self.save(review, answer, review_path)
        next_before, _, next_path = self.add(17, round_name='second', day='02', closed=False, pending=True)
        pairs = summary.records(self.root)
        self.assertEqual(summary.earned(pairs, 'recovery', self.root)['count'], 0)
        self.assertEqual(self.route(next_before, next_path)['level'], 0)

    def test_model_change_and_change_back_cannot_restore_round_authority(self):
        before, after, path = self.ready()
        before['answers'][1]['model'] = 'replacement'
        self.save(before, after, path)
        next_before, _, next_path = self.add(16, round_name='second', day='02', closed=False, pending=True)
        self.assertEqual(self.route(next_before, next_path)['level'], 0)

    def test_distinct_later_reversals_reset_rebuilt_trust_and_block_credit_filing(self):
        self.ready()
        settlements = []
        for number, ruling in ((15, 'second-26'), (16, 'second-27')):
            before, after, path = self.add(number, round_name='second', day='02', agents=True)
            before['ruling'] = after['ruling'] = ruling
            self.save(before, after, path)
            settlements.append((before, after, path))

        def reverse(number, original, round_name, day):
            before, after, path = self.add(number, round_name=round_name, day=day, closed=round_name != 'fourth')
            before.update(reviews={'ruling': original['ruling'], 'saved_outcome': 'recovers'}, group=original['group'])
            before['answers'][1].update(confidence='medium', short_of_high=['fact-reported'], would_settle=False)
            after.update(outcome='does-not-recover', miss={'cause': 'owner-weighs-differently', 'note': 'Owner reversed the settlement.'})
            self.save(before, after, path)

        reverse(17, settlements[0][0], 'third', '03')
        for number in range(18, 33):
            self.add(number, round_name='third', day='03')
        before, after, path = self.add(34, round_name='fourth', day='04', closed=False, pending=True)
        self.assertEqual(self.route(before, path), {'level': 1, 'reasons': []})
        reverse(33, settlements[1][0], 'fourth', '04')
        self.assertEqual(self.route(before, path)['level'], 0)
        self.assertIn('reset', ' '.join(self.route(before, path)['reasons']))
        pairs = summary.records(self.root)
        earned = summary.earned(pairs, 'recovery', self.root)
        self.assertEqual(earned['count'], 0)
        self.assertEqual(len(earned['resets']), 2)
        for old_before, old_after, old_path in settlements:
            self.assertEqual(record.after_faults(old_before, old_after, old_path, self.root), [])
        decision = {'target': before['target'], 'revision': {'head': 'fixture'}, 'review': {'path': 'review', 'sha256': 'fixture'},
                    'attempt_id': 'att-1', 'item_id': 'item-1', 'family_id': 'GT-1', 'says_what': 'yes', 'identifies_cause': 'no',
                    'settled_by': 'agents', 'receipt': self.policy['receipt'], 'receipt_scope': self.policy['adoption_scope'],
                    'reason': 'The agents settled under policy v2.'}
        before['filing'] = {key: decision[key] for key in ('revision', 'review', 'attempt_id', 'item_id', 'family_id')}
        after.update(settled_by='agents', policy=self.pin(self.root / delegation.POLICY), asked=0)
        self.save(before, after, path)
        decision['settlement'] = self.pin(path.with_name(path.name.replace('.before.', '.after.')))
        with self.assertRaisesRegex(ValueError, 'agents cannot settle: .*reset'):
            delegation.filing_faults(decision, 'recovery', self.root)

    def test_problem_and_reserved_types_stay_with_owner(self):
        for kind in ('candidate', 'band', 'grouping', 'control', 'reconciliation'):
            before, _, path = self.add(100, kind, kind, '03', closed=False, pending=True)
            if kind == 'candidate':
                for answer in before['answers']:
                    answer['outcome'] = 'problem'
                self.write(path, before)
            self.assertTrue(self.route(before, path)['reasons'])

    def test_both_current_filing_paths_reject_missing_settlement(self):
        for kind in ('candidate', 'recovery'):
            self.assertIn('pinned after-record', delegation.filing_faults({'settled_by': 'agents'}, kind, self.root)[0])

    def test_candidate_filing_binds_the_exact_subject_and_outcome(self):
        before, after, path = self.ready(kind='candidate')
        decision = {'target': before['target'], 'revision': {'head': 'fixture'}, 'subject': 'candidate-1',
                    'dimension': 'eligibility', 'status': 'approved', 'outcome': 'advisory', 'authority': 'human',
                    'settled_by': 'agents', 'receipt': self.policy['receipt'], 'receipt_scope': self.policy['adoption_scope'],
                    'reason': 'The agents settled under policy v2.'}
        before['filing'] = {key: decision[key] for key in ('revision', 'subject')}
        after.update(settled_by='agents', policy=self.pin(self.root / delegation.POLICY), asked=0)
        self.save(before, after, path)
        decision['settlement'] = self.pin(path.with_name(path.name.replace('.before.', '.after.')))
        self.assertEqual(delegation.filing_faults(decision, 'candidate', self.root), [])
        for key, value in [('subject', 'other'), ('outcome', 'eligible'), ('authority', 'automation'), ('dimension', 'impact')]:
            with self.subTest(key):
                self.assertTrue(delegation.filing_faults({**decision, key: value}, 'candidate', self.root))
        without_marker = {key: value for key, value in decision.items() if key not in ('settled_by', 'settlement')}
        self.assertTrue(delegation.filing_faults(without_marker, 'candidate', self.root))

    def test_credit_fact_disagreement_asks_and_owner_fact_change_is_a_miss(self):
        before, after, path = self.ready()
        before['answers'][1]['facts'] = {'says_what': 'yes', 'identifies_cause': 'yes'}
        self.write(path, before)
        self.assertTrue(self.route(before, path)['reasons'])
        before['answers'][1]['facts'] = before['answers'][0]['facts'].copy()
        after['facts'] = {'says_what': 'yes', 'identifies_cause': 'yes'}
        self.save(before, after, path)
        self.assertTrue(any('miss' in fault for fault in record.after_faults(before, after, path, self.root)))
        after['miss'] = {'cause': 'fact-misread', 'note': 'The owner changed the cause fact.'}
        self.save(before, after, path)
        self.assertEqual(summary.earned(summary.records(self.root), 'recovery', self.root)['count'], 0)

    def test_later_closure_cannot_raise_a_past_opening_level(self):
        for number in range(15):
            self.add(number, closed=False)
        before, _, path = self.add(15, round_name='second', day='02', closed=False, pending=True)
        first = self.root / 'docs/research/first/rulings/round.json'
        sheet = record.load(first)
        sheet['closed'] = True
        self.write(first, sheet)
        self.assertEqual(self.route(before, path)['level'], 0)

    def test_opening_dependencies_order_same_day_rounds(self):
        for number in range(15):
            self.add(number, round_name='z-first')
        before, _, path = self.add(15, round_name='a-second', closed=False, pending=True)
        self.assertEqual(self.route(before, path), {'level': 1, 'reasons': []})

    def test_credit_filing_binds_receipt_source_revision_and_facts(self):
        before, after, path = self.ready()
        decision = {'target': before['target'], 'revision': {'head': 'fixture'}, 'review': {'path': 'review', 'sha256': 'fixture'},
                    'attempt_id': 'att-1', 'item_id': 'item-1', 'family_id': 'GT-1', 'says_what': 'yes', 'identifies_cause': 'no',
                    'settled_by': 'agents', 'receipt': self.policy['receipt'], 'receipt_scope': self.policy['adoption_scope'],
                    'reason': 'The agents settled under policy v2.'}
        before['filing'] = {key: decision[key] for key in ('revision', 'review', 'attempt_id', 'item_id', 'family_id')}
        after.update(settled_by='agents', policy=self.pin(self.root / delegation.POLICY), asked=0)
        self.save(before, after, path)
        decision['settlement'] = self.pin(path.with_name(path.name.replace('.before.', '.after.')))
        self.assertEqual(delegation.filing_faults(decision, 'recovery', self.root), [])
        for key, value in [('item_id', 'item-2'), ('says_what', 'no'), ('receipt_scope', 'other'), ('target', 'other')]:
            with self.subTest(key):
                self.assertTrue(delegation.filing_faults({**decision, key: value}, 'recovery', self.root))


if __name__ == '__main__':
    unittest.main()
