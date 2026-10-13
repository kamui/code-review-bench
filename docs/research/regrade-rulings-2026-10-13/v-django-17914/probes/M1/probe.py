"""M1: what does a person see with OPTIONS["pool"] set, by driver and by entry point?

Usage: PYTHONPATH=<django checkout> python probe.py <case> <options-json> <host> <port>

Cases, each run in its own process:
  cursor             connection.cursor()
  ensure_connection  connection.ensure_connection()
  connect            connection.connect()
  orm                ContentType.objects.count(), an ordinary ORM query
  cursor-twice       connection.cursor() two times, to see whether the second call differs
  query              cursor() and SELECT 1, reporting whether a pool is in use

<options-json> is the OPTIONS dictionary of the database, optionally with the
key "_settings" holding extra keys for the database entry (CONN_MAX_AGE, ...).
"""
import importlib
import json
import os
import sys
import time
import traceback

import django
from django.conf import settings

case, options, host, port = sys.argv[1], json.loads(sys.argv[2]), sys.argv[3], sys.argv[4]
extra = options.pop("_settings", {})
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "rr_db",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "OPTIONS": options,
            **extra,
        }
    },
    INSTALLED_APPS=["django.contrib.contenttypes"],
    USE_TZ=True,
)
django.setup()
from django.db import connection  # noqa: E402
from django.db.backends.postgresql import base as pg_base  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(django.__file__)) + os.sep


def importable(name):
    try:
        importlib.import_module(name)
        return "yes"
    except Exception as e:
        return f"no ({type(e).__name__}: {str(e).splitlines()[0]})"


def reachable():
    try:
        if pg_base.Database.__name__ == "psycopg2":
            c = pg_base.Database.connect(host=host, port=port, user="postgres", dbname="rr_db")
        else:
            c = pg_base.Database.connect(host=host, port=port, user="postgres", dbname="rr_db")
        c.close()
        return "yes"
    except Exception as e:
        return f"no ({type(e).__name__}: {str(e).splitlines()[0][:90]})"


def describe(e):
    frames = [f for f in traceback.extract_tb(e.__traceback__) if f.filename.startswith(ROOT)]
    route = " -> ".join(f"{f.name}" for f in frames)
    last = frames[-1] if frames else traceback.extract_tb(e.__traceback__)[-1]
    text = f"{type(e).__module__}.{type(e).__name__}: {str(e)!r}"
    text += f"\n      raised at {last.filename.replace(ROOT, '')}:{last.lineno} in {last.name}"
    text += f"\n      route inside Django: {route}"
    if e.__cause__ is not None:
        c = e.__cause__
        text += f"\n      caused by {type(c).__module__}.{type(c).__name__}: {str(c).splitlines()[0]!r}"
    return text


def attempt(label, fn):
    started = time.monotonic()
    try:
        result = fn()
        outcome = f"OK {result}"
    except Exception as e:
        outcome = describe(e)
    print(f"  {label}: after {time.monotonic() - started:.1f}s\n    {outcome}")


def do_cursor():
    with connection.cursor():
        # The commit before the change has no `pool` attribute.
        return "cursor opened; connection.pool is " + ("a pool" if getattr(connection, "pool", None) else "None")


def do_query():
    with connection.cursor() as cursor:
        cursor.execute("SELECT 1")
        row = cursor.fetchone()
    pool = getattr(connection, "pool", None)
    kind = type(pool).__module__ + "." + type(pool).__name__ if pool else "None"
    return f"SELECT 1 gave {row}; connection.pool is {kind}"


def do_orm():
    from django.contrib.contenttypes.models import ContentType

    return f"count gave {ContentType.objects.count()}"


print(f"case={case} OPTIONS={json.dumps(options)} extra={json.dumps(extra)}")
print(
    f"  Django {django.get_version()}; driver module {pg_base.Database.__name__} "
    f"{pg_base.Database.__version__.split(' ')[0]}; is_psycopg3={pg_base.is_psycopg3}; "
    f"psycopg_pool importable: {importable('psycopg_pool')}; server reachable: {reachable()}"
)
if case == "cursor":
    attempt("connection.cursor()", do_cursor)
elif case == "cursor-twice":
    attempt("connection.cursor(), first call", do_cursor)
    attempt("connection.cursor(), second call", do_cursor)
elif case == "ensure_connection":
    attempt("connection.ensure_connection()", lambda: connection.ensure_connection() or "connected")
elif case == "connect":
    attempt("connection.connect()", lambda: connection.connect() or "connected")
elif case == "orm":
    attempt("ContentType.objects.count()", do_orm)
elif case == "query":
    attempt("cursor() and SELECT 1", do_query)
else:
    raise SystemExit(f"unknown case {case}")
