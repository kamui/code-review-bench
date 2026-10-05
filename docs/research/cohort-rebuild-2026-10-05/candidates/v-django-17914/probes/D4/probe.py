"""D4: threads use the ORM connection and end without calling connection.close().

Modes
  nopool   no pool option.
  pool     OPTIONS["pool"] = {"min_size": 2, "max_size": 4, "timeout": 3}
           (psycopg_pool defaults are min_size=4, max_size=min_size, timeout=30;
           small numbers keep the run short).

Ten threads run one after another. Each runs one query and returns. Nothing
calls connection.close(). After each thread the probe forces a garbage
collection and prints how many server sessions exist.

Usage: PYTHONPATH=<django checkout> python probe.py <mode> <sock dir> <port>
"""
import gc
import sys
import threading
import time
import warnings

import django
import psycopg
from django.conf import settings

mode, host, port = sys.argv[1], sys.argv[2], sys.argv[3]
warnings.simplefilter("ignore", ResourceWarning)
options = {"pool": {"min_size": 2, "max_size": 4, "timeout": 3}} if mode == "pool" else {}
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "postgres",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "OPTIONS": {**options, "application_name": "d4probe"},
        }
    },
    INSTALLED_APPS=[],
)
django.setup()
from django.db import connection

admin = psycopg.connect(host=host, port=port, user="postgres", dbname="postgres", autocommit=True)


def sessions():
    return admin.execute(
        "SELECT count(*) FROM pg_stat_activity WHERE application_name = 'd4probe'"
    ).fetchone()[0]


print(f"django {django.get_version()} mode={mode}")
results = {}


def work(number):
    started = time.monotonic()
    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
            cursor.fetchone()
        results[number] = "query OK"
    except Exception as e:
        results[number] = (
            f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]} "
            f"(after {time.monotonic() - started:.1f}s)"
        )
    # The thread ends here without connection.close().


for number in range(1, 11):
    thread = threading.Thread(target=work, args=(number,))
    thread.start()
    thread.join()
    del thread
    gc.collect()
    time.sleep(0.3)
    line = f"thread {number:2}: {results[number]}; server sessions now: {sessions()}"
    pool = getattr(connection, "pool", None)
    if pool is not None:
        stats = pool.get_stats()
        line += (
            f"; pool counts {stats.get('pool_size')} connections of max {stats.get('pool_max')}, "
            f"{stats.get('pool_available')} available"
        )
    print(line)

print("main thread, same process, afterwards:")
work("main")
print(f"  {results['main']}")
