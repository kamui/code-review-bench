# Thermo nuclear review — django/django#17914 (PostgreSQL connection pool)

Range reviewed: `bcccea3ef3..fad334e1a9`. No PostgreSQL server was available, so this is a static trace of the code. Nothing was run against a live database.

## Findings

### 1. Medium — The alias-keyed pool is not reset when the database NAME changes, and `_nodb_cursor`'s fallback can create it against the wrong database
Files: `django/db/backends/postgresql/base.py` lines 200-246 (pool cache) and 514-541 (`_nodb_cursor` fallback); `django/db/backends/postgresql/creation.py` lines 58-90.

Evidence traced:
- `DatabaseWrapper._connection_pools` is a class-level dict keyed only by `self.alias`. The pool is built once from `self.get_connection_params()`, so its `kwargs` (including `dbname`) are frozen at creation.
- The patch calls `close_pool()` only in `ensure_timezone`, `_clone_test_db` and `_destroy_test_db`.
- `BaseDatabaseCreation.create_test_db` (`base/creation.py` lines 64-66) calls `connection.close()` and then rewrites `settings_dict["NAME"]` to the test database name, without closing the pool. `set_as_test_mirror` and `setup_worker_connection` also rewrite `NAME` in place without `close_pool()`.
- `DatabaseWrapper._nodb_cursor` falls back when connecting to the `postgres` database fails (for example, a role with no access to it). The fallback builds `self.__class__({..., "NAME": <first postgresql connection's NAME>}, alias=self.alias)`. The alias is the real alias, for example `"default"`, not `NO_DB_ALIAS`. The `pool` property therefore creates a pool for that alias, pointing at the original non-test database, and caches it in `_connection_pools["default"]`.
- Nothing closes that pool afterwards. Once `create_test_db` switches `NAME` to `test_xxx`, `get_new_connection` calls `self.pool.getconn()` and gets the cached pool, which still connects to the original database. The wrapper's own `NAME` is ignored.

Failure: with `OPTIONS["pool"]` set and the `postgres` maintenance database unreachable, `manage.py test` creates the test database but runs migrations, fixtures and tests against the original (development or production) database through the stale pool. That can modify or destroy real data. `destroy_test_db` then calls `close_pool()` only after the damage is done. The patch handles staleness in the clone and destroy paths but misses the create, mirror and worker paths, and the fallback path actively seeds the stale pool.

Likely fixes: key the pool on the effective connection params or NAME rather than the alias alone, or call `close_pool()` wherever `NAME` is changed. The fallback should also not reuse the real alias.

### 2. Low — Docs say the `pool` option is "ignored with psycopg2", but the code raises
Files: `docs/ref/databases.txt` lines 270-271; `django/db/backends/postgresql/base.py` lines 290-292.

`get_connection_params` raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")` whenever `pool` is truthy and psycopg2 is in use. The patch's own test, `test_connect_pool_setting_ignored_for_psycopg2`, asserts this raise. A user who follows the docs and keeps one settings file for psycopg2 and psycopg3 environments gets a hard failure at first connect.

### 3. Low — Pool requirements and restrictions are undocumented
Files: `docs/ref/databases.txt` lines 248-271; `django/db/backends/postgresql/base.py` lines 208-212 and 232.

- `ConnectionPool.check_connection`, used when `CONN_HEALTH_CHECKS=True`, needs `psycopg_pool>=3.2`. The tests pin `psycopg-pool>=3.2.0`, but the docs only say `psycopg[pool]` or `psycopg-pool`. With an older pool package, enabling health checks raises `AttributeError` at pool creation.
- `CONN_MAX_AGE` other than 0 (including `None`) raises `ImproperlyConfigured` lazily on first connect. The docs do not say so.
- With a pool, `CONN_HEALTH_CHECKS` behaviour changes (`close_if_health_check_failed` is skipped). The docs do not say so either.

## Checked, no issue found
- `ensure_connection` guard against reconnecting in an atomic block after the pooled `_close()` sets `connection = None`. `Atomic.__exit__` handles `closed_in_transaction` correctly.
- Autocommit, isolation level and `_configure_connection` run by the pool in autocommit mode, so there is no INTRANS problem.
- `is_usable` now returns False when `connection is None`.
- `putconn` is routed to `connection._pool`, so connections go back to the correct pool after `close_pool()`.
