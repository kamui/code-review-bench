import json, re

RUN = '/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-002/clone-work/ce-review-artifacts/ce-code-review/20261002-154306-f793a808'
fi = json.load(open('finish-input.json'))
def norm(t): return re.sub(r'\s+', ' ', t).strip().lower()
final = json.load(open('mechanical-findings-final.json'))
pass1 = json.load(open('mechanical-findings.json'))
carry = json.load(open('scratch/merge-carry.json'))
assert final['status'] == 'complete' and not final['malformed_findings'] and not final['pre_existing_findings'] and not final['suppressed_findings']

def sel(evs, prefixes):
    out = []
    for p in prefixes:
        hits = [e for e in evs if e.startswith(p)]
        assert len(hits) >= 1, p
        # longest matching entry wins when several share the prefix
        out.append(max(hits, key=len))
    assert len(set(out)) == len(out)
    return out

# Curated (near-duplicate-free) evidence for the semantic merges; every string is verbatim from a contributing artifact.
src_all = {(e['reviewer'], e['title']): e for e in json.load(open('source-detail-map.json'))}
def ev_of(*keys):
    out = []
    for k in keys:
        out += src_all[(k[0], norm(k[1]))]['evidence']
    return out

A_all = ev_of(('reliability', 'Pool configure hook deadlocks when assume_role is set'),
              ('adversarial', 'Pool plus assume_role: configure hook re-enters wrapper, every connect times out'),
              ('maintainability', "Pool configure hook's no-self rule is comment-only, already broken"))
A_ev = sel(A_all, [
  'django/db/backends/postgresql/base.py:101 --',
  'django/db/backends/postgresql/base.py:381 --',
  'django/db/backends/postgresql/operations.py:192-193 --',
  'django/db/backends/postgresql/psycopg_any.py:20-22 --',
  'django/db/backends/postgresql/base.py:231 -- configure=self._configure_connection,   (run by',
  'django/db/backends/postgresql/base.py:370-373 -- # This function',
  'psycopg_pool/pool.py:634-635 --',
  'Chain: wrapper.connect()',
  'Offline reproduction (fake connection_class passed through',
  'Offline reproduction (scratch/adversarial/exp_a.py',
  'Offline check (mocked, no server)',
  'tests/backends/postgresql/tests.py:402 --',
])
B_all = ev_of(('adversarial', 'Alias-keyed pool survives NAME switch: tests run against the non-test database'),
              ('correctness', 'Per-alias pool keeps stale connection params after settings change'),
              ('reliability', 'Per-alias pool keeps old database after settings change'))
B_ev = sel(B_all, [
  'django/db/backends/postgresql/base.py:208 -- if self.alias not in self._connection_pools:  (followed',
  'django/db/backends/postgresql/base.py:224-229 --',
  'django/db/backends/base/creation.py:64-66 -- self.connection.close(); settings',
  'django/db/backends/base/creation.py:383-384 -- self.connection.settings_dict.update(settings_dict) / self',
  'django/db/backends/postgresql/base.py:529-535 --',
  'django/db/backends/postgresql/creation.py:61 and :90 --',
  'Chain (fallback trigger)',
  'Chain (app-init trigger)',
  'Offline reproduction (scratch/adversarial/exp.py)',
  'Offline check (no server, scratch/correctness/t2.py)',
  'Not confirmed against a live server',
])
C_all = ev_of(('adversarial', 'New atomic guard permanently bricks wrapper after close in non-autocommit atomic'),
              ('correctness', 'Non-autocommit connection cannot reconnect after close inside atomic'))
C_ev = sel(C_all, [
  'django/db/backends/base/base.py:274 -- if self.in_atomic_block and self.closed_in_transaction:  (raises ProgrammingError at',
  'django/db/transaction.py:309-313 -- elif not connection.savepoint_ids and not connection.commit_on_exit: /',
  'django/db/backends/base/base.py:352 --',
  'django/db/backends/base/base.py close():',
  'Reproduced offline on file-backed SQLite (scratch/correctness/t1.py)',
  'Reproduction on file-backed SQLite at HEAD (scratch/adversarial/exp_b.py)',
  'Same script with ensure_connection patched',
])
D_all = ev_of(('api-contract', 'Pool docs say psycopg2 ignores option; code raises'),
              ('maintainability', 'Docs and test name say psycopg2 ignores pool; code raises'),
              ('adversarial', 'Docs say pool is ignored with psycopg2, code raises ImproperlyConfigured'),
              ('correctness', 'Docs say pool is ignored with psycopg2; code raises'),
              ('testing', 'psycopg2 pool test name and docs say ignored; code raises'))
D_ev = sel(D_all, [
  'docs/ref/databases.txt:270-271 -- This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed /',
  'django/db/backends/postgresql/base.py:290-292 -- pool_options = conn_params.pop("pool", None) /',
  'tests/backends/postgresql/tests.py:349-354 -- def test_connect_pool_setting_ignored_for_psycopg2(self):',
  'django/db/backends/postgresql/base.py:209-212 --',
  'docs/ref/databases.txt:294 --',
  'Offline run (is_psycopg3 patched to False, pool=True)',
])
curated = {
  norm('Pool configure hook deadlocks when assume_role is set'): A_ev,
  norm('Alias-keyed pool survives test NAME switch, targets original database'): B_ev,
  norm('Non-autocommit connection cannot reconnect after close inside atomic'): C_ev,
  norm('Docs say pool is ignored with psycopg2; code raises'): D_ev,
}
extra_notes = {
  norm('Empty pool options dict silently disables pooling'):
    'Single-reviewer adversarial P3 kept primary because it quotes the documented dict form (docs/ref/databases.txt:255-258) and an offline check proves the current behavior (pool={} gives wrapper.pool is None). correctness independently noted the same behavior as a residual risk, not as a finding; that note is folded here and adds no confidence.',
  norm('Reserved or non-dict pool options fail with raw TypeError'):
    'Single-reviewer api-contract P3 kept primary: an offline run against the checkout with psycopg_pool 3.3.3 proves the TypeError for check/configure/open/kwargs and for non-dict truthy values. correctness independently noted the reserved-key collision as a residual risk, not as a finding; that note is folded here and adds no confidence.',
  norm('ensure_role() removed; ensure_timezone() override no longer used on connect'):
    'Disagreement kept visible: correctness raised this as a P3 finding; api-contract and maintainability listed the same removal only as a residual risk, and api-contract declined to raise it because database backend internals are private API under Django\'s stability policy and no in-repo consumer exists (only django/test/signals.py:82 calls ensure_timezone(); no in-tree caller or override of ensure_role()). Kept primary at P3 because docs/ref/databases.txt:1204-1208 documents subclassing the built-in backends as supported, which establishes the operating condition; the consequence is limited to third-party subclasses. Its fix path overlaps #1 (both restructure how the role/time zone setup is invoked).',
}

by_num = {f['#']: f for f in final['findings']}
PRIMARY = [1, 2, 3, 4, 10, 11, 12]
DEMOTE = {
  5: ('residual_risks', 'Single-reviewer maintainability P2 (plus a fast-pass duplicate that does not count as agreement): no explicit violated contract and no proven user-facing defect; the consequence is construction side effects and a check-then-act on teardown paths reached from test infrastructure.'),
  6: ('residual_risks', 'Single-reviewer reliability P2: depends on application threads that use the ORM outside the request cycle and never call connection.close(); docs/ref/databases.txt:100-102 already states that a connection created outside the request-response cycle stays open until explicitly closed, so the pool behavior follows the existing documented caveat rather than breaking a repository contract. Kept as a risk because the consequence under pooling (pool exhaustion) is material.'),
  7: ('testing_gaps', 'Single-reviewer testing P2 about test fragility; no failure was observed (no PostgreSQL server).'),
  8: ('testing_gaps', 'Single-reviewer testing P2 absence-of-coverage finding; no umbrella testing finding is kept primary because the regression tests it asks for are already part of the suggested fixes of #1 and #3.'),
  9: ('testing_gaps', 'Single-reviewer testing P2 absence-of-coverage finding; the pooled assume_role test it asks for is already part of the suggested fix of #1.'),
  13: ('residual_risks', 'Single-reviewer maintainability P3: coupling to a third-party private attribute with no current failure.'),
  14: ('testing_gaps', 'Single-reviewer testing P3: test asserts a third-party private attribute; no current failure.'),
}
assert sorted(PRIMARY + list(DEMOTE)) == list(range(1, 15))

def hydrate(f):
    c = carry[norm(f['title'])]
    out = {
      '#': f['#'], 'title': f['title'], 'severity': f['severity'], 'file': f['file'], 'line': f['line'],
      'confidence': f['confidence'], 'autofix_class': f['autofix_class'], 'owner': f['owner'],
      'requires_verification': f['requires_verification'], 'pre_existing': f['pre_existing'],
      'suggested_fix': f.get('suggested_fix'), 'first_evidence': f.get('first_evidence'),
      'why_it_matters': c['why_it_matters'],
      'evidence': curated.get(norm(f['title']), c['evidence']),
      'reviewers': c['reviewers'], 'independent_reviewers': c['independent_reviewers'],
      'source_keys': c['source_keys'],
    }
    note = c['merge_note'] or extra_notes.get(norm(f['title']))
    if note: out['synthesis_note'] = note
    assert out['why_it_matters'] and out['evidence'] and out['first_evidence']
    return out

primary = [hydrate(by_num[n]) for n in PRIMARY]
demoted = []
for n, (bucket, reason) in sorted(DEMOTE.items()):
    h = hydrate(by_num[n]); h['demoted_to'] = bucket; h['demotion_reason'] = reason
    demoted.append(h)

residual_detail = [
 {'text': "Pool exhaustion when a wrapper is dropped without close(): a pooled connection goes back to the pool only through DatabaseWrapper._close() (django/db/backends/postgresql/base.py:395). A thread that uses the ORM outside the request cycle and ends without connection.close() keeps its slot; with 'pool': True the psycopg_pool default is 4 connections and a 30s timeout, so four such threads leave later requests waiting and failing with OperationalError. Without pooling the same pattern only cost one private connection. Shown offline with a fake connection class (max_size=2), not against a live server. The new docs section does not mention it. Reviewer-proposed response: release through weakref.finalize at checkout, or document that every thread must call connection.close().",
  'severity': 'P2', 'from_finding': 6, 'sources': ['reliability (finding)', 'adversarial (residual risk)']},
 {'text': "The pool property is both the 'is pooling on?' test and a get-or-create factory (django/db/backends/postgresql/base.py:203-241). close_pool() at :244 therefore builds a ConnectionPool just to close it, evaluates the property twice, and does a separate `del` on the class-level registry (check-then-act, no lock: two concurrent callers can raise KeyError). ensure_timezone() calls close_pool() first, so the test-only setting_changed receiver (django/test/signals.py:82) tears down the alias-wide pool shared by every thread and can raise the CONN_MAX_AGE / psycopg_pool ImproperlyConfigured errors from a teardown call. Only test infrastructure reaches these paths today. Reviewer-proposed response: `pool = self._connection_pools.pop(self.alias, None)` in close_pool() and a side-effect-free predicate for the branch tests.",
  'severity': 'P2', 'from_finding': 5, 'sources': ['maintainability (finding)', 'fast-pass (candidate)', 'reliability (residual risk)', 'adversarial (residual risk)', 'api-contract (residual risk)']},
 {'text': "Fork inheritance of the class-level DatabaseWrapper._connection_pools was not confirmed either way. If a pool for an alias is open in the parent at fork time (parallel test workers with the fork start method, or a preforking server that queries before forking), the child sees an already-open pool, so pool.open() is a no-op, but the pool's worker threads do not exist in the child and idle sockets are shared with the parent; setup_worker_connection() (django/db/backends/base/creation.py:377-384) rewrites NAME and calls close() without close_pool(). Whether the test-runner parent holds a live pool when workers are forked was not established. Related to #2.",
  'severity': None, 'from_finding': None, 'sources': ['correctness', 'reliability', 'adversarial']},
 {'text': "Fail-fast is lost when the database is down or misconfigured: connection errors (refused connection, bad password, bad time zone, a role that does not exist) happen in pool worker threads and are only logged at WARNING on the 'psycopg.pool' logger; the request thread blocks for the pool timeout (30s by default) and gets a generic 'couldn't get a connection' error instead of the real cause. Tunable through OPTIONS['pool']['timeout'], which the new docs section does not mention.",
  'severity': None, 'from_finding': None, 'sources': ['reliability', 'testing']},
 {'text': "With the default CONN_HEALTH_CHECKS=False the pool is built with check=None (django/db/backends/postgresql/base.py:227-232), so after a server restart, failover or idle drop each stale pooled connection fails one request before psycopg_pool discards it; non-pooled CONN_MAX_AGE=0 never reused connections. The comment at base.py:503 ('The pool only returns healthy connections') holds only when CONN_HEALTH_CHECKS is True, and the new docs do not recommend enabling it.",
  'severity': None, 'from_finding': None, 'sources': ['reliability']},
 {'text': "No minimum psycopg-pool version is documented or enforced: the backend always passes check= and references ConnectionPool.check_connection, which the api-contract reviewer reports exist only in psycopg-pool >= 3.2 (tests/requirements/postgres.txt pins psycopg-pool>=3.2.0; the user docs only say psycopg-pool must be installed). With an older psycopg-pool the first query would likely fail with a raw TypeError/AttributeError rather than ImproperlyConfigured. Not verified offline (only psycopg_pool 3.3.3 is installed).",
  'severity': None, 'from_finding': None, 'sources': ['api-contract']},
 {'text': "Every pooled close goes through `self.connection._pool.putconn(...)` (django/db/backends/postgresql/base.py:392-395), an undocumented private attribute that psycopg_pool sets on the connection; the code comment says it is a workaround so tests can swap pools on setting changes. A psycopg-pool release that renames it breaks every pooled close. Reviewer-proposed response: remember the originating pool on the wrapper at checkout and return the connection through that reference.",
  'severity': 'P3', 'from_finding': 13, 'sources': ['maintainability (finding)']},
]
testing_detail = [
 {'text': "No test opens a pooled connection with OPTIONS['assume_role']: test_connect_role (tests/backends/postgresql/tests.py:402) was switched to no_pool_connection(), which is what hides #1. Add a psycopg3-only pooled test (SET ROLE to the login role, then assert SHOW ROLE), and a bad-role case asserting the exception type under pooling, where the role error surfaces as a pool timeout rather than InvalidParameterValue.",
  'severity': 'P2', 'from_finding': 9, 'sources': ['testing (finding)', 'reliability', 'adversarial']},
 {'text': "The new ensure_connection() guard is tested only by setting in_atomic_block and closed_in_transaction by hand on a wrapper that never connected (tests/backends/postgresql/tests.py:334-337), under a psycopg3 skip. No test closes a pooled connection inside a real atomic block (so the test would still pass if _close() stopped clearing self.connection after putconn()), and no backend-agnostic test covers the shared base guard. Related to #3.",
  'severity': 'P2', 'from_finding': 8, 'sources': ['testing (finding)', 'correctness', 'reliability', 'adversarial']},
 {'text': "No test asserts that a pooled alias connects to the new database after settings_dict['NAME'] changes (create_test_db, setup_worker_connection, set_as_test_mirror), that the _nodb_cursor() fallback wrapper does not register a pool under the real alias, or that the close_pool() calls added to _clone_test_db() and _destroy_test_db() run. Related to #2.",
  'severity': None, 'from_finding': None, 'sources': ['correctness', 'reliability', 'adversarial', 'testing']},
 {'text': "test_connect_pool (tests/backends/postgresql/tests.py:242-244) uses min_size 0, max_size 2 and timeout 0.1, so each of the first two acquisitions must open, authenticate and configure a new PostgreSQL connection inside the same 100ms budget that is used to prove exhaustion; on a loaded or remote-database CI host it can fail with PoolTimeout before reaching its assertion. Pre-warm the pool (min_size 2, pool.open(wait=True)) or raise the timeout. Not observed (no server).",
  'severity': 'P2', 'from_finding': 7, 'sources': ['testing (finding)']},
 {'text': "The pooled path is not exercised for per-connection options: eleven existing connection tests (isolation_level, assume_role, server_side_binding, cursor_factory, client_encoding, non-autocommit) were switched to no_pool_connection(), no in-tree test settings enable OPTIONS['pool'], and only the time zone has a pooled test, so the conditional django_test_skips, the _close() stale-pool workaround and test database clone/destroy under pooling run only with an external pool-enabled settings file.",
  'severity': None, 'from_finding': None, 'sources': ['testing', 'maintainability']},
 {'text': "ensure_timezone() now calls close_pool() on every TIME_ZONE/USE_TZ setting change (django/test/signals.py:82); no test covers closing the pool while a connection is still checked out and then returning it to the stale pool through connection._pool.putconn() in _close(), that the next connection comes from a fresh pool with the new time zone, or calling close_pool()/ensure_timezone() on a wrapper whose pool was never created.",
  'severity': None, 'from_finding': None, 'sources': ['testing', 'maintainability']},
 {'text': "test_pooling_health_checks asserts psycopg_pool's private ConnectionPool._check (tests/backends/postgresql/tests.py:318, :324) and never opens the pool, so it does not prove a pooled connection is checked on checkout; the pooled early return in close_if_health_check_failed() has no test, and the two base health-check tests are skipped under pooling. Patch ConnectionPool.check_connection and assert the call instead.",
  'severity': 'P3', 'from_finding': 14, 'sources': ['testing (finding)']},
]
rejected = [
 {'claim': 'Custom backends that override get_connection_params() to mint short-lived credentials are evaluated once at pool creation (reliability residual risk).', 'reason': 'Assumed configuration; the reviewer states no such backend exists in this repository.'},
 {'claim': "A test that leaks a pool under the shared alias 'default_pool' makes later tests reuse it (testing residual risk).", 'reason': 'No leak path was shown; every pool-creating test in the diff closes its pool in a finally block.'},
 {'claim': 'Error type after connection.close() inside atomic() differs between pooled (ProgrammingError) and non-pooled (driver InterfaceError) connections (api-contract residual risk).', 'reason': 'The new guard is the intended behavior per the intent summary; no significant consequence established.'},
 {'claim': 'psycopg_pool ImportError branch and the is_usable() None guard have no tests (testing gaps).', 'reason': 'Completeness requests without evidence of a problem.'},
 {'claim': 'No test covers pool exhaustion recovery after a discarded wrapper or a broken connection returned through _close() (reliability testing gap).', 'reason': 'Narrower case of the pool-exhaustion residual risk; adds no consequential scenario beyond it.'},
]
folded = [
 "Four 'no PostgreSQL server was available' statements (correctness, testing, maintainability, adversarial residual risks) are verification limits, not risks; they are carried in Coverage.",
 "correctness residual risks about OPTIONS['pool'] = {} and reserved pool keys are covered by primary findings #10 and #11.",
 "maintainability and api-contract residual risks about the removed ensure_role() method are covered by primary finding #12.",
 "api-contract residual risk about ensure_timezone() closing the alias-wide pool and reliability/adversarial residual risks about the close_pool() check-then-act are folded into the pool-property residual risk.",
 "adversarial residual risk about pool slots released only by close() is folded into the pool-exhaustion residual risk.",
 "Testing gaps asking for a non-autocommit close-inside-atomic regression test (correctness, adversarial) are part of the suggested fix of #3.",
]

triage_groups = [
 {'title': 'Per-connection setup hook: role and time zone on pooled connections', 'findings': [1, 12], 'kind': 'decision-gate',
  'context': '#1 and #12 both come from moving the role/time zone setup out of wrapper methods into module-level functions that the pool configure hook calls through self.ops.',
  'preferred_resolution': 'Handle #1 first: compose SET ROLE on the raw connection passed to the hook so it no longer reaches the Django wrapper. Decide in the same edit whether the setup steps become overridable methods that take the raw connection (resolves #12) or stay module-level functions; both findings are routed manual/gated with verification, so stop for that choice before applying #12.',
  'why': 'One restructuring of ensure_role()/ensure_timezone()/_configure_connection() resolves the P1 deadlock and the removed-hook regression together; fixing them separately would rewrite the same lines twice.'},
 {'title': "OPTIONS['pool'] contract: documentation and option validation", 'findings': [4, 10, 11], 'kind': 'apply-queue',
  'context': "The documented description of OPTIONS['pool'] and what the pool property accepts disagree in three places: psycopg2 handling, an empty dict, and reserved or non-dict values.",
  'preferred_resolution': "Fix the docs sentence and test name first (#4, no verification needed), then normalize and validate the option value once in DatabaseWrapper.pool: treat only None/False as off (#10) and raise ImproperlyConfigured for non-dict values and reserved keys (#11). #10 and #11 edit the same block (base.py:204-233) and should land together with tests.",
  'why': 'All three are mechanical, localized changes to one documented setting; doing them together keeps the documented contract and the enforced one aligned.'},
]

mechanics = {
  'pass_1': {'input_returns': 7, 'input_candidates': 25, 'findings': len(pass1['findings']), 'pre_existing_findings': len(pass1['pre_existing_findings']),
             'suppressed_findings': len(pass1['suppressed_findings']), 'suppressed_by_confidence': pass1['suppressed_by_confidence'],
             'malformed_findings': len(pass1['malformed_findings']) if isinstance(pass1['malformed_findings'], list) else pass1['malformed_findings'],
             'malformed_returns': len(pass1['malformed_returns']) if isinstance(pass1['malformed_returns'], list) else pass1['malformed_returns'],
             'first_evidence_backfilled': pass1['first_evidence_backfilled'], 'output': RUN + '/mechanical-findings.json', 'input': RUN + '/helper-input-pass1.json'},
  'pass_2_final': {'input_candidates': 14, 'findings': len(final['findings']), 'pre_existing_findings': 0, 'suppressed_findings': 0,
                   'suppressed_by_confidence': final['suppressed_by_confidence'], 'malformed_findings': 0, 'malformed_returns': 0,
                   'first_evidence_backfilled': final['first_evidence_backfilled'], 'output': RUN + '/mechanical-findings-final.json', 'input': RUN + '/helper-input-pass2.json'},
  'suppressed_by_confidence': pass1['suppressed_by_confidence'],
  'first_evidence_backfilled': pass1['first_evidence_backfilled'] + final['first_evidence_backfilled'],
  'quote_gate_demotions': 0,
  'semantic_merges': 5, 'candidates_after_reconciliation': 14,
  'soft_bucket_demotions': {'total': len(DEMOTE), 'residual_risks': sum(1 for b, _ in DEMOTE.values() if b == 'residual_risks'), 'testing_gaps': sum(1 for b, _ in DEMOTE.values() if b == 'testing_gaps')},
  'settled_conflicts_discarded': 0, 'hydration_dropped': 0,
  'numbering_note': 'Stable # values come from the final helper pass over all 14 reconciled candidates; the seven demoted candidates keep their numbers (5, 6, 7, 8, 9, 13, 14), so the primary set is 1, 2, 3, 4, 10, 11, 12.',
}

coverage = [
  'Stage 5 mechanics: 7 compact returns (6 personas plus fast-pass) carried 25 candidates; the helper kept 23 findings, suppressed 2 at anchor 50 (both fast-pass candidates), and reported 0 malformed findings, 0 malformed returns and 0 pre-existing findings.',
  'Quote-the-line check: 0 findings at anchor 75/100 were demoted for a missing first_evidence; first_evidence_backfilled is 0.',
  'Semantic reconciliation merged 25 candidates into 14: pool + assume_role deadlock (reliability, adversarial, maintainability -> #1), stale alias-keyed pool (adversarial, correctness, reliability -> #2), non-autocommit atomic guard (adversarial, correctness -> #3), psycopg2 docs/test wording (api-contract, maintainability, adversarial, correctness, testing, fast-pass -> #4), and the pool-property side effect (maintainability, fast-pass -> #5). Both anchor-50 fast-pass candidates were duplicates of persona findings and were folded into them; fast-pass does not count as agreement.',
  'No confidence was raised by agreement: every reviewer ran in-process on one serving model and there is no adversarial-<provider> artifact with independence_verified: true.',
  'Reviewer disagreements kept visible: #2 severity P1 (correctness, adversarial) vs P2 (reliability), P1 kept; #4 severity P2 (api-contract, maintainability) vs P3 (correctness, adversarial, testing, fast-pass), P2 kept; #1 and #3 route gated_auto vs manual, the more cautious manual kept; #12 raised as a finding by correctness while api-contract declined to raise it (backend internals are private API), kept primary at P3 with that dissent recorded.',
  'Soft-bucket demotion (mode:agent, report-only): 7 single-reviewer P2/P3 candidates left the primary set before validation, 3 to residual_risks (#5 pool property side effects, #6 pool exhaustion on unclosed wrapper, #13 private _pool attribute) and 4 to testing_gaps (#7 100ms timing budget, #8 pooled close inside atomic untested, #9 pooled assume_role untested, #14 private _check assertion). 7 primary findings remain: 2 P1, 2 P2, 3 P3.',
  'Persona-supplied soft items: 22 residual risks and 17 testing gaps were deduplicated with the demoted findings into 7 residual risks and 7 testing gaps; 3 residual-risk claims and 3 testing-gap claims were rejected for no established consequence, and the rest were folded into primary findings, other soft entries, or Coverage.',
  'No testing-only umbrella finding is primary: the regression tests the testing reviewer asked for are already part of the suggested fixes of #1 and #3, and the remaining coverage gaps are listed under testing_gaps.',
  'Settlement suppression was not evaluated: no plan document was discovered.',
  'Detail hydration: all 7 primary findings carry why_it_matters and evidence from their source artifacts; 0 candidates were dropped as malformed.',
  'Verification limits: no reviewer ran against a live PostgreSQL server. #1 and #3 were reproduced offline (fake connection class with real psycopg_pool 3.3.3; file-backed SQLite against head and base). #2 rests on code tracing plus a property-level offline check; its end-to-end effect on a real test run was not observed.',
  'Protected artifacts: no finding recommends deleting, removing or gitignoring a file under docs/plans, docs/solutions or docs/brainstorms.',
  'Stage 5b selection: 0 findings skipped validation through the cross-model shortcut (it requires an ordinary reviewer plus an adversarial-<provider> reviewer with independence_verified: true; none exists in this run). All 7 primary findings (2 P1 and 5 actionable P2/P3) are selected into one validator batch, within the normal cap of eight.',
  fi['peer']['coverage'],
] + fi['coverage_notes']

synth = {
  'run_id': fi['run_id'], 'run_dir': RUN, 'stage': 'merge (Stage 5, Stage 5b steps 1-3)',
  'mode': fi['mode'],
  'findings': primary,
  'pre_existing_findings': [],
  'actionable_queue': [f['#'] for f in primary if f['autofix_class'] in ('gated_auto', 'manual') and f['owner'] == 'downstream-resolver'],
  'report_only_queue': [f['#'] for f in primary if not (f['autofix_class'] in ('gated_auto', 'manual') and f['owner'] == 'downstream-resolver')],
  'triage_groups': triage_groups,
  'ungrouped_findings': [2, 3],
  'residual_risks': [r['text'] for r in residual_detail],
  'testing_gaps': [t['text'] for t in testing_detail],
  'soft_buckets': {'residual_risks': residual_detail, 'testing_gaps': testing_detail},
  'demoted_findings': demoted,
  'internal_decisions': {'rejected_claims': rejected, 'folded_claims': folded},
  'learnings': [], 'agent_native_gaps': [], 'deployment_notes': [],
  'requirements_completeness': None,
  'fold_in': {'peer_outcome': fi['peer']['outcome'], 'peer_artifact': fi['peer']['artifact'], 'peer_coverage': fi['peer']['coverage'],
              'note': 'No adversarial-<provider> artifact to fold; the in-process adversarial return was already in raw-returns.json and was merged as reviewer adversarial with no agreement promotion.'},
  'mechanics': mechanics,
  'validation_selection': {'shortcut_skipped': 0, 'shortcut_skip_basis': 'none: no finding has an adversarial-<provider> reviewer with independence_verified: true (peer outcome in-process-fallback); same-model corroboration never licenses the shortcut',
                           'selected': PRIMARY, 'batches': 1, 'validator_input': RUN + '/validator-input.json'},
  'coverage': coverage,
  'failed_reviewers': fi['collection']['failed_reviewers'], 'bound_exceeded': fi['collection']['bound_exceeded'],
}
json.dump(synth, open('synthesized-findings.json', 'w'), indent=1)

vfields = ('#', 'title', 'severity', 'file', 'line', 'confidence', 'autofix_class', 'owner', 'requires_verification', 'pre_existing',
           'suggested_fix', 'first_evidence', 'why_it_matters', 'evidence', 'reviewers', 'independent_reviewers')
order = sorted(primary, key=lambda f: (f['severity'], f['#']))
vin = {
  'run_id': fi['run_id'], 'run_dir': RUN,
  'template': fi['skill_dir'] + '/references/validator-batch-template.md',
  'verdicts_path': RUN + '/validator-verdicts.json',
  'batch': {'count': len(order), 'order': [f['#'] for f in order], 'ordering': 'severity, then stable #', 'normal_cap': 8, 'cap_expanded': False, 'batches': 1},
  'findings': [{k: f[k] for k in vfields} for f in order],
  'skipped': {'count': 0, 'findings': [],
              'evidence_basis': 'No finding qualifies for the cross-model shortcut: it requires first_evidence plus an ordinary reviewer and an adversarial-<provider> reviewer whose artifact records independence_verified: true. The peer outcome is in-process-fallback, there is no adversarial-<provider> artifact, and same-model corroboration never licenses a skip.'},
  'diff': fi['scope']['diff'], 'files': fi['scope']['files'],
  'scope_context': {
    'mode': fi['scope']['mode'], 'base': fi['scope']['base'], 'diff_a': fi['scope']['diff_a'], 'diff_b': fi['scope']['diff_b'],
    'branch': fi['scope']['branch'], 'head_sha': fi['scope']['head_sha'], 'tree_is_reviewed_head': fi['scope']['tree_is_reviewed_head'],
    'repo_path': fi['scope']['repo_path'], 'remote_refs': None, 'pr': fi['scope']['pr'], 'untracked_excluded': fi['scope']['untracked_excluded'],
    'summary': 'standalone scope: explicit base bcccea3ef31c777b73cba41a6255cd866bf87237 on the current checkout; the working tree at ' + fi['scope']['repo_path'] + ' is the reviewed head fad334e1a9b54ea1acb8cce02a25934c5acfe99f (branch review-head), so cited files, callers and guards are inspected in that checkout with read-only tools. No remote refs; no PR head ref was fetched.',
    'intent': fi['intent'],
    'constraints': fi['invocation']['constraints'],
  },
}
json.dump(vin, open('validator-input.json', 'w'), indent=1)
print('primary', [f['#'] for f in primary], 'selected', vin['batch']['order'], 'residual', len(residual_detail), 'testing', len(testing_detail))
for f in primary: print(f['#'], f['severity'], f['confidence'], f['autofix_class'], f['file'], f['line'], '|', f['title'], '| ev', len(f['evidence']), f['reviewers'], f['independent_reviewers'])
