# Detail 02 — base wrapper, docs, tests

Verification status: static reading of `django/db/backends/base/base.py` (full `close()`, `connect()`, `ensure_connection()` bodies), docs and test diffs. Nothing executed (no PostgreSQL server).

## Finding E — Pool-specific state handling added to `BaseDatabaseWrapper.ensure_connection` (`django/db/backends/base/base.py:274-277`)

Evidence. `ensure_connection` now raises `ProgrammingError("Cannot open a new connection in an atomic block.")` when `self.connection is None and self.in_atomic_block and self.closed_in_transaction`. `BaseDatabaseWrapper.close()` (line 344) only sets `closed_in_transaction = True` and keeps `self.connection` non-None when a close happens inside an atomic block, so for every backend except the new pooled PostgreSQL path this branch is unreachable. The PostgreSQL `_close` sets `self.connection = None` itself (returning the connection to the pool so it cannot be reused by another thread), which creates a state the base class did not previously have: connection gone, `in_atomic_block` still true.

Problem. A pool-only invariant has leaked into the shared base class, and it also adds a new import (`ProgrammingError`) there. The state machine now has two meanings for "closed in transaction": connection retained but closed, or connection discarded. `connect()` resets `in_atomic_block` unconditionally, so without the guard a pooled wrapper would silently reopen mid-transaction. The guard is the load-bearing piece, yet it lives far from the code that creates the condition.

Code-judo proposal. Keep the check in the PostgreSQL wrapper (override `ensure_connection`, or put the guard in `get_new_connection` where the pool is consulted), next to the `_close` that creates the state. Alternatively keep `self.connection` set to a sentinel closed wrapper, so the base class's existing `closed_in_transaction` flow handles it with no new branch.

## Finding F — Documentation, error behavior and test name disagree (`docs/ref/databases.txt:271`, `base.py:289-291`, tests)

Evidence. The new docs section says the option "is ignored with `psycopg2`". The implementation does the opposite: `get_connection_params` raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")` when `pool` is truthy and psycopg2 is in use. The test is named `test_connect_pool_setting_ignored_for_psycopg2` and asserts that exact exception. The docs also say nothing about the new constraint that pooling requires `CONN_MAX_AGE = 0` (otherwise `ImproperlyConfigured` at first connect), or that `CONN_HEALTH_CHECKS` is delegated to `ConnectionPool.check_connection` and Django's own health check is skipped.

Problem. A user following the docs would expect harmless ignoring and gets a hard failure. The test name keeps the stale claim alive. Pick one behavior, document it, and name the test to match. The constraint on persistent connections is a documentation-worthy gotcha.

## Finding G — Test scaffolding is a repeated hack and reaches into pool internals (`tests/backends/postgresql/tests.py:24-31`, `236-355`)

Evidence. The new helper `no_pool_connection()` carries the comment "this is kind of a hack, but we cannot easily change the pool connections", and it is spread through about fifteen existing tests purely to opt them out of pooling (mechanical churn). Six new tests each repeat the same four lines: build `no_pool_connection(alias="default_pool")`, set `OPTIONS["pool"]`, try/finally with `close_pool()`. `test_pooling_health_checks` asserts on the private `pool._check`. `django_test_skips` is rewritten as a `cached_property` solely to skip two health-check tests when pooling is on.

Problem. Test noise is a smell that the production API lacks a clean seam (see Finding A): there is no way to build or discard a pool without module-global state. A fixture/context manager, e.g. `pooled_connection(options)`, that creates the wrapper and guarantees `close_pool()`, would remove repetition and the finally boilerplate, and drive the production seam toward an explicit pool registry.
