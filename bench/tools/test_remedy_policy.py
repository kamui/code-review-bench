"""Exercise safety scope through real preparation, validation, mapping and audit filing; no model calls."""

import copy
import shutil
import unittest

import current_grading as current
import grade
import grading_validation as validation
import test_evaluator_audit as audit_fixture
from test_grade import RUN, TARGET, grade as command, write_json


def activate(case):
    source = current.BENCH / 'rubric/remedy-policy.v1.json'
    policy = current.read_json(source)
    for role in ('first', 'second'):
        for pin in policy[role].values():
            destination = case.root / pin['path']
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(current.ROOT / pin['path'], destination)
    policy['validation_policy'] = current.pin_file(case.current / 'validation-policy.json', case.root)
    path = case.root / 'bench/rubric/remedy-policy.v1.json'
    write_json(path, policy)
    write_json(case.current / 'remedy-policy.json', current.pin_file(path, case.root))


class RemedyPolicy(unittest.TestCase):
    def setUp(self):
        self.case = audit_fixture.PreExisting('test_each_admitted_review_and_problem_is_a_unit_and_the_sample_records_the_checks_and_the_label')
        self.case.setUp()
        self.addCleanup(self.case.tearDown)

    def first(self, outcomes=('sufficient',)):
        case = self.case
        activate(case)
        reviews = case.reviews()
        recommendation = reviews['att-001']['recommendations'][0]
        recommendation['addressed_claims'] = ['c1']
        recommendation['sufficiency'] = [
            {'family': f'GT-t{n}', 'outcome': outcome, 'reason': 'Checked the proposed repair.',
             'evidence': [] if outcome == 'unassessed' else ['Read main.go.']}
            for n, outcome in enumerate(outcomes, 1)]
        if len(outcomes) == 2:
            reviews['att-001']['items']['1']['claims'][0]['known_problems'].append(
                {'family': 'GT-t2', 'says_what': 'yes', 'identifies_cause': 'no', 'reason': 'The claim covers both failures.'})
        args = case.assessed(case.work, case.key, 'first', reviews)
        return args, current.read_json(case.work / 'verdicts.json'), current.read_json(case.work / 'validator/inputs.json')

    def test_first_grader_safety_gate_and_legacy_snapshot(self):
        _, verdicts, snapshot = self.first(('sufficient', 'partial'))
        self.assertEqual(snapshot['safety_policy'], validation.CONDITIONAL_SAFETY)
        self.assertIn('Assess safety once per recommendation only when', (self.case.work / 'prompt.md').read_text())
        recommendation = next(r for review in verdicts['reviews'].values() for r in review['recommendations'])
        for outcomes, assess in [(('sufficient', 'ineffective'), True), (('partial', 'unassessed'), True),
                                 (('ineffective', 'ineffective'), False), (('unassessed', 'ineffective'), False)]:
            for state in ('safe', 'unsafe', 'unassessed'):
                with self.subTest(outcomes=outcomes, state=state):
                    for entry, outcome in zip(recommendation['sufficiency'], outcomes):
                        entry.update(outcome=outcome, evidence=[] if outcome == 'unassessed' else ['Read main.go.'])
                    recommendation['safety'].update(state=state, evidence=[] if state == 'unassessed' else ['Read main.go.'])
                    problems = validation.validate(verdicts, snapshot)
                    self.assertEqual(not problems, assess or state == 'unassessed', problems)
        recommendation['sufficiency'] = []
        recommendation['addressed_claims'] = []
        recommendation['safety'].update(state='unsafe', evidence=['Read main.go.'])
        self.assertIn('skip safety', '\n'.join(validation.validate(verdicts, snapshot)))
        legacy = {k: v for k, v in snapshot.items() if k != 'safety_policy'}
        self.assertEqual(validation.validate(verdicts, legacy), [])
        recommendation['safety'].update(state='unassessed')
        self.assertIn('skip safety', '\n'.join(validation.validate(verdicts, snapshot)))
        recommendation['safety']['evidence'] = []
        self.assertEqual(validation.validate(verdicts, snapshot), [])

    def test_ineffective_fix_maps_without_changing_detection_or_claiming_safety(self):
        args, _, _ = self.first(('ineffective',))
        done = command('map', '--root', str(self.case.root), *args)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        checked = self.case.check()
        self.assertEqual(checked.returncode, 0, checked.stdout + checked.stderr)
        batch = self.case.saved()['batches'][0]
        review = next(r for r in batch['reviews'] if r['attempt_id'] == 'att-001')
        self.assertEqual((review['families'][0]['outcome'], review['families'][0]['sufficiency']), ('caught', 'ineffective'))
        self.assertEqual(review['recommendations'][0]['safety']['state'], 'unassessed')
        receipt = current.read_json(current.resolve_pin(batch['assessor']['receipt'], self.case.root))
        self.assertEqual(receipt['remedy_policy']['role'], 'first')
        self.assertFalse((self.case.work / 'clone').exists())

    def test_useful_fix_still_needs_independent_safety_confirmation(self):
        args, verdicts, _ = self.first()
        token, review = next((token, review) for token, review in verdicts['reviews'].items() if review['recommendations'])
        review['recommendations'][0]['safety'].update(state='unsafe', evidence=['Read main.go.'])
        write_json(self.case.work / 'verdicts.json', verdicts)
        checks = self.case.root / 'checks.json'
        write_json(checks, {'checker': 'independent person', 'independent_of': 'first', 'checks': [
            {'review': token, 'recommendation': 'r1', 'result': 'confirmed', 'reason': 'The proposed lock also deadlocks another caller.'}]})
        done = command('map', '--root', str(self.case.root), *args, '--safety-checks', str(checks))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        review = self.case.saved()['batches'][0]['reviews'][0]
        self.assertEqual(review['recommendations'][0]['safety']['state'], 'unsafe')
        self.assertEqual(review['families'][0]['outcome'], 'caught')
        self.assertEqual(grade.recorded_safety({'state': 'unsafe', 'reason': 'Harm.'}, [])['state'], 'unassessed')

    def test_second_grader_omits_safety_but_preserves_inventory_and_sufficiency(self):
        case = self.case
        case.drawn()
        before = (case.current / 'grades.json').read_bytes()
        activate(case)
        args = case.second()
        work = case.root / 'work-second'
        verdicts, snapshot = current.read_json(work / 'verdicts.json'), current.read_json(work / 'validator/inputs.json')
        self.assertEqual(snapshot['safety_policy'], validation.NO_SAFETY)
        self.assertIn('Do not assess safety or include a safety field', (work / 'prompt.md').read_text())
        self.assertNotIn('"safety":', (work / 'prompt.md').read_text())
        self.assertTrue(validation.validate(verdicts, snapshot))
        for review in verdicts['reviews'].values():
            for recommendation in review['recommendations']:
                del recommendation['safety']
                recommendation['addressed_claims'] = ['c1']
                recommendation['sufficiency'] = [{'family': 'GT-t1', 'outcome': 'ineffective',
                                                  'reason': 'The proposed fix leaves the failure intact.', 'evidence': ['Read main.go.']}]
        self.assertEqual(validation.validate(verdicts, snapshot), [])
        invalid = copy.deepcopy(verdicts)
        recommendation = next(r for review in invalid['reviews'].values() for r in review['recommendations'])
        del recommendation['sufficiency']
        self.assertTrue(validation.validate(invalid, snapshot))
        write_json(work / 'verdicts.json', verdicts)
        denied = command('map', '--root', str(case.root), *args)
        self.assertIn('cannot be mapped', denied.stdout + denied.stderr)
        done = case.audit('second', *args)
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        done = case.audit('compare')
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual((case.current / 'grades.json').read_bytes(), before)
        output = case.round / f'second/{RUN}/{TARGET}/assessment-1'
        saved = current.read_json(output / 'assessment.json')
        recommendation = next(r for review in saved['reviews'] for r in review['recommendations'])
        self.assertEqual(recommendation['safety']['state'], 'unassessed')
        self.assertEqual(recommendation['sufficiency'][0]['outcome'], 'ineffective')
        self.assertEqual(current.read_json(output / 'receipt.json')['remedy_policy']['role'], 'second')
        self.assertFalse((work / 'clone').exists())

    def test_policy_pin_and_fixed_inventory_are_enforced(self):
        _, verdicts, snapshot = self.first()
        snapshot['safety_policy'] = validation.NO_SAFETY
        self.assertIn('pinned claim inventory', '\n'.join(validation.validate(verdicts, snapshot)))
        source = self.case.root / 'bench/rubric/grader.safety-first.v1.md'
        source.write_text(source.read_text() + '\nchanged\n')
        done = self.case.prepare(self.case.root / 'fresh', self.case.root / 'fresh-key.json')
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('hash', done.stdout + done.stderr)


if __name__ == '__main__':
    unittest.main()
