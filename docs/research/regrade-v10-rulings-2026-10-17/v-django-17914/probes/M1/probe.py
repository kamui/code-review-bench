"""M1 probe: the pool's setup callback and the wrapper that created the pool.

usage: python probe.py CASE SOCKET_DIR PORT

Cases
  static            pool and assume_role in the settings from the start; the main thread queries three times
  static-lifetime   the same with max_lifetime 3 s and min_size 2, running 20 s
  static-threads    the same as static, from three threads at once
  runtime-change    pool on, no role; the main thread connects and keeps its connection; the probe then puts
                    assume_role into the live OPTIONS; a second thread queries
  runtime-lifetime  as runtime-change, with min_size 2 and max_lifetime 3 s: the main thread keeps its connection; a
                    second thread borrows and returns a connection once a second for 10 s, so the pool replaces
                    connections that reach their lifetime
  direct-call       no pool; the main thread connects; a second thread calls the private callback (at the base,
                    ensure_role()) on the main thread's wrapper
  role-only         assume_role in the settings, no pool
  gtv11             a DatabaseWrapper subclass whose ensure_role() runs SET ROLE rr_role; no pool, no assume_role
Every call of the setup callback is recorded: thread, whether the wrapper it belongs to held a connection, outcome.
"""
import logging
import sys
import threading
import time

import django
from django.conf import settings

case, sock, port = sys.argv[1], sys.argv[2], sys.argv[3]
T0 = time.monotonic()


def say(*args):
    print(f"[{time.monotonic() - T0:6.2f}s {threading.current_thread().name}]", *args, flush=True)


class Handler(logging.Handler):
    def emit(self, record):
        msg = record.getMessage()
        say(f"LOG {record.name} {record.levelname}: {msg[:300]}")


logging.getLogger("psycopg.pool").addHandler(Handler())
logging.getLogger("psycopg.pool").setLevel(logging.WARNING)

options = {}
if case.startswith("static"):
    options = {"pool": {"min_size": 1, "max_size": 3, "timeout": 5}, "assume_role": "rr_role"}
    if case == "static-lifetime":
        options["pool"] = {"min_size": 2, "max_size": 3, "timeout": 5, "max_lifetime": 3}
elif case == "runtime-change":
    options = {"pool": {"min_size": 1, "max_size": 3, "timeout": 5}}
elif case == "runtime-lifetime":
    options = {"pool": {"min_size": 2, "max_size": 3, "timeout": 5, "max_lifetime": 3}}
elif case == "role-only":
    options = {"assume_role": "rr_role"}

settings.configure(
    USE_TZ=True,
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "probe",
            "USER": "probe",
            "HOST": sock,
            "PORT": port,
            "OPTIONS": options,
        }
    },
)
django.setup()

from django.db import connection, connections  # noqa: E402
from django.db.backends.postgresql import base as pg_base  # noqa: E402

say("django", django.get_version(), "has _configure_connection:", hasattr(pg_base.DatabaseWrapper, "_configure_connection"))

calls = []
original = getattr(pg_base.DatabaseWrapper, "_configure_connection", None)
if original is not None:

    def recording(self, raw):
        held = self.connection is not None
        entry = {"thread": threading.current_thread().name, "wrapper_held_connection": held}
        calls.append(entry)
        try:
            result = original(self, raw)
        except BaseException as exc:
            entry["outcome"] = f"{type(exc).__module__}.{type(exc).__name__}: {str(exc)[:160]}"
            say("callback raised", entry)
            raise
        entry["outcome"] = "returned"
        say("callback returned", entry)
        return result

    pg_base.DatabaseWrapper._configure_connection = recording


def query(label):
    try:
        with connections["default"].cursor() as cursor:
            cursor.execute("SELECT current_user, pg_backend_pid()")
            say(label, "->", cursor.fetchone())
    except Exception as exc:
        say(label, "->", f"{type(exc).__module__}.{type(exc).__name__}: {str(exc)[:200]}")


def summary():
    held = sum(1 for c in calls if c["wrapper_held_connection"])
    thread_check = sum(1 for c in calls if "same thread" in c.get("outcome", ""))
    say(f"SUMMARY callback calls={len(calls)} with_wrapper_holding_connection={held} thread_check_errors={thread_check}")
    for c in calls:
        say("  call", c)


if case in ("static", "static-lifetime"):
    end = T0 + (20 if case == "static-lifetime" else 0)
    for i in range(3):
        query(f"query {i + 1}")
        connections["default"].close()
    while time.monotonic() < end:
        time.sleep(1)
    summary()
elif case == "static-threads":
    def worker(n):
        query(f"thread query {n}")
        connections["default"].close()
    threads = [threading.Thread(target=worker, args=(n,), name=f"app-{n}") for n in range(3)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    summary()
elif case in ("runtime-change", "runtime-lifetime"):
    query("main, before the role is set")
    say("main keeps its connection; the probe now sets OPTIONS['assume_role'] = 'rr_role' in the running process")
    connection.settings_dict["OPTIONS"]["assume_role"] = "rr_role"
    if case == "runtime-change":
        def second():
            query("second thread")
            time.sleep(0.5)
        t = threading.Thread(target=second, name="app-2")
        t.start()
        def third():
            time.sleep(0.2)
            query("third thread")
        t3 = threading.Thread(target=third, name="app-3")
        t3.start()
        t.join(); t3.join()
    else:
        def borrower():
            end = time.monotonic() + 10
            n = 0
            while time.monotonic() < end:
                n += 1
                query(f"second thread, borrow {n}")
                connections["default"].close()
                time.sleep(1)
        t = threading.Thread(target=borrower, name="app-2")
        t.start(); t.join()
    query("main, after")
    summary()
elif case == "direct-call":
    query("main")
    main_wrapper = connections["default"]
    raw = main_wrapper.connection
    def other():
        try:
            if hasattr(connection, "_configure_connection"):
                main_wrapper.settings_dict["OPTIONS"]["assume_role"] = "rr_role"
                main_wrapper._configure_connection(raw)
                say("other thread: _configure_connection returned")
            else:
                main_wrapper.settings_dict["OPTIONS"]["assume_role"] = "rr_role"
                main_wrapper.ensure_role()
                say("other thread: ensure_role returned")
        except Exception as exc:
            say("other thread ->", f"{type(exc).__module__}.{type(exc).__name__}: {str(exc)[:200]}")
    t = threading.Thread(target=other, name="other")
    t.start(); t.join()
    summary()
elif case == "role-only":
    query("main")
    summary()
elif case == "gtv11":
    hook_calls = []

    class RoleOverride(pg_base.DatabaseWrapper):
        def ensure_role(self):
            hook_calls.append("override")
            with self.connection.cursor() as cursor:
                cursor.execute("SET ROLE rr_role")
            return True

    wrapper = RoleOverride(connections["default"].settings_dict.copy(), alias="override")
    wrapper.ensure_connection()
    with wrapper.cursor() as cursor:
        cursor.execute("SELECT current_user")
        say("subclass with ensure_role override: current_user ->", cursor.fetchone(), "override calls:", len(hook_calls))
    wrapper.close()
else:
    raise SystemExit(f"unknown case {case}")

pool = getattr(pg_base.DatabaseWrapper, "_connection_pools", {}).get("default")
if pool is not None:
    say("pool stats:", {k: v for k, v in pool.get_stats().items() if k in ("pool_min", "pool_max", "pool_size", "pool_available", "requests_waiting", "connections_num", "connections_errors", "requests_errors")})
    try:
        connections["default"].close()
    except Exception as exc:
        say("close ->", type(exc).__name__)
    pool.close(timeout=1)
say("done")
