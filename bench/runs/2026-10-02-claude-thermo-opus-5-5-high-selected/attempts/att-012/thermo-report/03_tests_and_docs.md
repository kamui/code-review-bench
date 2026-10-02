# 03 — Tests and documentation

Scope: `tests/backends/postgresql/tests.py`, `docs/ref/databases.txt`,
`docs/releases/5.1.txt`, `tests/requirements/postgres.txt`.

Covers summary finding 6.

Verification status: read from the diff at `fad334e1a9`. None of the PostgreSQL
tests can run here (no server is provisioned). The one behavioural claim that
rests on a measurement (pool validation being skipped when a pool already
exists for the alias) is probe section 5b in
`01_pool_ownership_and_lifecycle.md`.

## Finding 6 — the tests route around the design, and the docs contradict the code

### `no_pool_connection()` is applied to eleven pre-existing tests

`tests/backends/postgresql/tests.py:24-30` adds:

```python
def no_pool_connection(alias=None):
    new_connection = connection.copy(alias)
    new_connection.settings_dict = copy.deepcopy(connection.settings_dict)
    # Ensure that the second connection circumvents the pool, this is kind
    # of a hack, but we cannot easily change the pool connections.
    new_connection.settings_dict["OPTIONS"]["pool"] = False
    return new_connection
```

`grep -n no_pool_connection tests/backends/postgresql/tests.py` shows 18 call
sites. Eleven of them replace `connection.copy()` in tests that have nothing
to do with pooling (lines 190, 226, 369, 386, 402, 418, 439, 448, 457, 550
and 562). Those tests set an option on a
copy and expect the next connect to honour it. Under pooling it would not,
because the copy shares the alias and therefore the pool
(`01_pool_ownership_and_lifecycle.md`, probe section 7). The helper's own
comment calls it a hack.

Two things follow. First, when the suite is run with `"pool"` enabled, the
time-zone, role, isolation-level, cursor-factory, server-side-binding and
client-encoding tests all run without a pool, so those options have no pooled
coverage beyond `test_connect_pool_with_timezone`. Second, the helper is the
test-side shadow of the alias-keyed cache: if the pool were derived from the
connection parameters, a copy with different options would simply not match
the cached spec.

`connection.copy()` already deep-copies `settings_dict`
(`django/db/backends/base/base.py:783-792`), so the second line of the helper
is redundant.

### The new pool tests share one alias and one process-wide dict

Six tests use `alias="default_pool"` and rely on `try/finally:
close_pool()` to keep `DatabaseWrapper._connection_pools` clean.
`test_pooling_not_support_persistent_connections` (line 340) and
`test_cannot_open_new_connection_in_atomic_block` (line 329) have no cleanup.
The first is correct only if no pool for `default_pool` exists when it runs,
because the `CONN_MAX_AGE` check is skipped for an existing pool (probe 5b:
`pool returned with CONN_MAX_AGE=60: True`). Today each earlier test cleans up,
so it passes; the coupling is through global state rather than through the
test.

`test_cannot_open_new_connection_in_atomic_block` sets `in_atomic_block` and
`closed_in_transaction` by hand on a wrapper that never connects. It asserts
the new guard in `BaseDatabaseWrapper.ensure_connection()` but not the
behaviour that motivated it (closing a pooled connection inside `atomic()` and
then querying). The `pool = True` line in it has no effect on the assertion.

`test_pooling_health_checks` asserts on `new_connection.pool._check`, a private
attribute of `psycopg_pool.ConnectionPool`.

The pool tests are gated on `is_psycopg3` only. `psycopg_pool` is a separate
distribution; with psycopg 3 installed and `psycopg_pool` missing they error
with `ImproperlyConfigured` / `ImportError` instead of skipping.

### Docs and a test name say "ignored"; the code raises

`docs/ref/databases.txt:270-271`:

```text
This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed
and is ignored with ``psycopg2``.
```

`django/db/backends/postgresql/base.py:290-292`:

```python
pool_options = conn_params.pop("pool", None)
if pool_options and not is_psycopg3:
    raise ImproperlyConfigured("Database pooling requires psycopg >= 3")
```

`tests/backends/postgresql/tests.py:348-354` is named
`test_connect_pool_setting_ignored_for_psycopg2` and asserts that
`ImproperlyConfigured` is raised. The code and the assertion agree with each
other; the documentation and the test name describe the opposite behaviour.

The documentation also does not mention that `CONN_MAX_AGE` must be `0`, which
is the other configuration error a user can hit, nor that the dict must not
contain `kwargs`, `open`, `configure` or `check`.

### Remedy

Fix the sentence in `docs/ref/databases.txt` to say the option is not
supported with `psycopg2` and raises `ImproperlyConfigured`, and rename the
test accordingly. Mention the `CONN_MAX_AGE` requirement next to it. Once the
pool is keyed on its parameters, revert the eleven `no_pool_connection()`
substitutions to `connection.copy()` and delete the helper's `pool = False`
line; keep a small `pooled_connection(alias, **pool_options)` helper for the
pool tests that registers `addCleanup(new_connection.close_pool)` so cleanup
does not depend on each test remembering a `finally`. Replace the hand-set
flags test with one that closes a pooled connection inside `atomic()`. Gate the
pool tests on `psycopg_pool` being importable.
