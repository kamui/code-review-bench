# Thermo-nuclear code quality review — django/django#17914

"Refs #33497 -- Added connection pool support for PostgreSQL."
Range `bcccea3ef3..fad334e1a9` (`main...review-head`), 8 files, +325 / −44.

## Verdict

**Not approvable as written. Request changes.**

The feature is small and the intent is right, but the pool is modelled as "one lazily created object per alias, reachable through a property", and that model does not match how Django treats a connection's settings. Almost every awkward line in the diff is a compensation for it: a pool torn down from inside `ensure_timezone()`, a private psycopg attribute read "as a workaround for tests", a new branch in the shared base wrapper, a test helper whose comment calls itself a hack and which eleven existing tests had to be rerouted through. One combination of documented options, pooling with `assume_role`, cannot acquire a connection at all. There is a clear restructuring that deletes these compensations instead of rearranging them, and it is sketched and exercised offline in the detail files.

No file crosses the 1 000-line threshold (`postgresql/base.py` goes from 516 to 615 lines, `base/base.py` is 792, the test module is 569), so file size is not a blocker here.

## Scope and method

The review read the full diff and the head revision of every touched file, plus the callers that the change interacts with (`base/creation.py`, `test/signals.py`, `transaction.py`, `postgresql/operations.py`, `postgresql/psycopg_any.py`) and the installed `psycopg_pool` 3.3.3 source. No PostgreSQL server is provisioned, so the test suite could not be run. Behavioural claims were instead checked with three scratch scripts that drive the real Django and `psycopg_pool` code, replacing only `psycopg.Connection.connect` with a mock where a connection is needed. Each finding states whether it is verified that way, verified by reading, or unverified. This was a single-context review on `claude-opus-5-5`; no other reviewer or model was used.

## Findings

### F1 — Pooling with `assume_role` cannot acquire a connection: the pool's configure callback re-enters the wrapper

Where: `django/db/backends/postgresql/base.py:369-383` (and the registration at line 231).

`_configure_connection()` carries a comment saying it must not touch anything on `self` except variables, because the pool calls it on its own threads. Six lines later it passes `self.ops` to `ensure_role()`, which calls `ops.compose_sql()`, which is `mogrify(sql, params, self.connection)` with `self.connection` being the Django wrapper, and `mogrify` opens a Django cursor on it. On a pool worker thread that means `wrapper.cursor()` → `ensure_connection()` → `wrapper.connect()` → `pool.getconn()`, waiting for the very connection being configured, while the thread that owns the wrapper is itself blocked in `getconn()`. With a fake driver and otherwise real code, pooled connect succeeds in 0.0 s without `assume_role` and fails with `PoolTimeout` when it is set, with `wrapper.connect()` observed running on two pool worker threads. The only role test was moved to `no_pool_connection()` by this PR, so nothing covers the combination. The same bound-method registration also makes the pool hold a strong reference to whichever per-thread wrapper first touched `.pool` and run that wrapper's code on foreign threads for the life of the pool. The remedy is to compose the role statement against the raw connection that was passed in (`mogrify("SET ROLE %s", [role], connection)` works on a driver connection under both psycopg versions) and to hand the pool a callback closed over plain values rather than a bound wrapper method; add a pooled role test. Verified with a fake driver, not against a live server. Detail: `01_connection_configuration.md`.

### F2 — The pool is cached by alias but built from settings, so it goes stale when the settings change

Where: `django/db/backends/postgresql/base.py:202-241` (and `get_new_connection` at lines 345-350).

`BaseDatabaseWrapper.connect()` recomputes `get_connection_params()` on every connect and passes the result to `get_new_connection(conn_params)`, but the pooled branch ignores the argument and uses a pool that froze an earlier snapshot of the same parameters under the alias. Django mutates `settings_dict` in place in `create_test_db()` (switching `NAME` to the test database), in `destroy_test_db()` when `keepdb` is set, and in `setup_worker_connection()`, and none of those invalidates the pool; the PR added `close_pool()` only to `_clone_test_db()`, `_destroy_test_db()` and `ensure_timezone()`. The stale state is directly observable: after a pool exists, changing `settings_dict["NAME"]` leaves `get_connection_params()["dbname"]` at the new value and `pool.kwargs["dbname"]` at the old one, and a `copy()` with a different `NAME` and `TIME_ZONE` on the same alias is handed the original's pool. If a pooled connection has been opened on an alias before the test database is set up, the pool keeps serving connections to the pre-test database afterwards. Correctness should not depend on every site that edits `settings_dict` remembering a PostgreSQL-specific call. Store a signature (connection parameters, time zone name, role, pool options) next to the pool and rebuild when the parameters passed to `get_new_connection()` no longer match; that makes the argument meaningful again and lets the three `NAME`-changing sites work without pool code. Stale pool verified offline; reachability inside Django's own runner is question Q1. Detail: `02_pool_lifecycle.md`.

### F3 — `pool` is a get-or-create factory exposed as a property and used as a boolean

Where: `django/db/backends/postgresql/base.py:243-246` (also lines 391, 404 and 502).

`self.pool` is read eight times across five methods, and four of those methods only want to know whether the wrapper is pooled, yet every read can construct a `ConnectionPool`, import `psycopg_pool`, call `get_connection_params()` and raise `ImproperlyConfigured`. Measured consequences: `ensure_timezone()` on a wrapper that has never connected constructs a pool and immediately closes it, which happens for every initialised PostgreSQL connection each time a test overrides `TIME_ZONE` or `USE_TZ`; and `_close()` after `close_pool()` registers a brand-new pool purely because `if self.pool:` was evaluated while the connection is returned to the old one. `close_if_health_check_failed()` runs the same get-or-create path on every cursor. The registry is also unsynchronised: the comment justifies `setdefault()` for creation, but the membership test, the final lookup and `del` in `close_pool()` are separate operations, so a concurrent close produces `KeyError`. Split the concept in two: a pure `pool_options` property answering "is pooling configured", and a pool lookup that is called from exactly one place, `get_new_connection()`, under a lock; have `close_pool()` pop and close without constructing. Verified offline. Detail: `02_pool_lifecycle.md`.

### F4 — Test accommodations are implemented inside production methods and rely on a private psycopg attribute

Where: `django/db/backends/postgresql/base.py:362-367` (and `_close` at lines 391-395).

`ensure_timezone()` is documented on the base class as setting the connection's time zone and reporting whether it changed. Its first statement is now `self.close_pool()`, a process-wide action that fails every other client of the alias with `PoolClosed`, and it exists because the `setting_changed` receiver is the method's only caller. That forces `_close()` to return the connection through `self.connection._pool.putconn(...)`, which the code's own comment calls "a workaround for tests"; `_pool` is a private attribute of psycopg's connection class. This is test-only behaviour embedded in two production methods plus a dependency on a driver internal. Record on the wrapper which pool the current connection was checked out of (`self._connection_pool = pool` at checkout, cleared in `_close()`); `_close()` then returns the connection to that pool with no private access, `init_connection_state()` and `close_if_health_check_failed()` test that attribute instead of calling the factory, and with F2's signature check `ensure_timezone()` reverts to its pre-PR body. Verified by reading and by the offline sketch. Detail: `02_pool_lifecycle.md`.

### F5 — A pool-only state is handled by a new branch in the shared base wrapper

Where: `django/db/backends/base/base.py:274-277`.

`BaseDatabaseWrapper.ensure_connection()` now raises `ProgrammingError("Cannot open a new connection in an atomic block.")` when `connection is None` and `closed_in_transaction` are both true. For every other backend that state is unreachable: base `close()` deliberately keeps the driver handle when closing inside `atomic()`, and only the pooled PostgreSQL `_close()` nulls `self.connection` itself. So a feature-specific condition was added to a shared path, the base class now has two mechanisms for one invariant with nothing explaining when each applies, and two more guards were needed in the PostgreSQL wrapper (`is_usable()` and `init_connection_state()` both gain a `connection is None` check). Either override `ensure_connection()` in the PostgreSQL wrapper so the guard lives beside the code that creates the state and `base/base.py` stays untouched, or make it a stated contract of `_close()` in the base class and test it on every backend. In both cases the test should run the real sequence (pooled connection, `atomic()`, `close()`, query) rather than assigning the two flags by hand. Verified by reading every assignment to the two attributes. Detail: `03_base_wrapper_and_settings_validation.md`.

### F6 — Hoisting the setters into module functions removed `DatabaseWrapper.ensure_role()` and did not buy independence from the wrapper

Where: `django/db/backends/postgresql/base.py:89-104`.

The two setter bodies were moved to module-level `ensure_timezone(connection, ops, timezone_name)` and `ensure_role(connection, ops, role_name)`. The `ops` parameter is a back-pointer to the wrapper, so the functions are not actually wrapper-free (that is the mechanism behind F1). Meanwhile the `ensure_role()` method is gone and `init_connection_state()` no longer dispatches through `self.ensure_timezone()`, so a backend subclassing the PostgreSQL wrapper that overrides either hook silently loses its override, and the module now has a function and a method both named `ensure_timezone` that do different things. This moves complexity without deleting any. Keep both as methods parameterised by the connection they act on (`_configure_timezone(connection)`, `_configure_role(connection)`), keep `ensure_timezone()` and `ensure_role()` as one-line callers that pass `self.connection`, and delete the module functions. Verified by reading. Detail: `01_connection_configuration.md`.

### F7 — Pool configuration is validated in three unrelated places instead of the existing `check_settings()` hook

Where: `django/db/backends/postgresql/base.py:209-222` (and lines 290-292).

The psycopg 2 check sits in `get_connection_params()`, a parameter builder, between two unrelated `pop()` calls; the `CONN_MAX_AGE` check, the `psycopg_pool` import check and the `True` → `{}` normalisation sit inside the lazy `pool` property and therefore fire from whichever method first reads it, including `_destroy_test_db()` and a signal receiver; and option keys that collide with Django's own keywords (`check`, `kwargs`, `open`, `configure`) are not checked at all and surface as a bare `TypeError` from `ConnectionPool()`. `BaseDatabaseWrapper.check_settings()` is the canonical hook for invalid configuration and is the first call in `connect()`. Override it once in the PostgreSQL wrapper so that all of these checks live together, leave `get_connection_params()` with a plain `pop("pool", None)`, and read `CONN_MAX_AGE` by index like the neighbouring `CONN_HEALTH_CHECKS` since both keys are always populated. Verified by reading; the `TypeError` was reproduced for three of the four keys. Detail: `03_base_wrapper_and_settings_validation.md`.

### F8 — The documentation says the option is ignored with psycopg2; the code raises

Where: `docs/ref/databases.txt:270-271`.

The new documentation states that the `pool` option "is ignored with ``psycopg2``", but `get_connection_params()` raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")`, and the test asserting the raise is named `test_connect_pool_setting_ignored_for_psycopg2`. Raising is the better behaviour, so correct the sentence and rename the test. The same section should also say that `CONN_MAX_AGE` must be `0`, which is a hard error at runtime and the first thing someone moving from persistent connections will hit. Verified by reading. Detail: `03_base_wrapper_and_settings_validation.md`.

### F9 — The test changes route around pooling instead of testing it

Where: `tests/backends/postgresql/tests.py:24-30`.

`no_pool_connection()` exists, in its own words, because "we cannot easily change the pool connections", and the PR substitutes it for `connection.copy()` in eleven pre-existing tests, so isolation level, role, cursor factory, server-side binding and client encoding are all untested with a pool; for the role that hides F1. The helper deep-copies settings that `copy()` has already deep-copied, and the seven new pool tests call a function named "no pool" and then turn pooling on. `test_pooling_health_checks` asserts on the private `pool._check`, `test_cannot_open_new_connection_in_atomic_block` never involves the pool and sets the flags by hand, `test_connect_pool` creates its pool before entering the `try` that cleans it up, and four of the seven new tests need `try/finally: close_pool()` because the registry outlives the wrapper. Once pools are selected by what they were built from (F2), a `copy()` with different options gets a correct pool, the helper can be deleted, the eleven tests revert, and `addCleanup(new_connection.close_pool)` replaces the boilerplate. Add tests for a pooled role, a pooled non-default isolation level, a pooled close inside `atomic()`, and a `NAME` change after a pool exists. Verified by reading; the tests could not be run. Detail: `04_tests.md`.

## Questions for the author

### Q1 — Can the stale pool be reached in the test runner today?

Is there any path in Django's own runner, or in a typical project, that opens a pooled connection on an alias before `create_test_db()` switches `NAME`, or in a forked parallel worker before `setup_worker_connection()` does? `connection.features.is_postgresql_15` evaluated during discovery would do it through `temporary_connection()`. If so, are the tests then running against the pre-test database?

### Q2 — Is the `ensure_connection()` guard meant to be a general base-class contract?

If the intent is that any backend's `_close()` may release its handle itself, where should that be documented and tested? If not, is there an objection to moving the guard into the PostgreSQL wrapper?

## Proposed remediation sequence

1. Fix the configure callback first, since it is the one user-visible failure: compose the role statement against the raw connection, register a callback closed over values, restore `ensure_role()` and the method-based setters, and add a pooled role test (F1, F6).
2. Introduce `pool_options`, move all pool validation into a `check_settings()` override, and correct the documentation (F7, F8).
3. Replace the `pool` property with a single locked lookup called from `get_new_connection(conn_params)` that compares a stored signature, record `self._connection_pool` at checkout, and rewrite `_close()`, `init_connection_state()`, `close_if_health_check_failed()` and `close_pool()` against it. Revert `ensure_timezone()` to its original body. Consider moving the registry into `django/db/backends/postgresql/pool.py` (F2, F3, F4).
4. Decide where the atomic-block guard lives and test the real sequence (F5).
5. Delete `no_pool_connection()`, revert the eleven rerouted tests to `connection.copy()`, and add the missing pooled coverage (F9).

Steps 1 and 2 are independent of each other; step 3 depends on 2; step 5 depends on 3.

## What is fine

Keeping `close_pool()` in `_clone_test_db()` and `_destroy_test_db()` is correct, since both statements require that no connection, idle or not, is open to the database. Forcing `autocommit=True` on pooled connections and letting `connect()` set the real mode afterwards is sound. Delegating health checks to the pool and skipping the two base health-check tests under pooling is a reasonable consequence, and turning `django_test_skips` into a `cached_property` follows what other backends do. Rejecting `CONN_MAX_AGE != 0` is the right call.

## Verification status

Verified offline with real `psycopg_pool` and a mocked driver connection: F1 (timeout and foreign-thread re-entry), F2 (stale parameters), F3 (pools constructed as side effects), the key-collision part of F7, and the proposed restructuring (connects with a role, rebuilds on a `NAME` change, constructs nothing on close). Verified by reading the source: F4, F5, F6, F7, F8, F9. Not verified: anything requiring a live PostgreSQL server, including the test suite itself and the reachability asked about in Q1. The scratch scripts are in `clone-work/scratch/` (`probe.py`, `probe2.py`, `probe3.py`); the clone was not modified.

## Detail files

- `01_connection_configuration.md` — F1, F6: the configure callback, the role and time zone setters, reproduction output, worked proposal.
- `02_pool_lifecycle.md` — F2, F3, F4: registry model, mutation-site table, side-effect measurements, worked proposal and its offline check.
- `03_base_wrapper_and_settings_validation.md` — F5, F7, F8: the base-class guard, the validation table, the documentation mismatch.
- `04_tests.md` — F9: the helper, per-test notes, missing coverage.
