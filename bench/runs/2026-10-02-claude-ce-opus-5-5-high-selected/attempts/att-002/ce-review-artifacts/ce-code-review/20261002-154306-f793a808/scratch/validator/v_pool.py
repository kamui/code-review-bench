import sys, threading, time, traceback, warnings, logging
from unittest import mock
import django
from django.conf import settings
from psycopg.pq import TransactionStatus
import psycopg

class FakeConn:
    def __init__(self, dbname):
        self.dbname = dbname
        self.pgconn = mock.MagicMock(); self.pgconn.transaction_status = TransactionStatus.IDLE
        self.info = mock.MagicMock(); self.info.parameter_status.return_value = "UTC"
        self.info.server_version = 160000
        self.adapters = mock.MagicMock()
        self.closed = False; self.broken = False; self.autocommit = True; self.executed = []
    @classmethod
    def connect(cls, conninfo="", **kwargs):
        return cls(kwargs.get("dbname"))
    def cursor(self, *a, **k):
        conn = self
        class Cur:
            connection = conn
            adapters = mock.MagicMock()
            def __enter__(s): return s
            def __exit__(s, *a): return False
            def execute(s, sql, params=None): conn.executed.append(str(sql))
            def close(s): pass
        return Cur()
    def close(self): self.closed = True
    def commit(self): pass
    def rollback(self): pass

POOL = {"connection_class": FakeConn, "min_size": 1, "max_size": 2, "timeout": 3, "reconnect_timeout": 1}
def pg(name, **opts):
    return {"ENGINE": "django.db.backends.postgresql", "NAME": name, "OPTIONS": opts}
settings.configure(USE_TZ=True, TIME_ZONE="UTC", DATABASES={
    "default": pg("prod", pool=dict(POOL)),
    "role": pg("appdb", pool=dict(POOL), assume_role="app_role"),
    "empty": pg("appdb", pool={}),
})
django.setup()
from django.db import connections
from django.db.backends.postgresql.base import DatabaseWrapper
which = sys.argv[1]

if which == "f1":
    w = connections["role"]
    def dump():
        time.sleep(1.0)
        for tid, fr in sys._current_frames().items():
            names = [f.name for f in traceback.extract_stack(fr)]
            if "_configure_connection" in names:
                print("WORKER STACK:", " > ".join(n for n in names if n in (
                    "_connect", "_configure_connection", "ensure_role", "compose_sql", "mogrify",
                    "cursor", "_cursor", "ensure_connection", "connect", "get_new_connection", "getconn")))
                break
    threading.Thread(target=dump, daemon=True).start()
    t0 = time.monotonic()
    try:
        w.ensure_connection()
        print("f1 role: connected in %.2fs" % (time.monotonic() - t0))
    except Exception as e:
        print("f1 role: FAILED after %.2fs: %s.%s: %s" % (time.monotonic() - t0, type(e).__module__, type(e).__name__, e))
    print("pool stats:", {k: v for k, v in w.pool.get_stats().items() if k in ("pool_size", "pool_available", "connections_num", "connections_errors")})
    w.pool.close()
    # control: same pool settings without assume_role
    c = connections["default"]
    t0 = time.monotonic(); c.ensure_connection(); print("f1 control (no assume_role): connected in %.2fs" % (time.monotonic() - t0))
    c.close(); c.close_pool()

elif which == "f2":
    w = connections["default"]
    print("pools before:", dict(DatabaseWrapper._connection_pools))
    def refuse(*a, **k):
        raise psycopg.OperationalError("simulated: 'postgres' database unreachable (dbname=%r)" % k.get("dbname"))
    with mock.patch.object(psycopg, "connect", refuse), warnings.catch_warnings(record=True) as ws:
        warnings.simplefilter("always")
        name = w.creation._create_test_db(verbosity=0, autoclobber=True, keepdb=False)
    print("_create_test_db returned:", name, "| warnings:", [str(x.message)[:60] for x in ws])
    print("pool registered for 'default' after _create_test_db:", "default" in DatabaseWrapper._connection_pools,
          "dbname:", DatabaseWrapper._connection_pools["default"].kwargs.get("dbname"))
    # base/creation.py:64-66
    w.close()
    settings.DATABASES["default"]["NAME"] = name
    w.settings_dict["NAME"] = name
    print("get_connection_params dbname after switch:", w.get_connection_params()["dbname"])
    w.ensure_connection()
    print("wrapper NAME:", w.settings_dict["NAME"], "| raw connection actually opened against dbname:", w.connection.dbname)
    with w.cursor() as cur:
        cur.execute("TRUNCATE some_table")
    print("statements on that raw connection:", w.connection.executed)
    w.close(); w.close_pool()

elif which == "f10":
    w = connections["empty"]
    print("pool={} -> wrapper.pool:", w.pool)

elif which == "f11":
    for val in ({"check": None}, {"configure": lambda c: None}, {"open": False}, {"kwargs": {}}, 1, "true", {"min_size": 1}):
        w = DatabaseWrapper({**connections["default"].settings_dict, "OPTIONS": {"pool": val}}, alias="x%d" % id(val))
        try:
            p = w.pool
            print(repr(val)[:40], "-> OK", type(p).__name__)
        except Exception as e:
            print(repr(val)[:40], "->", type(e).__name__, e)
