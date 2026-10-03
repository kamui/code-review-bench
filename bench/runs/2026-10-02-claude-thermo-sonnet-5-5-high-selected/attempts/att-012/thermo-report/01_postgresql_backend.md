# Detail 01 — django/db/backends/postgresql (base.py, creation.py, features.py)

Scope: `git diff main...review-head` for `django/db/backends/postgresql/*`. `base.py` is 615 lines after the change (measured with `wc -l`), so the 1k-line rule is not triggered. The concerns are structural, not size.

Verification status: static reading of the diff plus `grep -rnE "ensure_timezone|ensure_role|close_pool|\.pool\b"` across `django/`, `tests/` and `docs/`. No PostgreSQL server is available, so no behavior was executed. Claims about runtime races are reasoned from the code and marked as such.

## Finding A — Pool lifecycle is a property with side effects, checked in five places (`base.py:200-246`, `345`, `391`, `404`, `502`)

Evidence. `DatabaseWrapper.pool` (line 203) is a `@property` that does five jobs: it reads and normalizes `OPTIONS["pool"]`, special-cases `NO_DB_ALIAS`, validates `CONN_MAX_AGE`, imports `psycopg_pool`, and builds and registers a `ConnectionPool` in the class-level dict `_connection_pools`. The result is consulted through `if self.pool:` in `get_new_connection`, `_close`, `init_connection_state`, `close_if_health_check_failed` and `close_pool`, so every call re-runs the options lookup and dict probe.

`close_pool` (line 243) is `if self.pool: self.pool.close(); del self._connection_pools[self.alias]`. Calling it on a wrapper whose pool has not been built *constructs the pool* (importing `psycopg_pool`, calling `get_connection_params()`, possibly raising `ImproperlyConfigured`) only to close it. It is also not safe under concurrency: two threads that both pass `if self.pool` can both reach `del`, and the second raises `KeyError`. This is reasoned from the code, not executed.

The creation comment says the `setdefault` dance protects against races, but `close_pool` has no equivalent protection.

Problem. A getter that builds global state, plus scattered truthiness checks, is exactly the "weird if statements in random places" pattern. The reader has to hold "pooled or not" in mind across `get_new_connection`, `_close`, `init_connection_state` and the health-check override. Configuration validation (`CONN_MAX_AGE`, psycopg2 vs psycopg3, missing `psycopg_pool`) happens lazily on first connect instead of in `check_settings`/`get_connection_params`, which is where the base class already validates settings.

Code-judo proposal. Introduce a small module, e.g. `django/db/backends/postgresql/pool.py`, with a registry object:

- `get(alias, build)` takes a lock, returns the existing pool or builds one.
- `close(alias)` pops under the same lock and closes, returning without constructing anything.

Then `DatabaseWrapper` resolves the pool once per connection in `get_new_connection` and stores it (e.g. `self._pool`, or `None`). `_close`, `init_connection_state` and `close_if_health_check_failed` then read that one attribute. Splitting out a pooled-connection strategy would let `_close` and `init_connection_state` lose their branches entirely. Option validation moves to `check_settings`. `close_pool` becomes `registry.close(self.alias)`, with no property access and no `del`/KeyError window.

## Finding B — Test-only workarounds and signal-hook behavior live in production paths (`base.py:362-367`, `385-399`; `creation.py:61`, `88-90`)

Evidence. `ensure_timezone()` (line 362) now begins with `self.close_pool()`, "so new connections pick up the correct timezone". Its only non-test caller is `django/test/signals.py:82`, the `setting_changed` handler for TIME_ZONE/USE_TZ. So a method named `ensure_*` that is a no-op-or-set-timezone on the connection now also tears down a process-wide pool as a side effect. The `_close` comment says "This is a workaround for tests so a pool can be changed on setting changes", and the code reaches into the private `self.connection._pool.putconn(...)` rather than `self.pool.putconn(...)`. `creation.py` adds `close_pool()` calls in `_clone_test_db` and a new `_destroy_test_db` override.

Problem. Production `_close` carries a private-attribute access whose justification is test setting churn. Closing the pool inside `ensure_timezone` couples a settings-change hook to pool teardown. When a pool is closed while other threads hold connections, those connections stay checked out of the old pool and must be returned to it, which is the reason for the `_pool` indirection. That is a half-applied state model (two live pools for one alias) hidden behind a workaround.

Code-judo proposal. Key the pool registry by `(alias, timezone_name, role)`, or build the pool's `configure` from an immutable snapshot (see Finding C). A TIME_ZONE change then naturally yields a different pool and neither `ensure_timezone` nor `_close` needs special handling. If that is too large, at least keep the teardown in the test machinery (a `setting_changed` receiver in `django/test/signals.py`, or the test-DB creation hooks) so production `_close` is just `self._pool.putconn(self.connection)`.

## Finding C — `_configure_connection` contradicts its own contract, and shares a thread's wrapper through the pool (`base.py:89-107`, `362-383`)

Evidence. The comment says "make sure that whatever is done here does not access anything on self aside from variables", yet the body reads `self.ops`, `self.timezone_name` (a `cached_property`) and `self.settings_dict`. The pool is created once per alias with `configure=self._configure_connection`, a bound method of whichever thread's wrapper happened to build the pool first. Every later pooled connection, created from any thread's pool worker, therefore reads that first wrapper's cached `timezone_name`. That cache is only cleared on the wrappers `connections.all()` returns in the signals handler, so correctness leans on Finding B's pool closing.

Separately, the module-level functions `ensure_timezone(connection, ops, timezone_name)` and `ensure_role(connection, ops, role_name)` reuse the names of the wrapper methods, and `DatabaseWrapper.ensure_role` was deleted outright, which removes a method third-party backends or subclasses could have overridden. Both helpers return a bool "did we change something" and open a cursor each, preserved from the old methods.

Problem. The comment and code disagree, the callback captures per-thread mutable state in a shared object, and the same name now means two different things in one module.

Code-judo proposal. Build `configure` from a closure over plain values (`timezone_name`, `role_name`, and the `ops.set_time_zone_sql()` / `compose_sql` pieces), created at pool-construction time. That makes the "variables only" comment true and removes the shared bound-method capture. Keep `ensure_role` on the wrapper (delegating to the helper) to avoid an API removal, and give the module-level helpers distinct names (`_set_time_zone`, `_set_role`).

## Finding D — `is_usable` and `close_if_health_check_failed` carry pooling special cases (`base.py:489-506`)

Evidence. `is_usable` gains `if self.connection is None: return False`, and `close_if_health_check_failed` gains `if self.pool: return`. The base class `close_if_health_check_failed` already returns early when `self.connection is None`, so the `is_usable` guard only serves callers outside that path (and masks an invariant). The `pool` branch is another instance of the Finding A truthiness check. `features.py` also turns `django_test_skips` into a `cached_property` that peeks at `settings_dict["OPTIONS"]["pool"]`, a third place the "is pooling on" question is asked, this time in feature declarations.

Problem. Low severity individually, but each is another consumer of the same implicit mode flag. They are removed by Finding A's restructuring (resolve once, expose `self._pool`/`uses_pool`) rather than by polishing each.
