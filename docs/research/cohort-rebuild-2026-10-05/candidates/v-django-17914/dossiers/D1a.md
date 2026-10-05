# D1a: parallel test workers keep the parent's pool and run on the un-cloned test database

Split from group D1. D1 held two problems. This dossier covers the wrong database in parallel test workers. D1b covers what a fork does to an open pool in any program.

Candidates covered: NC-76e61ab0f564, NC-f034b4e5ce67. NC-4e87aff777de names the parallel runner only as a place where the D1b problem can occur.

## Problem

`manage.py test --parallel` gives each worker process its own copy of the test database, named `test_<name>_1`, `test_<name>_2` and so on. Each worker switches its connection settings to its copy when it starts.

With the pool enabled, a worker can keep using a pool that the parent process built for the un-cloned test database `test_<name>`. The worker's settings say `test_<name>_1`, but its queries go to `test_<name>`. All workers then share one database, see each other's rows, and the run fails or hangs.

This happens only if the parent process holds a pool for that alias at the moment it forks the workers. The stock runner on a simple project does not. Something has to query the alias in the parent after the last clone is made and before the workers start.

## What changed

The change adds a pool registry that every connection wrapper of the class shares, keyed only by the alias name:

```python
    _connection_pools = {}

    @property
    def pool(self):
        ...
        if self.alias not in self._connection_pools:
            ...
            connect_kwargs = self.get_connection_params()
            ...
            self._connection_pools.setdefault(self.alias, pool)

        return self._connection_pools[self.alias]
```

Closing a connection now returns it to that pool and keeps the pool:

```python
                if self.pool:
                    ...
                    self.connection._pool.putconn(self.connection)
                    # Connection can no longer be used.
                    self.connection = None
```

The worker setup code is unchanged by the pull request (`django/db/backends/base/creation.py`):

```python
    def setup_worker_connection(self, _worker_id):
        settings_dict = self.get_test_db_clone_settings(str(_worker_id))
        ...
        self.connection.settings_dict.update(settings_dict)
        self.connection.close()
```

Before the change, `close()` ended the session, and the next query opened a new one with the new name. After the change, `close()` hands the connection back to a pool that was built with the old name, and the next query takes a connection from that same pool. The pull request added `close_pool()` calls to the clone step and the destroy step. It added none to the worker setup step.

## Intended or announced

The pull request has no description. Its documentation and release note say only that the `"pool"` option now exists. Nothing says that test workers may run on a shared database. The pull request added `close_pool()` to `_clone_test_db` with the comment "CREATE DATABASE ... WITH TEMPLATE ... requires closing connections to the template database", which shows the authors meant parallel runs to work with a pool.

The change shipped in Django 5.1.

## What the affected person sees

Who: a developer who enables the pool and runs `manage.py test --parallel` on Linux, where worker processes are forked, in a project where something queries the database in the parent process after cloning. Two ordinary project features do this in my runs:

- a system check registered with the `database` tag that runs a query. Django's test runner runs these checks after cloning and before starting workers.
- a second database alias together with a `post_migrate` handler that queries the default alias without honouring the `using` argument.

What they see at head, from `probes/D1/result-head.txt`, variants 4 and 6:

```
worker pid=2111426 TestA: settings NAME=test_d1app_1 current_database()=test_d1app ... pool dbname=test_d1app worker threads alive=0/3
worker pid=2111427 TestB: settings NAME=test_d1app_2 current_database()=test_d1app ... pool dbname=test_d1app worker threads alive=0/3
AssertionError: Lists differ: ['TestA', 'TestB'] != ['TestA']
```

Tests that pass alone fail because another worker's rows appear. In no run with this trigger did the command print its "Ran N tests" summary. It either crashed inside the parallel runner or was still running when my 90 second limit killed it. In the first such run I watched the command, and it was still running after two minutes. Nothing in the output says that the workers are on the wrong database.

Before the change the pool did not exist. The same project without the pool option passes at base and at head (variants 2 and 5), with each worker on its own copy.

They are stuck until they turn the pool off for tests, pass `--parallel 1`, or remove the query from the parent. The symptoms point at the tests, not at the pool.

The stock runner with one alias and no such check is fine at head (variant 3). So is a two-alias project without the handler (variant 7).

## What the maintainers did

- Nobody raised the worker setup step in the review of this pull request or of its earlier version, django/django#17594.
- In #17594 a contributor reported the clone step failing with a pool ("source database "test_django" is being accessed by other users"), and the authors fixed that by closing the pool in `_clone_test_db`. https://github.com/django/django/pull/17594#discussion_r1425171983
- `setup_worker_connection` and the PostgreSQL `creation.py` are unchanged on Django `main` as of 2026-10-03 (commit a461af8c). The problem is still there.
- Jake Howard, a Django team member, wrote on ticket 31637 in 2026 that fork problems come up more often "due to manage.py test --parallel, which uses fork (usually). Disabling pooling in tests is a semi-valid work-around". That comment is about shared sockets (D1b). It does not mention the wrong database. https://code.djangoproject.com/ticket/31637#comment:9
- I found no ticket or fix about workers using the un-cloned database. The Django ticket tracker refuses automated searches, so I could search only GitHub and read single tickets.

No maintainer has acknowledged this exact problem.

## How each fact is known

- run: at head with the pool, workers report their clone name in settings and `test_d1app` from `current_database()`; tests fail on rows from another worker; the command never prints its summary (`probes/D1/result-head.txt`, variants 4 and 6; earlier unsaved runs of variant 4 gave the same picture).
- run: at head with the pool and no parent query, workers use their clones (variants 3 and 7).
- run: at base and at head without the pool, workers always use their clones (variants 1, 2, 5).
- run: at base the `"pool"` option is rejected with `invalid connection option "pool"` (variants 3, 4, 6, 7 in `result-base.txt`).
- read: the diff lines and `setup_worker_connection` quoted above.
- read: the review comments and the ticket comment linked above; `main` fetched on 2026-10-05.
- not run: the "spawn" start method used on macOS and Windows. Spawned workers do not inherit the parent's memory, so I expect no problem there, but I did not run it.

## Relation to existing reference bugs and ruled claims

This is reference bug GT-v2 with a second trigger. GT-v2 says a cached pool keeps the old database after test setup switches `NAME`. Its ruled claim CL-v-test-pool-database gives the mechanism as "The cache key is only the alias; close returns a connection without discarding its original pool kwargs." That is the mechanism here. The difference is where the name switch happens. GT-v2 covers the switch in `create_test_db`. This covers the switch in `setup_worker_connection`, in a forked worker.

The damage differs in degree. In GT-v2 the tests can run on the original, non-test database. Here they run on the wrong test database, so no production data is at risk.

D1b is a different problem that shows up in the same runs. The workers also share the parent's sockets and have no pool threads.

## Both sides

For treating it as GT-v2: same lines, same mechanism, same broken promise that tests run on the database the settings select. A fix that drops the pool whenever the alias's settings change would cure both.

For treating it as new: a narrow fix of GT-v2 inside `create_test_db` would leave this one in place. The trigger is different and the stock runner does not hit it. The consequence is a failed or hung test run, not writes to a non-test database.

For doubting it matters: it needs the pool in test settings, forked workers, and a parent query between cloning and forking. The failure is loud even if its cause is hidden.

## Recommendation

`duplicate` of GT-v2. It is a further trigger of the same mechanism, reproduced at head and absent without the pool.

Strongest argument against: GT-v2 as written is about the original database being used after `create_test_db`. A fix scoped to that function would not touch worker setup, and a reader of GT-v2 would not learn that parallel workers can share one test database. On that view this is a separate eligible bug.
