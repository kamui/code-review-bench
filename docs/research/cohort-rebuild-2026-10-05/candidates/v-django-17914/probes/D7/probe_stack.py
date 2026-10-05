"""D7a: where is the pool's setup thread while the caller waits?

Pool plus assume_role naming a role that does NOT exist. Two seconds after the
first query starts, print the Django frames of every pool worker thread.

Usage: PYTHONPATH=<django checkout> python probe_stack.py <sock dir> <port>
"""
import sys
import threading
import time
import traceback

import django
from django.conf import settings

settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": sys.argv[1],
            "PORT": sys.argv[2],
            "OPTIONS": {"pool": {"timeout": 5, "min_size": 1}, "assume_role": "d7_no_such_role"},
        }
    },
    INSTALLED_APPS=[],
)
django.setup()
from django.db import connection

print(f"django {django.get_version()}")


def report():
    time.sleep(2)
    names = {t.ident: t.name for t in threading.enumerate()}
    for ident, frame in sys._current_frames().items():
        name = names.get(ident, "?")
        if not name.startswith("pool-"):
            continue
        frames = [
            f"{f.filename.split('/django/')[-1] if '/django/' in f.filename else f.filename.split('/')[-1]}:{f.lineno} {f.name}"
            for f in traceback.extract_stack(frame)
            if "/django/" in f.filename or "psycopg_pool" in f.filename
        ]
        if any("_configure_connection" in f for f in frames):
            print(f"  thread {name} after 2s, outermost call first:")
            for f in frames:
                print(f"    {f}")


threading.Thread(target=report, daemon=True).start()
try:
    with connection.cursor() as cursor:
        cursor.execute("SELECT 1")
    print("  first query OK")
except Exception as e:
    print(f"  first query: {type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}")
