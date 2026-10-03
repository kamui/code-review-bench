# Pool ownership, invalidation, and database lifecycle

This subsystem carries F2 and F3 from the summary. The review is independent of
ambient repository guidance and covers only the pinned committed change and
the source paths needed to understand it. No implementation files were edited.

## F2: Acquisition and release are entangled

The PostgreSQL wrapper introduces a class-level `_connection_pools` registry at
`django/db/backends/postgresql/base.py:200`. The `pool` property at lines
202–241 is an acquisition operation: it imports a dependency, validates settings,
builds connection parameters, creates a `ConnectionPool`, and registers that
pool under the alias. It intentionally leaves the new pool unopened.

The same property is used as a boolean test in `close_pool()` at lines 243–246,
`_close()` at lines 385–399, `init_connection_state()` at lines 401–408, and
`close_if_health_check_failed()` at lines 501–506. A reader cannot tell whether
one of these apparently observational checks creates a process-wide resource.
The shutdown flow even makes two independent allocating lookups before deleting
the alias from the registry.

This is a concrete ownership defect. `ensure_timezone()` at lines 362–367 calls
`close_pool()` before checking whether there is a physical connection. On an
unused pooled wrapper, the first lookup constructs a new pool, immediately
closes it, and deletes it. The probe observed one allocation and no remaining
registration. No PostgreSQL server or connection was involved.

The more significant case is invalidating a pool while a connection is checked
out. The installed pool's documented-in-source close contract leaves checked-out
connections open until they are returned. In the PR, `_close()` first consults
the current registry via `self.pool`; after invalidation this constructs a new
pool. It then ignores that pool and returns the old connection through
`self.connection._pool.putconn()`. The probe observed one new allocation,
successful return to the original owner, and an unrelated replacement left in
the registry. This directly demonstrates why the connection's ownership and
the alias's current generation must be separate facts.

The workaround comment at lines 392–394 acknowledges that settings changes can
replace pools. Reaching into the driver's private `_pool` attribute patches
that symptom while retaining the allocating lookup that caused it. Django can
remember the pool it used for `getconn()` without depending on that private
attribute or on the alias's current settings.

The registry update is also non-atomic. The constructor's `setdefault()` at line
239 resolves competing insertions, but it does not synchronize the check,
multiple reads, `close()`, and `del` in `close_pool()`. A two-thread probe placed
a barrier inside a fake pool's `close()` so both callers reached deletion of
the same alias. One raised `KeyError: 'concurrent'`. That is a controlled
reproduction of the interleaving, not an estimate of its likelihood in normal
test execution.

### Worked simplification

Keep a small registry API with two explicit operations: get-or-create and
remove. Use a common lock for registry mutation. Since construction uses
`open=False`, construction need not run callbacks or start workers while the
registry lock is held. Actual pool closing belongs outside the registry lock.

The essential teardown shape is:

```python
def close_pool(self):
    with self._pool_lock:
        pool = self._connection_pools.pop(self.alias, None)
    if pool is not None:
        pool.close()
```

This performs no creation, is idempotent when the alias is absent, and removes
the entry before the potentially slow shutdown. Acquisition must participate
in the same registry protocol. A new generation may be registered after
removal; it must not be accidentally deleted by the older closer.

At checkout, save the concrete pool on the wrapper after successful acquisition:

```python
pool = self.pool
if pool is None:
    connection = self.Database.connect(**conn_params)
else:
    pool.open()
    connection = pool.getconn()
    self._leased_pool = pool
```

On return, use that lease reference rather than `self.pool` or
`connection._pool`. Clear the lease and the wrapper's connection as part of
the release path, retaining Django's error wrapping. Direct connections can
continue through `super()._close()`; the pool behavior need not duplicate the
shared close implementation. The final implementation must define cleanup
on failed checkout, failed initialization, and failed return, as it already
must for any acquired external resource.

This proposal deletes the repeated pool lookups and the private driver-attribute
workaround. It introduces one state item with a precise lifetime: the owner of
the currently checked-out connection. That state is narrower than attempting
to infer ownership from mutable settings at shutdown.

A dedicated PostgreSQL pooling module is a reasonable home for the registry
and generation policy if the final implementation needs it. Extraction is
justified by process-wide ownership and synchronization, not by an arbitrary
file-size target. Avoid a generic connection-provider hierarchy when these two
registry operations and a lease reference are sufficient.

### Verification status and acceptance cases

The acquisition-on-timezone and acquisition-on-return cases executed real Django
wrapper methods with only `ConnectionPool` replaced by a recording fake. The
concurrent close case additionally controlled thread scheduling with a barrier.
No live driver return/reset behavior was tested. The installed pool source at
`psycopg_pool/pool.py:432–480` confirms that checked-out connections can be
returned after shutdown, and lines 324–377 show the public `putconn()` path.
Those source line numbers refer to cached psycopg-pool 3.3.3, not a vendored
component or proof of every supported dependency version.

Acceptance checks should show zero pool construction when closing an absent
pool, zero replacement construction when returning an old-generation lease,
return to the exact pool used for checkout, idempotent concurrent removals,
and defined behavior for failures during connection initialization. Pool
construction and removal must share a synchronization contract.

## F3: Alias-only identity survives database changes

The pool cache at `django/db/backends/postgresql/base.py:208–241` captures driver
connection parameters once and then treats the alias as sufficient identity.
Direct connections previously recomputed parameters for each new connection.
With pooling, closing the wrapper returns its connection and leaves that
captured configuration active.

This matters in an existing canonical transition. In
`django/db/backends/base/creation.py:62–79`, `create_test_db()` creates the test
database using an administration connection, closes the normal wrapper, changes
its `NAME`, and invokes migrations. It does not know that the PostgreSQL wrapper
now has another database-target resource that survives close. The PR adds pool
cleanup to `_clone_test_db()` and `_destroy_test_db()` in PostgreSQL creation
at lines 61 and 90, but adds none for this initial transition.

The offline probe initialized the application's pool before calling the real
`create_test_db(verbosity=0, serialize=False)`. It mocked database creation,
management-command I/O, and the final connection establishment, while leaving
the real name-switching orchestration intact. At the `migrate` call the wrapper
name was `test_application`, but the pool still contained `dbname=application`
and was the identical original pool. The subsequent `createcachetable` call
observed the same mismatch. If those commands access the backend normally,
`get_new_connection()` at lines 345–348 draws from that stale pool and ignores
the newly computed direct `conn_params`.

This trigger requires a preexisting pool. A completely cold test-runner process
may avoid it, but database use during application initialization, an embedded
test runner, or an earlier alias access supplies the condition. The defect
does not depend on a race, an unsupported raw-driver call, or upstream
discussion. Its impact is routing test migrations to the application database.

There is a second statically traced gap in
`django/db/backends/base/creation.py:308–316`: `keepdb=True` skips
`_destroy_test_db()` and restores the original database name. Therefore the
new pool cleanup in the private drop hook does not run on that restoration
path. Mirror setup at lines 106–112 and worker setup at lines 377–384 also mutate
database settings, making it important to model this as configuration lifecycle
rather than adding only the next local close branch. The latter transitions
were inspected; their full live-database consequences were not executed.

### Worked simplification

A configuration-aware registry can preserve one current pool per alias while
representing its generation explicitly. The registry entry contains a stable
configuration snapshot and the concrete pool. On acquisition, compute the
current normalized pool configuration; reuse the entry only when its
connection-affecting settings match. Otherwise atomically replace the current
entry with an unopened pool for the new configuration, then close the old pool
outside the registry lock. Existing leases return to their remembered original
pool, as proposed for F2.

Conceptually, the entry is:

```python
@dataclass(frozen=True)
class PoolEntry:
    configuration: PoolConfiguration
    pool: ConnectionPool
```

`PoolConfiguration` should be an explicit backend model of connection target,
driver options, pool options, health-check policy, and session configuration.
It should snapshot values rather than retain a live wrapper or a mutable
settings dictionary. Its equality contract should cover actual supported
settings, including custom connection and cursor factories, without using a
generic hash or serialized representation of arbitrary Python objects. Do not
log credentials as part of configuration diagnostics.

For the demonstrated case, comparing the database name in this configuration
makes the first acquisition after `create_test_db()` replace the application's
pool. Restoring the name with `keepdb=True` similarly replaces the test pool.
This uses the backend's normal acquisition boundary and eliminates the need to
teach every caller that wrapper close now has a second hidden meaning.

An explicit backend configuration-change hook is another viable design if the
project wants to forbid automatic generation replacement. It must run before
new-target acquisition at every supported mutation site, including creation
and restoration with `keepdb`; adding another isolated override without that
inventory would repeat the existing scattering. Shutdown before cloning or
dropping a database remains necessary because idle physical connections, not
only settings freshness, prevent those operations.

### Verification status and acceptance cases

The initial creation transition is verified with real Django orchestration and
a recording fake pool. No migrations or SQL were executed. The `keepdb`, mirror,
and worker transition observations are source-level traces. A real PostgreSQL
server is required to confirm migration routing, clone/drop behavior, and
parallel-worker isolation end to end.

Acceptance cases should initialize a pool before test setup, switch the target,
and confirm that newly acquired physical connections report the test database.
Restoration with `keepdb` must recover the original target. Test mirrors and
worker clones must likewise receive their configured targets. Old leases may
outlive an invalidation, but their return must never resurrect the old target
or allocate a replacement as an incidental effect of closing.

## Reproduction commands and measurements

The shared reproducible probe is retained at `../review_probes.py`; its captured
stdout and stderr are in `evidence/probe-output.jsonl` and
`evidence/probe-stderr.txt`. From the attempt directories, the exact execution
was:

```sh
PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-002/clone \
/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-002/clone-cache/venv/bin/python \
/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-002/clone-work/review_probes.py
```

The final run exited 0 in less than one second. Its observations are runtime
outputs of defect-triggering scenarios, not a statement that the implementation
meets the proposed acceptance criteria. Relevant outputs were:

```text
timezone_on_unused_wrapper: allocations=1, registered=false
return_after_invalidation: allocations=1, returned_to_original=true,
  replacement_registered=true
concurrent_close: errors=["KeyError: 'concurrent'"]
test_db_transition: migrate and createcachetable both saw
  wrapper_db=test_application, pool_db=application, original_pool=true
```

Measurements from the pinned base and head show PostgreSQL `base.py` growing
516→615 lines and 29→41 AST `if` nodes, while PostgreSQL `creation.py` grows
86→91 lines without an increased `if` count. The important complexity is the
additional ownership protocol, not the raw line count. See
[03_verification.md](03_verification.md) for the full command and scope inventory.
