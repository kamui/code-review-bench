# 01 — Connection configuration (`configure` callback, time zone and role setters)

Scope: `django/db/backends/postgresql/base.py` lines 89–104 (new module functions
`ensure_timezone` / `ensure_role`), line 231 (`configure=self._configure_connection`),
lines 362–383 (`DatabaseWrapper.ensure_timezone`, `_configure_connection`) and lines
401–408 (`init_connection_state`).

Covers summary findings F1 and F6.

## What the PR did

Before the PR, `init_connection_state()` called two wrapper methods,
`self.ensure_timezone()` and `self.ensure_role()`, both of which ran SQL on
`self.connection`. The pool needs to run the same two statements on a raw psycopg
connection that no wrapper owns yet, so the PR:

- hoisted the two bodies into module-level functions taking `(connection, ops, name)`;
- deleted `DatabaseWrapper.ensure_role()` outright and turned
  `DatabaseWrapper.ensure_timezone()` into a forwarding shim;
- added `DatabaseWrapper._configure_connection(connection)` and registered the *bound
  method* as the pool's `configure` callback;
- left a comment asking future maintainers to keep that method free of anything "on self
  aside from variables".

## F1 — the callback is not wrapper-free, and pool + `assume_role` cannot connect

The comment at lines 370–373 states the invariant the design needs:

```python
    def _configure_connection(self, connection):
        # This function is called from init_connection_state and from the
        # psycopg pool itself after a connection is opened. Make sure that
        # whatever is done here does not access anything on self aside from
        # variables.
```

Six lines later the method violates it:

```python
        role_name = self.settings_dict["OPTIONS"].get("assume_role")
        commit_role = ensure_role(connection, self.ops, role_name)
```

`ensure_role()` calls `ops.compose_sql("SET ROLE %s", [role_name])`.
`DatabaseOperations.compose_sql` (`operations.py:192–193`) is
`mogrify(sql, params, self.connection)`, where `self.connection` is the Django
`DatabaseWrapper`, and `mogrify` (`psycopg_any.py:20–22`) opens a *Django* cursor on it:

```python
    def mogrify(sql, params, connection):
        with connection.cursor() as cursor:
            return ClientCursor(cursor.connection).mogrify(sql, params)
```

So when the pool's worker thread configures a freshly opened connection, it calls
`wrapper.cursor()` on the wrapper that happened to create the pool. That wrapper has no
connection yet (its own thread is blocked in `pool.getconn()`), so `ensure_connection()`
calls `wrapper.connect()` *from the pool worker thread*, which calls `pool.getconn()`
again, which waits for the very connection that is being configured. Every worker ends up
in the same wait, and the caller times out.

### Verification

No PostgreSQL server is available in this environment, so the scenario was reproduced with
the real `psycopg_pool` (3.3.3) and the real Django code, with only
`psycopg.Connection.connect` replaced by a function returning a mock connection that
reports `TransactionStatus.IDLE` and a `TimeZone` parameter. Script:
`clone-work/scratch/probe2.py`, run as

```
PYTHONPATH=<clone> <cache>/venv/bin/python -B probe2.py
```

Output (pool options `{"min_size": 1, "max_size": 2, "timeout": 2}`):

```
[pool, no assume_role] connected after 0.0s
[pool, no assume_role] wrapper.connect() calls by thread: [('wrapper.connect()', 'MainThread')]
[pool + assume_role] psycopg_pool.PoolTimeout: couldn't get a connection after 2.00 sec after 2.0s
[pool + assume_role] wrapper.connect() calls by thread: [('wrapper.connect()', 'MainThread'),
    ('wrapper.connect()', 'pool-2-worker-0'), ('wrapper.connect()', 'pool-2-worker-1')]
```

The second run differs from the first only by `OPTIONS["assume_role"] = "app_owner"`. The
two extra `wrapper.connect()` calls are made from pool worker threads on a wrapper that
belongs to the main thread. Status: **verified against a fake driver; not verified against
a live server.** The re-entry itself (`_configure_connection` → `wrapper.connect()` from a
foreign thread) was also confirmed in isolation by `probe.py` section 4.

The test suite cannot catch this: the only role test, `test_connect_role`
(`tests/backends/postgresql/tests.py:395`), was switched to `no_pool_connection()` by this
PR, so the pooled path with a role is never executed.

### A second consequence of binding the callback to a wrapper

`configure=self._configure_connection` stores a bound method, so the pool keeps a strong
reference to whichever per-thread wrapper first evaluated `.pool`, and reads that
wrapper's `ops`, `timezone_name` and `settings_dict` from pool worker threads for the
lifetime of the pool. `probe2.py` confirms it:

```
pool created in dead thread's wrapper; main thread wrapper is same object: False
main thread sees same pool: True | configure bound to dead thread's wrapper: True
```

In a threaded server this pins the wrapper of the first request thread for the life of
the process and runs its code on threads that `validate_thread_sharing()` would reject.

## F6 — the extraction changed the class's surface without achieving the isolation

`DatabaseWrapper.ensure_role()` no longer exists. It had no leading underscore and sits
next to `ensure_timezone()`, which is a documented hook on `BaseDatabaseWrapper`
(`base/base.py:127–132`). Any backend that subclasses the PostgreSQL wrapper and
overrides or calls `ensure_role()` — or overrides `ensure_timezone()` expecting
`init_connection_state()` to go through it — silently loses that behaviour:
`init_connection_state()` now calls `_configure_connection()`, which calls the module
function directly and never dispatches to `self.ensure_timezone()`.

The module functions take `(connection, ops, name)`. Passing `ops` is the tell: `ops` is
a back-pointer to the wrapper, so the functions are not actually independent of it, and
the reshaping bought nothing. This is the "refactor that moves complexity around but
doesn't delete it" pattern: three callables (`ensure_timezone` function, `ensure_timezone`
method, `_configure_connection`) where there used to be two, plus a name collision between
a module function and a method that do different things (the method also closes the pool).

## Worked proposal

Keep the setters on the class, parameterised by the connection they act on, and make the
SQL composition use the raw connection it is handed. `mogrify()` already works with a raw
driver connection under both psycopg 3 and psycopg2, because all it needs is
`connection.cursor()`:

```python
    def ensure_timezone(self):
        if self.connection is None:
            return False
        return self._configure_timezone(self.connection)

    def _configure_timezone(self, connection):
        conn_timezone_name = connection.info.parameter_status("TimeZone")
        timezone_name = self.timezone_name
        if timezone_name and conn_timezone_name != timezone_name:
            with connection.cursor() as cursor:
                cursor.execute(self.ops.set_time_zone_sql(), [timezone_name])
            return True
        return False

    def ensure_role(self):
        return self._configure_role(self.connection)

    def _configure_role(self, connection):
        if new_role := self.settings_dict["OPTIONS"].get("assume_role"):
            with connection.cursor() as cursor:
                # Compose against the connection being configured, never
                # through self.ops, which would open a cursor on the wrapper.
                cursor.execute(mogrify("SET ROLE %s", [new_role], connection))
            return True
        return False
```

That removes both module functions, restores `ensure_role()`, and makes the role path
safe to run from the pool. To also stop the pool pinning a per-thread wrapper, capture
plain values when the pool is built instead of a bound method:

```python
    args = (self.ops.set_time_zone_sql(), self.timezone_name, role_name)
    pool = ConnectionPool(..., configure=lambda conn: configure_connection(conn, *args))
```

`clone-work/scratch/probe3.py` implements this as a scratch subclass and runs it against
the same fake driver. With pool and `assume_role` both set it connects immediately, the
wrapper's `connect()` is entered once (main thread only), and the pool worker executes
exactly the two expected statements:

```
b. pool + assume_role connected in 0.00s; wrapper.connect() threads: ['MainThread']; db=prod_db
   configure SQL (thread, sql, params): [(['worker', '0'], "SELECT set_config('TimeZone', %s, false)", ['UTC']),
                                         (['worker', '0'], "SET ROLE 'app_owner'", None)]
```

Status of the proposal: exercised offline with a fake driver only; it needs a run against
PostgreSQL before it is adopted. Whichever shape is chosen, add a pooled variant of
`test_connect_role` so the path is covered.
