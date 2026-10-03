# 01 — Shared base layer: the `ensure_connection()` guard and the atomic-close invariant

Scope: `django/db/backends/base/base.py` (+5/−1), and the PostgreSQL code that made the base change necessary (`django/db/backends/postgresql/base.py`, `_close()` and `is_usable()`).

Review range: `bcccea3ef3..fad334e1a9` (`git diff main...review-head`).

## What the diff does

`BaseDatabaseWrapper.ensure_connection()` gains a new branch, for every backend:

```python
# django/db/backends/base/base.py:271-279 (head)
def ensure_connection(self):
    """Guarantee that a connection to the database is established."""
    if self.connection is None:
        if self.in_atomic_block and self.closed_in_transaction:
            raise ProgrammingError(
                "Cannot open a new connection in an atomic block."
            )
        with self.wrap_database_errors:
            self.connect()
```

The branch exists because the pooled PostgreSQL `_close()` sets `self.connection = None` by itself:

```python
# django/db/backends/postgresql/base.py:385-399 (head)
def _close(self):
    if self.connection is not None:
        with self.wrap_database_errors:
            if self.pool:
                self.connection._pool.putconn(self.connection)
                # Connection can no longer be used.
                self.connection = None
            else:
                return self.connection.close()
```

## The invariant that was broken

Before this PR the base class owned one clear state machine for "closed while inside `atomic()`":

- `BaseDatabaseWrapper.close()` (`base/base.py:344-361`) calls `_close()` and then, if `in_atomic_block`, sets `closed_in_transaction = True` and `needs_rollback = True` **and leaves `self.connection` pointing at the closed driver connection**. Only outside an atomic block does it set `self.connection = None`.
- Any query issued later in the same block hits the closed driver connection and fails with the driver's own "connection is closed" error.
- `Atomic.__exit__` (`django/db/transaction.py:301-313`) is the one place that finally sets `connection.connection = None`, at the outermost block.
- The next `ensure_connection()` reconnects, and `connect()` (`base/base.py:241-250`) resets `in_atomic_block`, `savepoint_ids`, `atomic_blocks`, `needs_rollback` and `closed_in_transaction`. The comment there says exactly this: "In case the previous connection was closed while in an atomic block".

So "`connection is None` while `in_atomic_block and closed_in_transaction`" was a state that only existed *after* the outermost atomic block had exited with autocommit disabled, and `connect()` was the designated way out of it.

The pooled `_close()` creates that same state *inside* the block (it has to drop the reference, because the driver connection now belongs to the pool and may be handed to another thread). The new base guard then treats the state as "still inside an atomic block" — but it cannot tell the two situations apart.

## Verified regression on every backend

With `AUTOCOMMIT: False`, `Atomic.__enter__` sets `in_atomic_block = True` and `commit_on_exit = False` (`transaction.py:196-201`). On exit, the "Outermost block exit when autocommit was disabled" branch (`transaction.py:308-313`) sets `connection.connection = None` when `closed_in_transaction` is set and deliberately does **not** clear `in_atomic_block` — it relied on `connect()` doing that. After this PR `ensure_connection()` raises before `connect()` can run, and nothing else resets the flags: `close()` returns early when `closed_in_transaction or self.connection is None`, and `close_if_unusable_or_obsolete()` is a no-op when `self.connection is None`. The wrapper is unusable for the rest of the thread's life.

Reproduction (SQLite file database, no PostgreSQL involved; scratch script `scratch/t_autocommit.py`, run once against a `git archive main` export and once against the review head):

```python
settings.configure(DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3",
                                          "NAME": db, "AUTOCOMMIT": False}})
with transaction.atomic():
    with connection.cursor() as c:
        c.execute("SELECT 1")
    connection.close()
# two further queries, outside any atomic block
```

Output:

```text
== main
after block: connection=None in_atomic_block=True closed_in_transaction=True
query 0 ok (1,)
query 1 ok (1,)
== head
after block: connection=None in_atomic_block=True closed_in_transaction=True
query 0 raised ProgrammingError Cannot open a new connection in an atomic block.
query 1 raised ProgrammingError Cannot open a new connection in an atomic block.
```

Verification status: **confirmed by execution** on both revisions. The error message is also wrong for this case: the query is not in an atomic block.

## Second-order patches caused by the same broken invariant

`DatabaseWrapper.is_usable()` in the PostgreSQL backend gains

```python
# django/db/backends/postgresql/base.py:489-491 (head)
def is_usable(self):
    if self.connection is None:
        return False
```

although the base contract says "This method may assume that self.connection is not None" (`base/base.py:565-576`). The guard is only needed because pooled `_close()` now nulls the connection inside an atomic block (offline probe, `scratch/t_pool.py` section 5):

```text
pooled=True  -> connection is None: True,  closed_in_transaction: True, putconn called: True
pooled=False -> connection is None: False, closed_in_transaction: True, putconn called: False
```

`init_connection_state()` likewise gains `if self.connection is not None and ...` (`postgresql/base.py:404`), which cannot be false at its only call site (`connect()` assigns `self.connection` on the line before calling it).

So one deviation in `_close()` produced three compensating conditionals in three different methods across two layers. That is the pattern the review standard calls spaghetti growth: the feature's state shape is patched around in shared code instead of being kept inside the feature.

## Worked code-judo proposal

Keep the base invariant and delete the compensations. The only thing the pooled path must guarantee is that the wrapper never keeps a *live* reference to a driver connection it has handed back. Inside an atomic block that can be achieved by really closing the driver connection before returning it; psycopg_pool discards closed connections on return and schedules a replacement (`psycopg_pool/pool.py`, `_return_connection`: `transaction_status == UNKNOWN` → "discarding closed connection" → `AddConnection`). The wrapper then holds a closed driver connection, exactly like the non-pooled path, and `BaseDatabaseWrapper.close()` / `Atomic.__exit__` keep owning the state machine.

```python
def _close(self):
    if self.connection is not None:
        with self.wrap_database_errors:
            if self.in_atomic_block or not self._pool_options:   # see detail 02 for _pool_options
                # Same behaviour as every other backend: the wrapper keeps a
                # closed connection until the outermost atomic block exits.
                self.connection.close()
            if self._pool_options:
                self.pool.putconn(self.connection)   # a closed connection is discarded by the pool
```

With that:

- the four added lines in `base/base.py` and the `ProgrammingError` import are deleted (the base file goes back to +0/−0);
- the `is_usable()` `None` guard is deleted;
- `self.connection = None` in `_close()` is deleted (the base `close()` already does it outside atomic blocks);
- a query after `close()` inside `atomic()` fails with the same driver error on pooled and non-pooled connections, instead of two different behaviours;
- `test_cannot_open_new_connection_in_atomic_block`, which today asserts the base behaviour from the PostgreSQL test module by setting `in_atomic_block` and `closed_in_transaction` by hand (`tests/backends/postgresql/tests.py:329-338`), is replaced by a test of the real scenario.

Cost: a connection closed inside an atomic block is not reused by the pool. That case is already an error path (the transaction is lost), so this is the right trade.

Verification status of the proposal: the pool's handling of closed connections was read in the installed psycopg_pool 3.3.3 source; the end-to-end behaviour was **not** run because no PostgreSQL server is provisioned.

If the author instead wants the new error as a deliberate cross-backend behaviour ("no silent reconnect inside an atomic block"), then it has to be done in the layer that owns the state: `Atomic.__exit__` must clear `in_atomic_block` in the autocommit-off branch, the change needs its own base-level test in `tests/transactions` or `tests/backends/base`, and a release note. It should not ride in as a side effect of a PostgreSQL feature.

## Commands

```text
git -C clone archive main django | tar -x -C clone-work/scratch/main
PYTHONPATH=clone-work/scratch/main  clone-cache/venv/bin/python clone-work/scratch/t_autocommit.py
PYTHONPATH=clone PYTHONDONTWRITEBYTECODE=1 clone-cache/venv/bin/python clone-work/scratch/t_autocommit.py
PYTHONPATH=clone PYTHONDONTWRITEBYTECODE=1 clone-cache/venv/bin/python clone-work/scratch/t_pool.py
```
