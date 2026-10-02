import threading, time, logging
from unittest import mock
import django
from django.conf import settings
from psycopg.pq import TransactionStatus

class FakeConn:
    """Stand-in for psycopg.Connection so the pool can 'connect' offline."""
    created = 0
    def __init__(self):
        self.pgconn = mock.MagicMock(); self.pgconn.transaction_status = TransactionStatus.IDLE
        self.info = mock.MagicMock(); self.info.parameter_status.return_value = "UTC"
        self.info.server_version = 160000
        self.closed = False; self.autocommit = True; self.executed = []
    @classmethod
    def connect(cls, conninfo="", **kwargs):
        cls.created += 1
        return cls()
    def cursor(self, *a, **k):
        conn = self
        class Cur:
            connection = conn
            def __enter__(s): return s
            def __exit__(s, *a): return False
            def execute(s, sql, params=None): conn.executed.append(str(sql))
        return Cur()
    def close(self): self.closed = True

POOL = {"connection_class": FakeConn, "min_size": 1, "max_size": 2, "timeout": 3, "reconnect_timeout": 1}
settings.configure(USE_TZ=True, TIME_ZONE="UTC", DATABASES={
    "default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"},
    "plain": {"ENGINE": "django.db.backends.postgresql", "NAME": "appdb", "OPTIONS": {"pool": dict(POOL)}},
    "role": {"ENGINE": "django.db.backends.postgresql", "NAME": "appdb", "OPTIONS": {"pool": dict(POOL), "assume_role": "app_role"}},
})
django.setup()
from django.db import connections
logging.getLogger("psycopg.pool").addHandler(logging.StreamHandler()); logging.getLogger("psycopg.pool").setLevel(logging.WARNING)

for alias in ("plain", "role"):
    w = connections[alias]
    t0 = time.monotonic()
    try:
        conn = w.get_new_connection(w.get_connection_params())
        print(alias, "-> got pooled connection in %.2fs; executed during configure: %r" % (time.monotonic() - t0, conn.executed))
        conn._pool.putconn(conn)
    except Exception as e:
        print(alias, "-> FAILED after %.2fs: %s: %s" % (time.monotonic() - t0, type(e).__name__, e))
    print("   wrapper state touched by pool threads: connection=%r" % (w.connection,))
    try: w.close_pool()
    except Exception as e: print("   close_pool:", type(e).__name__, e)
