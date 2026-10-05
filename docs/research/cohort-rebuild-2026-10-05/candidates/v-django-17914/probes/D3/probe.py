"""D3: close a connection inside atomic(), then query. Run at base and head.

Usage: PYTHONPATH=<django checkout> python probe.py <sqlite|postgresql> <sock dir> <port>
"""
import os
import sys
import tempfile

import django
from django.conf import settings

backend = sys.argv[1]
if backend == "sqlite":
    tmp = tempfile.mkdtemp()

    def db(autocommit):
        return {
            "ENGINE": "django.db.backends.sqlite3",
            "NAME": os.path.join(tmp, f"d3-{autocommit}.sqlite3"),
            "AUTOCOMMIT": autocommit,
        }
else:
    def db(autocommit):
        return {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": sys.argv[2],
            "PORT": sys.argv[3],
            "AUTOCOMMIT": autocommit,
        }

settings.configure(
    DATABASES={"default": db(True), "autocommit_off": db(False)}, INSTALLED_APPS=[]
)
django.setup()
from django.db import connections, transaction


def state(c):
    return (
        f"connection={'None' if c.connection is None else 'set'} "
        f"in_atomic_block={c.in_atomic_block} closed_in_transaction={c.closed_in_transaction}"
    )


def query(c, label):
    try:
        with c.cursor() as cursor:
            cursor.execute("SELECT 1")
            print(f"  {label}: OK {cursor.fetchone()}")
    except Exception as e:
        print(f"  {label}: {type(e).__module__}.{type(e).__name__}: {e}")


print(f"django {django.get_version()} backend={backend}")

for alias, title in (
    ("autocommit_off", "CASE A: AUTOCOMMIT False, close() inside atomic(), query AFTER the block"),
    ("default", "CASE B: default autocommit, close() inside atomic(), query AFTER the block"),
):
    print(title)
    c = connections[alias]
    query(c, "query before")
    try:
        with transaction.atomic(using=alias):
            c.close()
    except Exception as e:
        print(f"  atomic exit raised: {type(e).__name__}: {e}")
    else:
        print("  atomic exit raised nothing")
    print(f"  state after block: {state(c)}")
    query(c, "1st query after block")
    query(c, "2nd query after block")
    c.close()
    query(c, "query after an extra close()")
    from django.db import close_old_connections
    close_old_connections()
    query(c, "query after close_old_connections()")
    if alias == "autocommit_off":
        import threading

        t = threading.Thread(target=lambda: (query(connections[alias], "query from a NEW thread"), connections[alias].close()))
        t.start()
        t.join()
        try:
            c.connect()
            print("  calling the low-level c.connect() by hand: no error")
        except Exception as e:
            print(f"  calling the low-level c.connect() by hand: {type(e).__name__}: {e}")
        query(c, "query after the manual connect()")

print("CASE C: default autocommit, close() inside atomic(), query INSIDE the same block")
connections.close_all()
c = connections["default"]
c.in_atomic_block = False
c.closed_in_transaction = False
query(c, "query before")
try:
    with transaction.atomic():
        c.close()
        print(f"  state inside block after close: {state(c)}")
        query(c, "query inside block after close()")
        print(f"  state inside block after query: {state(c)}")
except Exception as e:
    print(f"  atomic exit raised: {type(e).__name__}: {e}")
else:
    print("  atomic exit raised nothing")
print(f"  state after block: {state(c)}")
query(c, "query after block")
