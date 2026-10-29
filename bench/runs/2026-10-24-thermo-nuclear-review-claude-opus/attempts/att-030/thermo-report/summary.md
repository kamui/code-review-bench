# Thermo nuclear review: django/django#17914, "Refs #33497 -- Added connection pool support for PostgreSQL."

Range reviewed: `bcccea3ef31c777b73cba41a6255cd866bf87237..fad334e1a9b54ea1acb8cce02a25934c5acfe99f` (`git diff main...review-head`).

I read every changed file and followed the new code into the code it calls: base `connect()`/`close()`/`_cursor()`, base test-DB creation, `test/signals.py`, PostGIS, and the installed `psycopg_pool` 3.3.3 (`_connect`, `getconn`, `putconn`, `_reset_connection`, `close`). No PostgreSQL server was available, so I confirmed the two High findings offline. The harness was `clone-work/scratch/probe.py`: it swaps `psycopg_pool.ConnectionPool` for a fake that records the `kwargs` it gets and raises `PoolTimeout` from `getconn()`, then drives the real Django backend code.

## Findings

### 1. [High] `assume_role` plus `pool` cannot produce a single connection: the pool's `configure` callback goes back into a thread-bound DatabaseWrapper

**Priority:** High. **File/lines:** `django/db/backends/postgresql/base.py:98-104` (`ensure_role`), `:228-235` (`configure=self._configure_connection`), `:369-384` (`_configure_connection`). Related: `django/db/backends/postgresql/operations.py:192-193`, `django/db/backends/postgresql/psycopg_any.py:20-22`.

**Evidence.** The pool gets `configure=self._configure_connection`, a method bound to whichever DatabaseWrapper first read `.pool`. psycopg_pool calls `configure` from `_connect()` (`psycopg_pool/pool.py:614-640`), and `_connect()` runs inside pool worker threads (`AddConnection` tasks). When `OPTIONS["assume_role"]` is set, `_configure_connection` calls `ensure_role(connection, self.ops, role_name)`, which calls `ops.compose_sql("SET ROLE %s", ...)`, which calls `mogrify(sql, params, self.connection)`, which runs `with connection.cursor()`. That `connection` is the Django DatabaseWrapper, not the raw psycopg connection passed to `configure`. So the worker thread calls `DatabaseWrapper.cursor()` → `_cursor()` → `ensure_connection()`. Two outcomes:
- If that wrapper is still inside its own `connect()` (`self.connection` is assigned only after `pool.getconn()` returns, so this always holds for the first connection), `ensure_connection()` calls `connect()` → `get_new_connection()` → `self.pool.getconn()` from the pool's own worker thread. Every worker that tries to create a connection blocks waiting on the pool it is supposed to fill. No connection is ever added, and the client fails with `PoolTimeout`, surfaced as `django.db.utils.OperationalError`, after `timeout` (30 s by default).
- If that wrapper already holds a connection, `_prepare_cursor()` → `validate_thread_sharing()` raises `DatabaseError: DatabaseWrapper objects created in a thread can only be used in that same thread`. `_connect()` re-raises it, the pool throws the new connection away and retries, and clients again time out.

Probe output: `probe2 calls: [('init', 'prod'), ('getconn', 'pool-worker')]` (`getconn` re-entered from the worker thread), and with the creator wrapper connected, `django.db.utils.DatabaseError: DatabaseWrapper objects created in a thread can only be used in that same thread...`. The PR does not test this combination: the only `assume_role` test was switched to `no_pool_connection()`.

**Failure.** Any deployment that combines the documented `assume_role` option with the new `pool` option gets no usable database connection. Every request waits for the pool timeout and then raises `OperationalError`. The timezone half of `_configure_connection` is not affected, because `ensure_timezone()` only uses the raw psycopg connection.

### 2. [High] Pools are cached by alias only and never dropped when `settings_dict["NAME"]` changes, so tests can run (and flush) against the non-test database

**Priority:** High. **File/lines:** `django/db/backends/postgresql/base.py:200-241` (class-level `_connection_pools` keyed by `self.alias`; `get_connection_params()` evaluated once into the pool's `kwargs`), `:524-538` (`_nodb_cursor` fallback builds a wrapper with `alias=self.alias`), `django/db/backends/postgresql/creation.py:57-61,89-90` (`close_pool()` added only to clone and destroy, not to create).

**Evidence.** The pool's connection kwargs, including `dbname`, are frozen when the pool is first created for an alias. Later reads of `.pool` return that cached pool whatever the wrapper's current `settings_dict` says. `BaseDatabaseCreation.create_test_db()` (`django/db/backends/base/creation.py:61-66`) only calls `self.connection.close()` and then rewrites `settings_dict["NAME"]` to the test name. With a pool, `close()` just returns the connection via `putconn()`, and the PR adds no `close_pool()` on this path. Concrete trigger: when the `postgres` maintenance DB is unreachable, `_create_test_db()` → `self.connection._nodb_cursor()` hits the existing fallback. That fallback builds `self.__class__({**self.settings_dict, "NAME": <original NAME>}, alias=self.alias)`; `self` is the real `"default"` wrapper, so the alias is `"default"`, not `NO_DB_ALIAS`. Opening its cursor creates and opens `_connection_pools["default"]` pointed at the original (non-test) database. `create_test_db()` then switches NAME to `test_<name>` and runs `migrate`. `connect()` → `self.pool` returns the cached pool, so migrations, serialization and the whole test run hit the original database. Probe: after creating the pool through such a wrapper and setting `settings_dict["NAME"] = "test_prod"`, `main.pool.kwargs["dbname"]` is still `prod`. Any other open of the default connection before `create_test_db()` triggers the same thing: an `AppConfig.ready()` query, a query at test-module import time, or a test harness that touches the DB early. The same root cause also leaves a stale pool when `destroy_test_db(keepdb=True)` restores NAME, and in fork-started parallel workers, which inherit `_connection_pools` from the parent: `setup_worker_connection()` changes NAME, but an inherited pool keeps serving the parent's database and sockets.

**Failure.** Migrations are applied to the developer's or deployment's real database. `TransactionTestCase` teardown issues `flush` (TRUNCATE of all tables) against it, which destroys data. Before this PR, `close()` followed by reconnecting always used the new NAME, so this is a regression created by the pool cache. Fix direction: key the pool on the connection parameters as well as the alias, or call `close_pool()` wherever `settings_dict["NAME"]` is rewritten, and make the `_nodb_cursor` fallback bypass the pool.

### 3. [Low] The pool docs say the option is ignored with psycopg2 (it is not), and omit the psycopg-pool version the code needs

**Priority:** Low. **File/lines:** `docs/ref/databases.txt:270-271`; `django/db/backends/postgresql/base.py:290-292` and `:223-233`; `tests/requirements/postgres.txt:3`.

**Evidence.** The docs say the option "is ignored with ``psycopg2``". In fact `get_connection_params()` raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")` whenever `pool` is truthy under psycopg2. `close_pool()`, which is now called from `_clone_test_db`/`_destroy_test_db`, also goes through `.pool`, which first tries `import psycopg_pool`. The PR's own test, `test_connect_pool_setting_ignored_for_psycopg2`, asserts that the error is raised, despite its name. Separately, the code passes `check=ConnectionPool.check_connection` and `open=False`, and the test requirements pin `psycopg-pool>=3.2.0`, but the docs only say "``psycopg[pool]`` or psycopg-pool" with no minimum version.

**Failure.** Users who rely on the documented "ignored with psycopg2" behaviour, for example a shared settings module across environments on different drivers, get a hard `ImproperlyConfigured` on the first query. Users with an older psycopg-pool installed get a constructor error instead of a clear requirement.

## Areas checked without findings

- The `ensure_connection()` guard in `base/base.py`: only reachable when `_close()` nulls `self.connection` inside an atomic block, which only the pooled path does. Non-pool backends are unchanged.
- `_close()` returning to `connection._pool`: this works after `close_pool()`, because a pool that is closed while a connection is checked out closes that connection when it comes back. Connections returned mid-transaction are rolled back by `_reset_connection`.
- Skipping `close_if_health_check_failed` under the pool, together with the forced `CONN_MAX_AGE == 0`.
- Recomputing timezone and adapters when tests close the pool on a `TIME_ZONE` change.
- The `features.django_test_skips` conversion to `cached_property`.
- PostGIS `get_new_connection` re-registering adapters on each checkout.
