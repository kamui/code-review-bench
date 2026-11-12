# Card: a code comment in Django's PostgreSQL backend says the pool only returns healthy connections

Pull request: django/django #17914, which adds a connection pool to Django's PostgreSQL backend (`OPTIONS["pool"]`). "Base" is the commit before the change and "head" the reviewed commit.

## The place

The change adds a method that skips Django's own per-request health check when the pool is on, with the comment "The pool only returns healthy connections." (*read*, `django/db/backends/postgresql/base.py` head lines 501 to 505). The pool tests a session before handing it out only when `CONN_HEALTH_CHECKS` is `True` (*read*, head line 232); its default is `False`. The change's own test asserts that the pool has no check with the default (*read*).

## The runs

Against a stub that speaks PostgreSQL's wire protocol, after every session is ended (*run*, `probes/N1/`):

| Pool, `CONN_HEALTH_CHECKS` | Head | Head without the method |
| --- | --- | --- |
| on, off | 4 of 6 requests fail with `OperationalError`, then requests succeed | the same |
| on, on | all succeed | the same |

## What the affected person sees

Nothing at run time: the method makes no difference to any request (*run*). A reader of the backend's source is told the pool only returns healthy connections; with the default setting it hands out sessions the server has ended (*run*).

## Impact facts

- **Who is affected:** people who read or change Django's PostgreSQL backend source.
- **How:** a sentence that is true with `CONN_HEALTH_CHECKS = True` and untrue with the default.
- **How stuck:** not stuck; no behaviour depends on the method (*run*).
- **How common:** not measured.
