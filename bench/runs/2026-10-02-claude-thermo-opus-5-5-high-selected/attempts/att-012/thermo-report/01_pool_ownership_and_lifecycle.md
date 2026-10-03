# 01 — Pool ownership, identity and lifecycle

Scope: `django/db/backends/postgresql/base.py` (the `_connection_pools` class
attribute, the `pool` property, `close_pool()`, the `pool` handling in
`get_connection_params()` and `get_new_connection()`), and
`django/db/backends/postgresql/creation.py`.

Covers summary findings 1 and 2.

## How the evidence was produced

No PostgreSQL server is provisioned, so nothing here was exercised against a
live database. `psycopg_pool.ConnectionPool(open=False)` does not connect when
it is constructed, which makes the pool *bookkeeping* observable offline. Two
scratch scripts were run from the work directory (not from the clone):

```text
PYTHONPATH=<clone> <cache>/venv/bin/python clone-work/scratch/probe.py
PYTHONPATH=<clone> <cache>/venv/bin/python clone-work/scratch/probe2.py
```

Installed versions: `psycopg 3.3.6`, `psycopg_pool 3.3.3`. Both scripts
configure one `default` alias with `ENGINE=django.db.backends.postgresql`,
`NAME="prod_db"`, `OPTIONS={"pool": True}` and never open a socket. The
interpreter wrote `__pycache__` directories into the clone while importing
Django; they were removed afterwards and `git status --short --ignored` is
empty again at `fad334e1a9`.

## Finding 1 — the pool is cached by alias but built from mutable settings

### The code

`postgresql/base.py:200` adds a class-level `_connection_pools = {}`.
`postgresql/base.py:202-241` is the `pool` property: on first access for an
alias it calls `self.get_connection_params()`, freezes the result into
`ConnectionPool(kwargs=...)`, binds `configure=self._configure_connection`, and
stores the pool under `self.alias`. Every later access for that alias returns
the stored pool without looking at the settings again.

`get_new_connection(conn_params)` at `postgresql/base.py:345-350` then does:

```python
if self.pool:
    # If nothing else has opened the pool, open it now.
    self.pool.open()
    connection = self.pool.getconn()
else:
    connection = self.Database.connect(**conn_params)
```

The `conn_params` argument, freshly computed by `BaseDatabaseWrapper.connect()`
on every connect, is discarded on the pooled branch.

### Why this is a structural problem

`settings_dict` is mutated in place by design, and the base backend relies on
"close the connection, change the dict, the next connect picks it up":

- `base/creation.py` `create_test_db()`: `self.connection.close()` then
  `self.connection.settings_dict["NAME"] = test_database_name`.
- `base/creation.py` `destroy_test_db()`: restores the old `NAME`.
- `base/creation.py` `set_as_test_mirror()`: rewrites `NAME`.
- `base/creation.py` `setup_worker_connection()`:
  `self.connection.settings_dict.update(settings_dict)` then `close()`.
- `django/test/signals.py:58-82`: time-zone changes via `setting_changed`.

A pool that outlives the wrapper's connection and is keyed by alias breaks that
contract: closing the connection no longer discards the parameters it was
opened with. The PR knows this. It patches three of the mutation sites by hand:

- `postgresql/base.py:362-364` — `ensure_timezone()` now starts with
  `self.close_pool()` ("Close the pool so new connections pick up the correct
  timezone").
- `postgresql/creation.py:61` — `_clone_test_db()` calls `close_pool()`.
- `postgresql/creation.py:89-91` — a new `_destroy_test_db()` override calls
  `close_pool()`.

and compensates for the remaining mismatch at
`postgresql/base.py:391-395`, where `_close()` returns the connection to
`self.connection._pool` rather than `self.pool` with the comment "This is a
workaround for tests so a pool can be changed on setting changes".

The `NAME` swap in `create_test_db()`, the restore in `destroy_test_db()`,
`set_as_test_mirror()` and `setup_worker_connection()` are not patched.

### Measured

Probe section 1 and 2 (`probe.py`):

```text
== 1. pool is keyed by alias; settings_dict NAME swap (create_test_db) is not seen
pool dbname before swap: prod_db
get_connection_params dbname: test_prod_db
pool dbname after swap  : prod_db | same pool object: True

== 2. setup_worker_connection-style settings update is not seen either
pool dbname: prod_db
```

and section 7:

```text
Sub._connection_pools is DatabaseWrapper._connection_pools: True
copy() with different NAME gets pool for: test_prod_db_1
```

So after the exact mutation `create_test_db()` performs,
`get_connection_params()` reports the test database while the pool the next
`connect()` will use still targets the original database.

### Consequence, and what it depends on

If a pool for an alias exists in the process before `create_test_db()` swaps
`NAME`, `call_command("migrate", database=alias)` and everything after it
check connections out of a pool that points at the non-test database. The
precondition is that something queried that alias earlier in the process.
Django's own runner only uses `_nodb_cursor()` (alias `NO_DB_ALIAS`, which the
property excludes) before the swap, so the stock path does not hit it. A
project that queries the database from `AppConfig.ready()` or at import time,
and then runs `manage.py test` with `"pool"` enabled, does. This end-to-end
path was not executed (no server); the stale-pool mechanism it rests on is the
measured part above.

The same mechanism applies to forked parallel test workers: they inherit the
class-level dict, and `setup_worker_connection()` rebinds the alias to
`test_<db>_<n>` without discarding a pool. Whether the parent has a live pool at
fork time depends on whether anything reconnected after the last
`_clone_test_db()`; that was not established and is raised as a question in the
summary rather than as a finding.

### Code-judo proposal

Stop treating the pool as a property of the alias. Treat it as a function of
the parameters `connect()` already computes and already passes to
`get_new_connection(conn_params)`. One pool per alias is still kept, but it
carries the spec it was built from, and a connect with a different spec
replaces it:

```python
# Sketch, not executed against a server.
_connection_pools = {}
_connection_pools_lock = threading.Lock()

def _pool_spec(self, conn_params):
    # Everything a pooled connection's state is derived from.
    return (
        {**conn_params, "autocommit": True},
        self.timezone_name,
        self.settings_dict["OPTIONS"].get("assume_role"),
        self.settings_dict["CONN_HEALTH_CHECKS"],
        self._pool_options,
    )

def _get_pool(self, conn_params):
    spec = self._pool_spec(conn_params)
    with self._connection_pools_lock:
        cached = self._connection_pools.get(self.alias)
        if cached is not None and cached[0] == spec:
            return cached[1]
        if cached is not None:
            cached[1].close()
        pool = self._create_pool(*spec)
        self._connection_pools[self.alias] = (spec, pool)
        return pool

def get_new_connection(self, conn_params):
    ...
    if self._pool_options is None:
        connection = self.Database.connect(**conn_params)
    else:
        pool = self._get_pool(conn_params)
        pool.open()
        connection = pool.getconn()
```

What this deletes:

- `self.close_pool()` inside `ensure_timezone()` — a time-zone change alters
  `timezone_name` and the adapters `context`, so the spec no longer matches and
  the next connect rebuilds the pool.
- The need to remember `close_pool()` at every `settings_dict` mutation site.
  The two calls in `creation.py` stay, but for their real reason: `CREATE
  DATABASE ... TEMPLATE` and `DROP DATABASE` need zero open connections.
- The second `get_connection_params()` call inside the property.
- The "first thread wins" `setdefault` comment block; a lock around a
  once-per-process operation is boring and obviously correct.
- The silent discarding of `conn_params`.

Trade-off to state openly: two wrappers that share an alias but carry different
settings would replace each other's pool. That only happens with
`connection.copy()` in tests, and `copy(alias=...)` already exists to give such
a copy its own alias.

## Finding 2 — `pool` is a predicate, a factory and a validator at once

### The code

`self.pool` is read eight times across five methods in `postgresql/base.py`:
lines 244 and 245 (`close_pool`), 345/347/348 (`get_new_connection`), 391
(`_close`), 404 (`init_connection_state`) and 502
(`close_if_health_check_failed`). In every one of those methods the first read
is a boolean test. "Is pooling on?" is therefore answered by a getter that may
import `psycopg_pool`, call `get_connection_params()`, raise
`ImproperlyConfigured`, construct a `ConnectionPool` and mutate a process-wide
dict. Only lines 245, 347 and 348 actually use the pool object.

### Measured

Probe section 4 — closing a pool that does not exist builds one first:

```text
ConnectionPool() constructions triggered by close_pool()+ensure_timezone() with no pool: ['test_prod_db_1', 'test_prod_db_1']
```

Probe section 5 — configuration errors surface from teardown and from a
`setting_changed` receiver:

```text
close_pool -> ImproperlyConfigured Pooling doesn't support persistent connections.
ensure_timezone -> ImproperlyConfigured Pooling doesn't support persistent connections.
```

Probe section 5b — the same validation is skipped once a pool exists:

```text
pool returned with CONN_MAX_AGE=60: True
```

Probe section 8 — `_close()` decides with `self.pool` but acts on
`self.connection._pool`; a connection that did not come from a pool (stand-in
object without `_pool`, matching psycopg, which only annotates the attribute
at `psycopg/_connection_base.py:126`) fails instead of being closed:

```text
AttributeError 'FakeConn' object has no attribute '_pool'
```

Probe section 6 — the `True | dict` option is splatted into a call that
already passes `kwargs`, `open`, `configure` and `check`:

```text
{'check': ...}      -> TypeError ... got multiple values for keyword argument 'check'
{'open': True}      -> TypeError ... got multiple values for keyword argument 'open'
{'configure': ...}  -> TypeError ... got multiple values for keyword argument 'configure'
{'kwargs': {}}      -> TypeError ... got multiple values for keyword argument 'kwargs'
1                   -> TypeError ... argument after ** must be a mapping, not int
'yes'               -> TypeError ... argument after ** must be a mapping, not str
```

Cost of the predicate on the per-cursor path (`_cursor()` →
`close_if_health_check_failed()` → `self.pool`), `probe2.py`: about 140 ns per
call with a pool, about 100 ns with pooling off. Small, but it is paid on every
cursor by every PostgreSQL user, pooled or not.

### Validation is in two non-canonical places

- `CONN_MAX_AGE != 0` is checked inside the lazy branch of the property
  (`postgresql/base.py:209-212`).
- `psycopg >= 3` is checked as a side effect of popping the option in
  `get_connection_params()` (`postgresql/base.py:290-292`).

`BaseDatabaseWrapper.check_settings()` (`base/base.py:263`) exists for exactly
this, is commented "Check for invalid configurations", and is the first thing
`connect()` calls. Nothing in the PostgreSQL backend overrides it today.

### Code-judo proposal

Split the three roles and put each where it already has a home:

```python
# Sketch, not executed against a server.
@cached_property
def _pool_options(self):
    """None when pooling is off, otherwise a dict of ConnectionPool options."""
    options = self.settings_dict["OPTIONS"].get("pool")
    if self.alias == NO_DB_ALIAS or not options:
        return None
    return {} if options is True else options

def check_settings(self):
    super().check_settings()
    if self._pool_options is None:
        return
    if not is_psycopg3:
        raise ImproperlyConfigured("Database pooling requires psycopg >= 3")
    if self.settings_dict["CONN_MAX_AGE"] != 0:
        raise ImproperlyConfigured("Pooling doesn't support persistent connections.")
    if reserved := {"kwargs", "open", "configure", "check"} & self._pool_options.keys():
        raise ImproperlyConfigured(...)

def close_pool(self):
    cached = self._connection_pools.pop(self.alias, None)
    if cached is not None:
        cached[1].close()

def _close(self):
    if self.connection is not None:
        with self.wrap_database_errors:
            if (pool := getattr(self.connection, "_pool", None)) is not None:
                pool.putconn(self.connection)
            else:
                return self.connection.close()
```

`_pool_options` is shown as a `cached_property` for brevity; if tests must keep
flipping `OPTIONS["pool"]` on a live wrapper it should be a plain property,
which is still a pure dict lookup.

After this, `close_pool()` cannot construct, cannot raise a configuration
error and cannot `KeyError` under a concurrent delete; the predicate is a pure
read; validation runs on every `connect()` rather than on the first pool
creation only; and `_close()` has one discriminator (where the connection came
from) instead of two that can disagree. Whether reserved keys should be
rejected or merged is the author's call; either is better than a bare
`TypeError` from inside a property.

## File size

No threshold is crossed. `postgresql/base.py` goes from 516 to 615 lines,
`base/base.py` from 788 to 792, `tests/backends/postgresql/tests.py` from 439
to 569 (`git show main:<path> | wc -l` versus `wc -l`).
