import threading, warnings
from unittest import mock
import django
from django.conf import settings
settings.configure(
    USE_TZ=True, TIME_ZONE="UTC",
    DATABASES={
        "default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"},
        "pg": {"ENGINE": "django.db.backends.postgresql", "NAME": "appdb", "USER": "u",
               "HOST": "127.0.0.1", "PORT": "1",
               "OPTIONS": {"pool": True, "assume_role": "app_role"}},
        "pg_empty": {"ENGINE": "django.db.backends.postgresql", "NAME": "appdb",
               "OPTIONS": {"pool": {}}},
    },
)
django.setup()
from django.db import connections, transaction

print("=== B: autocommit off + atomic + close-in-transaction (sqlite) ===")
c = connections["default"]
c.ensure_connection()
c.set_autocommit(False)
with transaction.atomic():
    c.close()
print("after exit: connection=%r in_atomic_block=%r closed_in_transaction=%r" % (c.connection, c.in_atomic_block, c.closed_in_transaction))
for i in range(2):
    try:
        with c.cursor() as cur:
            cur.execute("SELECT 1"); print("query ok", cur.fetchone())
    except Exception as e:
        print("query attempt", i, "->", type(e).__module__, type(e).__name__, e)
c.close(); c.close_if_unusable_or_obsolete()
try:
    c.cursor(); print("ok after close()")
except Exception as e:
    print("after close()/close_if_unusable_or_obsolete ->", type(e).__name__, e)

print("=== A: pool configure + assume_role re-enters wrapper ===")
pg = connections["pg"]
pool = pg.pool
print("pool configure is bound to wrapper:", pool._configure.__self__ is pg)
fake = mock.MagicMock(); fake.info.parameter_status.return_value = "UTC"
res = {}
def worker():
    with mock.patch.object(type(pg), "get_new_connection", side_effect=RuntimeError("REENTERED get_new_connection (would call pool.getconn())")) as m:
        try:
            pool._configure(fake)
            res["r"] = "no error"
        except Exception as e:
            res["r"] = "%s: %s" % (type(e).__name__, e)
t = threading.Thread(target=worker); t.start(); t.join()
print("configure from pool worker thread ->", res["r"])

print("=== C: pool keyed by alias survives NAME switch ===")
print("pool dbname before:", pg.pool.kwargs["dbname"])
pg.close()
pg.settings_dict["NAME"] = "test_appdb"
print("settings NAME now:", pg.settings_dict["NAME"], "| get_connection_params dbname:", pg.get_connection_params()["dbname"], "| pool dbname:", pg.pool.kwargs["dbname"], "| same pool:", pg.pool is pool)
other = type(pg)({**pg.settings_dict, "NAME": "some_other_db"}, alias=pg.alias)
print("fallback-style wrapper (same alias, other NAME) pool dbname:", other.pool.kwargs["dbname"])

print("=== D: misc ===")
print("pool={} ->", connections["pg_empty"].pool)
print("pools registered:", list(type(pg)._connection_pools))
pg.close_pool(); print("after close_pool:", list(type(pg)._connection_pools))
pg.ensure_timezone(); print("after ensure_timezone on poolless state:", list(type(pg)._connection_pools))
