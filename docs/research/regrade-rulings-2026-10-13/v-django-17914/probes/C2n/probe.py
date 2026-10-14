"""C2n: can a wrapper that already holds a connection reach the pool's setup callback,
and what happens then?

Usage: PYTHONPATH=<django checkout> python probe.py <case> <socket dir> <port> [seconds]

Cases, each run in its own process. The pool timeout is 6 seconds.
  natural         OPTIONS = {"pool": {...}, "assume_role": "rr_role"} from the start, as a
                  settings file gives it. The main thread queries again and again until
                  <seconds> have passed (default 40); after a query that succeeds it
                  closes its connection and waits two seconds. Every call of the setup callback
                  is recorded: its thread, whether the wrapper it belongs to held a
                  connection when it was called, and how it ended.
  two-threads     the same settings; two more threads query through their own wrappers
                  while the main thread does. Recorded the same way.
  settings-change OPTIONS = {"pool": {"min_size": 1, "max_size": 3, ...}} with no role.
                  The main thread connects and keeps its connection. The probe then sets
                  OPTIONS["assume_role"] = "rr_role" on the live settings, which nothing
                  documents. Two more threads then query and hold their connections, so
                  the pool has to open another connection and run the setup callback.
  direct-call     no pool; OPTIONS = {"assume_role": "rr_role"}. The main thread connects.
                  A second thread then opens a raw driver connection and calls the private
                  connection._configure_connection(raw) on the main thread's wrapper, the
                  call the pool makes from its worker threads. At the commit before the
                  change, which has no such method, the second thread calls the wrapper's
                  ensure_role() instead.

The recording wraps DatabaseWrapper._configure_connection before any pool exists. It
calls the original and changes nothing else.
"""
import logging
import sys
import threading
import time

import django
from django.conf import settings

case, host, port = sys.argv[1:4]
seconds = float(sys.argv[4]) if len(sys.argv) > 4 else 40
options = {
    "natural": {"pool": {"timeout": 6}, "assume_role": "rr_role"},
    "two-threads": {"pool": {"timeout": 6}, "assume_role": "rr_role"},
    "settings-change": {"pool": {"min_size": 1, "max_size": 3, "timeout": 6}},
    "direct-call": {"assume_role": "rr_role"},
}[case]
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "rr_db",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "OPTIONS": options,
        }
    },
    INSTALLED_APPS=[],
    USE_TZ=True,
)
django.setup()
from django.db import connection, connections  # noqa: E402
from django.db.backends.postgresql.base import DatabaseWrapper  # noqa: E402

calls = []
# The commit before the change has no _configure_connection: its role setter is the
# wrapper method ensure_role(), which init_connection_state calls on the caller's thread.
HAS_CALLBACK = hasattr(DatabaseWrapper, "_configure_connection")
original = DatabaseWrapper._configure_connection if HAS_CALLBACK else None


def recorded(self, raw):
    entry = {
        "thread": threading.current_thread().name,
        "wrapper_made_in_this_thread": self._thread_ident == threading.get_ident(),
        "wrapper_held_a_connection": self.connection is not None,
        "role": self.settings_dict["OPTIONS"].get("assume_role"),
    }
    calls.append(entry)
    try:
        result = original(self, raw)
        entry["ended"] = "returned"
        return result
    except Exception as e:
        entry["ended"] = f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0][:150]}"
        raise


if HAS_CALLBACK:
    DatabaseWrapper._configure_connection = recorded
else:
    print("this commit has no DatabaseWrapper._configure_connection; nothing is recorded")


class Collect(logging.Handler):
    def __init__(self):
        super().__init__(logging.WARNING)
        self.messages = []

    def emit(self, record):
        self.messages.append(record.getMessage().splitlines()[0])


collected = Collect()
logging.getLogger("psycopg.pool").addHandler(collected)
logging.getLogger("psycopg.pool").propagate = False


def query(label, conn=None):
    conn = conn or connection
    started = time.monotonic()
    try:
        with conn.cursor() as cursor:
            cursor.execute("SELECT current_user")
            outcome = f"OK current_user is {cursor.fetchone()[0]!r}"
    except Exception as e:
        outcome = f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0][:150]}"
    print(f"  {label}: after {time.monotonic() - started:.1f}s {outcome}", flush=True)


def report():
    print(f"  the setup callback was called {len(calls)} times:")
    groups = {}
    for c in calls:
        key = (
            c["thread"].rsplit("-", 1)[0] if c["thread"].startswith("pool-") else c["thread"],
            c["wrapper_made_in_this_thread"],
            c["wrapper_held_a_connection"],
            c["role"],
            c.get("ended", "still running when the probe ended"),
        )
        groups[key] = groups.get(key, 0) + 1
    for (thread, own, held, role, ended), n in groups.items():
        print(
            f"    {n} x on thread {thread}; wrapper made in that thread: {own}; "
            f"wrapper held a connection: {held}; role: {role!r}; ended: {ended}"
        )
    distinct = {}
    for m in collected.messages:
        distinct[m[:170]] = distinct.get(m[:170], 0) + 1
    print(f"  the pool library logged {len(collected.messages)} warnings:")
    for text, n in distinct.items():
        print(f"    {n} x {text}")


print(f"{case}: OPTIONS at the start = {options}")
if case == "natural":
    end = time.monotonic() + seconds
    attempt = 0
    while time.monotonic() < end:
        attempt += 1
        began = time.monotonic()
        query(f"main thread, query {attempt}")
        if time.monotonic() - began < 1:
            connection.close()
            time.sleep(2)
    report()
elif case == "two-threads":
    def worker(n):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            began = time.monotonic()
            query(f"thread app-{n}", connections["default"])
            if time.monotonic() - began < 1:
                connections["default"].close()
                time.sleep(2)

    threads = [threading.Thread(target=worker, args=(n,), name=f"app-{n}") for n in (1, 2)]
    for t in threads:
        t.start()
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        began = time.monotonic()
        query("main thread")
        if time.monotonic() - began < 1:
            connection.close()
            time.sleep(2)
    for t in threads:
        t.join()
    report()
elif case == "settings-change":
    main_wrapper = connections["default"]
    query("main thread, before the change")
    print(f"  main thread keeps its connection: {main_wrapper.connection is not None}")
    time.sleep(1)
    before = len(calls)
    print(f"  setup callback calls so far, all with no role: {before}; connections the pool holds: {main_wrapper.pool.get_stats().get('pool_size')}")
    main_wrapper.settings_dict["OPTIONS"]["assume_role"] = "rr_role"
    print('  OPTIONS["assume_role"] set to "rr_role" on the live settings')
    release = threading.Event()

    def hold(n):
        query(f"thread app-{n}", connections["default"])
        release.wait()
        connections["default"].close()

    threads = [threading.Thread(target=hold, args=(n,), name=f"app-{n}") for n in (1, 2)]
    for t in threads:
        t.start()
        time.sleep(0.5)
    time.sleep(7)
    release.set()
    for t in threads:
        t.join()
    query("main thread, after the change, on the connection it already holds")
    report()
elif case == "direct-call":
    main_wrapper = connections["default"]
    query("main thread")
    print(f"  main thread keeps its connection: {main_wrapper.connection is not None}")

    def call():
        from django.db.backends.postgresql.base import Database

        raw = Database.connect(host=host, port=port, user="postgres", dbname="rr_db", autocommit=True)
        if not HAS_CALLBACK:
            try:
                main_wrapper.ensure_role()
                print("  second thread: ensure_role() on the main thread's wrapper returned")
            except Exception as e:
                print(f"  second thread: ensure_role() on the main thread's wrapper raised "
                      f"{type(e).__module__}.{type(e).__name__}: {str(e)[:260]}")
            raw.close()
            return
        try:
            main_wrapper._configure_connection(raw)
            print(f"  second thread: _configure_connection(raw) on the main thread's wrapper returned; "
                  f"current_user on raw is {raw.execute('SELECT current_user').fetchone()[0]!r}")
        except Exception as e:
            print(f"  second thread: _configure_connection(raw) on the main thread's wrapper raised "
                  f"{type(e).__module__}.{type(e).__name__}: {str(e)[:260]}")
        finally:
            raw.close()

    t = threading.Thread(target=call, name="app-1")
    t.start()
    t.join()
    report()
