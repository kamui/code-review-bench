"""probes/M2/grouping: every failing case of M2 and GT-v2's first case, on one source tree.

Usage: python -I cases.py <django source dir> <label>

  gt-v2-case-1   pool on, NAME=m2_app; a query builds the alias's pool; then
                 creation.create_test_db() switches NAME to the test database and
                 closes the connection; which database does the next query reach?
                 (At a commit without the pool option the same steps run without it.)
  m2-copy-60     A (CONN_MAX_AGE=0) builds the pool; B=A.copy() with
                 CONN_MAX_AGE=60 queries: is it refused?
  m2-shared-60   the live settings dictionary of the alias is changed to
                 CONN_MAX_AGE=60 after the pool exists; a new thread queries
"""
import copy
import sys
import threading

src, label = sys.argv[1], sys.argv[2]
sys.path.insert(0, src)

import django  # noqa: E402
from django.conf import settings  # noqa: E402

POOL = {"min_size": 1, "max_size": 2, "timeout": 2}
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "HOST": "127.0.0.1", "PORT": "55432", "USER": "postgres",
            "PASSWORD": "bench", "NAME": "m2_app", "CONN_MAX_AGE": 0,
            "OPTIONS": {"pool": dict(POOL)},
        },
    },
    USE_TZ=True,
)
django.setup()

from django.db import connection, connections  # noqa: E402
from django.db.backends.postgresql import base as pg  # noqa: E402

HAS_POOL = hasattr(pg.DatabaseWrapper, "pool")
if not HAS_POOL:
    connections.settings["default"]["OPTIONS"].pop("pool")


def err(e):
    return f"{type(e).__module__}.{type(e).__name__}: {str(e).strip().splitlines()[0]}"


def cur_db(w):
    with w.cursor() as c:
        c.execute("SELECT current_database()")
        return c.fetchone()[0]


def drop_pools():
    if HAS_POOL:
        for alias in list(pg.DatabaseWrapper._connection_pools):
            pg.DatabaseWrapper._connection_pools.pop(alias).close()


print(f"label={label} pool option={'yes' if HAS_POOL else 'no (run without the pool)'}")

# GT-v2 case 1
print("gt-v2-case-1")
before = cur_db(connection)
connection.close()
old_name = connection.settings_dict["NAME"]
connection.creation.create_test_db(verbosity=0, autoclobber=True, serialize=False)
try:
    after = cur_db(connection)
    print(f"  before setup: {before}; NAME now: {connection.settings_dict['NAME']}; "
          f"next query reached: {after}; "
          f"{'reaches the selected database' if after == connection.settings_dict['NAME'] else 'reaches the earlier database'}")
finally:
    connection.close()
    connection.creation.destroy_test_db(old_name, verbosity=0)
    drop_pools()

if not HAS_POOL:
    print("m2-copy-60: not run (no pool option at this commit)")
    print("m2-shared-60: not run (no pool option at this commit)")
    sys.exit(0)

# M2 copy-60
print("m2-copy-60")
a = pg.DatabaseWrapper(copy.deepcopy(connections.settings["default"]), "m2_g_copy")
cur_db(a)
a.close()
b = a.copy()
b.settings_dict["CONN_MAX_AGE"] = 60
try:
    cur_db(b)
    print("  B with CONN_MAX_AGE=60 after the pool exists: accepted, no error")
except Exception as e:
    print(f"  B with CONN_MAX_AGE=60 after the pool exists: {err(e)}")
finally:
    try:
        b.close()
    except Exception:
        pass
    drop_pools()

# M2 shared-60
print("m2-shared-60")
out = {}


def t1():
    cur_db(connections["default"])
    connections["default"].close()


def t2():
    try:
        cur_db(connections["default"])
        out["r"] = "accepted, no error"
    except Exception as e:
        out["r"] = err(e)
    finally:
        try:
            connections["default"].close()
        except Exception:
            pass


th = threading.Thread(target=t1)
th.start()
th.join()
connections.settings["default"]["CONN_MAX_AGE"] = 60
th = threading.Thread(target=t2)
th.start()
th.join()
connections.settings["default"]["CONN_MAX_AGE"] = 0
print(f"  new thread after the live setting became 60: {out['r']}")
drop_pools()
