"""D2: put a key that Django also sets itself into OPTIONS["pool"].

Usage: PYTHONPATH=<django checkout> python probe.py <sock dir> <port>
"""
import sys

import django
import psycopg_pool
from django.conf import settings


def my_check(conn):
    conn.execute("SELECT 1")


def my_configure(conn):
    conn.execute("SET application_name = 'd2probe'")


cases = {
    "plain dict {'min_size': 1}": {"min_size": 1},
    "{'check': my_check}": {"check": my_check},
    "{'configure': my_configure}": {"configure": my_configure},
    "{'open': False}": {"open": False},
    "{'kwargs': {'connect_timeout': 3}}": {"kwargs": {"connect_timeout": 3}},
    "{'reset': my_check}  (a hook Django does not set)": {"reset": my_check},
}
settings.configure(
    DATABASES={
        f"db{i}": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": sys.argv[1],
            "PORT": sys.argv[2],
            "OPTIONS": {"pool": options},
        }
        for i, options in enumerate(cases.values())
    }
    | {"default": {}},
    INSTALLED_APPS=[],
)
django.setup()
from django.db import connections

print(f"django {django.get_version()} psycopg_pool {psycopg_pool.__version__}")
for i, title in enumerate(cases):
    connection = connections[f"db{i}"]
    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
            cursor.fetchone()
        print(f"OPTIONS['pool'] = {title}: first query OK")
    except Exception as e:
        print(
            f"OPTIONS['pool'] = {title}: first query raised "
            f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}"
        )
    finally:
        try:
            connection.close()
            if hasattr(connection, "close_pool"):
                connection.close_pool()
        except Exception as e:
            print(f"    and connection.close() raised {type(e).__name__}: {str(e).splitlines()[0]}")
