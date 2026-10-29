# Thermo Nuclear Review — django/django#17914 "Refs #33497 -- Added connection pool support for PostgreSQL."

Range reviewed: `bcccea3ef31c777b73cba41a6255cd866bf87237..fad334e1a9b54ea1acb8cce02a25934c5acfe99f` (`git diff main...review-head`), eight files. The change adds an opt-in `OPTIONS["pool"]` for the PostgreSQL backend backed by `psycopg_pool.ConnectionPool`, moves time-zone/role setup into a pool `configure` callback, teaches `_close()` to return pooled connections, adds a guard in `BaseDatabaseWrapper.ensure_connection()`, closes the pool around test-database clone/destroy, and documents the option.

No PostgreSQL server is available, so the new pooled tests could not be executed. I traced every new path by reading the installed `psycopg_pool` 3.3.3 / `psycopg` 3.3.6 sources and ran three offline reproductions from the work directory: a fake-driver harness injected into a real `ConnectionPool` (Scenarios A–C below), a file-backed SQLite run of the `ensure_connection()` guard with and without the new lines, and the existing `transactions`, `backends.base`, and `backends.tests` suites on SQLite against the modified base wrapper (217 tests, OK). The clone was left unchanged.

Four actionable findings follow, ordered by priority.

## Finding 1 — pooled connections can never be configured when `assume_role` is set; the pool deadlocks and every `connect()` times out

**Priority:** High

**Location:** `django/db/backends/postgresql/base.py` lines 369–383 (`_configure_connection`), lines 98–104 (`ensure_role`), line 231 (`configure=self._configure_connection`), line 404 (pooled connections skip `_configure_connection` in `init_connection_state`); `django/db/backends/postgresql/operations.py` line 192–193 (`compose_sql`); `django/db/backends/postgresql/psycopg_any.py` lines 20–22 (`mogrify`).

**Evidence traced:** The pool is constructed with `configure=self._configure_connection`, and `init_connection_state()` deliberately skips `_configure_connection` when `self.pool` is set (line 404), so for a pooled connection the role/time-zone setup runs only inside `psycopg_pool`'s `_connect()`, which is executed by the pool's daemon worker threads (`pool.py` `_connect` → `self._configure(conn)`), never by the thread that owns the `DatabaseWrapper`. The comment on lines 370–373 says the method must not touch anything on `self` except plain attributes, but `ensure_role()` calls `ops.compose_sql("SET ROLE %s", [role_name])`, and `compose_sql` is `mogrify(sql, params, self.connection)` where `self.connection` is the Django `DatabaseWrapper`; `mogrify` then does `with connection.cursor() as cursor:` — the wrapper's full `cursor()` path (`_cursor()` → `close_if_health_check_failed()` → `ensure_connection()` → `create_cursor()` → `_prepare_cursor()` → `validate_thread_sharing()`). Two things happen depending on wrapper state, both fatal:

- If the owning wrapper has no connection yet (the normal case on first use: it is blocked in `pool.getconn()` waiting for the very connection being configured), `ensure_connection()` calls `connect()` → `get_new_connection()` → `self.pool.getconn()` from inside the pool worker. The worker now waits for a connection that only workers can produce, and every worker that picks up an `AddConnection` task hits the same recursion. After `timeout` (default 30 s) the worker gets `PoolTimeout`, `_add_connection()` logs "error connecting" and reschedules with backoff for up to `reconnect_timeout` (default 300 s), and the application thread's own `getconn()` raises `PoolTimeout` (a `psycopg.OperationalError`, surfaced as `django.db.utils.OperationalError`). No `SET ROLE` is ever executed and no connection ever enters the pool.
- If the owning wrapper already holds a connection (pool growing or refilling later, triggered from another thread), `wrapper.cursor()` reaches `validate_thread_sharing()` and raises `DatabaseError("DatabaseWrapper objects created in a thread can only be used in that same thread…")` because the worker thread is not the creator thread. `_connect()` fails the same way.

Reproduced offline with a fake driver class swapped into a real `ConnectionPool` created by the `pool` property (`OPTIONS = {"pool": {"min_size": 1, "max_size": 1, "timeout": 2, "num_workers": 1}, "assume_role": "app_role"}`):

```
pool-1-worker-0 psycopg.pool: error connecting in 'pool-1': couldn't get a connection after 2.00 sec
=== Scenario A: pool + assume_role, wrapper has no connection ===
connect() raised after 2.0s: psycopg_pool.PoolTimeout: couldn't get a connection after 2.00 sec
SET ROLE statements executed: []
=== Scenario B: pool + assume_role, wrapper already holds a connection ===
configure from worker thread raised: django.db.utils.DatabaseError: DatabaseWrapper objects created in a thread can only be used in that same thread. The object with alias 'b' was created in thread id … and this is thread id …
```

The non-pooled path is unaffected because there `_configure_connection` still runs in the owning thread with `self.connection` already set (same as before the change). The diff does not cover this combination: the only role test, `test_connect_role_doesnt_exist` (`tests/backends/postgresql/tests.py` around line 402), was switched to `no_pool_connection()`, and none of the new `test_connect_pool*` tests set `assume_role`.

**Failure:** Any deployment that combines the new `"pool"` option with the documented `"assume_role"` option (the ephemeral-credentials use case the comment on lines 377–379 describes) cannot obtain a single database connection: first use blocks for the pool timeout and then raises `OperationalError`, and the pool keeps burning worker retries in the background. Because the configure callback is bound to whichever wrapper first created the pool, the fault is also independent of which thread later asks for a connection.

## Finding 2 — new `ensure_connection()` guard permanently disables a connection (all backends) after `close()` inside an outermost `atomic()` entered with autocommit off

**Priority:** Medium

**Location:** `django/db/backends/base/base.py` lines 274–277 (new guard); interaction with `django/db/transaction.py` lines 309–313 (`Atomic.__exit__`, "Outermost block exit when autocommit was disabled") and `django/db/backends/base/base.py` lines 352–361 (`close()`).

**Evidence traced:** The guard raises `ProgrammingError("Cannot open a new connection in an atomic block.")` whenever `self.connection is None and self.in_atomic_block and self.closed_in_transaction`. It was added so that a pooled connection, whose `_close()` now sets `self.connection = None` even inside `atomic()`, is not silently replaced by a fresh pooled connection mid-transaction. But the guard lives in the base class and the same state is reachable on every backend without a pool: when autocommit is disabled on entry (`DATABASES[...]["AUTOCOMMIT"] = False` or `transaction.set_autocommit(False)`), `Atomic.__enter__` sets `commit_on_exit = False` and implements the outermost block as a savepoint. If `connection.close()` is called inside that block (a common recovery step after an `OperationalError`), `close()` sets `closed_in_transaction = True`. On exit, `Atomic.__exit__` pops the last savepoint id, keeps `in_atomic_block` True, and in the `elif not connection.savepoint_ids and not connection.commit_on_exit:` branch sets `connection.connection = None` without clearing `in_atomic_block` (lines 310–311; only the non-closed `else` branch on line 313 clears it). The wrapper is left with `in_atomic_block=True, closed_in_transaction=True, connection=None`.

Before this change the next `ensure_connection()` simply called `connect()`, which resets `in_atomic_block`, `closed_in_transaction`, `needs_rollback`, and reconnects. After this change every subsequent `cursor()` raises `ProgrammingError`, and nothing in the normal lifecycle can clear it: `close()` returns early because `closed_in_transaction` is set, `close_old_connections()` → `close_if_unusable_or_obsolete()` returns because `connection is None`, and `set_autocommit(True)` raises `TransactionManagementError` because `in_atomic_block` is True. Only a direct `connection.connect()` recovers. Verified on a file-backed SQLite database (in-memory SQLite is excluded because its `close()` is a no-op), running the identical scenario at head and with the pre-change `ensure_connection` body:

```
=== HEAD (with new guard) ===
HEAD state after atomic: in_atomic_block=True closed_in_transaction=True connection=None
HEAD attempt 1: django.db.utils.ProgrammingError: Cannot open a new connection in an atomic block.
HEAD attempt 2: django.db.utils.ProgrammingError: Cannot open a new connection in an atomic block.
HEAD close(): no error; connection = None
=== merge-base behaviour (guard removed) ===
BASE state after atomic: in_atomic_block=True closed_in_transaction=True connection=None
BASE attempt 1: query OK, reconnected
BASE attempt 2: query OK, reconnected
```

No test in the diff or in `tests/transactions` exercises close-inside-atomic with autocommit disabled; the existing `test_atomic_prevents_queries_in_broken_transaction_after_client_close` runs in autocommit mode, where `Atomic.__exit__` clears `in_atomic_block` and the guard is not triggered.

**Failure:** For projects using `AUTOCOMMIT: False` or manual `set_autocommit(False)` on any backend (SQLite, MySQL, Oracle, PostgreSQL with or without a pool), a single `connection.close()` inside an outermost `atomic()` block turns the thread's persistent `DatabaseWrapper` into one that raises `ProgrammingError` on every query until the process restarts. This is a regression from self-healing reconnection to a permanent per-thread outage, introduced by the base-class guard rather than by the pool itself.

## Finding 3 — pool connection parameters are frozen at pool creation, so `create_test_db()` (and other `NAME` rewrites) keep using the original database when a pool already exists

**Priority:** Medium

**Location:** `django/db/backends/postgresql/base.py` lines 224–239 (`connect_kwargs = self.get_connection_params()` captured once into `ConnectionPool(kwargs=...)`), lines 345–348 (`get_new_connection` ignores the freshly computed `conn_params` when pooled), lines 385–399 (`_close()` returns the connection to the pool instead of closing it); `django/db/backends/postgresql/creation.py` lines 58–61 and 89–91 (`close_pool()` added only to `_clone_test_db` and `_destroy_test_db`); `django/db/backends/base/creation.py` lines 64–66 (`create_test_db` relies on `self.connection.close()` before rewriting `NAME`).

**Evidence traced:** The pool is keyed only by alias in the class-level `_connection_pools` dict and stores the `dbname` (and every other connection parameter) computed when it was first created. `connect()` still calls `get_connection_params()` each time, but `get_new_connection()` discards the result when `self.pool` is truthy and takes whatever the existing pool hands out. `close()` → `_close()` only does `putconn()`. The base `create_test_db()` flow is: `self.connection.close()`, then `settings_dict["NAME"] = test_database_name`, then `call_command("migrate")`, serialize, and run the suite — it depends on the close/reconnect cycle picking up the new `NAME`. The author recognised that `close()` is no longer enough and added `close_pool()` to `_clone_test_db` and `_destroy_test_db`, but not to the create path, `set_as_test_mirror()` (`NAME` rewritten for mirrors), or `setup_worker_connection()` (parallel workers rewrite `NAME` to their clone and then call `close()`). Reproduced offline with the fake driver:

```
=== Scenario C: pool kwargs frozen across NAME change (create_test_db flow) ===
first connect dbname: proddb
get_connection_params() now says dbname = test_proddb
pool.kwargs still says dbname = proddb
second connect() reused pooled conn; new physical connects: none (reused prod connection)
```

The precondition is that the pool exists before `create_test_db()` runs. Django core does not connect before `setup_databases()` under `manage.py test` (the `test` command disables system checks and nothing in `DiscoverRunner` touches the database earlier), so this requires project code that opens a connection during start-up — `AppConfig.ready()` database access, module-level queries in test modules collected by `build_suite()`, custom test runners, or a management command/script that uses the database and then calls `create_test_db()`. Each of these is discouraged but common, and today Django handles them correctly because `close()` really closes the connection. With `"pool"` enabled the same code silently keeps using the original database. The parallel-runner fork path has the analogous shape: a child inherits `_connection_pools` (and any idle sockets) from the parent, `setup_worker_connection()`'s `close()` cannot discard them, and the inherited pool still carries the parent's `dbname`; in the stock flow this is masked only because `_clone_test_db` happened to close the parent pool shortly before the fork.

**Failure:** When any connection to the configured database is opened before test-database creation and `OPTIONS["pool"]` is set, `migrate`, fixture loading, `TransactionTestCase` flushes, and every test query run against the **original** (potentially production) database while the freshly created `test_<NAME>` database stays empty and is later dropped. With pooling disabled the identical project behaves correctly, so enabling the option turns a benign start-up query into data loss in the configured database.

## Finding 4 — documentation says the option is ignored with psycopg2, but the code raises `ImproperlyConfigured`

**Priority:** Low

**Location:** `docs/ref/databases.txt` lines 270–271; `django/db/backends/postgresql/base.py` lines 290–292; `tests/backends/postgresql/tests.py` `test_connect_pool_setting_ignored_for_psycopg2`.

**Evidence traced:** The new docs paragraph ends "This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed and is ignored with ``psycopg2``." The code path added to `get_connection_params()` does the opposite: `if pool_options and not is_psycopg3: raise ImproperlyConfigured("Database pooling requires psycopg >= 3")`, and the new test (despite its name "…setting_ignored_for_psycopg2") asserts that `connect()` raises that error.

**Failure:** A psycopg2 user who follows the documentation and sets `"pool": True` expecting a no-op gets an `ImproperlyConfigured` exception on the first connection attempt, i.e. an application that fails to start. The documentation (or the behaviour) needs to be aligned; the same sentence should also mention that `"pool"` is incompatible with `CONN_MAX_AGE != 0`, which the code enforces.

## Notes on areas reviewed without an actionable finding

- `_close()` returning via `self.connection._pool.putconn()` is correct: `_pool` is set on every connection handed out by `getconn()`, and routing through it lets a connection drawn from a pool that has since been closed by `close_pool()` be closed rather than leaked.
- The pool creates connections with `autocommit=True`; Django then applies `settings_dict["AUTOCOMMIT"]` on every `connect()`, so a connection returned with `autocommit=False` is normalised on next checkout. `ConnectionPool.check_connection` handles both modes.
- `close_if_health_check_failed()` short-circuits for pooled connections; this is consistent because `CONN_HEALTH_CHECKS` controls both Django's check and the pool's `check` callback.
- `ensure_timezone()` closing the pool on `TIME_ZONE`/`USE_TZ` changes is only reached from `django.test.signals`, and the adapters template captured in the pool kwargs is rebuilt because the pool is recreated.
- Pool worker threads are daemon threads, so a leftover open pool does not block interpreter exit.
- `features.django_test_skips` becoming a `cached_property` is compatible with the base class attribute and the test runner's usage.
- `tests/requirements/postgres.txt` gaining `psycopg-pool>=3.2.0` is an ordinary requirements-file update, not a developer-experience break.
