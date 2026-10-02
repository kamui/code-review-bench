# Thermo-nuclear code quality review — django/django#17914

"Refs #33497 -- Added connection pool support for PostgreSQL."
Range `bcccea3ef3..fad334e1a9` (`git diff main...review-head`), 8 files, +325 / -44.

## Verdict

Not approvable as it stands. The feature is small and the diff is readable, but
the pool is bolted onto the wrapper at the wrong seam: it is cached per alias
while everything it is built from lives in a `settings_dict` that Django
mutates in place. Most of the remaining complexity in the PR is compensation
for that one decision: hand-placed `close_pool()` calls, a "workaround for
tests" in `_close()`, a new branch in the shared `BaseDatabaseWrapper`, a
`None` guard that contradicts the base contract, a test helper its own comment
calls a hack, and two test skips. There is a visible restructuring that deletes
most of it: derive the pool from the connection parameters `connect()` already
computes, and keep the base class's "closed in transaction" invariant instead
of adding a new state to it.

No file crosses the 1000-line threshold (`postgresql/base.py` 516 → 615,
`base/base.py` 788 → 792, the test module 439 → 569).

Six findings and two questions follow, most severe first. One finding is a
documentation error that contradicts the code; the other five are structural.

## Findings

### 1. The pool is cached by alias but built from mutable settings, so it goes stale

`django/db/backends/postgresql/base.py:200-241` stores one `ConnectionPool` per alias in a class-level dict and freezes `get_connection_params()` into it on first access; `get_new_connection()` at lines 345-350 then ignores the `conn_params` it is handed and checks out from that pool. Django's own lifecycle depends on closing the connection, editing `settings_dict` in place and reconnecting: `create_test_db()` swaps `NAME` to the test database that way, and `destroy_test_db()`, `set_as_test_mirror()` and `setup_worker_connection()` also edit `settings_dict` in place. The PR patches three such sites by hand (`ensure_timezone()` at `postgresql/base.py:362-364`, `postgresql/creation.py:61` and `postgresql/creation.py:89-91`) and leaves the rest. An offline probe confirms the mechanism: after the same `NAME` swap `create_test_db()` performs, `get_connection_params()` reports the test database while `connection.pool.kwargs["dbname"]` is still the original one, and it is the same pool object. The practical consequence is that if anything in the process queried the alias before the test database is set up (a query in `AppConfig.ready()` under `manage.py test` is the obvious case), migrations and tests check connections out of a pool that still points at the non-test database. That end-to-end path was not run, because no PostgreSQL server is available; the stale-pool behaviour it rests on was measured. The fix is to make the pool a function of the parameters rather than of the alias: keep one pool per alias, store the spec it was built from (connection parameters, time zone name, role, health-check flag, pool options), and rebuild it when a connect arrives with a different spec. That removes `close_pool()` from `ensure_timezone()`, removes the need to remember invalidation at every settings mutation, and stops discarding `conn_params`. Evidence and a worked sketch are in `01_pool_ownership_and_lifecycle.md`, "Finding 1".

### 2. The `pool` property is a predicate, a lazy factory and a config validator at once

`self.pool` is read eight times across five methods in `django/db/backends/postgresql/base.py` (lines 244-245, 345-348, 391, 404 and 502), and in every one of those methods the first read is a boolean test: "is pooling on?" is being asked through a getter that can import `psycopg_pool`, call `get_connection_params()`, raise `ImproperlyConfigured`, construct a pool and mutate process-wide state. The probes show what that costs. `close_pool()` on an alias with no pool constructs one in order to close it. `close_pool()` and `ensure_timezone()` raise "Pooling doesn't support persistent connections" from teardown and from a `setting_changed` receiver. The same `CONN_MAX_AGE` check is skipped entirely once a pool exists for the alias. `_close()` decides with `self.pool` but acts on `self.connection._pool`, so a connection that did not come from a pool raises `AttributeError` instead of being closed. The `True | dict` option is splatted into a call that already passes `kwargs`, `open`, `configure` and `check`, so a user supplying any of those gets a bare `TypeError` from inside a property. Validation also lives in two non-canonical places (the property at lines 209-212 and `get_connection_params()` at lines 290-292), while `BaseDatabaseWrapper.check_settings()` exists for exactly this and is the first thing `connect()` calls. Split the roles: a pure `_pool_options` accessor that returns `None` or a dict, a `check_settings()` override for the three configuration errors, pool construction only inside `get_new_connection()`, `close_pool()` as `pop` then `close`, and `_close()` dispatching on where the connection came from. Evidence and a worked sketch are in `01_pool_ownership_and_lifecycle.md`, "Finding 2".

### 3. A pool-only state leaks into `BaseDatabaseWrapper.ensure_connection()`

Before this PR, closing inside an atomic block left the wrapper holding a closed driver handle until `Atomic.__exit__` cleared it, so `self.connection is None` never coincided with `closed_in_transaction`. The pooled branch of `_close()` at `django/db/backends/postgresql/base.py:385-399` sets `self.connection = None` itself, which creates that combination, and the PR then compensates in three places: a new branch in the shared `ensure_connection()` at `django/db/backends/base/base.py:274-277` that is unreachable for every non-pooled backend, an `if self.connection is None` guard in `is_usable()` at `postgresql/base.py:490-491` that contradicts the base docstring ("may assume that self.connection is not None"), and an unexplained `self.connection is not None` guard in `init_connection_state()` at line 404. The error a user sees after closing inside `atomic()` now also differs between pooled and non-pooled connections. This is feature logic leaking into a shared path. There is a move that keeps the base invariant instead: when a pooled connection is closed inside an atomic block, really close it and then return it, because `psycopg_pool` discards a returned connection that is closed and opens a replacement. The wrapper then holds a dead handle exactly as it does without a pool, and the `base/base.py` change, the `ProgrammingError` import and the hand-set-flags test can all be deleted. The proposal is from reading the `psycopg_pool` source and was not run against a server; the `is_usable()` guard has to stay because an existing test calls `_close()` directly. Details are in `02_connection_state_machine.md`, "Finding 3".

### 4. The configuration callback is half-extracted and pins a per-thread wrapper in a process-wide pool

`ensure_timezone` and `ensure_role` were moved to module level (`django/db/backends/postgresql/base.py:89-104`) so they would not depend on a wrapper, but the pool is still given a bound method, `configure=self._configure_connection`, at line 231. The pool is shared by the whole process and wrappers are per thread, so the pool keeps alive, and calls into from its worker threads, whichever wrapper happened to create it; a probe confirms the wrapper of a finished thread stays reachable through `pool._configure`. The only thing protecting that is a comment at lines 370-373 asking future authors not to touch anything on `self` "aside from variables", while the method reads `self.ops`, `self.settings_dict` and the lazily computed `self.timezone_name`. Meanwhile the method `DatabaseWrapper.ensure_timezone()` at lines 362-367 now shares a name with the module function, and gained a side effect its name and base docstring do not suggest: it tears down the alias's pool for every thread, on every `TIME_ZONE` or `USE_TZ` override in a test run. `DatabaseWrapper.ensure_role()` was removed without comment, and `_configure_connection()` returns a commit flag that one of its two callers ignores. Collapse this to one wrapper-free `configure_connection(connection, *, ops, timezone_name, role_name)` that `init_connection_state()` calls directly and the pool receives through `functools.partial`, and let `ensure_timezone()` go back to only setting the time zone once the pool is keyed on its spec. Details are in `02_connection_state_machine.md`, "Finding 4".

### 5. The `close_if_health_check_failed()` override appears to be dead, and it costs two test skips

`django/db/backends/postgresql/base.py:501-505` short-circuits the health check when pooling is on, and `django/db/backends/postgresql/features.py:86-108` turns `django_test_skips` from a class attribute into a `cached_property` that reads the settings so two `ConnectionHealthChecksTests` can be skipped. Under the only configuration pooling accepts, `CONN_MAX_AGE == 0`, the base method already returns at its first guard: the single place that clears `health_check_done` is `close_if_unusable_or_obsolete()`, which then always closes the connection because `close_at` has already passed, and the pooled `_close()` leaves `self.connection` as `None`. So the override changes no outcome; it only adds a `self.pool` evaluation to every `_cursor()` and `set_autocommit()` call for every PostgreSQL user (about 100-140 ns measured). The two skipped tests reach the overridden path only because their helper forces `CONN_MAX_AGE=None` on a live wrapper, which pooling rejects at creation. Delete the override, re-run those two tests with a pool, and either restore the plain class attribute or reword the skip reason to say what actually conflicts. This is reasoning from the code; the tests could not be run here. Details are in `02_connection_state_machine.md`, "Finding 5".

### 6. The tests route around the design, and the docs say "ignored" where the code raises

`tests/backends/postgresql/tests.py:24-30` adds `no_pool_connection()`, whose own comment calls it "kind of a hack", and substitutes it for `connection.copy()` in eleven pre-existing tests that have nothing to do with pooling, because a copy that shares the alias would otherwise silently get the alias's pool and ignore the option under test. When the suite runs with `"pool"` enabled, the role, isolation-level, cursor-factory, server-side-binding and client-encoding options therefore have no pooled coverage. The six new pool tests share the alias `"default_pool"` and a process-wide dict; `test_pooling_not_support_persistent_connections` is only correct while no earlier test leaks a pool for that alias, since the check is skipped for an existing pool. `test_cannot_open_new_connection_in_atomic_block` sets two flags by hand on a wrapper that never connects, so it asserts the guard but not the scenario that motivated it, and `test_pooling_health_checks` asserts on the private `pool._check`. Separately, `docs/ref/databases.txt:270-271` says the option "is ignored with ``psycopg2``" and the test at `tests/backends/postgresql/tests.py:348-354` is named `test_connect_pool_setting_ignored_for_psycopg2`, while `django/db/backends/postgresql/base.py:290-292` raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")` and the test asserts exactly that. Correct the documentation and the test name, document the `CONN_MAX_AGE` requirement beside it, and once the pool is keyed on its parameters revert the eleven substitutions and give the pool tests a helper that registers cleanup. Details are in `03_tests_and_docs.md`, "Finding 6".

## Questions

### Q1. What happens to the pool in forked parallel test workers?

`_connection_pools` is a class attribute, so a worker forked by the parallel test runner inherits whatever pool the parent has for an alias, including sockets shared with the parent and without the pool's worker threads. `setup_worker_connection()` in `django/db/backends/base/creation.py:377-384` then points the alias at `test_<db>_<n>` and closes the wrapper's connection, but nothing discards the pool. `_clone_test_db()` closes the pool in the parent at `django/db/backends/postgresql/creation.py:61`; is it guaranteed that nothing in the parent reconnects on that alias between the last clone and the fork (system checks, a second alias's migrations, `--debug-sql`)? If not, should `setup_worker_connection()` be the place that drops the pool? I could not establish this either way without a server. Context is in `01_pool_ownership_and_lifecycle.md`, "Consequence, and what it depends on".

### Q2. Is a checked-out connection ever returned if its wrapper is never closed?

Without a pool, a thread that uses the ORM and exits without `connections.close_all()` loses its connection to garbage collection and nothing else is affected. With a pool, the only return path is `_close()` at `django/db/backends/postgresql/base.py:385-399`; a wrapper that is collected while holding a connection never calls `putconn()`, and as far as I can read `psycopg_pool` it does not reclaim that slot. Is the intended contract that every thread must close its connections, and should the documentation in `docs/ref/databases.txt:250-271` say so? A related narrow case: if `connection.isolation_level = ...` at `postgresql/base.py:351-352` raises after `getconn()`, the connection is not yet assigned to the wrapper and is not returned.

## Proposed remediation sequence

1. Correct `docs/ref/databases.txt` and rename the psycopg2 test. This is independent of everything else and is simply wrong today.
2. Introduce `_pool_options` and move the `psycopg >= 3`, `CONN_MAX_AGE` and reserved-key checks into a `check_settings()` override. Rewrite `close_pool()` as `pop` then `close`. This alone fixes most of finding 2 without changing the design.
3. Key the pool on its spec inside `get_new_connection(conn_params)` and delete the `pool` property. Remove `close_pool()` from `ensure_timezone()`. Keep the two `creation.py` calls, with a comment that they exist because `CREATE DATABASE ... TEMPLATE` and `DROP DATABASE` need zero open connections.
4. Replace `_configure_connection` and the two module functions with one wrapper-free `configure_connection()` passed to the pool via `functools.partial`.
5. Change the pooled `_close()` to really close inside an atomic block; then delete the `ensure_connection()` branch in `base/base.py`, the `ProgrammingError` import and the hand-set-flags test, or keep the guard deliberately and test it through a real close inside `atomic()`.
6. Delete the `close_if_health_check_failed()` override, re-run the two skipped tests with a pool, and restore `django_test_skips` to a class attribute if they pass.
7. Revert the eleven `no_pool_connection()` substitutions and give the pool tests a helper with `addCleanup`.
8. Answer Q1 and Q2, and document the answers.

Steps 2 to 6 are each behaviour-preserving for non-pooled connections and can be reviewed separately.

## What was and was not verified

No PostgreSQL server is provisioned, so no test in `tests/backends/postgresql/` was run and nothing about transactions, check-out or return was observed against a live database. What was run is two offline scripts that construct real `DatabaseWrapper` and unopened `psycopg_pool.ConnectionPool` objects (psycopg 3.3.6, psycopg_pool 3.3.3). They establish the stale pool after a `NAME` change, the shared class-level dict, `close_pool()` constructing a pool, the configuration errors raised from `close_pool()` and `ensure_timezone()`, the skipped validation for an existing pool, the reserved-keyword `TypeError`s, the bound `configure` callback pinning a wrapper, the `AttributeError` in `_close()` for a connection without `_pool` (using a stand-in object), and the cost of the `self.pool` lookup. Everything in findings 3 and 5, the end-to-end consequence in finding 1, and both questions are from reading the code and the installed `psycopg` / `psycopg_pool` sources. The code sketches in the detail files were not executed.

The review was done in a single context with `claude-opus-5-5`; no other model or reviewer was used. The clone is unchanged: the probes wrote `__pycache__` directories into it while importing Django, and those were removed.

## Detail files

- `01_pool_ownership_and_lifecycle.md` — findings 1 and 2, probe output, the pool-spec and `check_settings()` sketches, file sizes.
- `02_connection_state_machine.md` — findings 3, 4 and 5, the `_close()` and `configure_connection()` sketches, the health-check reasoning.
- `03_tests_and_docs.md` — finding 6.
