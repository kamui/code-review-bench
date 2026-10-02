# Detail 01 — PostgreSQL backend pooling (django/db/backends/postgresql/*, base/base.py)

Scope: `git diff main...review-head` for `django/db/backends/postgresql/base.py`, `creation.py`, `features.py` and `django/db/backends/base/base.py`.
Measurements: `postgresql/base.py` goes 615 lines after the change (about 500 before); no file crosses 1000 lines, so the file-size rule is not triggered.
Verification: no PostgreSQL server is available, so live behavior was not run. Structural claims were checked by reading the code and by an offline probe (`clone-work/probe.py`, run with the cache venv, `PYTHONPATH=clone`) that constructs `DatabaseWrapper` objects with `OPTIONS={"pool": ...}` without connecting.

## F1. Pool-or-not branching is scattered across eight call sites (spaghetti growth)

Evidence: `self.pool` is consulted in `get_new_connection` (base.py:345-348, three evaluations in five lines), `init_connection_state` (404), `_close` (391), `close_if_health_check_failed` (502), `ensure_timezone` (362-367, via `close_pool`), `close_pool` (244-246), `creation.py` (`_clone_test_db`, `_destroy_test_db`) and indirectly `base/base.py:ensure_connection`.
Each site holds a different fragment of one idea, "where do connections come from and go back to". Reading `connect()` now means holding both the direct path and the pooled path in your head at every step.
`pool` is also a property with a side effect: it builds a `ConnectionPool` on first access, and it is used as a boolean (`if self.pool:`).

Code-judo proposal: introduce one small connection-source abstraction with two implementations. For example, `_DirectConnections` has `acquire(wrapper, conn_params)`, `release(conn)` and `configure_after_connect(wrapper, conn)`. `_PooledConnections` wraps the `ConnectionPool` and owns the open, getconn, putconn, close and configure logic. The wrapper resolves it once (`self._connections = ...`) and `get_new_connection`, `_close`, `init_connection_state` and `close_if_health_check_failed` delegate to it with no `if self.pool`. The three `if self.pool` fragments in `get_new_connection`, `_close` and `init_connection_state` then disappear, and `close_pool` becomes a method on the pooled strategy only.

## F2. `close_pool()` builds a pool just to close it, and can raise from teardown

Evidence: `close_pool` (base.py:243-246) does `if self.pool: self.pool.close(); del ...`. `self.pool` constructs and registers a pool when none exists. It calls `get_connection_params()` and may raise `ImproperlyConfigured` (for example CONN_MAX_AGE != 0, or psycopg2). The probe confirms that after `close_pool()` on a fresh wrapper the registry is empty, so a pool object was constructed and then immediately discarded.
`close_pool` is called from `ensure_timezone` (every TIME_ZONE/USE_TZ setting change in tests), from `_clone_test_db` and from `_destroy_test_db`, which is exactly where raising is least wanted.
`del self._connection_pools[self.alias]` is also an unguarded KeyError race when two threads close the same alias, and `self.pool` is read three times with no stable reference between them.
Also in `_close` (391): after `close_pool()` has dropped the registry entry, `if self.pool:` silently creates a fresh unopened pool for the alias just to decide which branch to take.

Remedy: `close_pool` should be `pool = self._connection_pools.pop(self.alias, None); if pool: pool.close()`. That is atomic, never constructs, and never raises. `_close` should decide from `self.connection` itself (pool-owned connections have `_pool`), not from `self.pool`.

## F3. Process-global mutable registry keyed by alias, with the first wrapper pinned as the pool's configure target

Evidence: `_connection_pools = {}` is a class attribute (base.py:200), shared across all wrappers, threads and test copies, keyed only by alias, not by settings.
The probe shows that a second wrapper for the same alias gets the same pool object, and that `pool._configure.__self__` is the first wrapper: the pool keeps the first thread's `DatabaseWrapper` alive and runs `_configure_connection` against that instance's `ops`, `timezone_name` (a cached property) and `settings_dict`.
The comment in `_configure_connection` ("make sure that whatever is done here does not access anything on self aside from variables") is contradicted by the body, which reads `self.ops`, `self.timezone_name` and `self.settings_dict`.
The probe also shows that the CONN_MAX_AGE check lives inside the lazy-construction branch, so once a pool exists a wrapper with `CONN_MAX_AGE=60` gets the pool with no error. A validation rule that is only enforced on first construction is not a validation rule.
The probe also shows that a user-supplied pool dict containing `open`, `kwargs`, `configure` or `check` fails with a raw `TypeError: got multiple values for keyword argument 'open'` rather than an `ImproperlyConfigured`.
The tests hack around the alias-keyed registry (`no_pool_connection` is commented "kind of a hack", and `test_connect_pool` relies on `copy()` reusing an alias).

Remedy: make the configure callback a module-level function or a small callable holding only `(ops, timezone_name, role_name)` captured when the pool is built. Validate the pool options and CONN_MAX_AGE in `get_connection_params` or a system check, where they always run. Reject or merge reserved keys explicitly. Consider owning the registry in `django.db.backends.postgresql` as a small `PoolRegistry` rather than as class state on the wrapper.

## F4. Test-only workaround embedded in production code: `ensure_timezone` closes the pool, `_close` returns via `connection._pool`

Evidence: `DatabaseWrapper.ensure_timezone` (362-367) now begins with an unconditional `self.close_pool()`, "so new connections pick up the correct timezone". `django/test/signals.py:82` is the only caller (on TIME_ZONE/USE_TZ setting changes). `_close` (385-402) carries the comment "This is a workaround for tests so a pool can be changed on setting changes", and calls `self.connection._pool.putconn(...)`, which reads a private attribute of psycopg's connection.
So production `_close` has a code path whose only reason to exist is the test harness's mid-flight pool replacement, and it depends on `psycopg_pool` internals.
The close-pool call also happens before the `self.connection is None` early return, so it fires for connections that were never opened.

Remedy: keep the hot path trivial. Give the pooled strategy (F1) the connection-to-pool binding at checkout, for example by storing `(pool, conn)` on the wrapper or wrapping `putconn`, rather than reaching for `conn._pool`. Move the "TIME_ZONE changed, drop pool" behavior into the test signal handler (`conn.close_pool()` when the backend offers it), or have the pool re-run configure, rather than hiding it in a method named `ensure_timezone`.

## F5. Duplicate module-level `ensure_timezone` / `ensure_role` shadow the method of the same name, and exist only to avoid `self`

Evidence: base.py:89-105 adds free functions `ensure_timezone(connection, ops, timezone_name)` and `ensure_role(connection, ops, role_name)`. The method `ensure_timezone` (362) has the same name as the free function it wraps. `ensure_role` as a method is deleted outright, which is a silent removal of a method on a public backend class. The free functions take `ops` as a separate argument purely so `_configure_connection` can claim not to touch self, and F3 shows it touches self anyway.
This is a pass-through layer with no clarity benefit: one method plus the callable in F3 would do. It is also a naming trap, since two `ensure_timezone` symbols with different signatures live in one module.

Remedy: keep one `_configure_connection(connection)` that does both steps inline, drop the free functions, and restore `ensure_role` only if it is genuinely public API (check usage; otherwise inlining is fine).

## F6. Shared-layer leak: pool semantics in `BaseDatabaseWrapper.ensure_connection`

Evidence: `django/db/backends/base/base.py:274-277` adds `if self.in_atomic_block and self.closed_in_transaction: raise ProgrammingError(...)` for every backend. With the old non-pooled flow, `close()` inside an atomic block left `self.connection` set to a closed psycopg connection, so `ensure_connection` never took this branch. The branch is reachable only because the pooled `_close` sets `self.connection = None` while `in_atomic_block` (F4). The new check is therefore PostgreSQL-pool-only logic living in the base class and importing `ProgrammingError` into it, and its only tests (`test_cannot_open_new_connection_in_atomic_block`) are in the PostgreSQL test module and set `in_atomic_block` / `closed_in_transaction` by hand rather than driving a real `close()`.

Remedy: either override `ensure_connection` in the PostgreSQL wrapper, or (better, via F1) keep the released connection object in place on the wrapper so the base class's existing "closed connection in an atomic block" failure behavior continues to apply with no base-class change.

## F7. `django_test_skips` turned into a settings-reading cached_property

Evidence: `features.py` replaces a static dict with a `cached_property` that reads `connection.settings_dict["OPTIONS"].get("pool")` and conditionally `update()`s a nested dict, skipping two health-check tests with a reason ("Pool does implicit health checks") that carries no context.
This follows an existing pattern in other backends, so it is a minor point. It is still another place that re-derives "is pooling on" from raw settings, which F1's single resolution point would remove. The two skips are also a signal that the pooled wrapper changes the behavior contract of `CONN_HEALTH_CHECKS` (`close_if_health_check_failed` returns early), which is documented nowhere.

## Things checked and found acceptable

- File size: `postgresql/base.py` at 615 lines stays far below 1000.
- `creation.py` additions (close pool before cloning the template DB, close pool on destroy) are minimal and correctly placed. They would be simpler once F2 makes `close_pool` safe to call unconditionally.
- Using `setdefault` for the registry to keep first writer wins is sound for an unopened pool, since losers hold no threads.
