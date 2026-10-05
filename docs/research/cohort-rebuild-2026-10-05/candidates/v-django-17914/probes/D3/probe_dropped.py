"""D3, second trigger: the server drops the session during atomic() with AUTOCOMMIT False.

No explicit connection.close() in application code.
  mid:  the session is terminated, then the block runs another query (the block raises).
  tail: the session is terminated after the block's last query (the block body raises
        nothing; Django finds the dead connection when it releases the savepoint at exit
        and calls connection.close() itself, transaction.py "Drop it").

Usage: PYTHONPATH=<django checkout> python probe_dropped.py <sock dir> <port> <mid|tail>
"""
import sys

import django
import psycopg
from django.conf import settings

host, port, when = sys.argv[1], sys.argv[2], sys.argv[3]
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "AUTOCOMMIT": False,
        }
    },
    INSTALLED_APPS=[],
)
django.setup()
from django.db import close_old_connections, connection, transaction


def query(label):
    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
            print(f"  {label}: OK {cursor.fetchone()}")
    except Exception as e:
        print(f"  {label}: {type(e).__module__}.{type(e).__name__}: {e}")


print(f"django {django.get_version()}")
print(f"CASE D-{when}: AUTOCOMMIT False, server terminates the session during atomic()")
query("query before")
try:
    with transaction.atomic():
        with connection.cursor() as cursor:
            cursor.execute("SELECT pg_backend_pid()")
            pid = cursor.fetchone()[0]
        with psycopg.connect(
            host=host, port=port, user="postgres", dbname="postgres", autocommit=True
        ) as admin:
            admin.execute("SELECT pg_terminate_backend(%s)", [pid])
        if when == "mid":
            with connection.cursor() as cursor:
                cursor.execute("SELECT 1")
except Exception as e:
    print(f"  block raised: {type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}")
else:
    print("  block raised nothing")
print(
    f"  state after block: connection={'None' if connection.connection is None else 'set'} "
    f"in_atomic_block={connection.in_atomic_block} "
    f"closed_in_transaction={connection.closed_in_transaction}"
)
query("1st query after block")
close_old_connections()
query("query after close_old_connections() (what a new request does)")
connection.close()
query("query after connection.close()")
try:
    connection.connect()
    print("  explicit connection.connect(): OK")
except Exception as e:
    print(f"  explicit connection.connect(): {type(e).__name__}: {e}")
query("query after explicit connection.connect()")
