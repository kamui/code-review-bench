# Card: with Django's PostgreSQL pool on and health checks off, requests are handed sessions the server has ended

Pull request: django/django #17914, which adds a connection pool to Django's PostgreSQL backend (`OPTIONS["pool"]`). Django is a web framework; each web request takes a database connection at its first query and gives it back at the end. "Base" is the commit before the change and "head" the reviewed commit.

## What the setting does

With the pool on, giving a connection back hands its server session to the pool instead of ending it, and the next request takes a session from the pool. The pool is built with a test of each session on checkout only when `CONN_HEALTH_CHECKS` is `True`; its default is `False` (*read*, `django/db/backends/postgresql/base.py` head line 232). The pool library, psycopg-pool, documents: "By default no check is made on the connection." Without the pool, Django's default `CONN_MAX_AGE = 0` ends the session at the end of each request, so the next request opens a new one.

## The runs

No PostgreSQL server was available. The runs use a stub that speaks PostgreSQL's wire protocol. "Restart" ends every open session with PostgreSQL's own shutdown message; "drop" closes the sockets without a message, as a proxy that resets idle connections does. Python 3.13.5, psycopg 3.1.18, psycopg-pool 3.2.1. Probes and outputs: `probes/N1/`. All *run*. Six requests follow the event.

| Settings | Base | Head |
| --- | --- | --- |
| no pool, `CONN_MAX_AGE = 0` (the defaults) | all succeed | all succeed |
| no pool, `CONN_MAX_AGE = 600`, health checks off | the first fails, the rest succeed | the same |
| no pool, `CONN_MAX_AGE = 600`, health checks on | all succeed | the same |
| `OPTIONS["pool"] = True`, health checks off | the option does not exist: `invalid connection option "pool"` | the first four fail, the last two succeed |
| `OPTIONS["pool"] = True`, health checks on | the option does not exist | all succeed |

A saved run on a real PostgreSQL 16.15 server, with a pool of two sessions and health checks off, gave the same pattern: the first two queries after the server killed both sessions failed, the next three succeeded (*read*).

## What the affected person sees

`django.db.utils.OperationalError: consuming input failed: terminating connection due to administrator command`, or `... server closed the connection unexpectedly`, on the first query of the request, usually as an error page (*run*). One request fails per idle session in the pool: four with `"pool": True`, more if the pool was configured larger and had grown. Then the pool has replaced the sessions and requests succeed without action (*run*).

## Impact facts

- **Who is affected:** sites that turn on the new pool and leave `CONN_HEALTH_CHECKS` at its default, when the database server restarts or fails over, or a proxy or network device drops idle connections, between two requests.
- **How:** a bounded number of requests fail with an error that names the closed connection. Nothing is written wrongly, and no wrong data is returned (*run*).
- **How stuck:** the pool recovers by itself after those requests (*run*). Setting `CONN_HEALTH_CHECKS = True` prevents the failures (*run*). The pool's documentation does not mention that setting, and the setting's own documentation speaks only of persistent connections (*read*). The pool library's own remedy, passing `check` in the pool options, fails at head with a `TypeError` (*run*).
- **How common:** not measured. Server restarts, failovers and idle-connection resets are ordinary operational events. The pool option was new and unreleased at the time of the change (*read*).
- **Neighbours:** Django's persistent connections without health checks already failed one request per worker thread in the same events, before and after the change, and Django's documentation says so (*run*, *read*).
- **Limits:** a firewall that silently discards packets, where a request would wait for a network time-out, was not run. Concurrent workers were not run.
