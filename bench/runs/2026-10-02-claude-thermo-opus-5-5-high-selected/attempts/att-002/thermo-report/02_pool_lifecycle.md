# 02 — Pool identity and lifecycle

Scope: `django/db/backends/postgresql/base.py` lines 200–246 (`_connection_pools`, `pool`,
`close_pool`), 345–350 (`get_new_connection`), 362–367 (`ensure_timezone`), 385–399
(`_close`), 401–408 (`init_connection_state`), 501–505 (`close_if_health_check_failed`),
and `django/db/backends/postgresql/creation.py` lines 61 and 89–91.

Covers summary findings F2, F3 and F4.

## The model the PR chose

- A class-level dict `DatabaseWrapper._connection_pools` maps **alias → pool**.
- `DatabaseWrapper.pool` is a property that returns `None` when pooling is off and
  otherwise *gets or creates* the alias's pool, performing validation, the
  `psycopg_pool` import and `get_connection_params()` on first creation.
- The pool's connection kwargs, time zone and role are whatever the first wrapper that
  touched `.pool` had at that moment.
- `close_pool()` closes and unregisters the alias's pool.

Three independent problems follow from that model. They share one fix.

## F2 — the cache key is the alias, but the cached value depends on the settings

`get_connection_params()` is recomputed by `BaseDatabaseWrapper.connect()` on every
connect and handed to `get_new_connection(conn_params)`. In the pooled branch the argument
is ignored:

```python
        if self.pool:
            # If nothing else has opened the pool, open it now.
            self.pool.open()
            connection = self.pool.getconn()
        else:
            connection = self.Database.connect(**conn_params)
```

The pool was built from an earlier snapshot of the same function, and nothing compares
the two. Django mutates `settings_dict` in place in several places:

| Site | Mutation | Pool invalidated? |
| --- | --- | --- |
| `base/creation.py:63–65` `create_test_db()` | `NAME` → test database | **no** |
| `base/creation.py:313–316` `destroy_test_db()` with `keepdb=True` | `NAME` → original | **no** (`_destroy_test_db` is skipped) |
| `base/creation.py:377–384` `setup_worker_connection()` | `NAME` → per-worker clone | **no** |
| `postgresql/creation.py:61` `_clone_test_db()` | none, but needs no open connections | yes (added by PR) |
| `postgresql/creation.py:89–91` `_destroy_test_db()` | none, but needs no open connections | yes (added by PR) |
| `test/signals.py:72–82` on `TIME_ZONE` / `USE_TZ` | time zone | yes, via `ensure_timezone()` (added by PR) |

`probe.py` section 1 shows the stale state directly (no server needed, the pool is never
opened):

```
== 1. stale pool after settings_dict change (create_test_db-style NAME swap)
pool dbname: prod_db
settings NAME now: test_prod_db | get_connection_params dbname: test_prod_db | pool dbname: prod_db | same pool: True
```

and section 6 shows that a `copy()` with a different `NAME` and `TIME_ZONE` on the same
alias gets the original's pool:

```
orig pool dbname: prod_db | copy pool dbname: prod_db
```

What this means for `create_test_db()`: if anything has opened a pooled connection on the
alias before the test database is set up, the pool keeps handing out connections to the
pre-test database after `NAME` has been switched, and `migrate`, `flush` and the tests run
against it. The precondition is easy to meet — `connection.features.is_postgresql_15`
resolves `pg_version` through `temporary_connection()` (`features.py:148–156`,
`base/base.py:685–699`), which opens and "closes" (returns to the pool) a connection, and
application code that queries during startup does the same. Status: the stale pool is
**verified**; whether Django's own runner trips it before `create_test_db()` is **not
verified** (no server here) and is raised as question Q1 in the summary. The structural
point does not depend on the answer: correctness currently relies on every site that
mutates `settings_dict` remembering to call `close_pool()`. The PR covered the time-zone
signal and the two sites that need idle connections closed, and none of the three sites
that change `NAME`.

## F3 — `pool` is a factory dressed as an attribute, and is used as a boolean

`self.pool` is read eight times in five methods (`grep -n "self.pool"` on the head
revision: lines 244, 245, 345, 347, 348, 391, 404, 502): `close_pool`,
`get_new_connection`, `_close`, `init_connection_state` and
`close_if_health_check_failed`. Four of those methods only want to know "is this wrapper
pooled?", but each read can construct a pool. `close_if_health_check_failed()` is called
from `_cursor()` and `set_autocommit()` (`base/base.py:295, 472`), so the get-or-create
path runs on every cursor.

Measured side effects (`probe.py`, sections 2 and 3, spying on
`ConnectionPool.__init__` / `close`):

```
== 2. close_pool() with no pool builds one just to close it
registry before: {}
calls from ensure_timezone() on a never-connected wrapper: ['init', 'close']
registry after: {}
== 3. _close() after close_pool() builds a fresh pool as a side effect of `if self.pool`
calls during _close(): ['init'] | putconn went to conn._pool: True | new registry pool is old: False
```

Section 2 is what happens for every initialised PostgreSQL connection each time a test
overrides `TIME_ZONE` or `USE_TZ`. Section 3 is the state after `ensure_timezone()` on a
wrapper that holds a connection: closing that wrapper registers a brand-new pool purely
because `if self.pool:` was evaluated.

Because the first read is also where validation lives, the `ImproperlyConfigured` errors
for `CONN_MAX_AGE` and a missing `psycopg_pool` surface from whichever method happens to
touch `.pool` first, including `_destroy_test_db()` and `ensure_timezone()` inside a
`setting_changed` receiver.

The registry is also unsynchronised. The comment explains that `setdefault()` makes the
losing thread's pool harmless, which is true for creation, but
`return self._connection_pools[self.alias]` and `del self._connection_pools[self.alias]`
are separate dictionary operations from the membership test, so a concurrent
`close_pool()` yields `KeyError`. A lock is the boring answer and removes the need for the
explanatory comment.

## F4 — test accommodations live in production methods

`DatabaseWrapper.ensure_timezone()` is documented on the base class as "Ensure the
connection's timezone is set to `self.timezone_name` and return whether it changed or
not." After the PR its first statement is `self.close_pool()`, a process-wide action that
makes every waiting and future client of that alias fail with `PoolClosed`. Its only
caller in Django is the `setting_changed` receiver, so the behaviour exists for tests.

That in turn forces the second accommodation, which the code labels itself:

```python
                if self.pool:
                    # Ensure the correct pool is returned. This is a workaround
                    # for tests so a pool can be changed on setting changes
                    # (e.g. USE_TZ, TIME_ZONE).
                    self.connection._pool.putconn(self.connection)
```

`_pool` is a private attribute of psycopg's connection class
(`psycopg_pool/pool.py` sets it at lines 284 and 632 and clears it at 755 and 830 in the
installed 3.3.3). Production code now depends on a driver internal to recover from a
state that only the test signal creates.

Both disappear if the wrapper records which pool its current connection came from.

## Worked proposal: separate "is pooling configured" from "the pool", and remember the origin

```python
    _connection_pools = {}  # alias -> (signature, pool)
    _connection_pools_lock = threading.Lock()
    _connection_pool = None  # Pool self.connection was checked out of.

    @property
    def pool_options(self):
        """ConnectionPool keyword arguments, or None when pooling is off."""
        options = self.settings_dict["OPTIONS"].get("pool")
        if not options or self.alias == NO_DB_ALIAS:
            return None
        return {} if options is True else options

    def _get_pool(self, conn_params):
        signature = (conn_params, self._configure_args(), self.pool_options)
        with self._connection_pools_lock:
            current = self._connection_pools.get(self.alias)
            if current is not None and current[0] != signature:
                current[1].close()
                current = None
            if current is None:
                current = self._connection_pools[self.alias] = (
                    signature,
                    self._create_pool(conn_params),
                )
        return current[1]

    def close_pool(self):
        with self._connection_pools_lock:
            current = self._connection_pools.pop(self.alias, None)
        if current is not None:
            current[1].close()

    def get_new_connection(self, conn_params):
        ...
        if self.pool_options is None:
            connection = self.Database.connect(**conn_params)
        else:
            pool = self._get_pool(conn_params)
            pool.open()
            connection = pool.getconn()
            self._connection_pool = pool
        ...

    def _close(self):
        if self.connection is None:
            return
        pool, self._connection_pool = self._connection_pool, None
        with self.wrap_database_errors:
            if pool is None:
                return self.connection.close()
            pool.putconn(self.connection)
            self.connection = None

    def close_if_health_check_failed(self):
        if self._connection_pool is None:
            super().close_if_health_check_failed()
```

What this deletes or fixes:

- `get_new_connection(conn_params)` uses its argument again; a changed `NAME`, time zone
  or role rebuilds the pool at the next connect, so `create_test_db()`,
  `setup_worker_connection()` and the `keepdb` teardown need no pool-specific code.
- `ensure_timezone()` returns to its pre-PR body. The `close_pool()` call and the
  `connection._pool` access both go away, together with the "workaround for tests"
  comment.
- No read ever constructs a pool except the one place that is about to check a connection
  out. `init_connection_state()` and `close_if_health_check_failed()` ask a fact about the
  current connection (`self._connection_pool`), not a factory.
- The two `creation.py` hooks stay: `CREATE DATABASE ... TEMPLATE` and `DROP DATABASE`
  genuinely require that idle pooled connections be closed.
- The ~45 lines of pool construction fit naturally in a small
  `django/db/backends/postgresql/pool.py` (registry, lock, `get`, `close`), which keeps
  `DatabaseWrapper` about the wrapper. `base.py` grows from 516 to 615 lines in this PR;
  that is well under the 1 000-line threshold, so this is a preference, not a blocker.

Offline check of the sketch (`clone-work/scratch/probe3.py`, fake driver):

```
a. pools constructed by close_pool()/ensure_timezone() on idle wrapper: []
c. after close(): connection is None | _connection_pool is None | pools built: ['prod_db']
d. after NAME swap: connected to test_prod_db | pools built: ['prod_db', 'test_prod_db']
e. _close() after close_pool(): pools built during close: [] | old pool closed: True
```

Status of the proposal: exercised offline with a fake driver only. One caveat to settle
when implementing: the signature contains `conn_params["context"]`, which comes from the
`lru_cache`d `get_adapters_template()` and so compares by identity correctly, but any
unhashable or non-comparable value a user puts in `OPTIONS` must compare with `==` for
the mismatch test to be meaningful; a dict comparison is sufficient, hashing is not
required.

## Smaller observations in the same area

- `pool.open()` is called on every checkout "if nothing else has opened the pool". It is
  idempotent in psycopg_pool, so this is harmless, but opening belongs with creation.
- User-supplied pool options are splatted after Django's own keywords, so the keys
  `check`, `kwargs` and `open` raise a bare `TypeError` (`probe.py` section 5:
  `ConnectionPool() got multiple values for keyword argument 'check'`); `configure`
  collides by the same mechanism. The comment in `_close()` also notes that a
  user-supplied `reset` breaks error wrapping. See file 03 for the validation proposal.
- Line 236 has a typo: "it's init" should be "its init".
