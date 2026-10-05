"""D4, narrow sub-case: does setting the isolation level right after a pool
checkout fail and strand the connection?

The pool hands out connections whose server sessions were killed (a database
restart). OPTIONS["isolation_level"] is set, so Django assigns
connection.isolation_level right after getconn().

Usage: PYTHONPATH=<django checkout> python probe_isolation.py <sock dir> <port>
"""
import sys
import time

import django
import psycopg
from django.conf import settings

host, port = sys.argv[1], sys.argv[2]
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "OPTIONS": {
                "pool": {"min_size": 2, "max_size": 2, "timeout": 3},
                "isolation_level": psycopg.IsolationLevel.READ_COMMITTED,
                "application_name": "d4iso",
            },
        }
    },
    INSTALLED_APPS=[],
)
django.setup()
from django.db import connection

print(f"django {django.get_version()}")


def query(label):
    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
            cursor.fetchone()
        print(f"  {label}: OK")
    except Exception as e:
        print(f"  {label}: {type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}")
    finally:
        connection.close()
    time.sleep(0.5)
    stats = connection.pool.get_stats()
    print(f"    pool counts {stats.get('pool_size')} of max {stats.get('pool_max')}, {stats.get('pool_available')} available")


try:
    query("first query")
except Exception as e:
    print(f"  not runnable at this commit: {type(e).__name__}: {e}")
    sys.exit(0)
with psycopg.connect(host=host, port=port, user="postgres", dbname="postgres", autocommit=True) as admin:
    n = admin.execute(
        "SELECT count(pg_terminate_backend(pid)) FROM pg_stat_activity WHERE application_name = 'd4iso'"
    ).fetchone()[0]
print(f"  server killed {n} pooled sessions")
time.sleep(0.5)
for i in range(1, 6):
    query(f"query {i} after the kill")
