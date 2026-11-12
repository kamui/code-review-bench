"""Close round 3 in audits.json under audit decision 17 and set the round 4 seed.

Usage: python3 -I open.py <repo> [--write]
"""
import copy, hashlib, json, sys
from pathlib import Path

repo, write = Path(sys.argv[1]), '--write' in sys.argv
path = repo / 'bench/grading/current/audits.json'
receipt = 'bench/grading/rulings/cohort-rebuild-audit.v12.md'
old_seed, new_seed = 'evaluator-audit-v2-2026-10-29-round-3', 'evaluator-audit-v2-2026-10-29-round-4'
conclusion = json.loads((repo / f'bench/grading/current/audit/{old_seed}/conclusion.json').read_text())

audits = json.loads(path.read_text())
assert audits['plan']['seed'] == old_seed, audits['plan']['seed']
assert not any(entry['plan']['seed'] == old_seed for entry in audits['superseded'])

audits['superseded'].append({
    'closed_at': conclusion['concluded_at'],
    'reason': 'Concluded as failed. It sampled the rule-version-6 grades and was assessed a second time by Claude Sonnet 5.5 at high effort; the owner reconciled its disagreements, and the refuted stratum held 3 confirmed first-grader errors against its tolerance of 1. The suggestion stratum held 2 against its tolerance of 2, and every other sampled stratum held none. The owner\'s calls were filed as saved claims, the batches they reached were graded again, and the plan draws round 4 with the seed followed by -round-4.',
    'authority': 'human',
    'receipt': {'path': receipt, 'sha256': hashlib.sha256((repo / receipt).read_bytes()).hexdigest()},
    'receipt_scope': 'Round `evaluator-audit-v2-2026-10-29-round-3` concluded as failed on 2026-11-10. Audit decision 17 keeps the plan\'s strata, sizes, tolerances and selection rule and draws round 4 with the seed `evaluator-audit-v2-2026-10-29-round-4`.',
    'plan': copy.deepcopy(audits['plan']),
    'evidence': [{'path': name, 'sha256': hashlib.sha256((repo / name).read_bytes()).hexdigest()} for name in (
        *(f'bench/grading/current/audit/{old_seed}/{part}.json' for part in ('sample', 'comparison', 'reconciliation', 'conclusion')),
        'bench/grading/rulings/cohort-rebuild-audit.v10.md', 'bench/grading/rulings/cohort-rebuild-audit.v11.md')],
})
audits['plan']['seed'] = new_seed
audits['reason'] = 'The plan is declared. Round evaluator-audit-v2-2026-10-29-round-3 concluded as failed on 2026-11-10, the owner\'s calls on its errors were filed and their batches graded again. Round 4, seed evaluator-audit-v2-2026-10-29-round-4, is drawn: 682 units in 169 batches (audit decision 17). Its second grader and spend are not approved yet, so no agreement rate exists.'

text = json.dumps(audits, indent=2, ensure_ascii=path.read_text().isascii()) + '\n'
if write:
    path.write_text(text)
print(f'{old_seed} closed at {conclusion["concluded_at"]}; seed now {new_seed}')
