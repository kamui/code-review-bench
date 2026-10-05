"""D2 side question: pool=True with psycopg-pool older than 3.2 (no `check` argument).

Usage: PYTHONPATH=<django checkout> python probe_old_pool.py <sock dir> <port>
"""
import sys

import django
import psycopg_pool
from django.conf import settings

settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": sys.argv[1],
            "PORT": sys.argv[2],
            "CONN_HEALTH_CHECKS": sys.argv[3] == "checks-on",
            "OPTIONS": {"pool": True},
        }
    },
    INSTALLED_APPS=[],
)
django.setup()
from django.db import connection

print(f"django {django.get_version()} psycopg_pool {psycopg_pool.__version__} CONN_HEALTH_CHECKS={sys.argv[3]}")
try:
    with connection.cursor() as cursor:
        cursor.execute("SELECT 1")
    print("  pool=True: first query OK")
except Exception as e:
    print(f"  pool=True: first query raised {type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}")
