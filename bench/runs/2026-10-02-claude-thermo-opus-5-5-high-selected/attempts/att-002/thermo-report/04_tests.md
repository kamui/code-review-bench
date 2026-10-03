# 04 — Tests

Scope: `tests/backends/postgresql/tests.py` (+141 / −11), `django/db/backends/postgresql/features.py`
lines 86–108.

Covers summary finding F9.

## F9 — the test changes route around pooling instead of testing it

### `no_pool_connection()` is a suite-wide workaround

```python
def no_pool_connection(alias=None):
    new_connection = connection.copy(alias)
    new_connection.settings_dict = copy.deepcopy(connection.settings_dict)
    # Ensure that the second connection circumvents the pool, this is kind
    # of a hack, but we cannot easily change the pool connections.
    new_connection.settings_dict["OPTIONS"]["pool"] = False
    return new_connection
```

The PR replaces `connection.copy()` with this helper in eleven pre-existing tests (lines
190, 226, 369, 386, 402, 418, 439, 448, 457, 550, 562) and uses it in all seven new ones.
The comment says why: a copy on the same alias would be handed the alias's existing pool,
whose parameters cannot be changed. That is finding F2 seen from the test side. Every
existing test that copies a connection and alters `OPTIONS` (isolation level, role,
cursor factory, server-side binding, client encoding) had to be rerouted around pooling,
which means none of those options is tested with a pool. For `assume_role` that gap hides
a real failure (F1).

Two details of the helper itself:

- `connection.copy()` already deep-copies `settings_dict` (`base/base.py:789`), so the
  second `copy.deepcopy()` is redundant.
- The new pool tests call `no_pool_connection(alias="default_pool")` and then immediately
  set `OPTIONS["pool"]` to `True` or a dict. The helper's name says the opposite of what
  those tests want; they are using it only for the alias and the deep copy.

### Individual tests

- `test_pooling_health_checks` (lines 312–326) asserts on `new_connection.pool._check`, a
  private attribute of `ConnectionPool`. It checks that Django passed a keyword, not that
  an unhealthy pooled connection is replaced.
- `test_cannot_open_new_connection_in_atomic_block` (lines 329–338) is gated on psycopg 3
  and sets `pool = True`, but then assigns `in_atomic_block` and `closed_in_transaction`
  by hand and calls `ensure_connection()`. The pool is never created, the pooled
  `_close()` that produces the state is never run, and the branch under test lives in
  `BaseDatabaseWrapper`. See F5.
- `test_connect_pool_setting_ignored_for_psycopg2` (lines 349–355) asserts that
  `ImproperlyConfigured` is raised. See F8.
- Every pool test needs `try/finally: new_connection.close_pool()` because pools are held
  in a class-level registry that outlives the wrapper. Four of the seven new tests carry
  that boilerplate (five blocks); a leaked pool from a failing assertion before the `try` (for example
  `self.assertIsNotNone(new_connection.pool)` at line 246, which creates the pool outside
  the `try` block) stays registered under `default_pool` for the rest of the run.

- `test_connect_pool` (line 248) binds a local list to the name `connections`, shadowing
  the `django.db.connections` handler imported at the top of the module. It is harmless
  here and easy to rename.

### Missing coverage

There is no test for: a pooled connection with `assume_role`; a pooled connection with a
non-default isolation level; a pooled `close()` inside `atomic()` followed by a query; a
settings change (`NAME`) after a pool exists; `close_pool()` on an alias with no pool; or
the `keepdb` teardown path. Each corresponds to a finding in files 01–03.

### What the remedy does to the tests

If pools are selected by what they were built from (file 02's proposal), a `copy()` with
different `OPTIONS` simply gets a correctly configured pool, `no_pool_connection()` can be
deleted, and the eleven rerouted tests revert to `connection.copy()` and run under both
configurations. `self.addCleanup(new_connection.close_pool)` registered before first use
replaces the `try/finally` blocks.

### `features.py`

`django_test_skips` becomes a `cached_property` so that two base health-check tests can be
skipped when a pool is configured. Other backends already use a property for conditional
skips, so the shape is idiomatic. The skips exist because
`close_if_health_check_failed()` is overridden to do nothing under pooling; that is a
reasonable consequence of delegating health checks to the pool and is not flagged.

Status: verified by reading the diff and the head revision of the test file; the tests
themselves could not be run because no PostgreSQL server is provisioned.
