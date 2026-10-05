# D1b: a pool that is open when a process forks is shared by the child processes

Split from group D1. This dossier covers what a fork does to an open pool in any program. D1a covers the wrong database in parallel test workers.

Candidates covered: NC-4e87aff777de, NC-f034b4e5ce67, NC-76e61ab0f564. The last two also describe D1a.

## Problem

Many deployments start one process, load the application, and then fork worker processes. Task queues and `multiprocessing` do the same. If the parent process has used the database before it forks, the pool it built stays in memory and every child inherits it.

Each child then believes it owns the same open server connections. Children send queries down the same sockets and read each other's answers. The pool's own helper threads do not exist in a child, so when a child needs a new connection it waits until the pool timeout.

Before the change there was a simple way to fork safely: close the connections first. With the pool that no longer works, because closing only returns the connection to the pool.

## What changed

The pool registry lives on the class, so a forked child gets a copy that points at the same open sockets:

```python
    _connection_pools = {}
    ...
            pool = ConnectionPool(
                kwargs=connect_kwargs,
                open=False,  # Do not open the pool during startup.
                configure=self._configure_connection,
                ...
            )
            self._connection_pools.setdefault(self.alias, pool)
```

The first query opens the pool, and `close()` keeps the session open inside it:

```python
        if self.pool:
            # If nothing else has opened the pool, open it now.
            self.pool.open()
            connection = self.pool.getconn()
...
                    self.connection._pool.putconn(self.connection)
                    # Connection can no longer be used.
                    self.connection = None
```

Nothing in the change resets the registry in a child process. The installed `psycopg_pool` has no fork handling either; I searched its source for `fork` and `getpid` and found nothing.

## Intended or announced

The pull request has no description. The documentation and the 5.1 release note do not mention forking.

The authors knew. In the earlier version of this pull request, django/django#17594, a co-author asked what happens after a fork, and the answer was that Django does not try to be fork safe. Quotes are under "What the maintainers did". They chose to open the pool only on first use, so a parent that never queries is safe. They added no warning to the documentation.

The change shipped in Django 5.1.

## What the affected person sees

Who: anyone who enables the pool in a program that queries the database and then forks. Examples from upstream reports are gunicorn with worker processes, and the Huey task queue started through a management command. `manage.py test --parallel` on Linux is another, when something queries in the parent after cloning (see D1a).

What they see at head, from `probes/D1/result-head.txt`, mode `pool`. The parent ran one query, called `connections.close_all()`, then forked three children that each ran 150 small queries:

```
server sessions still open and owned by the parent at fork time: [2038378, 2038379, 2038380, 2038382]
  child 2: asked for 'child2-request6', received 'child3-request2'
  child 3: asked for 'child3-request2', received 'child2-request6'
child 1 pid=2039202: correct answers=120 WRONG answers=30 errors=0 stuck until 45s alarm=False
child 2 pid=2039204: correct answers=121 WRONG answers=29 errors=0 stuck until 45s alarm=False
child 3 pid=2039205: correct answers=44 WRONG answers=35 errors=0 stuck until 45s alarm=True
```

One process received the rows another process asked for, with no error. One child stopped making progress. In the parallel test runs of the same file, two workers reported the same server session and the command never printed its summary.

In a web application the wrong answers mean one request can receive another request's data.

What the same person saw before the change, and still sees without the pool (modes `closed` and `left-open`, same at base and head):

- parent closes its connections, then forks: every child gets its own sessions, 150 of 150 answers correct.
- parent forks with a connection left open: at most one error per child (`server closed the connection unexpectedly`), then normal work. No wrong answers in my runs.

How stuck: the symptoms are wrong data, hangs and protocol errors that do not name the pool. The known workaround is to clear `DatabaseWrapper._connection_pools` in a post-fork hook, which a user reported on the Django ticket. Calling `connections.close_all()` before forking does not help.

## What the maintainers did

Before the merge, in django/django#17594:

- Co-author bluetech: "Another regular safety hazard with mutable global resources aside from thread safety is "fork safety" ... AFAICT psycopg pool is not "fork safe" ... (With existing connections/persistent connections code this is OK because the connections are stored in a thread local which gets reset on fork.)" https://github.com/django/django/pull/17594#discussion_r1424970675
- Co-author apollo13: "I don't think anything in Django is written with fork safety in mind. While WSGI servers use fork it generally only works if the fork early enough I think (preferably before loading the WSGI app)" https://github.com/django/django/pull/17594#discussion_r1425038928
- sarahboyce, on queries during start-up: "Since Django 5.0, this raises a Runtimewarning. So not guaranteed not to happen but actively discouraged." https://github.com/django/django/pull/17594#discussion_r1425806546 bluetech: "This is good enough for me!" https://github.com/django/django/pull/17594#discussion_r1426557356

After the release:

- Ticket 36957, "Django psycopg connection pool + fork()", filed in 2026 as a bug against Django 6.0 with gunicorn, describes exactly this. Simon Charette: "I would say this is close if not a duplicate of #31637. The problem of forking after connection creation is not specific to psycopg connection pools." Mariusz Felisiak, the author of this pull request, closed it: "Agreed, duplicate of #31637." https://code.djangoproject.com/ticket/36957
- Ticket 31637, "Registering database connections for cleanup on fork", is an accepted new-feature ticket from 2020. It is still open. A user added in 2026: "We got hit by #36957 as well. We use Huey with multiprocessing workers as a task queue." https://code.djangoproject.com/ticket/31637
- django/django#20790 proposed a fix and was closed in favour of django/django#20803, which is open and unmerged.
- The pool code on `main` as of 2026-10-03 has no fork handling. The documentation now says "Django maintains a separate pool for each database alias in each process", which is not true for a pool opened before a fork.

So the maintainers accept that Django should handle forks and have not fixed it. They do not attribute it to this pull request. They call it a long-standing general limitation.

## How each fact is known

- run: head, pool, close then fork: children use only the parent's four sessions, get wrong answers, one child stalls (`probes/D1/result-head.txt`, part 1; an earlier unsaved run gave 19, 20 and 4 wrong answers).
- run: base and head without the pool, close then fork: all answers correct, no shared sessions.
- run: base and head without the pool, fork with an open connection: at most one error per child, no wrong answers.
- run: parallel test workers at head show `worker threads alive=0/3` and a shared server session (part 2, variants 4 and 6).
- run: base rejects the pool option.
- read: the diff lines above; the `psycopg_pool` 3.3.3 source; the upstream comments and tickets quoted; `main` fetched on 2026-10-05.
- reported: the gunicorn and Huey failures in tickets 36957 and 31637. I did not run gunicorn or Huey.
- not run: a real pre-forking web server. The probe uses `os.fork()` directly.

## Relation to existing reference bugs and ruled claims

None of GT-v1 to GT-v5 covers this. GT-v2 is about the pool keeping an old database name. This is about two processes using one socket, and it happens with the correct database name.

It overlaps with D1a only in the parallel test runner, where both show up in the same run.

## Both sides

For eligible:

- The change makes a known hazard worse, and the runs show how. Closing connections before a fork was enough at base. At head it is not. The failure changes from one visible error to silent wrong answers.
- The person hurt can receive another process's query results. That is a correctness and privacy problem, and it is hard to trace to its cause.
- A reviewer raised it during the real review, so it is the kind of thing a reviewer can be expected to raise.
- Real users reported it against released versions.

Against:

- The maintainers discussed it before merging and accepted it. They treat it as part of an older, general limitation tracked as a new-feature request, and said so when they closed the 2026 report.
- Django does not promise fork safety anywhere, and it discourages database queries during start-up with a warning.
- A parent that does not touch the database before forking is unaffected, and the pool is built to open late for that reason.

## Recommendation

`eligible`. The pool turns a recoverable fork hazard into silent sharing of connections between processes, the usual precaution stops working, and the documentation gives no warning.

Strongest argument against: the authors considered this exact scenario before merging and decided Django does not support forking after database use; the project later classified a user's bug report as a duplicate of a 2020 feature request and said the problem is not specific to pools. On that reading it is `pre-existing` behaviour that the pool exposes, not a defect of this change.
