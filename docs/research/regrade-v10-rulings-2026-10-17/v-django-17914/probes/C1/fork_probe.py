"""C1 fork probe: what a process forked after the parent used a pooled alias inherits.

usage: python fork_probe.py SOCKET_DIR PORT POOL(0|1)

The parent runs one query on the alias, then calls connections.close_all() (Django's documented way to drop
connections), records the pool's idle connections (server process id and socket file number), and forks three
children. Each child records what it inherited, then runs SELECT pg_backend_pid(), <its own number> thirty times and
checks that every answer carries its own number. A child still waiting after 20 seconds is stopped and says so.
"""
import os
import sys
import threading
import time

import django
from django.conf import settings

sock, port, use_pool = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
options = {"pool": {"min_size": 4, "max_size": 4, "timeout": 5}} if use_pool else {}
settings.configure(USE_TZ=True, DATABASES={"default": {
    "ENGINE": "django.db.backends.postgresql", "NAME": "probe", "USER": "probe",
    "HOST": sock, "PORT": port, "OPTIONS": options}})
django.setup()
from django.db import connections  # noqa: E402
from django.db.backends.postgresql.base import DatabaseWrapper  # noqa: E402

print("django", django.get_version(), "pool option:", use_pool, flush=True)
print("DatabaseWrapper has class attribute _connection_pools:", "_connection_pools" in vars(DatabaseWrapper), flush=True)
with connections["default"].cursor() as c:
    c.execute("SELECT pg_backend_pid()")
    print(f"parent pid={os.getpid()} query used server process {c.fetchone()[0]}", flush=True)
connections.close_all()
pools = getattr(DatabaseWrapper, "_connection_pools", {})
pool = pools.get("default")
time.sleep(1)


def describe(p):
    idle = [(conn.info.backend_pid, conn.pgconn.socket) for conn in list(p._pool)]
    alive = sum(t.is_alive() for t in p._workers)
    return f"pool id={id(p)} name={p.name} idle (server pid, socket fd)={idle} worker threads alive={alive}/{len(p._workers)}"


if pool is not None:
    print(f"parent after close_all(): {describe(pool)}", flush=True)
else:
    print("parent after close_all(): no pool registered", flush=True)
parent_idle = {conn.info.backend_pid for conn in list(pool._pool)} if pool is not None else set()

import signal
import tempfile

outdir = tempfile.mkdtemp(prefix="c1fork-")
children = []
for n in range(1, 4):
    pid = os.fork()
    if pid == 0:
        path = os.path.join(outdir, f"child{n}.txt")

        def emit(text):
            with open(path, "a") as f:
                f.write(text + "\n")

        def stuck(signum, frame):
            emit(f"child {n}: still waiting after 20 seconds, at query {state['i'] + 1}; stopped")
            os._exit(1)

        state = {"i": 0}
        signal.signal(signal.SIGALRM, stuck)
        signal.alarm(20)
        p = getattr(DatabaseWrapper, "_connection_pools", {}).get("default")
        emit(f"child {n} pid={os.getpid()} inherited: " + (describe(p) if p is not None else "no pool"))
        wrong, used = 0, set()
        for i in range(30):
            state["i"] = i
            try:
                with connections["default"].cursor() as c:
                    c.execute("SELECT pg_backend_pid(), %s", [n])
                    backend, echoed = c.fetchone()
                used.add(backend)
                if echoed != n:
                    wrong += 1
                    emit(f"child {n} query {i + 1}: asked for {n}, received {echoed} (server process {backend})")
            except Exception as exc:  # noqa: BLE001
                emit(f"child {n} query {i + 1}: {type(exc).__module__}.{type(exc).__name__}: {str(exc)[:160]}")
            finally:
                try:
                    connections["default"].close()
                except Exception as exc:  # noqa: BLE001
                    emit(f"child {n} close: {type(exc).__name__}: {str(exc)[:120]}")
            time.sleep(0.01)
        shared = sorted(used & parent_idle)
        emit(f"child {n}: server processes used={sorted(used)}, of them the parent's idle sessions={shared}; answers carrying another number={wrong}/30")
        os._exit(0)
    children.append(pid)
for pid in children:
    os.waitpid(pid, 0)
for n in range(1, 4):
    path = os.path.join(outdir, f"child{n}.txt")
    print(open(path).read() if os.path.exists(path) else f"child {n}: wrote nothing", end="", flush=True)
if pool is not None:
    pool.close(timeout=1)
print("done", flush=True)
