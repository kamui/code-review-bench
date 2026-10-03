# PostgreSQL pool lifecycle and session hygiene

## Finding

In `django/db/backends/postgresql/base.py`, `_close()` calls `self.connection._pool.putconn(self.connection)` for pooled connections (lines 385–397). `init_connection_state()` applies `_configure_connection()` only when there is no pool (lines 401–408), while pooled connections receive that callback from `ConnectionPool` when the physical connection is configured (lines 228–233). Consequently, Django’s normal logical close returns the session for reuse, and a subsequent borrower does not reapply Django configuration.

The pool’s ordinary transaction cleanup does not reset arbitrary PostgreSQL session state. If one request executes session-level commands such as `SET search_path` or creates a temporary table, a later request may receive that same session with the changed state. This changes the isolation users get when `CONN_MAX_AGE=0`, where Django otherwise physically closes the connection at request end. The current implementation has no explicit reset callback or documented caveat for this behavior.

## Structural remedy and worked proposal

Treat each pool checkout as a fresh Django logical connection. Add one explicit session-reset operation at the pool boundary, invoked after rollback and before the connection is made available to another borrower. It should restore the PostgreSQL session to a known baseline, then reapply the Django-owned settings such as timezone and assumed role. Keep this policy in one pool lifecycle abstraction instead of relying on connection creation as a proxy for checkout; creation and checkout are different events once pooling is enabled.

A concrete implementation should either install a psycopg-pool `reset` callback that restores session state and then reinitializes Django-owned settings, or implement an equivalent before-checkout hook if the pool API supports that more directly. The implementation should explicitly account for PostgreSQL reset-command transaction restrictions and for `assume_role`/timezone being reapplied after reset. Add a test that changes session state through one logical wrapper, returns it, borrows again, and verifies the baseline; also cover the configured timezone and role.

## Evidence and measurements

The review range adds 122 lines and removes 23 in `django/db/backends/postgresql/base.py`; the file is 615 lines after the change and does not cross the skill’s 1,000-line threshold. The relevant paths were inspected with:

- `git diff --unified=16 main...review-head -- django/db/backends/postgresql/base.py`
- `nl -ba django/db/backends/postgresql/base.py | sed -n '360,410p'`
- `sed -n '340,365p' django/db/backends/base/base.py`

## Verification status

Static review only. No tests were run. A PostgreSQL server is unavailable under the execution packet, and the behavior at issue is specifically the session state retained by real pooled PostgreSQL connections.
