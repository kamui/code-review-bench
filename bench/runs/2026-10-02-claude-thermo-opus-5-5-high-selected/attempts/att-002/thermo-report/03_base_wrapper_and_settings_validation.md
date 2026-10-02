# 03 — Shared base wrapper and settings validation

Scope: `django/db/backends/base/base.py` lines 271–279 (`ensure_connection`),
`django/db/backends/postgresql/base.py` lines 204–222 (`pool` validation), 290–292
(`get_connection_params`), 385–399 (`_close`), 489–491 (`is_usable`), and
`docs/ref/databases.txt` lines 248–271.

Covers summary findings F5, F7 and F8.

## F5 — a pool-only state is handled by a new branch in the shared base class

The PR adds this to `BaseDatabaseWrapper.ensure_connection()`:

```python
        if self.connection is None:
            if self.in_atomic_block and self.closed_in_transaction:
                raise ProgrammingError(
                    "Cannot open a new connection in an atomic block."
                )
```

For every backend other than pooled PostgreSQL the branch is unreachable. The base
`close()` already owns this situation and deliberately keeps the driver handle when
closing inside `atomic()`:

```python
        try:
            self._close()
        finally:
            if self.in_atomic_block:
                self.closed_in_transaction = True
                self.needs_rollback = True
            else:
                self.connection = None
```

so `self.connection is None` and `closed_in_transaction` are never both true: later use of
the wrapper hits the closed driver connection and raises the driver's own error.
`closed_in_transaction` is only ever set here (`grep -rn closed_in_transaction django`
shows `base/base.py:88, 250, 274, 352, 358` and three reads in `transaction.py`).

The new state exists because the pooled `_close()` sets `self.connection = None` itself
(`postgresql/base.py:397`). That is a legitimate need — once a connection is back in the
pool the wrapper must not keep a handle another thread may now own — but it means a hook
that the base class treats as "close the driver connection" now also mutates lifecycle
state that `close()` manages, and the base class grew a branch to compensate. Two further
compensations sit in the PostgreSQL wrapper: `is_usable()` gains
`if self.connection is None: return False`, and `init_connection_state()` gains
`self.connection is not None`.

The result is two mechanisms for one invariant ("no queries on a connection closed inside
`atomic()`"): the pre-existing "keep the dead handle so the driver raises", and the new
"null the handle and raise `ProgrammingError`". Nothing in the base class explains when
each applies.

### Remedy

Pick one owner. Either:

1. Keep the guard where the state is created. Override `ensure_connection()` in the
   PostgreSQL wrapper:

   ```python
       @async_unsafe
       def ensure_connection(self):
           if self.connection is None and self.closed_in_transaction and self.in_atomic_block:
               raise ProgrammingError("Cannot open a new connection in an atomic block.")
           super().ensure_connection()
   ```

   and leave `base/base.py` and its new `ProgrammingError` import untouched; or

2. Make it a deliberate base-class contract: document on `BaseDatabaseWrapper._close()`
   that a backend may release the handle itself, say so in a comment at the guard, and
   cover the guard in `tests/backends/base/` on every backend rather than only in a
   psycopg 3-gated PostgreSQL test.

Option 1 is smaller and keeps feature logic out of the shared path. In either case the
existing test (`test_cannot_open_new_connection_in_atomic_block`,
`tests/backends/postgresql/tests.py:329–338`) should exercise the real sequence — pooled
connection, `atomic()`, `close()`, then a query — instead of assigning
`in_atomic_block` and `closed_in_transaction` by hand; as written it passes without the
pool ever being involved.

Status: reasoning from the source; the unreachability claim for other backends was
checked by reading every assignment to `closed_in_transaction` and `self.connection`.

## F7 — pool configuration is validated in three unrelated places instead of `check_settings()`

| Check | Where it lives | When it fires |
| --- | --- | --- |
| psycopg 2 is unsupported | `get_connection_params()` lines 290–292, between `pop("isolation_level")` and `pop("server_side_binding")` | every connect, and every pool creation |
| `CONN_MAX_AGE` must be 0 | inside the `pool` property, lines 209–212 | first read of `.pool` for the alias, from whatever method does it |
| `psycopg_pool` importable | inside the `pool` property, lines 217–222 | same |
| `"pool": True` → `{}` | inside the `pool` property, lines 214–215 | same |
| reserved option keys (`check`, `kwargs`, `open`, `configure`) | nowhere | `TypeError` from `ConnectionPool(...)` |

`BaseDatabaseWrapper` already has a hook for exactly this: `check_settings()`, the first
call in `connect()` ("Check for invalid configurations."). Using it puts every pool
check in one method, makes them fire at a predictable moment, and removes validation from
a parameter builder and a property getter:

```python
    def check_settings(self):
        super().check_settings()
        pool_options = self.pool_options
        if pool_options is None:
            return
        if not is_psycopg3:
            raise ImproperlyConfigured("Database pooling requires psycopg >= 3")
        if self.settings_dict["CONN_MAX_AGE"] != 0:
            raise ImproperlyConfigured("Pooling doesn't support persistent connections.")
        if reserved := pool_options.keys() & {"kwargs", "open", "configure", "check"}:
            raise ImproperlyConfigured(...)
```

`get_connection_params()` then only needs `conn_params.pop("pool", None)`.

A smaller inconsistency in the same block: `CONN_MAX_AGE` is read with
`settings_dict.get("CONN_MAX_AGE", 0)` while `CONN_HEALTH_CHECKS` four lines later is
read with `settings_dict["CONN_HEALTH_CHECKS"]`. Both keys are always populated by
`ConnectionHandler.configure_settings()`, and `connect()` itself indexes
`settings_dict["CONN_MAX_AGE"]` directly, so the defensive `.get()` hides nothing and
should match.

## F8 — the documentation says the option is ignored with psycopg2; the code raises

`docs/ref/databases.txt:270–271` says:

> This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed
> and is ignored with ``psycopg2``.

The code raises `ImproperlyConfigured("Database pooling requires psycopg >= 3")`, and the
test that asserts the raise is named `test_connect_pool_setting_ignored_for_psycopg2`
(`tests/backends/postgresql/tests.py:349–355`). One of the three is wrong. Raising is the
better behaviour (silently not pooling is a worse failure than a clear error), so the
docs sentence and the test name should change. The docs also do not mention that
`CONN_MAX_AGE` must be `0`, which is a hard error at runtime and the first thing a user
migrating from persistent connections will hit.

Status: verified by reading; the key-collision `TypeError` was reproduced in `probe.py`
section 5 for `check`, `kwargs` and `open`.
