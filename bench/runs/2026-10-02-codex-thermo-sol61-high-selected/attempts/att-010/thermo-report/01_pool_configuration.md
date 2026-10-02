# Pool configuration and option boundary

This subsystem covers the raw-connection initializers, construction callback,
connection parameters, and pool option interpretation. Findings here are the
wrapper-dependent role callback and the empty-mapping option contract, both
carried into `summary.md`.

## Evidence: role initialization crosses back into the wrapper

The newly extracted helpers at `django/db/backends/postgresql/base.py:89–105`
accept a raw connection and a Django operations object. `ensure_timezone()`
uses that object only for a SQL template. `ensure_role()` uses it to compose SQL
at line 103. Those operations have different dependency footprints despite
their similar-looking signatures.

`django/db/backends/postgresql/operations.py:192–193` implements `compose_sql()`
as `mogrify(sql, params, self.connection)`. Here `self.connection` is the
operations object's Django wrapper, established by `BaseDatabaseOperations`,
not the callback's raw connection. The psycopg 3 `mogrify()` implementation in
`django/db/backends/postgresql/psycopg_any.py` opens `connection.cursor()` before
constructing a `ClientCursor` from that cursor's underlying connection.

The resulting call chain is:

```text
pool worker -> _configure_connection(raw_connection)
            -> ensure_role(raw_connection, wrapper.ops, role_name)
            -> wrapper.ops.compose_sql(...)
            -> mogrify(..., wrapper)
            -> wrapper.cursor()
            -> wrapper.ensure_connection()
            -> same_pool.getconn()
```

`pool` stores `configure=self._configure_connection` at base.py:231. A new
wrapper has no connection while the pool configures its first raw connection.
No configured connection can satisfy the nested request. With a one-connection
pool this fails deterministically; with several workers the callbacks can all
block on the same dependency. If the wrapper already holds a connection,
`_prepare_cursor()` invokes `validate_thread_sharing()` from the pool worker.
The wrapper was created on another thread, so sharing is normally forbidden.

This dependency is new: before the PR, role configuration ran in
`init_connection_state()` after `BaseDatabaseWrapper.connect()` assigned
`self.connection`, on the wrapper's own thread. Extraction preserved an
incidental wrapper dependency while moving execution to a different owner and
thread. The comment at base.py:370–373 promises that the callback does not
access wrapper behavior, but its helper violates that promise indirectly.

## Verification: real pool, fake raw connection

Run the retained evidence script with bytecode output disabled:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=<clone> <cache>/venv/bin/python <work>/offline_review_probes.py
```

`test_role_callback_reenters_wrapper_and_times_out` creates the actual installed
`ConnectionPool` using a fake connection class and `min_size=max_size=1`, one
worker, and a 0.05-second checkout timeout. A role is configured in the Django
options. Pool initialization times out, its request counter records the nested
request from the callback, and its worker log contains
`couldn't get a connection after 0.05 sec`. No network or server is involved.

The control `test_timezone_only_callback_can_prepare_a_connection_offline`
uses the same connection-class approach without a role. It successfully waits
for initialization, gets a raw connection, and returns it. This separates the
role's wrapper dependency from a general fake-connection failure.

Verification status: the nested acquisition and timeout are reproduced, not
merely inferred. The already-connected thread-validation path is established
by source inspection, not a separate live SQL execution. Local dependencies
are psycopg 3.3.6 and psycopg-pool 3.3.3; minimum-version execution is unavailable.

## Worked code-judo proposal: configure only the supplied connection

Do not add locks, permit wrapper thread sharing, or special-case callback
re-entry. Remove the dependency instead. Reuse `psycopg_any.mogrify()` with the
raw connection. Capture the role, timezone name, and timezone SQL once when
constructing a pool, so its callback operates on an explicit configuration
snapshot. An illustrative initializer is:

```python
def configure_connection(connection, *, timezone_name, timezone_sql, role_name):
    changed = False
    current_timezone = connection.info.parameter_status("TimeZone")
    if timezone_name and current_timezone != timezone_name:
        with connection.cursor() as cursor:
            cursor.execute(timezone_sql, [timezone_name])
        changed = True
    if role_name:
        role_sql = mogrify("SET ROLE %s", [role_name], connection)
        with connection.cursor() as cursor:
            cursor.execute(role_sql)
        changed = True
    return changed
```

For pool construction, bind these values using `functools.partial()`; the
callback needs no wrapper, operations object, connection getter, or commit
policy. Pool connections already start in autocommit. For a direct connection,
invoke the same initializer synchronously and retain the existing conditional
commit when Django's configured autocommit is disabled. This preserves the
timezone/role rollback guarantees without making workers understand Django
wrapper state. SQL composition continues using the existing canonical helper;
do not substitute unescaped interpolation or change role-name semantics.

This sketch is a worked design proposal, not an applied or tested patch.
Validate pooled role SQL and role errors against PostgreSQL, initialization
with `AUTOCOMMIT=False`, timezone persistence across rollback, and that no
wrapper cursor is touched during the worker callback. The PR's current role
test explicitly disables pooling, so it cannot catch this failure.

## Evidence: falsiness obscures the pool option's model

`DatabaseWrapper.pool` at base.py:204–205 returns early for every falsy value.
The next branch converts `True` to an empty dictionary, but a caller supplying
that same dictionary never reaches construction. `get_connection_params()` at
290–292 and `DatabaseFeatures.django_test_skips` at features.py:99 repeat the
truthiness model. Three consumers now define enablement independently.

`docs/ref/databases.txt:255–258` says a dictionary is passed to the pool
constructor; it does not exclude an empty mapping. Passing an empty mapping
to the constructor is a legitimate default configuration. The behavior is
therefore an option-contract bug, not a requested spelling change.

`test_empty_options_disable_pool` confirms that `{}` returns `None` while
changing the value to `True` constructs an unopened pool. This probe does not
open a socket. Existing `test_connect_pool_set_to_true` covers only `True` and
the other pool test uses a nonempty options dictionary.

## Worked normalization proposal

Make the disabled sentinel explicit. An illustrative boundary is:

```python
def normalized_pool_options(value):
    if value is None or value is False:
        return None
    if value is True:
        return {}
    if isinstance(value, dict):
        return value.copy()
    raise ImproperlyConfigured("The pool option must be a dict or a boolean.")
```

Treat `None` as disabled and an empty dictionary as enabled everywhere. The
pool decision, psycopg version validation, and test skip selection should use
the same normalization contract. This helper earns its existence by replacing
three independent interpretations, not by wrapping a lone conditional. Retain
the `NO_DB_ALIAS` exclusion explicitly as an administrative connection policy.
Decide deliberately whether mappings beyond `dict` are accepted; do not let
truthiness silently accept integers or strings and fail later in `**options`.

Validate the option matrix without a database: missing, `None`, `False`,
`True`, `{}`, a nonempty mapping, and invalid input. Validate that administrative
connections remain direct. This reduces shape ambiguity and prevents option
validation and pool-specific capability selection from drifting apart.
