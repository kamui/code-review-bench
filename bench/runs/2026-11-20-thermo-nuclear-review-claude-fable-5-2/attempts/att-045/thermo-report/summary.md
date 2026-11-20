# Thermo-nuclear review: django/django#17914 — "Refs #33497 -- Added connection pool support for PostgreSQL."

Range reviewed: `bcccea3ef31c777b73cba41a6255cd866bf87237..fad334e1a9b54ea1acb8cce02a25934c5acfe99f` (`git diff main...review-head`).

The review found seven actionable findings: one High, three Medium, three Low. The High one makes the test runner run against the non-test (production) database when pooling is enabled.

How the findings were checked: no PostgreSQL server is available, so pooled behaviour was exercised against a small fake PostgreSQL wire-protocol listener on localhost (scratch code in the work directory, psycopg 3.3.6 / psycopg_pool 3.3.3 from the cache venv). Findings 1, 2, 4, 5 and 7 were reproduced that way, and finding 3 was reproduced with an on-disk SQLite database on `main` versus `review-head`. Finding 6 is from reading the code only (psycopg2 is not installed). No live-database Django tests were run. The clone was not modified.

## Findings

### 1. [High] Pool is cached by alias only, so the test runner can run against the production database

**Where:** `django/db/backends/postgresql/base.py` lines 200-246 (`_connection_pools`, `pool`, `close_pool`), reached from `_nodb_cursor` (lines 529-535) and `get_new_connection` (lines 345-348); `django/db/backends/postgresql/creation.py` lines 58-61 and 89-91.

**Evidence.** The pool is stored in a class-level dict keyed by `self.alias`, and its connection kwargs (including `dbname`) are frozen when the pool is first created (lines 224-234). Nothing invalidates it when the alias's `settings_dict["NAME"]` changes. The PR closes the pool only in `_clone_test_db`, `_destroy_test_db` and `ensure_timezone`. `BaseDatabaseCreation.create_test_db()` (`django/db/backends/base/creation.py` lines 64-66) does `self.connection.close()` and then repoints `NAME` at the test database; with a pool, `close()` just returns the connection to the pool. `set_as_test_mirror()` and `setup_worker_connection()` repoint `NAME` the same way. Two triggers were reproduced by calling the real `create_test_db()`:

- **`postgres` database not reachable (the documented fallback path).** `DatabaseWrapper._nodb_cursor()` falls back to `self.__class__({**self.settings_dict, "NAME": connection.settings_dict["NAME"]}, alias=self.alias)`. That wrapper carries the real alias and the `pool` option, so running `CREATE DATABASE` through it creates and opens a pool under the real alias, bound to the production database name. Result after `create_test_db()`: `settings_dict["NAME"] == "test_prod"`, but `SELECT current_database()` on the connection returns `prod`, and the migrate introspection queries and a following `TRUNCATE` went to `prod`. Without `pool`, the same script talks to `test_prod`.
- **Any query on the alias before test databases are set up** (import-time query during test discovery, `AppConfig.ready()`, a custom runner). The pool already exists for the production name. Same result: `NAME` is `test_prod`, every statement goes to `prod`.

**Failure.** With `OPTIONS["pool"]` enabled, `manage.py test` can silently run migrations, fixtures, `TransactionTestCase` flushes (`TRUNCATE`) and test writes against the real database while reporting the test database name. That is data loss in the non-test database. The first trigger needs no user mistake at all, only a role that cannot connect to the `postgres` database. The reverse also happens: after `destroy_test_db()` on the fallback path, `NAME` is restored to `prod` but the alias's connection reports `test_prod` (reproduced), because the fallback wrapper in `_destroy_test_db` registered a new pool bound to the test name.

### 2. [Medium] `assume_role` combined with `pool` deadlocks every connection attempt

**Where:** `django/db/backends/postgresql/base.py` lines 98-104 (`ensure_role`) and 369-383 (`_configure_connection`), registered as the pool's `configure` callback at line 231.

**Evidence.** `_configure_connection` runs inside psycopg_pool worker threads. Its comment says it must not touch anything on `self` except variables, but `ensure_role()` calls `ops.compose_sql("SET ROLE %s", [role_name])`. That is `mogrify(sql, params, self.connection)` (`operations.py` line 193), and `mogrify` opens `connection.cursor()` on the Django `DatabaseWrapper` (`psycopg_any.py` line 21). On the wrapper that created the pool, `self.connection` is still `None` at that point, so `cursor()` → `ensure_connection()` → `connect()` → `self.pool.getconn()` runs inside the worker that is supposed to be producing the connection. Captured worker stack: `_add_connection → _connect → _configure_connection → ensure_role → compose_sql → mogrify → cursor → ensure_connection → connect → get_new_connection → getconn → wait`.

Reproduced with `OPTIONS = {"pool": {...}, "assume_role": "app_role"}`: every `connection.cursor()` fails with `OperationalError: couldn't get a connection after N sec`, on three consecutive attempts, and `SET ROLE` is never sent. The same settings without `pool` work and send `SET ROLE 'app_role'`.

**Failure.** Two documented PostgreSQL options cannot be used together. Each request blocks for the pool timeout (30 s by default) and then fails, and the pool workers keep retrying and timing out. The failure is loud and immediate, hence Medium. If the creating wrapper already holds a connection when the pool grows, the same call instead hits `validate_thread_sharing()` from the worker thread ("DatabaseWrapper objects created in a thread can only be used in that same thread"), and the new connection is still not added. That variant was reproduced by setting `assume_role` after the first connection was open: the second thread's connect timed out.

### 3. [Medium] New guard in `BaseDatabaseWrapper.ensure_connection()` permanently breaks a connection closed inside `atomic()` when autocommit is off (all backends)

**Where:** `django/db/backends/base/base.py` lines 274-277.

**Evidence.** The guard raises `ProgrammingError("Cannot open a new connection in an atomic block.")` when `self.connection is None and self.in_atomic_block and self.closed_in_transaction`. It was added for pooled connections, but that state is also reachable on every backend. When the outermost `atomic()` is entered with autocommit off, `Atomic.__enter__` sets `in_atomic_block = True` and `commit_on_exit = False`. If the connection is closed inside the block, `Atomic.__exit__` takes the "outermost block exit when autocommit was disabled" branch (`django/db/transaction.py` lines 308-311) and sets `connection.connection = None` while leaving `in_atomic_block` and `closed_in_transaction` both `True`. Before this PR the next query called `connect()`, which resets those flags. Now the guard fires first.

Reproduced with on-disk SQLite, `AUTOCOMMIT: False`, `with transaction.atomic(): connection.close()`, then a query:

- `main`: the query succeeds.
- `review-head`: `ProgrammingError: Cannot open a new connection in an atomic block.` on every later query.

With `AUTOCOMMIT: True` both revisions work.

**Failure.** A regression for SQLite, MySQL, Oracle and PostgreSQL users who manage transactions manually (`AUTOCOMMIT: False` or `transaction.set_autocommit(False)`) and whose code closes the connection inside an atomic block (`connection.close()`, `connections.close_all()`, `close_old_connections()`). The thread's wrapper never recovers: `close()` returns early because `closed_in_transaction` is set, and `close_if_unusable_or_obsolete()` does nothing because `connection` is `None`. Only a direct `connect()` call clears it. The trigger is narrow, hence Medium.

### 4. [Medium] `ensure_role()` is removed and `init_connection_state()` no longer calls the `ensure_timezone()` method, so subclass overrides are silently skipped

**Where:** `django/db/backends/postgresql/base.py` lines 362-408 (`ensure_timezone`, `_configure_connection`, `init_connection_state`), with the new module-level helpers at lines 89-104.

**Evidence.** On `main`, `init_connection_state()` called `self.ensure_timezone()` and `self.ensure_role()`. On `review-head` it calls `_configure_connection()`, which uses module-level `ensure_timezone()` / `ensure_role()` functions. `DatabaseWrapper.ensure_role` no longer exists, and the `ensure_timezone()` method (part of the `BaseDatabaseWrapper` interface) is now called only by `django/test/signals.py`. A subclass overriding both hooks, connected with `assume_role` and a per-database `TIME_ZONE` set:

- `main`: both overrides are called and no statement is sent.
- `review-head`: neither override is called; `SELECT set_config('TimeZone', ...)` and `SET ROLE 'app_role'` are sent anyway, and `hasattr(DatabaseWrapper, "ensure_role")` is `False`.

Separately, `DatabaseWrapper.ensure_timezone()` now calls `self.close_pool()` unconditionally (line 364), even when the time zone is unchanged. Because `close_pool()` reads `self.pool`, it builds a pool just to close it when none exists.

**Failure.** Third-party backends that subclass the PostgreSQL wrapper and customise or disable the per-connection time zone or role setup (for example behind a transaction-mode pooler, or to pick the role dynamically) silently lose that behaviour: the session `SET` statements come back, or the custom role logic never runs. Code that calls `connection.ensure_role()` gets `AttributeError`. Code that calls `connection.ensure_timezone()` on a pooled alias at runtime tears down the shared pool for all threads. Only subclasses and external callers are affected, and the release notes do not mention the change.

### 5. [Low] `"pool": {}` silently disables pooling

**Where:** `django/db/backends/postgresql/base.py` lines 204-206.

**Evidence.** `pool` returns `None` when `not pool_options`, and an empty dict is falsy. The docs added by this PR say the option may be "a dict to be passed to `ConnectionPool`". Reproduced: with `OPTIONS = {"pool": {}}`, `connection.pool` is `None` and plain unpooled connections are used.

**Failure.** A configuration that reads as "pool with default options" (for example after removing the last custom key, or a dict built from environment variables that ends up empty) silently runs without a pool, with no error or warning.

### 6. [Low] Docs say `pool` is "ignored with psycopg2", but the code raises `ImproperlyConfigured`

**Where:** `docs/ref/databases.txt` lines 270-271; `django/db/backends/postgresql/base.py` lines 290-292 and 217-222.

**Evidence.** `get_connection_params()` raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")` when `pool` is truthy under psycopg2, and the PR's own `test_connect_pool_setting_ignored_for_psycopg2` asserts that exception. On the normal `cursor()` path `close_if_health_check_failed()` reads `self.pool` first, so a psycopg2-only install gets the other message, "Error loading psycopg_pool module. Did you install psycopg[pool]?", because `psycopg_pool` cannot be imported without psycopg 3. Not executed here: psycopg2 is not installed in the review environment.

**Failure.** Settings shared between psycopg and psycopg2 environments, which the docs say are safe, fail on the first connection under psycopg2, and the usual error message points at the wrong fix. Either the docs or the behaviour should change.

### 7. [Low] A pooled connection held by a thread that exits without `close()` is never returned, permanently shrinking the pool

**Where:** `django/db/backends/postgresql/base.py` lines 345-348 (`getconn()`) and 385-399 (`_close()`, the only place `putconn()` is called).

**Evidence.** The pooled connection is returned only when the wrapper's `close()` runs. If a thread uses the ORM and ends without closing its connection, its thread-local wrapper and the psycopg connection are garbage-collected. psycopg's `Connection.__del__` does nothing for pooled connections, and psycopg_pool still counts the slot as checked out. Reproduced with `max_size=2`: after short-lived threads that each ran one query and exited, `get_stats()` reports `pool_size: 2, pool_available: 0`, and the main thread's next query fails with `OperationalError: couldn't get a connection after 3.00 sec`.

**Failure.** Without a pool, such a thread's connection was simply closed on garbage collection. With `"pool": True` the default `max_size` is 4, so four such threads (ad-hoc `threading.Thread` workers, tasks that never call `connection.close()`) exhaust the pool for the life of the process, and every later request blocks for the pool timeout and then fails. The docs added by this PR do not mention the requirement. Low because it needs application code that never closes its connections.

## Questions

### Q1. Is a pool that survives a `fork()` meant to be handled?

`_connection_pools` is process-global and psycopg pools are not fork-safe. In the common parallel test run the parent's pool is already closed by `_clone_test_db()` before workers fork, so no failure was shown. If anything reopens the pool in the parent before the fork (for example a database-tagged system check that queries), forked workers inherit an "open" pool with no worker threads, bound to the un-cloned test database and sharing sockets with the parent, and `setup_worker_connection()` does not reset it. Should `setup_worker_connection()` (or the `NAME`-changing helpers in general, see finding 1) drop the pool?

## Checked and not reported

- Closing a pooled connection inside `atomic()`, including nested blocks: the pool rolls the transaction back, later queries in the block raise the new `ProgrammingError`, and the wrapper recovers after the block. Works as intended against the fake server.
- Autocommit, isolation level and health-check handling for connections reused from the pool: no defect found.
- The non-pooled connect path, the `NO_DB_ALIAS` connection, and the PostGIS wrapper's `get_new_connection()`: behave as before.
- `django_test_skips` becoming a `cached_property`: no in-tree subclass depends on the class attribute.
- The new `psycopg-pool>=3.2.0` test requirement is an ordinary dependency addition, not a developer-workflow break.
