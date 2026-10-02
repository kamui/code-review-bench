# PostgreSQL pool lifecycle

## Scope and measurements

Reviewed the committed changes in `django/db/backends/postgresql/base.py`, `django/db/backends/postgresql/creation.py`, `django/db/backends/postgresql/features.py`, the shared base wrapper, and the PostgreSQL tests. The PostgreSQL backend wrapper grows from 516 to 615 lines (+99 net); it remains below the skill's 1,000-line threshold. The test module grows from 439 to 569 lines (+130 net). The finding here concerns ownership and lifecycle rather than file size.

Commands used for these measurements and checks were `git diff --stat main...review-head`, `git show main:django/db/backends/postgresql/base.py | wc -l`, `git show review-head:django/db/backends/postgresql/base.py | wc -l`, `git diff --check main...review-head`, and focused `nl -ba`/`sed` reads of the pool lifecycle and its call sites.

## Finding: Make pool acquisition and release use explicit pool ownership

The `pool` property at lines 202–241 is not a lookup: it imports psycopg-pool, validates persistent connection settings, builds connection kwargs, constructs a `ConnectionPool`, and inserts it in the alias-level cache. `close_pool()` at lines 243–246 then uses `if self.pool`, so cleanup is also a pool-creation path. `ensure_timezone()` calls `close_pool()` at lines 362–367, and PostgreSQL test database creation/destruction calls it in `creation.py` at lines 61 and 90. Those lifecycle operations can therefore instantiate a configured pool merely to close it, even when no connection has ever acquired it. This is surprising control flow for cleanup and undermines the lazy construction expressed by `open=False` and the accessor's placement in connection acquisition.

The release path at lines 385–399 has a second ownership problem: it returns a checked-out connection through `self.connection._pool.putconn(...)`. `_pool` is a private psycopg attribute. Django already knows the pool at acquisition time (`get_new_connection()` obtains it at lines 345–349), so reaching backward through the driver object is unnecessary coupling. It also makes it difficult to tell from the wrapper state which pool owns a leased connection, especially as `close_pool()` deletes the alias cache entry.

### Worked code-judo proposal

Make pool construction a single explicit operation used by `get_new_connection()`, with a separate non-creating lookup for lifecycle cleanup. When acquisition uses a pool, save that pool alongside the returned connection on the wrapper. `_close()` can then return the connection to the saved owner and clear both references; direct connections continue using `connection.close()`. `close_pool()` should only close and remove an existing cache entry. This removes the private driver reach-through and the accidental pool-construction side effect without changing alias-level sharing or connection configuration.

Keep the state transition small and explicit: no owner means the wrapper has no pooled lease; successful pooled acquisition sets the connection and owner together; pooled release returns through that owner and clears both; direct release closes the connection. If a pool is closed or replaced while leases remain, define that transition at the pool lifecycle boundary rather than inferring ownership from a driver-private field.

### Verification status

Static evidence only. The repository's `git diff --check` completed without output, and `git status --short` was empty after review. The supplied execution policy states there is no PostgreSQL server, so live pool lifecycle behavior could not be verified and no tests were run. The concern is directly supported by the accessor and call-site control flow and does not rely on a test failure.
