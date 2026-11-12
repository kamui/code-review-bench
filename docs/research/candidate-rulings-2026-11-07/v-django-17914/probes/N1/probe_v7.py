"""GT-v7's failing cases: a pool options dictionary holding check, configure,
open or kwargs. One wrapper per key, each on its own alias, against the stub
server in stub_server.py (not PostgreSQL).

Usage: PYTHONPATH=<django checkout> python probe_v7.py
"""
import copy

from stub_server import StubServer

server = StubServer()

import django  # noqa: E402
from django.conf import settings  # noqa: E402

db = {
    "ENGINE": "django.db.backends.postgresql",
    "NAME": "stub",
    "USER": "stub",
    "HOST": "127.0.0.1",
    "PORT": str(server.port),
    "CONN_MAX_AGE": 0,
    "OPTIONS": {"sslmode": "disable"},
}
settings.configure(
    DATABASES={"default": db}, INSTALLED_APPS=[], USE_TZ=True, TIME_ZONE="UTC"
)
django.setup()
from django.db import connections  # noqa: E402
from django.db.backends.postgresql.base import DatabaseWrapper  # noqa: E402

calls = []
values = {
    "check": lambda conn: calls.append("user check"),
    "configure": lambda conn: calls.append("user configure"),
    "open": False,
    "kwargs": {},
}
for key, value in values.items():
    d = copy.deepcopy(connections["default"].settings_dict)
    d["OPTIONS"]["pool"] = {key: value, "min_size": 1, "max_size": 1, "timeout": 3}
    w = DatabaseWrapper(d, alias="v7_" + key)
    calls.clear()
    try:
        with w.cursor() as c:
            c.execute("SELECT 1")
            result = f"OK {c.fetchone()}"
    except Exception as e:
        result = f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}"
    print(f"pool option {key!r}: first query {result}; user callbacks run: {calls}")
    try:
        w.close()
        w.close_pool()
    except Exception:
        pass
