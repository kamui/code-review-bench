"""D6 in isolation: copy a database through Django's no-database fallback.

No test runner and no change of NAME, so reference bug GT-v2 is not involved.
The login role d6user may not connect to the "postgres" database, so
connection._nodb_cursor() falls back to the alias's own database d6tpl. The
probe then counts the sessions on d6tpl and runs the statement Django uses to
clone a test database.

Usage: PYTHONPATH=<django checkout> python probe_clone.py <nopool|pool> <sock dir> <port>
"""
import sys
import time
import warnings

import django
import psycopg
from django.conf import settings

mode, host, port = sys.argv[1], sys.argv[2], sys.argv[3]
warnings.simplefilter("ignore", RuntimeWarning)
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "d6tpl",
            "USER": "d6user",
            "HOST": host,
            "PORT": port,
            "OPTIONS": {"pool": True} if mode == "pool" else {},
        }
    },
    INSTALLED_APPS=[],
)
django.setup()
from django.db import connection

admin = psycopg.connect(host=host, port=port, user="postgres", dbname="postgres", autocommit=True)
admin.execute("DROP DATABASE IF EXISTS d6tpl_copy WITH (FORCE)")
admin.execute("DROP DATABASE IF EXISTS d6tpl WITH (FORCE)")
admin.execute("CREATE DATABASE d6tpl OWNER d6user")

print(f"django {django.get_version()} mode={mode}")
try:
    with connection._nodb_cursor() as cursor:
        cursor.execute("SELECT current_database(), pg_backend_pid()")
        database, me = cursor.fetchone()
        time.sleep(1.0)
        others = admin.execute(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = 'd6tpl' AND pid <> %s", [me]
        ).fetchone()[0]
        print(f"  fallback cursor is connected to {database}; other sessions on d6tpl: {others}")
        try:
            cursor.execute('CREATE DATABASE "d6tpl_copy" WITH TEMPLATE "d6tpl"')
            print("  CREATE DATABASE d6tpl_copy WITH TEMPLATE d6tpl: OK")
        except Exception as e:
            print(f"  CREATE DATABASE d6tpl_copy WITH TEMPLATE d6tpl: {type(e).__module__}.{type(e).__name__}: {' / '.join(str(e).splitlines())}")
except Exception as e:
    print(f"  fallback cursor: {type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}")
time.sleep(0.5)
left = admin.execute("SELECT count(*) FROM pg_stat_activity WHERE datname = 'd6tpl'").fetchone()[0]
print(f"  sessions on d6tpl after the fallback cursor was closed: {left}")
if hasattr(connection, "close_pool"):
    connection.close_pool()
admin.execute("DROP DATABASE IF EXISTS d6tpl_copy WITH (FORCE)")
admin.execute("DROP DATABASE IF EXISTS d6tpl WITH (FORCE)")
