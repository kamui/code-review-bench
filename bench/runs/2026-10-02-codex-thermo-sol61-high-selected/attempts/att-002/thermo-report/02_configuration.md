# Physical connection configuration and public option contracts

This subsystem carries F1, F4, and F5. It also evaluates the existing direct
configuration behavior so that the proposed simplification preserves semantics.

## F1: Worker configuration crosses the wrapper boundary

At `django/db/backends/postgresql/base.py:228–234`, the pool retains
`configure=self._configure_connection`. This is a bound method on the first
thread-local wrapper that successfully registers the alias's pool. The callback
uses `self.ops`, `self.timezone_name`, and `self.settings_dict` at lines 369–383.
The comment warning against accessing anything besides variables does not
establish a safe boundary: those variables include an operations object with
a back-reference to the wrapper, as well as a mutable settings dictionary.

The timezone helper at lines 89–95 uses `ops.set_time_zone_sql()`, which merely
returns SQL. In contrast, the role helper at lines 98–104 calls
`ops.compose_sql("SET ROLE %s", [role_name])`. The actual call graph is:

```text
pool worker configures new raw connection
  DatabaseWrapper._configure_connection(raw)
    ensure_role(raw, wrapper.ops, role_name)
      wrapper.ops.compose_sql(...)
        postgresql.psycopg_any.mogrify(..., wrapper)
          wrapper.cursor()
            wrapper.ensure_connection()
              wrapper.connect()
                wrapper.get_new_connection(...)
                  same_pool.getconn()
```

The operations helper lives at
`django/db/backends/postgresql/operations.py:192–193`; the psycopg 3 implementation
at `django/db/backends/postgresql/psycopg_any.py:20–22` deliberately obtains a
Django wrapper cursor. That was valid in the previous direct initialization
flow, where the wrapper already held a physical connection and ran on its
own thread. Moving the same helper into a background callback changes those
preconditions.

During the first pooled checkout, the main thread cannot set
`self.connection` until `getconn()` returns. The raw connection cannot be made
available by the pool until its configuration callback returns. Opening the
wrapper cursor from the callback therefore requests a different pool checkout
while the first is still being configured. With one worker and a one-connection
pool, neither checkout can complete before timeout. Increasing the pool size
does not establish the missing invariant; every newly created raw connection
still requires the role callback before admission.

There is another failure mode when the retained wrapper already has a physical
connection. Its cursor path eventually calls `validate_thread_sharing()` in
`django/db/backends/base/base.py:637–652`. A pool worker thread is different from
the thread that constructed that wrapper, and there is no thread-sharing
authorization in the PR. It raises a Django `DatabaseError`. Even authorizing
sharing would leave SQL composition using the wrapper's unrelated leased
connection rather than the raw connection being configured.

### Verification

Three complementary probes support the finding. First, the real role helper
chain was executed with a mocked raw connection and a sentinel at the wrapper's
`ensure_connection()` boundary. It reached that forbidden acquisition exactly
once. This isolates the boundary leak from driver behavior.

Second, the cached real `psycopg_pool.ConnectionPool` ran its actual worker with
a fake physical `connection_class`. The fake provides an idle transaction
status, reports UTC, and performs no network I/O. With `assume_role="app_role"`,
the foreground connect failed with `PoolTimeout`, the wrapper never acquired
a connection, and pool statistics showed two requests: the foreground request
and callback re-entry. Stderr contained the expected configuration failure,
`couldn't get a connection after 0.10 sec`. A control using the same fake driver
without a role acquired and returned a raw connection successfully.

Third, the callback was run in a background thread after the retained wrapper
was assigned a mocked physical connection. Only driver cursor construction was
mocked; the actual wrapper cursor preparation and thread check remained active.
It raised `DatabaseError: DatabaseWrapper objects created in a thread can only
be used in that same thread` for alias `connected_callback`.

These probes confirm the Django call graph and current installed library worker
behavior. They do not verify role SQL against PostgreSQL or demonstrate a full
application deployment. Cached psycopg-pool is 3.3.3; the PR's minimum is 3.2.0.
The dependency-independent boundary violation is visible in the pinned Django
source regardless of that version difference.

### Worked code-judo proposal

Capture foreground configuration values before pool creation. The background
callback should not close over the wrapper, `DatabaseOperations`, or a live
settings dictionary. A focused immutable configurator can serve both direct
and pooled initialization:

```python
from dataclasses import dataclass
from .psycopg_any import sql


@dataclass(frozen=True)
class ConnectionConfiguration:
    timezone_sql: str
    timezone_name: str | None
    role_name: str | None

    def __call__(self, connection):
        changed = False
        current = connection.info.parameter_status("TimeZone")
        if self.timezone_name and current != self.timezone_name:
            with connection.cursor() as cursor:
                cursor.execute(self.timezone_sql, [self.timezone_name])
            changed = True
        if self.role_name:
            with connection.cursor() as cursor:
                cursor.execute(
                    sql.SQL("SET ROLE {}").format(sql.Identifier(self.role_name))
                )
            changed = True
        return changed
```

Construct this in the foreground from `self.ops.set_time_zone_sql()`,
`self.timezone_name`, and the optional `assume_role`. The `sql` import above is
the existing driver-neutral export. Identifier composition keeps quoting with
the driver and executes directly on the supplied physical connection; it does
not acquire a Django cursor just to interpolate a role name. An equivalent
partial over a pure function is also sufficient if a dataclass adds no clarity.

The direct initialization path can call this configurator and commit when it
reports a change and wrapper autocommit is off, preserving the existing behavior
at lines 401–408. The pool continues to create connections in autocommit mode
and calls the same configurator without a wrapper-level commit. The driver-only
timezone operation can also be reused by `ensure_timezone()` on an active
connection. That method's pool invalidation belongs to the registry lifecycle
described in F2, not to physical SQL configuration.

This eliminates the operations-object dependency and the unsafe bound-method
lifetime. It uses one configuration algorithm rather than adding role-specific
worker exceptions or relaxing Django's thread safety. The immutable snapshot
can be part of the pool generation in F3 so its driver kwargs and session
configuration describe the same settings.

Acceptance cases should cover a real pooled `assume_role` startup, pool growth
while the original wrapper is connected, unusual quoted role names, timezone
configuration, and direct initialization with `AUTOCOMMIT=False`. A worker
callback test should fail immediately if any wrapper cursor or acquisition is
attempted. The sketch is a proposal, not an applied or live-database-tested fix.

## F4: The public option shape has no single contract

The user-facing documentation at `docs/ref/databases.txt:255–258` accepts a
dictionary passed to `ConnectionPool`, or `True` for defaults. However, the
activation test at `django/db/backends/postgresql/base.py:204–206` rejects every
falsey value, including `{}`. It translates `True` to `{}` only after passing
the activation test at lines 213–215. Thus the normalized defaults are accepted
only when the user wrote a different value.

The offline constructor probe observed:

| Option | Pool enabled |
| --- | --- |
| `False` | No |
| `True` | Yes |
| `{}` | No |
| `{"min_size": 0}` | Yes |

This matters when callers build pool options dynamically: removing the last
custom option silently switches off the entire feature rather than requesting
the same pool with defaults. It can also make psycopg2 validation inconsistent,
because the driver guard at lines 290–292 separately tests truthiness of the
original option. PostgreSQL features at lines 101–114 use yet another truthiness
check to choose test skips. The copied shape checks are a missing boundary
contract, not simply a boolean-expression style concern.

### Worked normalization

Normalize once, before driver validation or registry access. In the following
sketch, `None` is the disabled sentinel; an empty mapping is enabled:

```python
def normalize_pool_options(value):
    if value is None or value is False:
        return None
    if value is True:
        return {}
    if isinstance(value, dict):
        return value.copy()
    raise ImproperlyConfigured("OPTIONS['pool'] must be a dict or a boolean.")
```

Consumers then use `options is None` to identify disabled pooling. The copy
establishes an owned snapshot at the boundary; normalization should be retained
in the wrapper/pool configuration rather than reimplemented independently in
features and connection-parameter construction. Driver requirements and
`CONN_MAX_AGE` validation should operate on this enabled state, independent of
whether the mapping contains custom options. The administrative `NO_DB_ALIAS`
exception remains an explicit acquisition policy.

The project can choose a narrower public contract, but then it must explicitly
document `{}` as disabled rather than promising an arbitrary options dictionary.
Supporting the documented mapping contract is the simpler and less surprising
remedy. Include `{}`, `True`, `False`, and absence in focused configuration
checks; this does not require a live database.

Verification is a real wrapper property execution with a recording pool
constructor. The inconsistency in the other consumers is statically verified.
No claim is made about unsupported option types beyond the proposed validation.

## F5: The documented driver policy contradicts the code

`docs/ref/databases.txt:270–271` states that pooling requires the pool package
and is ignored with psycopg2. That is directly contradicted by
`django/db/backends/postgresql/base.py:290–292`, which raises
`ImproperlyConfigured` when an enabled pool option is used with psycopg2.
The new `test_connect_pool_setting_ignored_for_psycopg2` at
`tests/backends/postgresql/tests.py:348–354` also expects that error, despite
its name.

This is a user-facing compatibility error. A user can reasonably read “ignored”
as permission to retain the option in a deployment using psycopg2, then encounter
a startup failure on its first attempted connection. The remedy is to document
the implemented requirement: enabled pooling requires psycopg 3; psycopg2 rejects
the setting. The requirement for the separately installed pool package can
remain as written. This does not require a compatibility mode or changes to
the intended rejection behavior.

Verification combines exact source comparison with a branch-simulation probe:
patching only `is_psycopg3=False` and executing `get_connection_params()` raised
`ImproperlyConfigured: Database pooling requires psycopg >= 3`. This is not a
test with an installed psycopg2 driver, but the exception is raised before any
driver operation, so it verifies the relevant policy branch. The committed test
provides additional evidence of the author's intended behavior.

## Commands and retained evidence

The shared command, script, environment versions, and limitations are documented
in [03_verification.md](03_verification.md). The raw observations are preserved
in `evidence/probe-output.jsonl`, and the actual pool worker's expected timeout
log is in `evidence/probe-stderr.txt`.

Relevant source reads used `nl -ba`/`sed` on the changed wrapper and documentation,
and `rg -n 'compose_sql|def get_adapters_template|def register_tzloader'` on the
existing PostgreSQL operations and adaptation modules. The installed pool source
was read directly at its cached path; no dependency or upstream material was
downloaded.
