"""D1: a process uses the database, then forks worker processes (a pre-forking
server with application preloading, a task queue, multiprocessing).

Modes
  closed       no pool option. The parent runs a query, calls connections.close_all()
               (the documented step before forking), then forks.
  left-open    no pool option. The parent runs a query and forks WITHOUT closing.
  pool         OPTIONS["pool"] set. The parent runs a query, calls
               connections.close_all(), then forks.

Each of 3 children runs 150 small requests: open a cursor, SELECT a token unique
to that process and request, compare the answer with what was sent, close.

Usage: PYTHONPATH=<django checkout> python fork_probe.py <mode> <sock dir> <port>
"""
import collections
import os
import signal
import sys
import time

import django
import psycopg
from django.conf import settings

mode, host, port = sys.argv[1], sys.argv[2], sys.argv[3]
options = {"pool": {"timeout": 5}} if mode == "pool" else {}
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "OPTIONS": {**options, "application_name": "d1forkprobe"},
        }
    },
    INSTALLED_APPS=[],
)
django.setup()
from django.db import connection, connections

print(f"django {django.get_version()} mode={mode}", flush=True)


def server_sessions():
    with psycopg.connect(host=host, port=port, user="postgres", dbname="postgres") as c:
        return sorted(
            r[0]
            for r in c.execute(
                "SELECT pid FROM pg_stat_activity WHERE application_name = 'd1forkprobe'"
            )
        )


try:
    with connection.cursor() as cursor:
        cursor.execute("SELECT pg_backend_pid()")
        print(f"parent pid={os.getpid()} ran a query on server session {cursor.fetchone()[0]}")
except Exception as e:
    print(f"parent: {type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}")
    sys.exit(0)
if mode != "left-open":
    connections.close_all()
    print("parent called connections.close_all()")
time.sleep(1.5)
parent_sessions = server_sessions()
print(f"server sessions still open and owned by the parent at fork time: {parent_sessions}", flush=True)


class Hung(Exception):
    pass


def child(number):
    def on_alarm(signum, frame):
        raise Hung()

    signal.signal(signal.SIGALRM, on_alarm)
    signal.alarm(45)
    used, wrong, errors, ok = set(), 0, collections.Counter(), 0
    hung = False
    try:
        for i in range(150):
            token = f"child{number}-request{i}"
            try:
                with connection.cursor() as cursor:
                    cursor.execute("SELECT %s, pg_backend_pid()", [token])
                    got, backend = cursor.fetchone()
                used.add(backend)
                if got == token:
                    ok += 1
                else:
                    wrong += 1
                    if wrong <= 2:
                        print(f"  child {number}: asked for {token!r}, received {got!r}", flush=True)
            except Hung:
                raise
            except Exception as e:
                errors[f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0][:110]}"] += 1
            finally:
                try:
                    connection.close()
                except Hung:
                    raise
                except Exception as e:
                    errors[f"on close: {type(e).__name__}: {str(e).splitlines()[0][:90]}"] += 1
    except Hung:
        hung = True
    inherited = sorted(used & set(parent_sessions))
    print(
        f"child {number} pid={os.getpid()}: correct answers={ok} WRONG answers={wrong} "
        f"errors={sum(errors.values())} stuck until 45s alarm={hung}\n"
        f"  distinct server sessions used: {len(used)}; of these inherited from the parent: {inherited}",
        flush=True,
    )
    for text, count in errors.most_common(6):
        print(f"  {count}x {text}", flush=True)
    os._exit(0)


pids = []
for number in (1, 2, 3):
    pid = os.fork()
    if pid == 0:
        child(number)
    pids.append(pid)
for pid in pids:
    os.waitpid(pid, 0)
print("all children finished")
