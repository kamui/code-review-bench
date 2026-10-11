# K3: Django pull request 17914, a failed connection reaches the caller as a pool timeout

Pull request: django/django #17914, "Refs #33497 -- Added connection pool support for PostgreSQL." Commit before the change: `bcccea3e`. Head: `fad334e1`. Last push, the cut-off: 2024-03-01T08:02:02Z.

Decisions: `AD-CL-v-pool-connect-timeout` and `AD-NC-92658b9418a9-eligibility`. Saved ruling: first-round ruling 44 (D7b), "D7b is advisory". Kind proposed on 2026-10-11: suggestion (improvement), low confidence.

This dossier's reading agrees with the proposal. It does not read the claim as a problem.

Each fact is marked *run* (executed on 2026-10-11, probe and output saved under `probes/`), *read* (in the diff, the source or a fetched record) or *reported*.

## Problem

Django's new option `OPTIONS["pool"]` makes the PostgreSQL backend take its connections from a pool kept by the library `psycopg_pool`.

Without the pool, Django opens a connection on the caller's own thread and then sets the time zone and the role on it. If the server refuses anything, the caller gets the server's message at once, as the exception.

With the pool, background threads of the pool library open the connections and run Django's setup. When that fails, the pool library logs a warning, drops the connection and tries again. The caller waits for the pool's timeout, 30 seconds by default, and then gets "couldn't get a connection after 30.00 sec". The server's message is not in that exception. It is in the pool library's log, once per attempt.

This happens for any failure while a pooled connection is opened or set up: a wrong database name, a wrong password, a server that is down, a time zone the server does not know.

## What changed

All *read*, `django/db/backends/postgresql/base.py` at the head. The pool is built with Django's setup as its callback and is not opened:

```python
            pool = ConnectionPool(
                kwargs=connect_kwargs,
                open=False,  # Do not open the pool during startup.
                configure=self._configure_connection,
                check=ConnectionPool.check_connection if enable_checks else None,
                **pool_options,
            )
```

The caller's side becomes:

```python
        if self.pool:
            # If nothing else has opened the pool, open it now.
            self.pool.open()
            connection = self.pool.getconn()
        else:
            connection = self.Database.connect(**conn_params)
```

And the setup on the caller's thread is skipped when a pool is used:

```python
        if self.connection is not None and not self.pool:
            commit = self._configure_connection(self.connection)
```

`getconn()` waits for a ready connection and raises `PoolTimeout` when none arrives in time. Django's existing wrapper turns that into `django.db.utils.OperationalError`.

At the commit before the change the option does not exist. Setting it fails at once with `invalid connection option "pool"` (*run*).

## Promised?

The exact operation: with pooling on, when a connection cannot be opened or set up, the calling code is told the server's reason without waiting for a timeout.

Who maintains it: opening connections in the background, retrying, and what a waiting caller is told belong to `psycopg_pool`, a dependency. Django's part is choosing to hand its setup to the pool's `configure` callback, calling `getconn()`, and wrapping the exception.

**Written, by Django.** From the documentation at the head (*read*; searches in `upstream/search-django-docs-pool.txt` and `search-django-docs-connection-errors.txt`):

- The new section, the whole of what Django says about the pool: "To use a connection pool with `psycopg`_, you can either set ``"pool"`` in the :setting:`OPTIONS` part of your database configuration in :setting:`DATABASES` to be a dict to be passed to :class:`~psycopg:psycopg_pool.ConnectionPool`, or to ``True`` to use the ``ConnectionPool`` defaults" and "This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed and is ignored with ``psycopg2``." (`docs/ref/databases.txt`, added by the change.)
- The release note, added by the change: "``"pool"`` option is now supported in :setting:`OPTIONS` on PostgreSQL to allow using :ref:`connection pools <postgresql-pool>`."
- Searched and found silent: nothing in `docs/` or `django/` at the head mentions `PoolTimeout`, the pool's timeout, the pool's logger, or how a failed connection is reported with a pool.
- An older page, not touched by the change, `docs/ref/exceptions.txt`: "Django wraps the standard database exceptions so that your Django code has a guaranteed common implementation of these classes." and "As per :pep:`3134`, a ``__cause__`` attribute is set with the original (underlying) database exception, allowing access to any additional information provided."

**Written, by the dependency Django points to.** Django's section names `psycopg_pool.ConnectionPool` as what the option's dictionary is passed to and whose defaults `True` uses. Under rule Promised 4b a dependency's documentation counts for the features the project points to. psycopg's pool documentation as it stood before the cut-off (*read*; `upstream/psycopg-docs_advanced_pool.rst@558cbe7ef7ef6a6d1f6c65e368aaaf9f9a10108e.rst`, last changed 2024-02-26, and `upstream/psycopg-docs_api_pool.rst@7b4e7a899926f2012eddca31c0f67d1ba06a0c33.rst`, last changed 2023-10-14):

- "After a pool is open, it can accept new clients even if it doesn't have `!min_size` connections ready yet. However, if the application is misconfigured and cannot connect to the database server, the clients will block until failing with a `PoolTimeout`."
- "If you want to make sure early in the application lifetime that the environment is well configured, you can use the `~ConnectionPool.wait()` method after opening the pool, which will block until `!min_size` connections have been acquired, or fail with a `!PoolTimeout` if it doesn't happen in time".
- "If an attempt to create a connection fails, a new attempt will be made soon after, using an exponential backoff to increase the time between attempts, until a maximum of `!reconnect_timeout` is reached. When that happens, the pool will call the `!reconnect_failed()` function, if provided to the pool, and just start a new connection attempt. You can use this function either to send alerts or to interrupt the program and allow the rest of your infrastructure to restart it."
- "The pool uses the `logging` module to log some key operations to the ``psycopg.pool`` logger."
- `timeout`: "The default maximum time in seconds that a client can wait to receive a connection from the pool", default 30 seconds. `configure`: "A callback to configure a connection after creation."

So the dependency's documentation describes what happens here. A misconfigured application's callers block and then fail with `PoolTimeout`. It promises no server message to the caller.

**Announced.** The pull request's description is empty. Its code comment on the setup function reads "This function is called from init_connection_state and from the psycopg pool itself after a connection is opened." Nothing in the change says how errors are reported (*read*). In the earlier pull request for the same work, a co-author rejected running the setup on the caller's thread at each checkout: "Now we configure the connection every time on pool checkout this is certainly wrong!" (#17594, 2023-12-13, *read*, `docs/research/cohort-rebuild-2026-10-05/candidates/v-django-17914/upstream/pr-17594-review-comments.json`).

**Built.** The change's tests (*read*, `tests/backends/postgresql/tests.py` at the head):

- `test_connect_pool` expects `PoolTimeout` when a pool of two connections is asked for a third.
- `test_connect_role` asserts the server's message, `role "…" does not exist`, for a role that does not exist. The change makes it run with the pool off (`no_pool_connection()`). No test makes a connection fail with the pool on.

**How it behaved before.** Pooling is new and opt-in. Without the option the caller gets the server's message at once, at both commits (*run*). Nothing that worked before the change behaves differently after it.

**Did the owners say anything about it by the cut-off?** No statement on error reporting with the pool was found in either pull request's discussion (*read*, the saved review and issue comments of #17594 and #17914, scanned for timeout, logging, error and failure).

My answer is **no**. Nothing Django wrote, announced or built says that a caller is told the server's reason when a pooled connection cannot be opened. The documentation of the feature Django points to says the caller gets a `PoolTimeout`.

One sentence could be read the other way. The exceptions page says `__cause__` holds "the original (underlying) database exception, allowing access to any additional information provided". With the pool on, `__cause__` is the pool's `PoolTimeout`, which is the exception the driver raised to the caller, and the server's own error is not reachable from it (*run*). Whether that sentence covers an error raised on another thread and never raised to the caller is the owner's to read. I read it as kept, because it describes how Django wraps what the driver raises.

## Delivered?

Asked only if the answer above is yes. The facts it would need are under the next heading.

## What the affected person sees

Who: someone who turns the pool on and has a connection or setup failure. A mistyped database name or password on first trying the option is the plainest case.

Runs on PostgreSQL 16.15 with psycopg 3.1.18 and psycopg_pool 3.2.1, the newest releases from before the cut-off (`probes/result-head.txt`). The pool timeout was set to 6 seconds, except in the last row.

| Failure | Pool | After | What the first query raises | Server's message in the exception chain | Lines on standard error |
| --- | --- | --- | --- | --- | --- |
| Time zone the server rejects | off | 0.0 s | `DataError: invalid value for parameter "TimeZone": "Mars/Olympus_Mons"` | yes | 0 |
| The same | on | 6.1 s | `OperationalError: couldn't get a connection after 6.00 sec`, caused by `psycopg_pool.PoolTimeout` | no | 12 |
| Database that does not exist | off | 0.0 s | `OperationalError: connection failed: FATAL:  database "k3_no_such_db" does not exist` | yes | 0 |
| The same | on | 6.1 s | the same timeout | no | 12 |
| Wrong password | off | 0.0 s | `OperationalError: connection failed: FATAL:  password authentication failed for user "k3_pwuser"` | yes | 0 |
| The same | on | 6.0 s | the same timeout | no | 12 |
| Nothing listening | off | 0.0 s | `OperationalError: connection is bad: No such file or directory` | yes | 0 |
| The same | on | 6.0 s | the same timeout | no | 24 |
| Nothing wrong | on | 0.0 s | the query returns | | 0 |
| Time zone the server rejects | on, default timeout | 30.0 s | `OperationalError: couldn't get a connection after 30.00 sec` | no | 21 |

Each line on standard error is the pool library's warning with the server's message, for example:

```
error connecting in 'pool-1': connection failed: FATAL:  password authentication failed for user "k3_pwuser"
```

Where that warning goes depends on the project's logging (*run*, the time zone case with the pool on):

| Logging configuration | Warning lines seen |
| --- | --- |
| No `LOGGING` setting, which is Django's default | 12, on standard error |
| A console handler on the root logger | 12 |
| A `LOGGING` setting that names only the `django` logger | 12, on standard error |
| The root logger and its handler set to `ERROR` | 0 |

So with default logging the reason is printed many times while the caller waits. A project that keeps only errors sees the timeout and no reason.

At the commit before the change every pool row fails at once with `invalid connection option "pool"`, and every row without the pool is the same as at the head (*run*, `probes/result-base.txt`).

How stuck: slowed, not blocked. Every request fails with an error after the wait. Nothing proceeds with wrong data. The reason is in the log unless the project discards warnings. The pool's own documentation tells a reader that a misconfigured application ends in `PoolTimeout`.

How likely the triggers are: connection failures such as a wrong name, password or host are common when first setting up. Of Django's two setup steps, a role failure with the pool is the known problem `GT-v1`, which never reaches the server. A time zone failure needs a zone that Django's own machine knows and the server does not, because Django checks the name against local time zone data when it loads a settings file. The run got round that check with `settings.configure()`.

## How each fact is known

| Fact | Known by | Knowable at the cut-off |
| --- | --- | --- |
| What the caller gets, after how long, with the pool on and off, for four failures, at both commits | run (`probes/result-base.txt`, `result-head.txt`) | yes; run with library versions from before the cut-off |
| The server's message is not in the exception chain with the pool on | run (the probe searches the chain) | yes |
| The pool library logs the server's message at warning level on each attempt | run; read (`psycopg_pool/pool.py` 3.2.1, line 641) | yes |
| Where the warning goes under four logging configurations | run | yes |
| The same outcomes with psycopg 3.3.6 and psycopg_pool 3.3.3 | run on 2026-10-05, saved (`docs/research/cohort-rebuild-2026-10-05/candidates/v-django-17914/probes/D7/result-head.txt`); not run again | later versions |
| The diff lines and the tests | read (mirror, both commits) | yes |
| Django's documentation, quoted and searched | read (`upstream/search-django-docs-*.txt`) | yes |
| psycopg's pool documentation | read (two files fetched at their last commit before the cut-off) | yes, 2024-02-26 and 2023-10-14 |
| The co-author's comment on the earlier pull request | read (saved review comments) | yes, 2023-12-13 |
| A maintainer's advice on the ticket to "enable pool logging" | read (saved `trac-33497.html`, comment 45, 2024-10-01) | **later** |
| Django's `main` on 2026-10-11 still calls `pool.open()` and `pool.getconn()`, and its pool section still says nothing about errors | read (`upstream/django-main--*`) | **later** |
| A GitHub search of django/django for `PoolTimeout` returns two pull requests, neither about this | read (`upstream/search-github-django-PoolTimeout.json`) | search made in 2026 |
| A web request through a server, a project settings file with a zone the server lacks, a `reconnect_failed` callback | not run | |
| Django's ticket tracker | not searched; it refuses automated searches | |

## The saved ruling's reasons, checked

- "The error is still raised to the caller on every attempt, and the psycopg.pool logger records the server's own message at warning level for each failed attempt; nothing proceeds with wrong data": holds (*run* again with older library versions).
- "It is how the pool library the option hands control to reports failures": holds, and psycopg's documentation from before the cut-off says so in words.
- "A role failure with the pool is the already recorded GT-v1, and a time zone failure needs a zone name the local time zone data accepts and the server rejects": holds (*read*).
- The first-round dossier cited the maintainer's advice to enable pool logging. That comment is from 2024-10-01, after the cut-off. No reason above depends on it.

## Relation to known problems

Not part of one. `GT-v1` has the same symptom, but there the setup can never finish even when nothing is misconfigured, and the pool logs nothing. `GT-v12` is about an unstated minimum version. Here the pool works when the configuration is right.

## Both sides

For a suggestion (improvement):

- No promise was found. Django's two paragraphs on the pool say nothing about errors and hand the reader to the pool library, whose documentation states this behaviour.
- Nothing that worked before stops working. The option is new and off by default, and without it the server's message arrives as before.
- Nothing printed is untrue. "couldn't get a connection after 30.00 sec" is what happened.

For a minor defect:

- Pooling is a documented feature, so using it is promised use. It is delivered, because the pool works when the settings are right.
- The thing wrong would be the report of a failure, which comes up to 30 seconds late and with no reason in the exception.

For a problem:

- Rule Delivered 1 counts "a message a person needs" as an outcome, and a wrong password is the first thing a new user of the option is likely to meet. With warnings discarded they have no reason at all (*run*).
- The exceptions page says `__cause__` gives access to the original database exception's information, and here the server's information cannot be reached from the exception.

Against a problem:

- Delivered 1 is asked only when something is promised, and the only documentation on what a pooled caller is told says `PoolTimeout`.
- The message is not absent. With Django's default logging it is printed on every attempt.

## Recommendation

**Suggestion or observation, sub-kind improvement. Confidence: medium.** This is the kind proposed, with the confidence raised from low. The proposal had not read psycopg's documentation, which describes this behaviour, and had not run the logging cases.

- Promised: no. Searched Django's documentation, release note, tests and both pull requests' discussions, and the dependency's documentation.
- Sub-kind: improvement, because nothing stops working. The use is supported, so "outside supported use" does not fit, and nobody is shown depending on a server message from a pooled connection.

Where the rule does not decide it:

- Promised 4b says a dependency's documentation counts for the features the project points to. Here that documentation describes a limit, not a benefit. The rule has no line on whether a documented limit of a pointed-to dependency settles "not promised" for the project, or whether the project owes its own readers the same warning.
- Delivered 1's "a message a person needs" has no test for a message that is in the log and not in the exception.
- The exceptions page sentence about `__cause__` was written before pools existed in Django. Whether it reaches this case is a reading only the owner can make.

The strongest argument for another kind is that a new user who mistypes a password gets a 30 second hang and an error that says nothing about passwords, where the same mistake without the pool is answered at once with the server's words. Django chose to route every connection failure through `getconn()` and did not pass on the pool library's own advice to call `wait()` or to watch its log. If the owner reads Django's exceptions page as covering this, it is promised and not delivered, which is a problem and reopens ruling 44. If the owner counts the pool as promised use with a late and empty error as the thing wrong, it is a minor defect.

Not run: a web request through a server, an asynchronous connection, psycopg2, a `reconnect_failed` callback, and a settings file with a real zone the server lacks.
