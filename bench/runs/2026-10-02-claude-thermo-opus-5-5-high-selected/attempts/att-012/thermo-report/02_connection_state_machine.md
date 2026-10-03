# 02 — Connection state machine, configuration callback, health checks

Scope: `django/db/backends/base/base.py` (`ensure_connection`),
`django/db/backends/postgresql/base.py` (`_close`, `is_usable`,
`init_connection_state`, `_configure_connection`, `ensure_timezone`,
`ensure_role`, `close_if_health_check_failed`) and
`django/db/backends/postgresql/features.py` (`django_test_skips`).

Covers summary findings 3, 4 and 5.

Verification status for this file: everything is from reading the code at
`fad334e1a9` and the installed `psycopg 3.3.6` / `psycopg_pool 3.3.3` sources,
plus the two offline probes described in `01_pool_ownership_and_lifecycle.md`.
No transaction or pool behaviour was exercised against a live server.

## Finding 3 — a pool-only state leaks into `BaseDatabaseWrapper`

### The invariant that existed before the PR

`BaseDatabaseWrapper.close()` (`base/base.py:344-361`) has two outcomes:

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

Inside an atomic block the wrapper deliberately keeps the (now closed) driver
connection. Later queries fail on that dead handle, and `Atomic.__exit__`
(`django/db/transaction.py:304` and `:310`) is what finally sets
`connection.connection = None`. So `closed_in_transaction` implied
"`self.connection` is a closed handle", and `self.connection is None` implied
"not in a closed-in-transaction state". `closed_in_transaction` is set at
exactly one place, `base/base.py:358`.

### What the PR does to it

The pooled branch of `_close()` (`postgresql/base.py:385-399`) hands the
connection back and then clears the wrapper's reference itself:

```python
if self.pool:
    self.connection._pool.putconn(self.connection)
    # Connection can no longer be used.
    self.connection = None
else:
    return self.connection.close()
```

That is necessary given the choice to return a live connection (the wrapper
must not keep using something another thread can check out). But it creates a
state no backend could reach before: `in_atomic_block and
closed_in_transaction and connection is None`. In that state
`ensure_connection()` would quietly open a new connection in the middle of an
atomic block. Three compensations follow, in three different places:

- `base/base.py:274-277` — a new branch in the shared `ensure_connection()`
  raising `ProgrammingError("Cannot open a new connection in an atomic
  block.")`, plus the new `ProgrammingError` import at `base/base.py:20`. For
  every non-pooled backend this branch is unreachable.
- `postgresql/base.py:490-491` — `is_usable()` gains `if self.connection is
  None: return False`, although the base docstring (`base/base.py:565-576`)
  says "This method may assume that self.connection is not None". The existing
  test `tests/backends/tests.py:476-494` calls `connection._close()` directly
  and then `is_usable()`, which is why this guard is needed once `_close()`
  nulls the reference.
- `postgresql/base.py:404` — `init_connection_state()` gains `if
  self.connection is not None`, with no comment saying when it could be `None`
  there. `connect()` assigns `self.connection` immediately before calling it.

The user-visible error after closing inside `atomic()` also forks: non-pooled
connections keep raising the driver's "connection is closed" error, pooled
ones raise the new `ProgrammingError`.

### Code-judo proposal

Keep the base invariant instead of teaching the base class a new state. When a
pooled connection is closed inside an atomic block, really close it and then
return it; `psycopg_pool` discards a returned connection whose transaction
status is `UNKNOWN` and schedules a replacement
(`psycopg_pool/pool.py:727-732`, "discarding closed connection"). `conn.close()`
on a pooled psycopg connection is a real close unless the pool was created with
`close_returns` (`psycopg/connection.py:187-200`), which this PR does not set.

```python
# Sketch, not executed against a server.
def _close(self):
    if self.connection is not None:
        with self.wrap_database_errors:
            pool = getattr(self.connection, "_pool", None)
            if pool is None:
                return self.connection.close()
            if self.in_atomic_block:
                # Same contract as a non-pooled connection: the wrapper keeps a
                # dead handle until the outermost atomic block exits. The pool
                # discards it and opens a replacement.
                self.connection.close()
                pool.putconn(self.connection)
            else:
                pool.putconn(self.connection)
                # A returned connection may be handed to another thread.
                self.connection = None
```

With that, the branch in `BaseDatabaseWrapper.ensure_connection()`, the
`ProgrammingError` import and `test_cannot_open_new_connection_in_atomic_block`
can be deleted, and closing inside `atomic()` behaves the same with and without
a pool. The cost is one pooled connection replaced per close-inside-atomic,
which is an error path. The `is_usable()` guard still has to stay because of
the direct `_close()` call in `tests/backends/tests.py:484`; it should then say
so in a comment, since it contradicts the base docstring.

If the author prefers to keep the base guard as a general safety net, it
should be justified as one (and tested through a real close inside `atomic()`),
not introduced as a side effect of one backend's `_close()`.

## Finding 4 — the configuration callback is half-extracted

### The shape after the PR

There are now four things involved in "set time zone and role on a new
connection":

1. module function `ensure_timezone(connection, ops, timezone_name)`
   (`postgresql/base.py:89-95`);
2. module function `ensure_role(connection, ops, role_name)`
   (`postgresql/base.py:98-104`);
3. method `DatabaseWrapper._configure_connection(self, connection)`
   (`postgresql/base.py:369-383`), which calls 1 and 2 with values read from
   `self`;
4. method `DatabaseWrapper.ensure_timezone(self)` (`postgresql/base.py:362-367`),
   which shares its name with 1, and now first closes the alias's pool and then
   calls 1.

`DatabaseWrapper.ensure_role()` was removed outright.
`BaseDatabaseWrapper.ensure_timezone()` (`base/base.py:127`) is documented as
"Ensure the connection's timezone is set to `self.timezone_name` and return
whether it changed or not"; its only caller in the repository is the
`setting_changed` receiver at `django/test/signals.py:82`.

### Problems

The functions were pulled out to module level so they would not depend on a
wrapper, but the callback given to the pool is still a bound method:
`configure=self._configure_connection` (`postgresql/base.py:231`). The pool is
process-wide and the wrapper is per-thread, so the pool pins whichever
thread's wrapper happened to create it, and calls into it from pool worker
threads. `probe2.py`:

```text
configure is bound to first wrapper: True
wrapper of dead thread still alive (held by pool.configure): True
```

The safety of that is carried by a comment (`postgresql/base.py:370-373`):
"Make sure that whatever is done here does not access anything on self aside
from variables." The method then reads `self.ops`, `self.settings_dict` and
`self.timezone_name`, the last of which is a `cached_property` that may be
computed for the first time on a pool worker thread. A constraint that matters
this much should be enforced by the signature, not by a comment.

The method `ensure_timezone()` now has a side effect its name and its base
docstring do not suggest: it tears down a pool shared by every thread in the
process. It does so for every initialized connection on every `TIME_ZONE` /
`USE_TZ` override in a test run. That is test-support behaviour living inside a
production method, and it is the mirror image of the "workaround for tests"
comment in `_close()`.

`_configure_connection()` returns `commit_role or commit_tz`. The pool ignores
the return value; only `init_connection_state()` uses it. One function is
serving two callers with different contracts.

### Code-judo proposal

One wrapper-free function, closed over plain values when handed to the pool:

```python
# Sketch, not executed against a server.
def configure_connection(connection, *, ops, timezone_name, role_name):
    """Apply session state. Return True if anything was changed."""
    changed = False
    if timezone_name and connection.info.parameter_status("TimeZone") != timezone_name:
        with connection.cursor() as cursor:
            cursor.execute(ops.set_time_zone_sql(), [timezone_name])
        changed = True
    if role_name:
        with connection.cursor() as cursor:
            cursor.execute(ops.compose_sql("SET ROLE %s", [role_name]))
        changed = True
    return changed
```

The pool gets `functools.partial(configure_connection, ops=self.ops,
timezone_name=..., role_name=...)` built from the same values that form the
pool spec in `01_pool_ownership_and_lifecycle.md`. `init_connection_state()`
calls the same function with `self.connection`. The method
`ensure_timezone()` keeps its pre-PR meaning and stops closing pools, because a
changed time zone changes the spec. That removes two module functions, one
method, the name collision and the comment-enforced constraint, and the pool no
longer holds a wrapper. `psycopg2` returns `connection.info` and `ops` the same
way, so nothing here is psycopg3-specific. (`ops` still references its wrapper;
if that matters, pass the two SQL strings instead.)

Whether removing `DatabaseWrapper.ensure_role()` is acceptable for third-party
subclasses of the PostgreSQL backend is a compatibility call for the author;
the PR does not mention it.

## Finding 5 — `close_if_health_check_failed()` override looks dead, and costs two test skips

### The code

`postgresql/base.py:501-505`:

```python
def close_if_health_check_failed(self):
    if self.pool:
        # The pool only returns healthy connections.
        return
    return super().close_if_health_check_failed()
```

and `postgresql/features.py:86-108`, where `django_test_skips` changes from a
class-level dict to a `cached_property` so that two
`ConnectionHealthChecksTests` can be skipped when `"pool"` is set.

### Why the override changes nothing in a supported configuration

The base implementation (`base/base.py:578-589`) returns early when
`self.connection is None`, when health checks are disabled, or when
`self.health_check_done` is true. `connect()` sets `health_check_done = True`
(`base/base.py:253`). The only place that sets it back to `False` is
`close_if_unusable_or_obsolete()` (`base/base.py:597`), and that method then
closes the connection whenever `time.monotonic() >= self.close_at`
(`base/base.py:614-616`). Pooling requires `CONN_MAX_AGE == 0`
(`postgresql/base.py:209-212`), so `close_at` equals the connect time and that
condition is always true. After the close the pooled `_close()` leaves
`self.connection = None`. So with pooling on, every path that makes
`health_check_done` false also makes `self.connection` `None`, and the base
method returns at its first guard. The override never changes the outcome; it
only adds a `self.pool` evaluation to every `_cursor()` and
`set_autocommit()` call for all PostgreSQL users.

The two skipped tests reach the overridden path only because their helper
patches `CONN_MAX_AGE` to `None` on a live wrapper
(`tests/backends/base/test_base.py:224-234`), a configuration the pool rejects
at creation time. The skip reason, "Pool does implicit health checks", is also
only true when `CONN_HEALTH_CHECKS` is on (`check=... if enable_checks else
None`, `postgresql/base.py:232`).

### Remedy

Delete the override. Then re-run the two tests against a pooled configuration:
if they pass, `django_test_skips` can go back to being a plain class attribute
and the settings read in `features.py` disappears. If they still fail, they
fail because the helper forces `CONN_MAX_AGE=None` under pooling, and the skip
reason should say that. This reasoning is from reading the code; the tests
could not be run here.
