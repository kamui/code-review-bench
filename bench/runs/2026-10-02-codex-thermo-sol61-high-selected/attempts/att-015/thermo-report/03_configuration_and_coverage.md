# Configuration contract, coverage, and measurements

## Finding and public contract

This file supports “Define one consistent pool configuration contract” in summary.md. django/db/backends/postgresql/base.py:204–206 treats every falsey value as disabling the pool, before True is converted to an empty dictionary at lines 214–215. docs/ref/databases.txt:255–258 describes dictionary configuration without excluding an empty dictionary. True and {} should represent the same constructor defaults under that contract, but the implementation selects fundamentally different lifetimes.

get_connection_params() removes the pool key at base.py:290 and rejects a truthy value with psycopg2 at lines 291–292. docs/ref/databases.txt:270–271 says that driver ignores the setting. The new test named test_connect_pool_setting_ignored_for_psycopg2 actually expects ImproperlyConfigured at tests.py:348–354. The rejection policy is reasonable, but its documentation and test name must agree with it.

The pool constructor separately rejects CONN_MAX_AGE other than zero at base.py:209–212. The new user-facing section does not mention this compatibility constraint, even though the same database documentation recommends persistent connection settings earlier. Include it where users enable the feature.

Verification used a real DatabaseWrapper and a real unopened ConnectionPool from the cached environment. OPTIONS['pool']={} returned None. Patching the backend's is_psycopg3 flag to False made get_connection_params() raise the stated configuration error. This tests the driver-selection branch, not an actual psycopg2 installation or database session.

## Worked normalization proposal

Normalize at the backend option boundary rather than repeat .get('pool') truthiness in wrappers, feature skips, and parameter handling. A small function can describe the three meaningful states without a policy-object hierarchy:

```python
def pool_options(options):
    value = options.get("pool", False)
    if value is False or value is None:
        return None
    if value is True:
        return {}
    if isinstance(value, dict):
        return value.copy()
    raise ImproperlyConfigured("OPTIONS['pool'] must be False, True, or a dict.")
```

At each canonical boundary use `normalized is not None` to test enabled state; do not fall back to `if normalized`, which would recreate the empty-dictionary problem. NO_DB_ALIAS remains exempt from application pool acquisition. Apply the supported-driver and persistent-connection validations consistently rather than only when an alias is absent from the registry. State whether None is accepted as disabled; the sketch preserves its current behavior. If compatibility requires accepting further forms, define them explicitly rather than treating unrelated falsey objects as options.

This function is justified by a repeated semantic contract; it is not an identity wrapper. It eliminates independent interpretations of the setting. Combine it with the acquisition-only registry API described in 02_pool_lifecycle.md. The sketch is unimplemented and untested; add tests for absent, False, True, {}, a populated dictionary, invalid shapes, psycopg2 rejection, and CONN_MAX_AGE.

## Open question: reserved constructor hooks

ConnectionPool is called with explicit kwargs, open, configure, and check plus **pool_options at base.py:228–234. An options dictionary containing check raises TypeError for duplicate values before constructor entry. The offline probe confirmed this without starting workers. That matters because the documentation currently describes passing a dictionary to the driver constructor without stating reserved keys.

The intended extension contract is not supplied in the packet. This is recorded as a question, not an additional finding asserting that all driver hooks must be supported. If reserved, validate those keys with a backend configuration error and document them. If supported, design composition carefully: user configure must not suppress Django's timezone or role setup, and worker reset exceptions are not synchronously wrapped by Django. Avoid an implicit override policy implemented by dictionary merge order.

## Coverage assessment

The new checkout/reuse/exhaustion test covers a maximum of two raw connections and confirms backend PID reuse. True configuration, timezone setup, health-check callback selection, persistent-connection rejection, driver rejection, and the atomic reconnection guard also receive focused tests. Those are useful additions.

The timezone test manually deletes a cached timezone_name on its unregistered copied wrapper. It checks initial pool configuration, not invalidation of an existing registered pool with an outstanding lease. The health-check test checks pool._check rather than live recovery. The role and isolation-level tests now bypass pooling through no_pool_connection(); direct-driver regression tests should remain, but corresponding pool behavior needs its own coverage. No new creation/keepdb transition tests or empty-dictionary tests were added.

no_pool_connection() at tests.py:24–31 deep-copies settings already deep-copied by BaseDatabaseWrapper.copy(). That redundancy is a minor local cleanup, not a separate actionable finding. The helper earns a test-isolation purpose; renaming it or removing its comment would not fix the production ownership issues. The new feature-specific test skips use an existing canonical backend feature mechanism and are appropriately located, though their enabled predicate must follow the normalized option contract.

The shared atomic guard and is_usable() handling of a missing raw connection support the new release semantics. Neither warrants a cosmetic architectural objection. The release notes and requirement change are appropriately scoped. No separate release-note or dependency finding is supported by the evidence.

## Measurements

The committed diff contains eight files, 325 insertions, and 44 deletions. Counts were produced from git diff --stat main...review-head and git diff --numstat. Line counts use git show for both pinned branch revisions, not an edited working tree:

| File | Base lines | Head lines |
| --- | ---: | ---: |
| django/db/backends/base/base.py | 788 | 792 |
| django/db/backends/postgresql/base.py | 516 | 615 |
| django/db/backends/postgresql/creation.py | 86 | 91 |
| django/db/backends/postgresql/features.py | 149 | 163 |
| docs/ref/databases.txt | 1248 | 1273 |
| docs/releases/5.1.txt | 469 | 472 |
| tests/backends/postgresql/tests.py | 439 | 569 |
| tests/requirements/postgres.txt | 2 | 3 |

No file crosses the frozen skill's below-1,000 to above-1,000 boundary. Extracting configuration and giving the registry an explicit boundary is justified by worker safety and lifecycle ownership, not by an invented size blocker. A large-file split without changing those contracts would merely move complexity around.

## Execution record and verification limits

Checkout identity: main=bcccea3ef31c777b73cba41a6255cd866bf87237; HEAD=review-head=fad334e1a9b54ea1acb8cce02a25934c5acfe99f. git status --porcelain=v1 was empty before review and after the offline probes. git diff main...review-head --check passed. No clone files were edited or bytecode written by the probe; scratch scripts and reports live in clone-work.

The exact offline command was:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-015/clone /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-015/clone-cache/venv/bin/python /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-015/clone-work/offline_review_checks.py
```

Eight probes passed in the final run (0.012 seconds reported by unittest). The first scratch run had three harness assertion errors: unittest.TestCase has no assertRaisesMessage(), and the actual thread diagnostic says “that same thread.” These were corrected in scratch, with no target changes. The final probes assert observed defects, not desired corrected behavior. This is evidence gathering, not a claim that a proposed fix passed tests.

The cached interpreter is Python 3.10, psycopg is 3.3.6, and psycopg_pool is 3.3.3. The diff specifies psycopg-pool>=3.2.0 for tests. Minimum-version behavior and an actual psycopg2 environment were not checked. No server was provisioned, no live database test ran, no dependency was fetched, no network request was made, and no upstream PR discussion or reference review was read.

Reading commands included rg searches for ensure_timezone, compose_sql, setup_worker_connection, closed_in_transaction, and pool-related tests; numbered backend and documentation reads; base connection/creation and transaction reads; test-runner/settings-signal reads; and local installed pool source reads. No ambient guidance, client configuration, memories, or additional skill were loaded. The frozen skill references no required resource files or child-review workflow, so this remained a single primary reviewer.
