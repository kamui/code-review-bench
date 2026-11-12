# The comment on the pool's skip of Django's per-request health check

Pull request: django/django #17914, "Refs #33497 -- Added connection pool support for PostgreSQL." Base is `bcccea3ef31c777b73cba41a6255cd866bf87237`, the commit before the change. Head is `fad334e1a9b54ea1acb8cce02a25934c5acfe99f`, the reviewed commit. The last push was at 2024-03-01T08:02:02Z, and the description was empty then (*read*, task packet). Every fact below was available by that time unless it sits under the last heading. Each fact is marked *run* (executed, output saved under `probes/N1/`), *read* or *reported*.

## Background in plain words

- `CONN_HEALTH_CHECKS`, default `False`: when `True`, Django tests a connection it kept from an earlier request before using it, once per request, in `close_if_health_check_failed()` (*read*, `django/db/backends/base/base.py` head lines 578 to 589).
- `connect()` marks every newly opened connection as already checked: `# New connections are healthy.` / `self.health_check_done = True` (*read*, head lines 252 to 253).
- The change adds a connection pool, `OPTIONS["pool"]`, which requires `CONN_MAX_AGE = 0`. With it, each request obtains its connection from the pool through `connect()` (*read*, head lines 208 to 211 and 345 to 348).

## What the change does at this place

It adds this method to the PostgreSQL wrapper (*read*, `django/db/backends/postgresql/base.py` head lines 501 to 505); at base the wrapper inherits the method unchanged:

```python
    def close_if_health_check_failed(self):
        if self.pool:
            # The pool only returns healthy connections.
            return
        return super().close_if_health_check_failed()
```

It builds the pool with (*read*, head line 232):

```python
                check=ConnectionPool.check_connection if enable_checks else None,
```

where `enable_checks` is `CONN_HEALTH_CHECKS`. psycopg-pool's documentation at tag `pool-3.2.1` (2024-01-07) says of `check`: "By default no check is made on the connection." (*read*, `upstream/owner-psycopg-pool-3.2.1--docs_api_pool.rst`).

## Runs

The server is a stub that speaks PostgreSQL's wire protocol (`probes/N1/stub_server.py`), not PostgreSQL. "Restart" ends every open session with PostgreSQL's shutdown message; "drop" closes the sockets. Python 3.13.5, psycopg 3.1.18, psycopg-pool 3.2.1. All *run*.

| Pool, `CONN_HEALTH_CHECKS` | Head | Head with lines 501 to 505 removed |
| --- | --- | --- |
| `True`, off | 4 of the 6 requests after a restart or a drop fail with `OperationalError`, then requests succeed | the same |
| `True`, on | all 6 requests succeed | the same |

Outputs: `probes/N1/result-head.txt`, `probes/N1/grouping/results-no-override.txt`, patch `probes/N1/grouping/no-override.patch`. Base has no pool option; every pooled request there fails with `invalid connection option "pool"` (*run*, `probes/N1/result-base.txt`).

## Statements on what checking the pool is meant to do

- The change's test `test_pooling_health_checks` (*read*, `tests/backends/postgresql/tests.py` head lines 312 to 326) sets `CONN_HEALTH_CHECKS` to `False` and asserts `self.assertIsNone(new_connection.pool._check)`, then to `True` and asserts `self.assertIsNotNone(new_connection.pool._check)`.
- With the pool on, the change skips two tests of Django's health checks that run with `CONN_HEALTH_CHECKS` on, giving the reason "Pool does implicit health checks" (*read*, `django/db/backends/postgresql/features.py` head line 100).
- Florian Apolloner, a co-author, on the earlier pull request #17594 on 2023-12-30, struck through: "~~One thing left on the todo: Incorporate health checking directly into the pool (supported as of 3.2.0)~~" (*read*, https://github.com/django/django/pull/17594#issuecomment-1872509486).
- The documentation the change adds for the pool says nothing about health checks (*read*, `docs/ref/databases.txt` head lines 248 to 271). The `CONN_HEALTH_CHECKS` entry speaks of persistent connections (*read*, `docs/ref/settings.txt` head lines 613 to 619).
- The linked ticket 33497 says nothing about health checks (*read*, `upstream/maint-trac-33497.html`).
- The method is called by Django itself from `_cursor()` and `set_autocommit()` (*read*, `django/db/backends/base/base.py` head lines 295 and 472). It is not mentioned in Django's documentation (*read*, `upstream/project-docs-search.txt`).

## What a person sees

At run time, nothing that depends on this method: requests behave the same with and without it (*run*). A person who reads the PostgreSQL backend's source reads "The pool only returns healthy connections." With `CONN_HEALTH_CHECKS` off, the pool hands out sessions the server has ended (*run*).

## Known problems of this pull request near this place

- **GT-v12** (psycopg-pool 3.2 needed and not stated) and **GT-v7** (a pool dictionary holding `check`, `configure`, `open` or `kwargs` fails with `TypeError`) concern the constructor call at head lines 228 to 234, not lines 501 to 505. Neither patch for them touches this method (*read*, `probes/N1/grouping/fix-v12.patch`, `fix-v7.patch`).
- No known problem concerns lines 501 to 505.

## Impact facts

- **Who:** people who read or change Django's PostgreSQL backend (*read*).
- **How:** the sentence describes the pool as always returning healthy connections; with the default `CONN_HEALTH_CHECKS = False` it does not (*run*).
- **How stuck:** not stuck; the method's return has no effect on requests in either setting (*run*).
- **How common:** not measured.

## Known only later

- Django 5.1 (2024-08-07) shipped the method and comment unchanged; Django's `main` on 2026-11-06 has them at lines 509 to 513 (*read*, `upstream/later-5.1-postgresql-base.py`, `upstream/later-main-postgresql-base.py`).
- Commit 516d06e4 of 2026-09-15 (ticket 37283) added to the pool documentation: "Unless a ``check`` callback is provided in the pool options, :setting:`CONN_HEALTH_CHECKS` determines whether connections are checked when retrieved from the pool." (*read*, `upstream/later-commit-516d06e48f.json`).
