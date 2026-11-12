"""N1: what a request sees after the server ends idle sessions.

Usage: PYTHONPATH=<django checkout>:<this dir> python probe.py CASE EVENT

CASE   per-request        CONN_MAX_AGE 0, no pool option (Django's default)
       persistent         CONN_MAX_AGE 600, no pool option
       persistent-checks  CONN_MAX_AGE 600, CONN_HEALTH_CHECKS True
       pool               OPTIONS["pool"] = True (CONN_MAX_AGE 0)
       pool-checks        OPTIONS["pool"] = True, CONN_HEALTH_CHECKS True
EVENT  restart            every open session gets FATAL 57P01 and is closed
       drop               every open socket is closed without a message

Each request is framed by Django's own request_started and request_finished
signals, which run close_old_connections(). The server is the stub in
stub_server.py, not PostgreSQL.
"""
import logging
import sys
import time

from stub_server import StubServer

CASE, EVENT = sys.argv[1], sys.argv[2]
server = StubServer()

logging.basicConfig(stream=sys.stdout, format="    [log %(name)s] %(message)s")
logging.getLogger("psycopg.pool").setLevel(logging.WARNING)

import django  # noqa: E402
from django.conf import settings  # noqa: E402

db = {
    "ENGINE": "django.db.backends.postgresql",
    "NAME": "stub",
    "USER": "stub",
    "HOST": "127.0.0.1",
    "PORT": str(server.port),
    "CONN_MAX_AGE": 600 if CASE.startswith("persistent") else 0,
    "CONN_HEALTH_CHECKS": CASE.endswith("-checks"),
    "OPTIONS": {"sslmode": "disable"},
}
if CASE.startswith("pool"):
    db["OPTIONS"]["pool"] = True
settings.configure(
    DATABASES={"default": db}, INSTALLED_APPS=[], USE_TZ=True, TIME_ZONE="UTC"
)
django.setup()

import psycopg  # noqa: E402
from django.core.signals import request_finished, request_started  # noqa: E402
from django.db import connection, connections  # noqa: E402

try:
    import psycopg_pool

    pool_version = psycopg_pool.__version__
except ImportError:
    pool_version = None

print(
    f"django {django.get_version()}, psycopg {psycopg.__version__}, "
    f"psycopg_pool {pool_version}"
)
print(
    f"case {CASE}: CONN_MAX_AGE={db['CONN_MAX_AGE']}, "
    f"CONN_HEALTH_CHECKS={db['CONN_HEALTH_CHECKS']}, OPTIONS={db['OPTIONS']}; "
    f"event {EVENT}"
)
has_pool_attr = hasattr(type(connections["default"]), "pool")
if has_pool_attr:
    try:
        p = connection.pool
    except Exception as e:
        p = None
        print(f"  pool property raised {type(e).__name__}: {e}")
    if p is not None:
        print(
            f"  pool built by Django: check={p._check!r}, min_size={p.min_size}, "
            f"max_size={p.max_size}, max_idle={p.max_idle}, "
            f"max_lifetime={p.max_lifetime}"
        )
    else:
        print("  pool property: None")
else:
    print("  this commit's DatabaseWrapper has no pool property")


def request(label):
    request_started.send(sender=None)
    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
            row = cursor.fetchone()
        result = f"OK {row}"
    except Exception as e:
        first = str(e).strip().splitlines()[0] if str(e).strip() else ""
        result = f"{type(e).__module__}.{type(e).__name__}: {first}"
    finally:
        request_finished.send(sender=None)
    time.sleep(0.3)
    extra = ""
    if has_pool_attr and CASE.startswith("pool"):
        s = connection.pool.get_stats()
        extra = (
            f"; pool size {s.get('pool_size')}, available "
            f"{s.get('pool_available')}, connections_lost "
            f"{s.get('connections_lost', 0)}, returns_bad {s.get('returns_bad', 0)}"
        )
    print(
        f"  {label}: {result}; wrapper holds a connection: "
        f"{connection.connection is not None}; server sessions open "
        f"{server.live_sessions()}{extra}"
    )


request("request 1")
time.sleep(0.5)
before = server.live_sessions()
ended = server.restart() if EVENT == "restart" else server.drop()
time.sleep(0.3)
print(f"  {EVENT}: server ended {ended} of {before} open sessions")
for i in range(2, 8):
    request(f"request {i}")
print(f"  sessions the server accepted in total: {server.opened}")
if has_pool_attr and CASE.startswith("pool"):
    connection.close()
    connection.close_pool()
