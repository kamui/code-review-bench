# Shared pool lifecycle and checkout ownership

This subsystem covers the alias registry, borrowing and returning connections,
timezone invalidation, and test-database identity changes. The two actionable
findings are stale registry configuration and release based on current settings
instead of acquisition ownership; both are carried into `summary.md`.

## Evidence: a shared resource outlives its configuration

`django/db/backends/postgresql/base.py:200–241` creates a class-level pool
registry keyed only by alias. The pool snapshots `get_connection_params()` into
`kwargs` at lines 224–229. Subsequent wrappers and connections with that alias
reuse the pool without comparing those parameters. Ordinary thread-local
wrappers should share one pool; that part of the architecture is appropriate.
Changing the target database under the same alias, however, must invalidate
the old configuration before another connection is borrowed.

The new PostgreSQL creation hooks call `close_pool()` before cloning a database
at `creation.py:61` and actual destruction at `creation.py:90`. That is partial
coverage of a broader existing lifecycle:

`BaseDatabaseCreation.create_test_db()` closes the wrapper at base/creation.py:64,
changes `settings_dict['NAME']` at 66, then invokes `migrate`. Wrapper close now
returns a connection to its pool; it does not destroy the registry entry.
If any feature check or application setup initialized the pool beforehand,
the migrate invocation sees the test database in settings but the original
database in the pool's saved connection parameters.

`BaseDatabaseCreation.setup_worker_connection()` at 377–384 updates the settings
to the worker's clone name and closes only the wrapper. This similarly leaves
an already-existing pool unchanged. Closing the pool during cloning does not
prove that no pool will be recreated before worker initialization; the worker
transition needs its own invariant.

`BaseDatabaseCreation.destroy_test_db()` at 310–316 skips `_destroy_test_db()`
when `keepdb=True`, then restores the original name. Thus the new PostgreSQL
destruction hook never runs in that path, and the test-database pool can survive
the restoration. These are concrete existing API transitions, not hypothetical
live application edits to arbitrary settings.

## Verification: exercise the actual creation methods offline

Run `offline_review_probes.py` using the command in the configuration detail.
Three checks use real unopened pools and the repository's creation methods:

`test_create_test_db_keeps_preexisting_production_pool` initializes a pool for
`production`, stubs only database creation, connection establishment, and
management commands, and calls `create_test_db(serialize=False)`. The intercepted
`migrate` observes `(settings NAME, pool dbname)` equal to
`('test_production', 'production')`. The pool object is unchanged.

`test_worker_database_switch_keeps_old_connection_parameters` calls the real
`setup_worker_connection(1)`. Settings become `production_1`, but the same pool
still has `dbname='production'`.

`test_keepdb_restore_keeps_test_database_pool` invokes the real
`destroy_test_db(old_database_name='production', keepdb=True)`. Settings return
to `production`, while the pool retains `dbname='test_production'`.

Verification status: stale parameter retention is reproduced with the actual
lifecycle methods. No database was created or migrated, and no production data
was touched. The risk of targeting the original database follows from the
verified parameter mismatch and `get_new_connection()` borrowing exclusively
from that pool. Live SQL and process-fork behavior were not executed.

## Worked code-judo proposal: make identity transitions explicit

The required invariant is simple: ordinary release preserves a shared pool;
changing a connection alias's target database invalidates the pool before the
new identity is used. A backend-level invalidation operation should implement
that policy once. Cover initial test creation, clone preparation, worker setup,
destruction, and restoration even when destruction itself is skipped.

Do not fix this by destroying pools on every request close: that removes the
feature's intended connection reuse. Do not merely append another private-pool
check wherever an error appears. A clean implementation could expose a
backend-owned identity-change hook called by the canonical creation transitions,
with a no-op default for backends without shared resources. Alternatively,
the pool registry can store a stable configuration key and replace a pool
when the target identity differs. If taking that route, define the key explicitly
and synchronize replacement; avoid generic deep hashing of arbitrary callback
options. Borrowed connections must still be returned to their original owner.

The explicit hook is easier to reason about for these existing test lifecycle
methods. If backend-specific overrides are chosen instead, ensure they cover
the complete transitions rather than only `_clone_test_db()` and
`_destroy_test_db()`. Tests should assert pool identity and parameters before
the migrate callback, after worker setup, and after both keepdb modes.

## Evidence: release consults a creating getter and a private attribute

`get_new_connection()` at base.py:345–348 looks up `self.pool` three times to
borrow a connection. `_close()` at 391 tests `self.pool` again, but at 395
returns to `self.connection._pool`, which can be a different object. The code
explicitly calls this a test workaround for timezone changes. Its real purpose
is acquisition ownership, which the backend should model directly.

`ensure_timezone()` closes the pool at 364 while a checked-out connection can
remain on the wrapper. Later `_close()` invokes the getter and constructs a
replacement pool before returning that connection to its old owner.
`close_pool()` itself calls the creating getter twice at 244–245; closing a
never-created pool constructs one simply to close and remove it. Configuration,
capability checks, acquisition, and cleanup therefore share a side-effectful
lookup operation.

The current-settings test is also not a valid ownership test when options are
changed on a copied test wrapper. If the pool option is disabled after checkout,
`_close()` calls raw `close()` instead of pairing `getconn()` with `putconn()`.
The pool still counts that checkout, with no notification that its capacity
can be replenished. Conversely, a direct connection followed by enabled options
can reach `_pool.putconn()` with no pool owner. This latter path is supported by
source inspection; it was not needed for the retained reproduction.

## Verification: cleanup creates resources and can lose capacity

`test_release_after_invalidation_constructs_a_replacement_pool` borrows a fake
raw connection from the real pool, calls the actual `close_pool()`, confirms
the alias entry is absent, then calls the actual wrapper `close()`. A new,
unopened pool appears in the registry solely during release. The connection
returns to the original closed pool.

`test_pool_disabled_after_checkout_loses_capacity` gets the sole connection
from a real one-slot pool, changes the wrapper's pool option to `False`, and
closes the wrapper. The raw connection is physically closed. A subsequent
request to the retained pool times out after 0.05 seconds because no matching
`putconn()` occurred. This checks an externally observable capacity failure,
not just a private call count.

Verification status: both paths are reproduced without network access using
the installed pool. Dynamic-option paths matter particularly to this PR's
testing design, which deliberately mutates copied wrapper settings. Live
PostgreSQL cleanup and custom reset callback execution are untested.

## Worked code-judo proposal: save the owner when borrowing

Introduce one wrapper-owned checkout field initialized to `None`, such as
`_leased_pool`. Acquire from a single local pool value and record that owner
alongside the assigned connection. A direct connection has no pool owner.
Use the owner for return and pooled initialization/health-check decisions;
the lazy registry getter should be an acquisition operation, not a release
predicate. The pooled part of `_close()` then becomes conceptually:

```python
def _close(self):
    if self._leased_pool is None:
        return super()._close()
    if self.connection is not None:
        with self.wrap_database_errors:
            self._leased_pool.putconn(self.connection)
        self.connection = None
        self._leased_pool = None
```

The direct branch retains the existing base close contract, including its
atomic-block behavior. The pooled branch detaches the returned connection so
the wrapper cannot keep using it. The new generic reconnect guard remains
necessary while an atomic block is closed. Final code must also return a
borrowed connection if acquisition initialization fails before assignment, and
define cleanup behavior when `putconn()` raises; the sketch is not a complete
exception-handling patch.

Registry shutdown should have no construction side effects:

```python
def close_pool(self):
    pool = self._connection_pools.pop(self.alias, None)
    if pool is not None:
        pool.close()
```

This directly removes the private `connection._pool` dependency, the release
lookup, and the need for a test-specific owner workaround. It also makes
shutdown idempotent and independent of current option truthiness. Coordinate
registry invalidation with concurrent acquisition if that lifecycle is allowed;
the current construction `setdefault()` only handles creation races, not
replacement or shutdown. Race failures are not an additional asserted finding.

Validate ordinary reuse, return after invalidation, option changes on copied
wrappers, atomic close and reconnect, cleanup of failed initialization, and
the custom-reset path. Retain the distinction between returning a checkout and
closing a shared pool. These proposals were not applied to the checkout.
