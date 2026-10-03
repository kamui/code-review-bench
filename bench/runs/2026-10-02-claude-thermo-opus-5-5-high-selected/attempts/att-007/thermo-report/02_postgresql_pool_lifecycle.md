# 02 — PostgreSQL backend: pool ownership, configuration callback, and lifecycle

Scope: `django/db/backends/postgresql/base.py` (+122/−23), `creation.py` (+5), `features.py` (+23/−9).

Review range: `bcccea3ef3..fad334e1a9`. All probes below ran offline with `PYTHONPATH=clone PYTHONDONTWRITEBYTECODE=1 clone-cache/venv/bin/python` (psycopg 3.3.6, psycopg_pool 3.3.3). No PostgreSQL server is available, so anything that needs a live connection is marked as not run.

## Measurements

| File | Lines at head | Crosses 1k? |
| --- | --- | --- |
| `django/db/backends/postgresql/base.py` | 615 | no |
| `django/db/backends/base/base.py` | 792 | no |
| `tests/backends/postgresql/tests.py` | 569 | no |

File size is not a concern in this PR. The concerns are about where state lives and how many places have to know about it.

Places that now branch on "is pooling on" (`self.pool` used as a boolean): `get_new_connection` (line 345), `_close` (391), `init_connection_state` (404), `close_if_health_check_failed` (502), `close_pool` (244), plus `features.django_test_skips` and `get_connection_params` reading `OPTIONS["pool"]` directly. Places that invalidate the pool: `ensure_timezone` (364), `creation._clone_test_db` (61), `creation._destroy_test_db` (90), and five `finally: close_pool()` blocks in the tests.

---

## A. The pool's `configure` callback is a bound method of one thread's wrapper, and it re-enters that wrapper

### Evidence

```python
# postgresql/base.py:228-234
pool = ConnectionPool(
    kwargs=connect_kwargs,
    open=False,  # Do not open the pool during startup.
    configure=self._configure_connection,
    ...
)
```

```python
# postgresql/base.py:369-383
def _configure_connection(self, connection):
    # This function is called from init_connection_state and from the
    # psycopg pool itself after a connection is opened. Make sure that
    # whatever is done here does not access anything on self aside from
    # variables.
    commit_tz = ensure_timezone(connection, self.ops, self.timezone_name)
    role_name = self.settings_dict["OPTIONS"].get("assume_role")
    commit_role = ensure_role(connection, self.ops, role_name)
    return commit_role or commit_tz
```

```python
# postgresql/base.py:98-104
def ensure_role(connection, ops, role_name):
    if role_name:
        with connection.cursor() as cursor:
            sql = ops.compose_sql("SET ROLE %s", [role_name])
            cursor.execute(sql)
        return True
    return False
```

`ops.compose_sql()` is `mogrify(sql, params, self.connection)` (`operations.py:192-193`), and `mogrify` is `with connection.cursor() as cursor: ...` on the **Django wrapper** (`psycopg_any.py:20-22`). So the comment's rule ("do not access anything on self aside from variables") is violated by the very first helper the method calls: `SET ROLE` composition opens a Django cursor on the wrapper that created the pool.

Django wrappers are per-thread objects; the pool is per-process, and psycopg_pool invokes `configure` from its own worker threads. The pool therefore pins the first thread's wrapper for its whole lifetime and calls into it from foreign threads.

### Probe (`scratch/t_pool2.py`, `scratch/t_role.py`)

The probe takes `pool._configure` — the exact callable psycopg_pool invokes — and calls it from another thread with a mock driver connection, with `OPTIONS = {"pool": {...}, "assume_role": "app_owner"}`.

```text
configure callback bound to first wrapper: True
{'distinct per-thread wrapper': True, 'shares pool': True,
 "pool callback still bound to first thread's wrapper": True}

(a) wrapper.connection is None -> OperationalError: couldn't get a connection after 0.50 sec
   call path: worker > _configure_connection > ensure_role > compose_sql > mogrify > inner > cursor
              > _cursor > inner > ensure_connection > __exit__ > ensure_connection > spy > inner
              > connect > inner > get_new_connection > getconn
   wrapper.connect() invoked from: ['pool-worker']
(b) wrapper already connected -> DatabaseError: DatabaseWrapper objects created in a thread can only
    be used in that same thread. The object with alias 'default' was created in thread id ...
   call path: worker > _configure_connection > ensure_role > compose_sql > mogrify > inner > cursor
              > _cursor > _prepare_cursor > validate_thread_sharing
(c) no assume_role -> ok
```

Case (a) is the first checkout: the owning thread is blocked in `pool.getconn()`, so `wrapper.connection` is still `None`; the worker's `configure` call runs `wrapper.connect()` on the worker thread, which calls `pool.getconn()` on the pool that is waiting for this very connection to finish configuring. Case (b) is any later pool growth: the thread-sharing check rejects the call. In both cases `configure` raises, so psycopg_pool cannot complete any connection.

Verification status: the call path and both exceptions are **confirmed by execution** with the real callback. The end-user symptom with a live server (expected: `PoolTimeout` after the pool timeout instead of a working connection) is inferred, not run. The existing role test cannot catch this because it was switched to `no_pool_connection()` (`tests/backends/postgresql/tests.py:402`).

### Why this is a structure problem, not just a bug

The diff extracted `ensure_timezone` and `ensure_role` to module level, which looks like a step toward "pure functions of a raw connection". But it stopped half-way:

- they still take `ops`, a wrapper-bound object, only to obtain one constant SQL string (`set_time_zone_sql()` returns a literal) and to call `compose_sql`, which reaches back into the wrapper;
- the thing handed to the pool is still `self._configure_connection`, a bound method that reads `self.timezone_name` (a `cached_property` that the test signal handler deletes and recomputes) and `self.settings_dict` at call time;
- the module function `ensure_timezone(connection, ops, timezone_name)` now shares its name with the method `DatabaseWrapper.ensure_timezone()` three screens below, which calls it and additionally closes the pool. Two different things with one name in one file;
- the public method `DatabaseWrapper.ensure_role()` was removed in the process.

The result is four layers (`init_connection_state` → `_configure_connection` → module helper → `ops`) for two `SET` statements, and the safety of the cross-thread call rests on a comment.

### Worked proposal

Make the callback a real pure function of a raw psycopg connection and plain values, and build it once where the pool is built.

```python
def _configure_connection(connection, *, timezone_name, role_name):
    """Session setup for a raw driver connection. Return whether anything changed."""
    changed = False
    if timezone_name and connection.info.parameter_status("TimeZone") != timezone_name:
        with connection.cursor() as cursor:
            cursor.execute("SELECT set_config('TimeZone', %s, false)", [timezone_name])
        changed = True
    if role_name:
        with connection.cursor() as cursor:
            cursor.execute(sql.SQL("SET ROLE {}").format(sql.Literal(role_name)))
        changed = True
    return changed
```

`sql` is already imported from `psycopg_any` in this module and exists for both drivers, so no wrapper, no `ops`, and no `mogrify` round trip through a Django cursor is needed. The wrapper then has exactly one adapter:

```python
def _session_config(self):
    return {
        "timezone_name": self.timezone_name,
        "role_name": self.settings_dict["OPTIONS"].get("assume_role"),
    }

# pool construction
configure=functools.partial(_configure_connection, **self._session_config())

# init_connection_state, non-pooled
if _configure_connection(self.connection, **self._session_config()) and not self.get_autocommit():
    self.connection.commit()
```

This deletes the two module helpers, the `ops` parameters, the "be careful" comment, and the name collision; the pool no longer holds a reference to any wrapper. `DatabaseWrapper.ensure_role()` can stay as a one-line method for compatibility. A pooled `assume_role` test must be added; it needs a live server and was not run here.

---

## B. The pool registry is keyed by alias only, so invalidation is hand-placed around the codebase

### Evidence

```python
# postgresql/base.py:200
_connection_pools = {}
```

The cached value is a function of `get_connection_params()` (database name, user, host, every `OPTIONS` entry, the adapters context built from `USE_TZ`/`TIME_ZONE`), `CONN_HEALTH_CHECKS`, the pool options, the time zone name and the role. The cache key is `self.alias`. Nothing ties the two together, so every place that can change an input must remember to invalidate:

1. `ensure_timezone()` — `self.close_pool()` with the comment "Close the pool so new connections pick up the correct timezone" (`base.py:362-364`). The base contract for this method is "Ensure the connection's timezone is set to `self.timezone_name` and return whether it changed or not" (`base/base.py:127-132`). It is called by the `setting_changed` handler in `django/test/signals.py:82` for every initialized connection. After this PR it tears down a process-wide resource that other threads are using.
2. `_close()` — `self.connection._pool.putconn(self.connection)` with the comment "This is a workaround for tests so a pool can be changed on setting changes" (`base.py:392-395`). Production code reaches into a private psycopg attribute because item 1 can swap the pool underneath a checked-out connection.
3. `creation._clone_test_db()` and a new `creation._destroy_test_db()` override (`creation.py:61`, `89-91`).
4. The tests: `no_pool_connection()` exists because `connection.copy()` shares the alias and therefore the pool, so per-test `OPTIONS` would be ignored; and five tests end with `finally: new_connection.close_pool()`.

Sites that change inputs and were **not** updated: `BaseDatabaseCreation.setup_worker_connection()` updates `settings_dict` in place to point at the clone database and then only calls `self.connection.close()` (`base/creation.py:377-384`).

### Probe (`scratch/t_pool.py`, section 1)

```text
dbname at creation: db_a
get_connection_params dbname: db_a_1 | pool still targets: db_a | same pool object: True
```

After the same in-place `settings_dict` update that `setup_worker_connection()` performs, `connect()` computes parameters for `db_a_1`, discards them (`get_new_connection` ignores `conn_params` in the pooled branch, `base.py:345-350`) and checks out a connection to `db_a`.

Verification status: staleness after an in-place settings update is **confirmed by execution**. Whether a forked parallel test worker actually inherits a live pool for the alias depends on whether the parent touched the database between cloning and forking; that was not run (no server) and is raised as a question in the summary rather than asserted.

### Worked proposal

The cheapest way to delete most of this is to notice that the time-zone half of the session setup is already idempotent and free: it compares `connection.info.parameter_status("TimeZone")` (client-side state, no round trip) with the wanted name and only issues `set_config` on mismatch. So it can run at every checkout, pooled or not, at zero cost in the steady state:

```python
def init_connection_state(self):
    super().init_connection_state()
    # Free when the session already has the right zone; self-heals pooled
    # connections after TIME_ZONE / USE_TZ changes.
    changed = self.ensure_timezone()
    if not self._pool_options:
        changed |= self.ensure_role()      # pooled connections got the role in `configure`
    if changed and not self.get_autocommit():
        self.connection.commit()
```

Consequences:

- `ensure_timezone()` goes back to its documented meaning and its pre-PR body; the `close_pool()` call disappears.
- No pool swap can happen under a checked-out connection any more, so `_close()` uses `self.pool.putconn(...)` and the `connection._pool` reach-in and its "workaround for tests" comment disappear.
- The adapters context captured in the pool's `kwargs` does not need refreshing either: `create_cursor()` already re-registers the tz loader per cursor when the connection's loader disagrees (`base.py:435-440`).
- The only remaining invalidation is the test-database lifecycle, which is a genuine "the target database is going away" event. That belongs in one place. `close_pool()` should be called from one `DatabaseCreation` hook that both clone and destroy go through, and `setup_worker_connection()` needs the same treatment.

If the author prefers to keep "one pool per configuration" explicit, the alternative is to store the inputs next to the pool (`_connection_pools[alias] = (signature, pool)`) and rebuild on signature mismatch in the single accessor. Either way the rule is: one owner for invalidation, not four call sites and a private-attribute escape hatch.

psycopg_pool also ships `ConnectionPool.drain()` ("useful to force a connection re-configuration, for example when the adapters map changes after the pool was created"; present in the installed 3.3.3 — availability at the PR's `>=3.2.0` floor was not checked). It is the library's own answer to "reconfigure without swapping the pool" and would be the canonical tool if a re-configuration hook is still wanted after the change above.

Verification status of the proposal: reasoning from the code and from libpq's parameter-status semantics; **not run** against a server.

---

## C. `pool` is a predicate, a lazy factory and a validator in one property

### Evidence

```python
# postgresql/base.py:202-246
@property
def pool(self):
    pool_options = self.settings_dict["OPTIONS"].get("pool")
    if self.alias == NO_DB_ALIAS or not pool_options:
        return None
    if self.alias not in self._connection_pools:
        if self.settings_dict.get("CONN_MAX_AGE", 0) != 0:
            raise ImproperlyConfigured(...)
        ...
        try:
            from psycopg_pool import ConnectionPool
        except ImportError as err:
            raise ImproperlyConfigured(...) from err
        ...
        self._connection_pools.setdefault(self.alias, pool)
    return self._connection_pools[self.alias]

def close_pool(self):
    if self.pool:
        self.pool.close()
        del self._connection_pools[self.alias]
```

Every `if self.pool:` in the class is therefore "build a pool if there isn't one, validate settings, maybe raise". Probes (`scratch/t_pool.py`, sections 2, 4, 6):

```text
== 2. close_pool() with no existing pool constructs one in order to close it
ConnectionPool() constructions during close_pool(): 1
... and after a following ensure_timezone(): 2

== 4. validation lives in the lazy property: ensure_timezone()/close_pool() raise config errors
ensure_timezone -> ImproperlyConfigured Pooling doesn't support persistent connections.
close_pool -> ImproperlyConfigured Pooling doesn't support persistent connections.
check_settings() (canonical hook) -> accepts the invalid combination

== 6. property evaluated per use
pool property evaluations in get_new_connection(): 3
in _close(): 1
in close_pool(): 2
```

Specific consequences:

- `close_pool()` on an alias with no pool creates a `ConnectionPool` (running `get_connection_params()` and the import) just to close and delete it. `ensure_timezone()` does this for every pooled alias on every `TIME_ZONE`/`USE_TZ` override in a test run; `_destroy_test_db()` does it at teardown.
- A teardown or signal handler can raise `ImproperlyConfigured` because a *lookup* validates settings.
- `get_new_connection()` evaluates the property three times (`if self.pool:` / `self.pool.open()` / `self.pool.getconn()`). Because `close_pool()` in another thread can delete the registry entry between evaluations, `open()` can land on the old (closed, not reopenable) pool or `getconn()` on a fresh unopened one. `close_pool()` itself is check-then-`del` with two evaluations.
- Validation is split between this property (`CONN_MAX_AGE`, import) and `get_connection_params()` (`pool_options and not is_psycopg3`, `base.py:290-292`), while the hook that exists for exactly this, `check_settings()` ("Check for invalid configurations", first call in `connect()`), is not used.

Verification status: construction counts, the raised errors and the evaluation counts are **confirmed by execution**. The cross-thread interleaving is reasoned from the code, not reproduced.

### Worked proposal

Split the three roles.

```python
@cached_property
def _pool_options(self):
    """Normalized ConnectionPool options, or None when pooling is off. No side effects."""
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
    if reserved := _RESERVED_POOL_OPTIONS & self._pool_options.keys():
        raise ImproperlyConfigured(...)        # see finding in summary on the option contract

@property
def pool(self):
    """The process-wide pool for this alias; only called when pooling is on."""
    try:
        return self._connection_pools[self.alias]
    except KeyError:
        return self._connection_pools.setdefault(self.alias, self._create_pool())

def close_pool(self):
    if pool := self._connection_pools.pop(self.alias, None):
        pool.close()
```

and at the use sites bind once:

```python
if self._pool_options is not None:
    pool = self.pool
    pool.open()
    connection = pool.getconn()
```

`_pool_options` being a `cached_property` needs the same care as `timezone_name` when tests mutate `settings_dict`; a plain property is equally fine since it is a dictionary lookup. The point is that asking "is pooling on?" no longer builds, validates or raises, `close_pool()` is atomic and never constructs, and all configuration errors surface from `check_settings()` at connect time.

---

## D. Smaller items checked and not raised as findings

- `features.django_test_skips` becoming a `cached_property` matches how the MySQL, Oracle and SQLite backends already express conditional skips. Acceptable.
- `close_if_health_check_failed()` override: a reasonable place for the pooled no-op, once it branches on the side-effect-free predicate from section C.
- The thread-safety comment on `setdefault()` is accurate for creation ("it's init" should be "its init").
- `get_new_connection()` sets `connection.isolation_level` on a pooled connection and never resets it; since every wrapper for the alias applies the same setting at checkout this is harmless today.

## Commands

```text
git diff main...review-head -- django docs tests/requirements
wc -l django/db/backends/base/base.py django/db/backends/postgresql/*.py tests/backends/postgresql/tests.py
PYTHONPATH=clone PYTHONDONTWRITEBYTECODE=1 clone-cache/venv/bin/python clone-work/scratch/t_pool.py
PYTHONPATH=clone PYTHONDONTWRITEBYTECODE=1 clone-cache/venv/bin/python clone-work/scratch/t_pool2.py
PYTHONPATH=clone PYTHONDONTWRITEBYTECODE=1 clone-cache/venv/bin/python clone-work/scratch/t_role.py
```
