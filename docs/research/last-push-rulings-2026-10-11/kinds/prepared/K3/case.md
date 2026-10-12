# Case K3: Django pull request 17914, a failed connection reaches the caller as a pool timeout

Pull request: django/django #17914, "Refs #33497 -- Added connection pool support for PostgreSQL.", opened by `felixxm` (a project member). It continues pull request #17594.

The cut-off for this case is the last push to the pull request: **2024-03-01T08:02:02Z**. Everything above the last heading was knowable at that moment. The last heading lists what became known only later.

Each fact is marked *run* (executed on 2026-10-11, probe and output saved under `probes/`), *read* (visible in the diff, the source or a fetched record) or *reported* (someone's claim, not checked).

## What a review comment says

> **Trigger.** With pooling enabled, opening or configuring a physical connection fails, for example because the server rejects the configured time zone or role.
>
> **Mechanism.** Role and time zone setup run only in the pool's configure callback on a pool worker thread, where the failure is logged and the connection is discarded and retried; the caller only waits in getconn() and the error is not raised to it or wrapped by wrap_database_errors.
>
> **Consequence.** The caller gets a generic PoolTimeout, wrapped as OperationalError, after the pool timeout instead of the immediate database error.
>
> **Relation to the change.** New with the pool: without it the setup runs on the caller's thread and the server's error is raised at once.

## Background

Django's PostgreSQL backend opens a database connection the first time a request needs one, then sets the time zone and, if configured, a role on it. The change adds an option, `OPTIONS["pool"]`, that makes the backend take its connections from a pool kept by the library `psycopg_pool`, which belongs to the psycopg project and not to Django. The option is off unless a project turns it on.

## What the change does

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

The setup on the caller's thread is skipped when a pool is used:

```python
        if self.connection is not None and not self.pool:
            commit = self._configure_connection(self.connection)
```

The setup function carries this comment: "This function is called from init_connection_state and from the psycopg pool itself after a connection is opened."

`getconn()` waits for a ready connection and raises `PoolTimeout` when none arrives in time. Django's existing wrapper turns that into `django.db.utils.OperationalError`.

Inside `psycopg_pool` 3.2.1, a background worker opens each connection and calls the callback. If either raises, the worker logs `error connecting in '<pool name>': <the error>` at warning level and schedules another attempt (*read*, `psycopg_pool/pool.py`, line 641).

At the commit before the change the option does not exist.

## What Django says

From the documentation at the head (*read*; searches in `upstream/search-django-docs-pool.txt` and `search-django-docs-connection-errors.txt`):

- The section the change adds to `docs/ref/databases.txt`, in full apart from its example: "To use a connection pool with `psycopg`_, you can either set ``"pool"`` in the :setting:`OPTIONS` part of your database configuration in :setting:`DATABASES` to be a dict to be passed to :class:`~psycopg:psycopg_pool.ConnectionPool`, or to ``True`` to use the ``ConnectionPool`` defaults" and "This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed and is ignored with ``psycopg2``."
- The release note the change adds: "``"pool"`` option is now supported in :setting:`OPTIONS` on PostgreSQL to allow using :ref:`connection pools <postgresql-pool>`."
- Nothing in `docs/` or `django/` at the head mentions `PoolTimeout`, the pool's timeout, the pool's logger, or how a failed connection is reported when a pool is used.
- `docs/ref/exceptions.txt`, an older page the change does not touch: "Django wraps the standard database exceptions so that your Django code has a guaranteed common implementation of these classes." and "As per :pep:`3134`, a ``__cause__`` attribute is set with the original (underlying) database exception, allowing access to any additional information provided."
- The pull request's description is empty.
- The change's tests (`tests/backends/postgresql/tests.py`): `test_connect_pool` expects `PoolTimeout` when a pool of two connections is asked for a third. `test_connect_role` asserts the server's message, `role "…" does not exist`, for a role that does not exist, and the change makes it run with the pool off. No test makes a connection fail with the pool on.
- In pull request #17594, on 2023-12-13, a co-author wrote about a version that ran the setup on the caller's thread at each checkout: "Now we configure the connection every time on pool checkout this is certainly wrong!" No comment in either pull request, up to the cut-off, is about how errors are reported with the pool.

## What psycopg says

psycopg's pool documentation as it stood before the cut-off (*read*; `upstream/psycopg-docs_advanced_pool.rst@558cbe7ef7ef6a6d1f6c65e368aaaf9f9a10108e.rst`, last changed 2024-02-26, and `upstream/psycopg-docs_api_pool.rst@7b4e7a899926f2012eddca31c0f67d1ba06a0c33.rst`, last changed 2023-10-14):

- "After a pool is open, it can accept new clients even if it doesn't have `!min_size` connections ready yet. However, if the application is misconfigured and cannot connect to the database server, the clients will block until failing with a `PoolTimeout`."
- "If you want to make sure early in the application lifetime that the environment is well configured, you can use the `~ConnectionPool.wait()` method after opening the pool, which will block until `!min_size` connections have been acquired, or fail with a `!PoolTimeout` if it doesn't happen in time".
- "If an attempt to create a connection fails, a new attempt will be made soon after, using an exponential backoff to increase the time between attempts, until a maximum of `!reconnect_timeout` is reached. When that happens, the pool will call the `!reconnect_failed()` function, if provided to the pool, and just start a new connection attempt. You can use this function either to send alerts or to interrupt the program and allow the rest of your infrastructure to restart it."
- "The pool uses the `logging` module to log some key operations to the ``psycopg.pool`` logger. If you are trying to debug the pool behaviour you may try to log at least the ``INFO`` operations on that logger."
- `timeout`: "The default maximum time in seconds that a client can wait to receive a connection from the pool (using `connection()` or `getconn()`)", default 30 seconds.
- `configure`: "A callback to configure a connection after creation."

## The runs

PostgreSQL 16.15, Python 3.12, psycopg 3.1.18 and psycopg_pool 3.2.1, the newest releases from before the cut-off. Each case ran in its own process. The pool timeout was set to 6 seconds, except in the last row. All *run*, `probes/result-head.txt`.

At the head:

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

Where that warning goes, for the time zone case with the pool on:

| Logging configuration | Warning lines seen |
| --- | --- |
| No `LOGGING` setting, which is Django's default | 12, on standard error |
| A console handler on the root logger | 12 |
| A `LOGGING` setting that names only the `django` logger | 12, on standard error |
| The root logger and its handler set to `ERROR` | 0 |

At the commit before the change (`probes/result-base.txt`), every row with the pool option fails at once with `ProgrammingError: invalid connection option "pool"`. Every row without it is the same as at the head.

The time zone case sets the zone through `settings.configure()`, which skips a check. When Django loads a project's settings file it refuses a zone name its own machine does not know. A project reaches this failure only with a name that Django's machine knows and the database server does not.

Of the two setup steps Django runs in the callback, the other is the role. Pooling together with a role setting is covered by a problem already known on this pull request. Its cause is different. The setup re-enters the pool and never reaches the server.

## What the affected person sees

Who: someone who turns the pool on and has a connection or setup failure, such as a mistyped database name or password, a server that is down, or a time zone the server lacks.

With the pool on, every request waits for the pool's timeout and then fails with an error that says a connection could not be had. The exception does not hold the server's message. The pool library writes that message as a warning on every attempt. With Django's default logging the warnings appear on standard error. A project that keeps only errors sees none of them.

Without the pool, at both commits, the server's message is the exception and arrives at once.

Nothing proceeds with wrong data in any run.

Not run: a web request through a server, an asynchronous connection, psycopg2, a `reconnect_failed` callback, and a settings file with a real zone the server lacks. Django's ticket tracker refuses automated searches and was not searched.

## Known only later

- On 2024-10-01, on Django's ticket for the feature, a co-author answered a user who reported occasional pool timeouts in production: "Ok, I'd recommend enabling pool logging (https://www.psycopg.org/psycopg3/docs/advanced/pool.html#pool-operations-logging) and see where it goes south" (*read*).
- Django's `main` branch on 2026-10-11 still calls `self.pool.open()` and `self.pool.getconn()`. Its pool section is longer and still says nothing about how a failed connection is reported (*read*).
- With psycopg 3.3.6 and psycopg_pool 3.3.3, releases of 2026, the outcomes are the same (*run* on 2026-10-05).
- A search of django/django on GitHub for `PoolTimeout`, made on 2026-10-11, returns two pull requests. Neither is about this (*read*).
