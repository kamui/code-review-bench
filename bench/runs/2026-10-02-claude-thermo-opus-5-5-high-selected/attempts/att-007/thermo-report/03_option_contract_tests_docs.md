# 03 — The `OPTIONS["pool"]` contract, tests, and documentation

Scope: `docs/ref/databases.txt` (+25), `docs/releases/5.1.txt` (+3), `tests/backends/postgresql/tests.py` (+141/−11), and the option handling in `django/db/backends/postgresql/base.py`.

Review range: `bcccea3ef3..fad334e1a9`.

## A. The documented option contract does not match the code

### Reserved keys

The documentation says `"pool"` may be "a dict to be passed to `psycopg_pool.ConnectionPool`" (`docs/ref/databases.txt:255-258`). The code passes that dict with `**pool_options` next to four keywords it sets itself:

```python
# postgresql/base.py:228-234
pool = ConnectionPool(
    kwargs=connect_kwargs,
    open=False,  # Do not open the pool during startup.
    configure=self._configure_connection,
    check=ConnectionPool.check_connection if enable_checks else None,
    **pool_options,
)
```

Probe (`scratch/t_pool.py`, section 3):

```text
check     -> TypeError psycopg_pool.pool.ConnectionPool() got multiple values for keyword argument 'check'
configure -> TypeError psycopg_pool.pool.ConnectionPool() got multiple values for keyword argument 'configure'
open      -> TypeError psycopg_pool.pool.ConnectionPool() got multiple values for keyword argument 'open'
kwargs    -> TypeError psycopg_pool.pool.ConnectionPool() got multiple values for keyword argument 'kwargs'
```

Verification status: **confirmed by execution**. `"check": ConnectionPool.check_connection` is the configuration the psycopg documentation itself recommends, so a user following both sets of documentation gets a bare `TypeError` raised from a property access during `connect()`. A user-supplied `reset` is accepted but, per the comment in `_close()` (`base.py:387-389`), silently changes error wrapping because the return is then deferred to a pool thread.

Remedy: decide the contract and state it. Either reject the reserved keys with `ImproperlyConfigured` in `check_settings()` and document them (`kwargs`, `open`, `configure`, `check` are owned by Django; `CONN_HEALTH_CHECKS` drives `check`), or let user values win by merging explicitly (`{**django_defaults, **pool_options}`) and compose `configure` rather than replace it. The first is simpler and is what detail file 02, section C sketches.

### psycopg2

The documentation says the option "is ignored with ``psycopg2``" (`docs/ref/databases.txt:270-271`). The code raises:

```python
# postgresql/base.py:290-292
pool_options = conn_params.pop("pool", None)
if pool_options and not is_psycopg3:
    raise ImproperlyConfigured("Database pooling requires psycopg >= 3")
```

and the test that asserts the raise is named `test_connect_pool_setting_ignored_for_psycopg2` (`tests/backends/postgresql/tests.py:349-354`). Raising is the better behaviour; the documentation sentence and the test name are wrong. Verification status: read from source; psycopg2 is not installed in the cache, so the branch was not executed.

### Interaction notes that the documentation leaves out

The section does not mention that `CONN_MAX_AGE` must be `0`, that `CONN_HEALTH_CHECKS` is what enables the pool's `check`, or that `ConnectionPool`'s default `min_size` of 4 means four server connections per process are opened on first use. These are the three things a user enabling `"pool": True` will trip over first.

## B. `no_pool_connection()` — a helper whose name and body both mislead

```python
# tests/backends/postgresql/tests.py:24-30
def no_pool_connection(alias=None):
    new_connection = connection.copy(alias)
    new_connection.settings_dict = copy.deepcopy(connection.settings_dict)
    # Ensure that the second connection circumvents the pool, this is kind
    # of a hack, but we cannot easily change the pool connections.
    new_connection.settings_dict["OPTIONS"]["pool"] = False
    return new_connection
```

Measurements from `grep -n no_pool_connection tests/backends/postgresql/tests.py`: 18 call sites. Eleven are pre-existing tests whose `connection.copy()` was replaced (isolation level, role, cursor factory, server-side binding, client encoding, health checks, version, compose_sql). Seven immediately set `settings_dict["OPTIONS"]["pool"]` to `True` or a dict on the next line (lines 240, 279, 289, 313, 330, 341, 350) — a helper named "no pool" is the standard way to build a pooled connection.

`BaseDatabaseWrapper.copy()` already deep-copies the settings (`base/base.py:783-792`: `settings_dict = copy.deepcopy(self.settings_dict)`), so the second `deepcopy` in the helper is redundant.

The helper's own comment calls it a hack. It is the test-side shadow of the alias-keyed registry described in detail file 02, section B: because every wrapper with the same alias shares one pool whose parameters were frozen by whoever created it, a test cannot vary `OPTIONS` on a copy. Eleven unrelated tests had to be touched to say "not pooled", which also means that when the suite runs with a pooled configuration, isolation level, `assume_role`, `cursor_factory`, `server_side_binding` and `client_encoding` are never exercised through the pool. The `assume_role` defect in detail file 02, section A sits exactly in that blind spot.

Remedy: once the pool is no longer swapped under live connections and its inputs are explicit (detail 02, B and C), tests can use `connection.copy(alias="…")` with a distinct alias and set `OPTIONS["pool"]` to what they need. Replace the helper with two honest ones if any helper is still wanted, e.g. `copy_connection(pool=None)` that sets the option to the given value, and add pooled variants for at least the role and isolation-level tests.

## C. Tests that assert internals or the wrong layer

- `test_cannot_open_new_connection_in_atomic_block` (`tests.py:329-338`) tests a `BaseDatabaseWrapper` behaviour from the PostgreSQL module, is skipped under psycopg2 although nothing in it depends on the driver, and reaches the state by assigning `in_atomic_block = True` and `closed_in_transaction = True` by hand. Because it never goes through `atomic()`, it could not notice the autocommit-off regression described in detail file 01.
- `test_pooling_health_checks` asserts on `new_connection.pool._check`, a private psycopg_pool attribute.
- `test_connect_pool_set_to_true` only asserts that a pool object can be constructed; it never connects.

These are secondary to the structural issues; they are listed so that the replacement tests are written against behaviour.

## Commands

```text
git diff main...review-head -- tests/backends docs
grep -n "no_pool_connection" tests/backends/postgresql/tests.py
grep -n "def copy" -A 9 django/db/backends/base/base.py
PYTHONPATH=clone PYTHONDONTWRITEBYTECODE=1 clone-cache/venv/bin/python clone-work/scratch/t_pool.py
```
