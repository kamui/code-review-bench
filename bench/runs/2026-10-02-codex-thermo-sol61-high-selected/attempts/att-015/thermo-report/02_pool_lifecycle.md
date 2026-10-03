# Pool registry, leases, and database lifecycle

## Findings and source evidence

This file supports “Invalidate pools at every database-identity transition” and “Separate pool acquisition from inspection and release” in summary.md. These concern different obligations: invalidating a shared resource when its configuration changes, and keeping cleanup free of resource creation.

The new class-level _connection_pools dictionary is indexed by alias at django/db/backends/postgresql/base.py:200–241. Sharing by alias is a reasonable way to give thread-local wrappers one pool, but it adds a lifetime distinct from wrapper.connection. get_new_connection() ignores its newly computed conn_params when an alias already has a pool, taking a lease from that existing object instead. Closing a wrapper now returns a lease; it does not invalidate the alias-wide resource or update its kwargs.

The creation diff handles _clone_test_db() by closing the wrapper and pool at creation.py:60–61, and handles SQL destruction in _destroy_test_db() at lines 89–91. BaseDatabaseCreation.create_test_db() at base/creation.py:64–66 still closes only the wrapper before changing NAME. If a pool exists, the subsequent migrations borrow from its old dbname despite the wrapper's new NAME. This is conditional on an existing pool, for example after an earlier database access or administrative fallback; it is not a claim that every ordinary fresh test run uses the wrong database.

BaseDatabaseCreation.destroy_test_db() at base/creation.py:290 closes the wrapper, skips _destroy_test_db() when keepdb=True at lines 310–311, and restores the original NAME at lines 314–316. A pool already created for the test run remains cached under that alias. Later access can borrow a test-database connection while the wrapper reports the original database name. This is relevant to reusable in-process test execution and subsequent access, even when process exit happens to hide it in one command-line run.

The existing PostgreSQL _nodb_cursor fallback at base.py:528–534 creates a wrapper with alias=self.alias, whereas its primary BaseDatabaseWrapper path uses NO_DB_ALIAS. The new pooling logic excludes NO_DB_ALIAS but not the fallback. It can therefore create or reuse an application pool from an administrative path. This is a source-confirmed contributor to lifetime ambiguity; a live postgres-denied fallback was not executed. It should use an explicitly non-pooled administrative wrapper with the selected database name.

The inherited setup_worker_connection() changes NAME then closes the wrapper. The new clone path often disposes of pools before fork, so this review does not separately claim that all parallel runs are broken. The same identity-transition obligation should nevertheless be explicit there, especially if another path creates a pool between clone preparation and worker setup. Mirror setup also changes NAME and needs auditing under the same rule.

## Verification of stale targets

test_create_test_db_retains_previous_database_pool constructs a real unopened pool for application and invokes the actual create_test_db(), mocking only SQL database creation, call_command(), and ensure_connection(). After setup, settings_dict['NAME'] is test_application, pool identity is unchanged, and pool.kwargs['dbname'] is application. The mock prevents SQL execution; the stale kwargs and selection behavior are real.

test_keepdb_teardown_retains_test_database_pool constructs a real unopened pool with NAME=test_application and calls the actual destroy_test_db(old_database_name='application', keepdb=True). NAME is restored to application, but the same cached pool still has dbname=test_application. No SQL or driver connections are involved. Both probes passed.

These are target-selection failures demonstrated offline. The review did not run migrations against the wrong database or confirm PostgreSQL DROP behavior with connected pool workers. Such operations are unnecessary to establish the retained configuration defect and were outside the available fixtures.

## Cleanup and ownership evidence

close_pool() at base.py:243–246 evaluates self.pool, which allocates on a cache miss. Therefore a request to dispose of a pool can import the optional dependency, validate configuration, generate adapter context, allocate a ConnectionPool, then immediately close it. test_close_and_timezone_inspection_construct_unused_pools wraps the installed constructor and observes one construction for close_pool() on an empty registry and a second for ensure_timezone(); both leave the registry empty and the wrapper unconnected.

ensure_timezone() at lines 362–367 calls close_pool() before inspecting wrapper.connection. _close() at lines 385–399 also evaluates self.pool, even though its release operation uses the old raw connection's private _pool attribute. If timezone invalidation deleted the registry entry while a borrower still holds a connection, this lookup creates a replacement pool just to return the old lease. That additional case follows directly from the source; the constructor instrumentation probe covers cold disposal and timezone inspection, not an actual outstanding server lease.

Pool checks now also appear in acquisition, init_connection_state(), and close_if_health_check_failed(). Some mode differences are inherent in pooling. The unnecessary complexity comes from using an allocating property as a mode test and then obtaining ownership from a different place. The “workaround for tests” comment acknowledges the split ownership but should not be the model defining production release.

Repeated lookup and deletion also lack one indivisible ownership transfer. Two invalidators could close the same object and race on del; this is a conditional concurrency concern from source inspection, not a separately reproduced finding. A non-creating pop, protected consistently with insertion if concurrent invalidation is supported, removes this category of race.

## Worked code-judo proposal

Give registry creation one explicit name and make invalidation consume the current entry. Continue sharing pools by alias; do not create one pool per wrapper or thread. Avoid a new hierarchy of pass-through providers when a small registry boundary and one owner field suffice.

```python
def close_pool(self):
    # Use the same registry synchronization policy as get-or-create.
    with pool_registry_lock:
        pool = self._connection_pools.pop(self.alias, None)
    if pool is not None:
        pool.close()

def borrow_connection(self):
    pool = self.get_or_create_pool()
    pool.open()
    connection = pool.getconn()
    self._borrowed_from = pool
    return connection

def _close(self):
    owner = self._borrowed_from
    if owner is None:
        return super()._close()
    with self.wrap_database_errors:
        owner.putconn(self.connection)
        self.connection = None
        self._borrowed_from = None
```

Initialize _borrowed_from once and maintain it only with a successful acquisition/release. Preserve the base wrapper's handling of closed_in_transaction, needs_rollback, and atomic exit. The new ensure_connection() guard is justified: returning a pooled raw connection must prevent its former wrapper from reopening or using it within the same atomic block. The guard probe passed, and this review does not recommend deleting it or treating that shared invariant as PostgreSQL-only feature leakage.

Close the borrowed connection and invalidate its registry pool before database identity changes. The canonical database-creation lifecycle should expose this resource boundary, with PostgreSQL overriding the backend-specific disposal. If a narrowly scoped patch instead uses PostgreSQL create_test_db(), destroy_test_db(), and worker/mirror overrides, make their order explicit and ensure keepdb is covered; do not add pool-specific checks throughout backend-neutral SQL paths. Disposal must precede changing NAME. Administrative fallback wrappers should use NO_DB_ALIAS while retaining the selected real database name.

A connection-parameter signature checked on acquisition is another way to protect registry identity, but avoid arbitrary settings hashing and silent eviction. The concrete test lifecycle boundaries are already known. An explicit invalidation contract is easier to reason about than discovering drift only after mutating settings.

The proposal deletes lazy lookup from cleanup, removes reliance on the driver's private ownership marker, and turns the two resource lifetimes into explicit obligations. It is not applied code. Validate live return to an old closed pool, pending checkouts, reconfiguration with a borrower outstanding, repeated setup/teardown with keepdb, and cloning/worker setup before choosing final locking and shutdown behavior.

## Commands and status

Evidence includes git diff main...review-head, numbered reads of postgresql/base.py and postgresql/creation.py, reads of base/creation.py:32–111, 283–329, and 377–384, django/test/signals.py:58–83, and django/test/runner.py worker setup. Local pool source was inspected for configure, getconn, putconn, close, and reset behavior. Offline probes passed; no live database behavior or proposed remedy was tested.
