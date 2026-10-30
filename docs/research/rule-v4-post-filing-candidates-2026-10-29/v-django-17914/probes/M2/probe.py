"""probes/M2: the pool's CONN_MAX_AGE check and a second wrapper for the same alias.

Usage: python -I probe.py <django source dir> <label>

Each case uses its own alias. The pool options are {"min_size": 1, "max_size": 1, "timeout": 2}
so that a slot held by one wrapper is visible to the next one.

  first-use      settings with the pool and CONN_MAX_AGE=60 from the start: the
                 first query on the alias
  copy-60        wrapper A (CONN_MAX_AGE=0) queries and closes, which builds the
                 alias's pool. B = A.copy() with B.settings_dict["CONN_MAX_AGE"]=60
                 queries; then the end-of-request cleanup that Django's request
                 signals run (close_if_unusable_or_obsolete) is called on B; then
                 wrapper C (CONN_MAX_AGE=0) queries from another thread.
  copy-0         the same with B at CONN_MAX_AGE=0 (control)
  shared-60      the alias "default" through django.db.connections: one thread
                 queries and closes; then the live settings dictionary is changed
                 to CONN_MAX_AGE=60; a new thread queries through
                 connections["default"] and runs the cleanup
  order          a fresh alias: B (60) queries first, then A (0), then B again
  no-pool-60     (both commits) a wrapper with CONN_MAX_AGE=60 and no pool option,
                 the persistent connection Django documents, and the cleanup
"""
import copy
import sys
import threading
import time

src, label = sys.argv[1], sys.argv[2]
sys.path.insert(0, src)

import django  # noqa: E402
from django.conf import settings  # noqa: E402

POOL = {"min_size": 1, "max_size": 1, "timeout": 2}
BASE = {
    "ENGINE": "django.db.backends.postgresql",
    "HOST": "127.0.0.1", "PORT": "55432", "USER": "postgres", "PASSWORD": "bench",
    "NAME": "postgres", "CONN_MAX_AGE": 0, "OPTIONS": {"pool": dict(POOL)},
}
settings.configure(DATABASES={"default": copy.deepcopy(BASE)}, USE_TZ=True)
django.setup()

from django.db import connections  # noqa: E402
from django.db.backends.postgresql import base as pg  # noqa: E402

HAS_POOL = hasattr(pg.DatabaseWrapper, "pool")


def sd(**over):
    d = copy.deepcopy(connections.settings["default"])
    d.update(over)
    return d


def err(e):
    return f"{type(e).__module__}.{type(e).__name__}: {str(e).strip().splitlines()[0]}"


def q(w):
    with w.cursor() as c:
        c.execute("SELECT pg_backend_pid()")
        return c.fetchone()[0]


def stats(alias):
    if not HAS_POOL or alias not in pg.DatabaseWrapper._connection_pools:
        return "no pool"
    s = pg.DatabaseWrapper._connection_pools[alias].get_stats()
    return f"pool_size={s.get('pool_size')} available={s.get('pool_available')}"


def in_thread(fn):
    out = {}

    def t():
        try:
            out["r"] = fn()
        except Exception as e:
            out["r"] = err(e)
    th = threading.Thread(target=t)
    th.start()
    th.join()
    return out["r"]


def step(name, fn):
    try:
        print(f"  {name}: {fn()}")
    except Exception as e:
        print(f"  {name}: {err(e)}")


def cleanup(alias):
    if HAS_POOL and alias in pg.DatabaseWrapper._connection_pools:
        pg.DatabaseWrapper._connection_pools.pop(alias).close()


print(f"label={label} django={django.get_version()} pool option={'yes' if HAS_POOL else 'no'}")

print("first-use")
w = pg.DatabaseWrapper(sd(CONN_MAX_AGE=60), "m2_first")
step("query", lambda: f"ok pid={q(w)}")
cleanup("m2_first")

for age in (60, 0):
    alias = f"m2_copy_{age}"
    print(f"copy-{age}")
    a = pg.DatabaseWrapper(sd(), alias)
    step("A query (CONN_MAX_AGE=0)", lambda: f"ok pid={q(a)}")
    step("A close", lambda: (a.close(), stats(alias))[1])
    b = a.copy()
    b.settings_dict["CONN_MAX_AGE"] = age
    step(f"B=A.copy() query (CONN_MAX_AGE={age})", lambda: f"ok pid={q(b)}")
    step("B end-of-request cleanup",
         lambda: (b.close_if_unusable_or_obsolete(),
                  f"B still holds a connection={b.connection is not None}; {stats(alias)}")[1])
    def c_query():
        c = pg.DatabaseWrapper(sd(), alias)  # created inside its own thread
        try:
            return f"ok pid={q(c)}"
        finally:
            c.close()

    t0 = time.monotonic()
    r = in_thread(c_query)
    print(f"  C query in another thread (CONN_MAX_AGE=0): {r} after {time.monotonic() - t0:.1f}s")
    try:
        b.close()
    except Exception:
        pass
    cleanup(alias)

print("shared-60")
step("thread 1 query and close via connections['default']",
     lambda: in_thread(lambda: (q(connections["default"]), connections["default"].close(),
                                stats("default"))[2]))
connections.settings["default"]["CONN_MAX_AGE"] = 60
print(f"  live settings changed: settings.DATABASES['default']['CONN_MAX_AGE']="
      f"{settings.DATABASES['default']['CONN_MAX_AGE']}")


def thread2():
    w = connections["default"]
    pid = q(w)
    w.close_if_unusable_or_obsolete()
    held = w.connection is not None
    return f"ok pid={pid}; after cleanup still holds a connection={held}; {stats('default')}"


step("thread 2 query via connections['default'] and cleanup", lambda: in_thread(thread2))
connections.settings["default"]["CONN_MAX_AGE"] = 0
cleanup("default")

print("order")
alias = "m2_order"
b = pg.DatabaseWrapper(sd(CONN_MAX_AGE=60), alias)
a = pg.DatabaseWrapper(sd(), alias)
step("B (60) first query", lambda: f"ok pid={q(b)}")
step("A (0) query, then close", lambda: (f"ok pid={q(a)}", a.close())[0])
step("B (60) query again", lambda: f"ok pid={q(b)}")
for x in (a, b):
    try:
        x.close()
    except Exception:
        pass
cleanup(alias)

print("no-pool-60")
n = pg.DatabaseWrapper(sd(CONN_MAX_AGE=60, OPTIONS={}), "m2_nopool")
step("query", lambda: f"ok pid={q(n)}")
step("end-of-request cleanup",
     lambda: (n.close_if_unusable_or_obsolete(), f"still holds a connection={n.connection is not None}")[1])
n.close()
