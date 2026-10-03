# Shared wrapper contract, tests, documentation, and measurements

This subsystem covers the generic reconnect guard, PostgreSQL feature skips,
test changes, and published configuration contract. Its actionable finding is
the psycopg2 documentation mismatch, carried into `summary.md`. Test gaps below
are verification requirements for the other findings, not additional findings.

## Evidence: documentation promises a fallback the implementation rejects

`docs/ref/databases.txt:270–271` says the new pool option is ignored with
psycopg2. The new branch at `django/db/backends/postgresql/base.py:290–292`
instead raises `ImproperlyConfigured('Database pooling requires psycopg >= 3')`
for enabled pooling when `is_psycopg3` is false.

`tests/backends/postgresql/tests.py:349–354` adds
`test_connect_pool_setting_ignored_for_psycopg2`, but despite its name this test
expects the exception from `connect()`. Both implementation and assertion
establish rejection, so changing the documentation to describe rejection is
the most direct remedy. Do not silently remove validation to match a sentence
unless that fallback is deliberately intended and tested.

An offline probe patches the imported `is_psycopg3` flag only for the call to
`get_connection_params()`. With `pool=True`, the exact documented configuration
raises that exception before any connection attempt. This verifies the branch,
not a full psycopg2 driver installation or live connection.

## Worked documentation and test remedy

Replace the ignored-driver sentence with wording such as:

> This option requires psycopg 3 and psycopg[pool] or psycopg-pool to be
> installed. Enabling pooling with psycopg2 raises ImproperlyConfigured.

Use the documentation's existing markup conventions in the final edit. Rename
the test to describe the psycopg2 rejection. Document the incompatible
`CONN_MAX_AGE` configuration near the new option while aligning the option
contract, since the constructor deliberately rejects persistent wrappers. The
missing explanatory sentence is remediation context, not an independent finding.

## Shared reconnect guard: appropriate canonical placement

`django/db/backends/base/base.py:274–278` adds a guard when a wrapper has no
connection but remains in an atomic block closed in transaction. This is a
generic wrapper invariant, not PostgreSQL-specific feature logic leaking into
a shared layer. The pooled `_close()` returns a connection and clears the
wrapper's pointer, so the old closed-raw-connection behavior can no longer
prevent a replacement connection from being opened implicitly.

The new guard prevents `connect()` from resetting transaction state halfway
through an atomic block. `Atomic.__exit__()` already uses
`closed_in_transaction` to wait for the outermost exit before restoring
connection usability. The `is_usable()` addition at PostgreSQL base.py:489–491
also correctly handles the newly possible detached state. Neither change is
an unnecessary wrapper or structural regression on the inspected evidence.

The PR's new test at tests.py:329–337 manually sets both state flags and
asserts `ProgrammingError`. It checks the guard, but does not exercise a real
pooled checkout, close, nested atomic exit, and subsequent reconnect. That
sequence should be covered when implementing the ownership remedy.

## Feature skips and existing tests

Converting `django_test_skips` to a cached property in PostgreSQL features.py
allows pool-dependent skips in the backend's existing test capability layer.
That is a canonical location for backend skip policy. Normalized option
enablement must match pool creation, as explained in the configuration detail.
The skips do not themselves prove that pooled health checks meet the old
wrapper contract, but the pool check callback is deliberately enabled by
`CONN_HEALTH_CHECKS` and the new test inspects its presence.

`no_pool_connection()` at tests.py:24–30 changes a copied wrapper to bypass
pooling. The switches cover existing timezone rollback, autocommit, isolation
level, role error, binding, encoding, and cursor tests. This is reasonable for
tests that intentionally vary same-alias connection parameters, but it means
those tests cannot validate the pooled equivalents. In particular, the role
test at 395–409 no longer catches worker configuration re-entry. The new tests
cover basic exhaustion/reuse, `True`, timezone initialization, health-check
callback presence, the reconnect guard, persistent-connection rejection, and
driver rejection; they omit the lifecycle transitions reproduced here.

The redundant settings copy in the test helper and minor wording issues are
not additional review findings. The demanding review bar should focus on the
missing owner/configuration model and its observable consequences.

## Native test execution

The focused native run used SQLite and an in-memory test database:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=<clone>:<clone>/tests TMPDIR=<attempt>/tmp \
  <cache>/venv/bin/python tests/runtests.py \
  backends.base.test_base.DatabaseWrapperTests transactions.tests.AtomicTests \
  --settings=test_sqlite --parallel=1 --verbosity=1
```

Result: 26 tests passed, no system-check issues, in 0.027 seconds of test time.
The command completed in under one second. The in-memory database was destroyed
normally. The retained log is `<work>/native-tests.log`.

The separate offline script ran nine evidence probes successfully in 0.464
seconds. It is retained as `<work>/offline_review_probes.py`, with output in
`<work>/offline-probes.log`. The script uses explicit settings and no ambient
guidance. Pools are shut down in cleanup, and bytecode writes are disabled.

All commands completed well within the five-minute allowance. No dependencies
were fetched, no upstream pages were accessed, and no live PostgreSQL tests
were attempted because a server is unavailable. These runs cannot validate
role quoting on a server, timezone SQL persistence, actual connection reset,
or process-fork behavior. The installed dependency versions exceed the minimum
versions declared by the patch; testing the minimum remains future validation.

## Measurements and decomposition judgment

The committed diff touches eight files with 325 insertions and 44 deletions.
`git diff --numstat main...review-head` produced those counts. A Python script
compared `git show main:<path>` with each head file's `splitlines()` and used
the AST to count wrapper methods and `self.pool` references.

| File | Base lines | Head lines |
| --- | ---: | ---: |
| django/db/backends/base/base.py | 788 | 792 |
| django/db/backends/postgresql/base.py | 516 | 615 |
| django/db/backends/postgresql/creation.py | 86 | 91 |
| django/db/backends/postgresql/features.py | 149 | 163 |
| docs/ref/databases.txt | 1,248 | 1,273 |
| docs/releases/5.1.txt | 469 | 472 |
| tests/backends/postgresql/tests.py | 439 | 569 |
| tests/requirements/postgres.txt | 2 | 3 |

The PostgreSQL wrapper grows from 15 to 19 methods and gains eight `self.pool`
reads, across acquisition, release, initialization, and health checking. This
is supporting evidence of scattered interpretation, not a numerical complexity
threshold or an independent finding. The concrete failures identify which
dependencies should disappear.

No file crosses from below 1,000 lines to above 1,000. The database reference
was already large and the new cohesive section does not justify demanding a
document split. The backend can stay in its existing file if the ownership and
initializer contracts are made direct; extracting a large generic pool manager
would add indirection without necessarily reducing the concepts involved.

The release note accurately advertises the feature. The optional test
dependency addition is consistent with the feature and health-check API use.
These portions yield no separate actionable findings.

## Checkout integrity and scope

The pinned head is `fad334e1a9b54ea1acb8cce02a25934c5acfe99f` and the pinned base
is `bcccea3ef31c777b73cba41a6255cd866bf87237`. `git status --short` was empty
before and after execution, and `git diff --check main...review-head` passed.
The initial index manifest is retained in `<work>/tree-before.txt` for the
final integrity comparison. No remedies were applied and no clone files were
created. The report uses only the task, frozen skill, diff, relevant local
source, and test execution evidence. There was no cross-model review or child
review context.
