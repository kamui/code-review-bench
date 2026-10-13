# Card: the pool's setup callback fails Django's thread check when the connection object it belongs to already holds a connection

Pull request: django/django #17914. It lets Django take PostgreSQL connections from a pool (`OPTIONS["pool"]`). An older option, `OPTIONS["assume_role"]`, makes Django run `SET ROLE` on each new connection. Django's connection object, the "wrapper", exists once per thread and refuses to be used from another thread: the thread check. "Base" is the commit before the change and "head" the reviewed commit.

## The runs

psycopg 3.1.18, psycopg-pool 3.2.1, PostgreSQL 16.15, Python 3.12, pool timeout 6 seconds. Probes and results: `probes/C2n/`. All *run*.

At the head the pool prepares each new connection on one of its own threads by calling a method of one wrapper, the one whose thread first created the pool. When a role is set, that method builds its `SET ROLE` statement by opening a cursor on the wrapper (*read*). The probe records each such call and whether the wrapper held a connection.

| Case | Base | Head |
| --- | --- | --- |
| `pool` and `assume_role` both in the settings from the start; one thread queries for 40 seconds | the option does not exist: `invalid connection option "pool"` | every query fails with `OperationalError: couldn't get a connection after 6.00 sec`; 18 calls of the setup method, none with the wrapper holding a connection; no thread-check error |
| the same, three threads for 20 seconds | not run | the same; 12 calls |
| the pool with no role; the main thread connects and keeps its connection; the program then sets `OPTIONS["assume_role"]` in the running process; two more threads query | not run | the pool opens a new connection and calls the setup method 4 times; each call ends in `DatabaseError: DatabaseWrapper objects created in a thread can only be used in that same thread.`; the thread waiting for that connection gets `OperationalError: couldn't get a connection after 6.00 sec`; threads that already held a connection go on under the old role |
| no pool; a second thread calls the wrapper's role setup on the main thread's connected wrapper | the same `DatabaseError` | the same `DatabaseError` |
| `assume_role` in the settings, no pool | works | works |

At the head a wrapper with the pool on gets its connection only from the pool (*read*), so with both options set from the start the wrapper never holds one.

Each variant below is the head with one patch (`probes/C2n/patches/`). All *run*.

| Variant | Both options from the start | A role set after connecting | A direct call |
| --- | --- | --- | --- |
| head | every query times out | thread-check error; the waiting thread times out | thread-check error |
| `SET ROLE` built with the raw connection, not through the wrapper | works | works | returns |
| the setup method allowed to use the wrapper from the pool's thread | every query times out | works | returns |
| both | works | works | returns |

## What the affected person sees

With both options in the settings file: a timeout on every query, never the thread check.

A program that sets the role in a running process after its pool has connections: threads that need a new connection wait for the pool's timeout and get `couldn't get a connection after ... sec`; the pool library logs "error connecting in 'pool-1': DatabaseWrapper objects created in a thread can only be used in that same thread. ..." for each attempt; the role is never applied.

## Impact facts

- **Who is affected:** a program that changes `OPTIONS["assume_role"]` in a running process after its pool has opened connections, or that calls the private setup method on another thread's wrapper.
- **How:** connections opened after the change cannot be prepared, and threads that need one time out. No data is touched.
- **How stuck:** the role cannot be applied by this route. A role in the settings file with no pool works.
- **How common:** not measured. Django's documentation says "You shouldn't alter settings in your applications at runtime." Two maintainers wrote during review, before the reviewed commit, that changing settings is not supported.
- **Neighbours:** a project with both options in its settings file gets a timeout on every query, from the same call in the setup method.

## Limits of the evidence

- Run: every row above.
- Read: that a pooled wrapper gets its connection only from the pool.
- Not run: a pool connection replaced after its maximum lifetime or after a server restart; an asynchronous pool.
