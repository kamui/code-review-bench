# Thermo-nuclear review — django/django#17914 "Added connection pool support for PostgreSQL"

Range reviewed: `bcccea3ef3..fad334e1a9` (`git diff main...review-head`).

There are 7 actionable findings: 1 high (P1), 4 medium (P2), 2 low (P3). The most serious is that a pool created before the test database is set up keeps pointing at the real database, so the test run executes against it.

## How this was checked

No PostgreSQL server is available, so the Django PostgreSQL test suite was not run. Findings 1, 2, 4 and 7 were reproduced by running the head code against a small fake PostgreSQL wire-protocol listener (scratch only, psycopg 3.3.6 / psycopg_pool 3.3.3), which records the database name and the SQL of every connection. Finding 3 was reproduced on SQLite against both `main` and `review-head`. Findings 5 and 6 are from reading the code. psycopg2 is not installed, so the psycopg2 behaviour in finding 6 is from the code path only.

## Findings

### F1 — [P1 / High] A pool created before test-database setup keeps serving connections to the real database, so the test run executes against it

**Where:** `django/db/backends/postgresql/base.py` lines 202–241 (`pool` property, cached per alias in the class-level `_connection_pools`), with `django/db/backends/postgresql/creation.py` lines 57–61 and 89–91 (the only places the pool is discarded).

**Evidence traced.** The pool is built once per alias from `self.get_connection_params()` (line 224), which freezes `dbname` at creation, and from then on `pool` returns `self._connection_pools[self.alias]` whatever `settings_dict` says. `BaseDatabaseCreation.create_test_db()` (`django/db/backends/base/creation.py` lines 62–64) switches to the test database by calling `self.connection.close()` and then rewriting `settings_dict["NAME"]`; it relies on the next `connect()` opening a new connection with the new name. With a pool, `close()` only returns the connection to the pool (`_close()`, line 395) and the next `connect()` calls `self.pool.getconn()` (line 348) on the old pool. The PR adds `close_pool()` to `_clone_test_db()` and `_destroy_test_db()` but not to the `create_test_db()` path.

Reproduced with the real `create_test_db()` (only `call_command` stubbed). One query was issued before test setup, which is what an `AppConfig.ready()` query, an import-time queryset or a `connection.pg_version` feature probe does:

- Without `"pool"`: after `create_test_db()` the live connection reports `dbname=test_prod_db`, and a `TRUNCATE` issued afterwards reaches `test_prod_db`.
- With `"pool"`: `settings_dict["NAME"]` is `test_prod_db`, but the live connection reports `dbname=prod_db`, and the `TRUNCATE` is received by the server on the `prod_db` connection.

**Failure caused.** In any project that enables `OPTIONS["pool"]` and touches the database before the test runner creates the test database (Django's testing docs acknowledge this happens), `manage.py test` runs `migrate`, every test, and every `TransactionTestCase` flush (which truncates all tables) against the configured development or production database. The freshly created test database stays empty and is dropped at the end. This is silent data destruction; without the pool the same project is isolated in the test database after `create_test_db()`.

The same root cause has a milder mirror image, also reproduced: with `--keepdb`, `destroy_test_db()` skips `_destroy_test_db()`, so the pool is never closed and, after `NAME` is restored to `prod_db`, later queries on the alias still go to `test_prod_db`.

**Direction.** Discard the alias's pool whenever the alias is re-pointed: at least in the `create_test_db()` path after `self.connection.close()`, in `destroy_test_db()` regardless of `keepdb`, and in `setup_worker_connection()`.

### F2 — [P2 / Medium] `"pool"` combined with `"assume_role"` can never obtain a connection

**Where:** `django/db/backends/postgresql/base.py` lines 98–104 (`ensure_role`), 228–234 (`configure=self._configure_connection`) and 369–383 (`_configure_connection`).

**Evidence traced.** The pool runs `configure` in its own worker thread for every new server connection. `_configure_connection` calls `ensure_role(connection, self.ops, role_name)`, which calls `ops.compose_sql("SET ROLE %s", ...)`. `compose_sql` is `mogrify(sql, params, self.connection)` (`operations.py` line 193), and `mogrify` opens `connection.cursor()` on the Django `DatabaseWrapper` (`psycopg_any.py` lines 20–22), not on the raw psycopg connection being configured. So the configure callback re-enters the wrapper from the pool worker thread: `cursor()` → `ensure_connection()` → `connect()` → `get_new_connection()` → `self.pool.getconn()` (line 348). The worker that is supposed to produce the pool's first connection is now waiting for the pool to give it one. The comment at lines 370–373 states the callback must not touch anything on `self` beyond variables; `self.ops.compose_sql` breaks that rule.

Reproduced: with `OPTIONS = {"assume_role": "app_role"}` the server receives `SET ROLE 'app_role'` and the query succeeds. Adding `"pool": {...}` makes the first query fail with `OperationalError: couldn't get a connection after N sec`. The server sees connections opened but never receives `SET ROLE`, and the traceback from the configure callback shows exactly the chain above ending in `getconn` raising `PoolTimeout` inside worker thread `pool-1-worker-N`.

**Failure caused.** Any deployment that uses both documented options has no working database access: every request blocks for the pool timeout (30 seconds by default) and then fails, while the pool keeps opening and discarding server connections in the background.

**Direction.** Build the `SET ROLE` statement from the raw connection handed to the callback (for example `psycopg.sql` composition or `ClientCursor(connection).mogrify`) instead of going through `self.ops`.

### F3 — [P2 / Medium] The new `ensure_connection()` guard permanently disables a connection closed inside `atomic()` while autocommit is off (all backends)

**Where:** `django/db/backends/base/base.py` lines 274–277.

**Evidence traced.** When `atomic()` is entered with autocommit already off, `Atomic.__enter__` sets `in_atomic_block = True` and `commit_on_exit = False`. If the connection is closed inside the block, `close()` sets `closed_in_transaction = True`, and the outermost `Atomic.__exit__` takes the "autocommit was disabled" branch (`django/db/transaction.py` lines 318–323), which sets `connection.connection = None` and deliberately leaves `in_atomic_block` and `closed_in_transaction` set. The existing design relies on the next `connect()` to reset them; `close()` says so in its comment ("The next connect() will reset the transaction state anyway"). The new guard raises before `connect()` can run, and nothing else clears the flags: `close()` returns early when `self.connection is None`, and `close_old_connections()` skips a `None` connection.

Reproduced on SQLite (file database) with `transaction.set_autocommit(False)`, then `with transaction.atomic(): connection.close()`:

- `main`: state after the block is `connection=None, in_atomic_block=True, closed_in_transaction=True`; the next query reconnects and succeeds.
- `review-head`: same state; every later query raises `ProgrammingError: Cannot open a new connection in an atomic block.`, including after `close_old_connections()`.

**Failure caused.** This is a regression in the shared base class, so it affects SQLite, MySQL, Oracle and non-pooled PostgreSQL too. For projects using `AUTOCOMMIT: False` or `set_autocommit(False)`, a `connection.close()` or `close_old_connections()` inside an `atomic()` block leaves that thread's connection wrapper unusable for the rest of the process's life (only an explicit `connection.connect()` recovers it), where before it healed on the next query.

**Direction.** Either reset `in_atomic_block` / `closed_in_transaction` in the autocommit-off exit branch of `Atomic.__exit__`, or restrict the guard so it cannot fire once the outermost block has exited.

### F4 — [P2 / Medium] A thread that ends without closing its connection permanently removes a slot from the pool

**Where:** `django/db/backends/postgresql/base.py` lines 345–348 (`getconn()` in `get_new_connection`) and 385–397 (`putconn()` only in `_close`).

**Evidence traced.** A pooled connection is handed back only when `DatabaseWrapper.close()` runs. Connection wrappers are thread-local; when a thread finishes without calling `connection.close()` / `close_old_connections()`, its wrapper is garbage collected with the psycopg connection still checked out. Nothing returns it: there is no finalizer on the wrapper, and psycopg_pool does not track or reclaim connections that were never `putconn()`-ed, so the pool keeps counting it against `max_size`. Without a pool the same thread's connection is simply closed when collected and there is no shared cap.

Reproduced with `"pool": {"min_size": 1, "max_size": 2, "timeout": 2}`: two short-lived threads each run one ORM query and return; after they are joined and collected, a query on the main thread fails with `OperationalError: couldn't get a connection after 2.00 sec`, and pool stats show `pool_size=2, pool_available=0`. The same script without `"pool"` succeeds.

**Failure caused.** With the default pool size (`min_size=4`, `max_size` equal to it), four such threads are enough to make every later database access in the process block for the timeout and fail, until the process is restarted. Code that works today — ORM calls in a `ThreadPoolExecutor` or `threading.Thread` worker, or via `sync_to_async(thread_sensitive=False)`, with no explicit close — becomes a process-wide outage once `"pool"` is enabled. Long-lived threads outside the request cycle also hold a slot indefinitely, because with `CONN_MAX_AGE=0` the only automatic close is the request signal handler.

**Direction.** Return the connection when the wrapper is collected (for example a `weakref.finalize` that calls `putconn`), and document that pooled connections must be closed explicitly in threads Django does not manage.

### F5 — [P2 / Medium] `DatabaseWrapper.ensure_role()` is removed and `ensure_timezone()` is no longer used at connection setup, so subclass overrides are silently ignored

**Where:** `django/db/backends/postgresql/base.py` lines 89–104 (new module-level functions), 362–367 (`ensure_timezone` method) and 369–383 (`_configure_connection`).

**Evidence traced.** On `main`, `init_connection_state()` called `self.ensure_timezone()` and `self.ensure_role()`, both overridable methods; `ensure_timezone` is also a hook declared on `BaseDatabaseWrapper` (`base/base.py` line 127). On head, `_configure_connection()` calls the module-level `ensure_timezone(connection, self.ops, self.timezone_name)` and `ensure_role(connection, self.ops, role_name)` directly. The `ensure_role` method no longer exists on the class, and the `ensure_timezone` method is now reached only from `django/test/signals.py` line 82. This applies whether or not a pool is configured.

**Failure caused.** Backends that subclass the PostgreSQL `DatabaseWrapper` for PostgreSQL-compatible servers and override these methods (for example to skip `SET TIME ZONE` or `SET ROLE` on a server that rejects them, or to set the role differently) keep importing and running, but their override is no longer applied when a connection is initialised; Django issues the statements itself. Any caller of `connection.ensure_role()` gets `AttributeError`. Neither change is mentioned in the release notes.

**Direction.** Keep role and timezone configuration as wrapper methods that take the raw connection, and have `_configure_connection` call them through `self`.

### F6 — [P3 / Low] Documentation says `"pool"` is ignored with psycopg2, but the code raises `ImproperlyConfigured`

**Where:** `docs/ref/databases.txt` lines 270–271; `django/db/backends/postgresql/base.py` lines 290–292.

**Evidence traced.** The new docs end with "This option requires `psycopg[pool]` or `psycopg-pool` to be installed and is ignored with `psycopg2`." `get_connection_params()` raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")` whenever the option is truthy and psycopg 3 is not in use, and `connect()` calls `get_connection_params()` first. The PR's own test `test_connect_pool_setting_ignored_for_psycopg2` asserts that exception despite its name.

**Failure caused.** A project that shares settings between psycopg and psycopg2 environments, trusting the documentation, cannot connect at all under psycopg2. Either the docs or the behaviour should change so they agree.

### F7 — [P3 / Low] `"pool": {}` silently disables pooling

**Where:** `django/db/backends/postgresql/base.py` lines 204–206.

**Evidence traced.** The docs say the option may be "a dict to be passed to `ConnectionPool`", or `True` for the defaults. The check is `if self.alias == NO_DB_ALIAS or not pool_options: return None`, so an empty dict — the natural way to write "a dict with all defaults" — is treated as "no pool". `get_connection_params()` pops the key without complaint. Reproduced: with `OPTIONS = {"pool": {}}`, `connection.pool` is `None`.

**Failure caused.** The deployment runs without a pool and with no error or warning, so the operator believes pooling is active. Testing `pool_options is None or pool_options is False` instead (or rejecting `{}`) would avoid it.

## Questions

### Q1 — Is psycopg-pool older than 3.2 meant to be supported?

`pool` (lines 228–234) always passes `check=` to `ConnectionPool` and uses `ConnectionPool.check_connection` when `CONN_HEALTH_CHECKS` is on. `tests/requirements/postgres.txt` pins `psycopg-pool>=3.2.0`, which suggests both were added in 3.2, but the docs name no minimum version and the code does not check one. I could not install an older psycopg-pool to confirm how it fails; if it rejects the keyword, users on 3.1.x would get a bare `TypeError` rather than an `ImproperlyConfigured` message.
