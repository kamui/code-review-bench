"""C1: what does the caller see with assume_role set, with and without the pool,
for a role that does not exist and for a role that exists?

Usage: PYTHONPATH=<django checkout> python probe.py <case> <socket dir> <port> <server log> [pool timeout]

Cases, each run in its own process. "+pool" adds OPTIONS["pool"]; "+connect" calls
connection.connect() directly, as the comment's example does, where the other cases
open a cursor and run a query.
  missing-role            assume_role names a role that does not exist
  valid-role              assume_role names a role that exists (rr_role)
  missing-role+pool
  missing-role+pool+connect
  valid-role+pool         the known problem's own trigger
  valid-role+pool+connect
  no-role+pool            control: the pool with no assume_role

With a fifth argument the pool's timeout is that many seconds. Without it OPTIONS["pool"]
is True and the pool library's default of 30 seconds applies.

After each case the probe reads what the PostgreSQL server logged meanwhile (the server
runs with log_statement=all and log_connections=on) and counts the sessions opened and the
SET ROLE statements received. Two seconds into a pooled case it also records where each
pool worker thread is.
"""
import logging
import os
import sys
import threading
import time
import traceback

import django
from django.conf import settings

case, host, port, server_log = sys.argv[1:5]
timeout = float(sys.argv[5]) if len(sys.argv) > 5 else None
parts = case.split("+")
options = {}
if "pool" in parts:
    options["pool"] = {"timeout": timeout} if timeout else True
if parts[0] == "missing-role":
    options["assume_role"] = "rr_no_such_role"
if parts[0] == "valid-role":
    options["assume_role"] = "rr_role"
settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "rr_db",
            "USER": "postgres",
            "HOST": host,
            "PORT": port,
            "OPTIONS": options,
        }
    },
    INSTALLED_APPS=[],
    USE_TZ=True,
)
django.setup()
from django.db import connection  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(django.__file__)) + os.sep


class Collect(logging.Handler):
    def __init__(self):
        super().__init__(logging.WARNING)
        self.messages = []

    def emit(self, record):
        self.messages.append(record.getMessage().splitlines()[0])


collected = Collect()
logging.getLogger("psycopg.pool").addHandler(collected)
logging.getLogger("psycopg.pool").propagate = False

stacks = []


def snapshot():
    time.sleep(2)
    names = {t.ident: t.name for t in threading.enumerate()}
    for ident, frame in sys._current_frames().items():
        name = names.get(ident, "?")
        if not name.startswith("pool-"):
            continue
        calls = [
            f"{f.name} ({f.filename.replace(ROOT, '').split('site-packages/')[-1]}:{f.lineno})"
            for f in traceback.extract_stack(frame)
            if "/django/" in f.filename or "psycopg_pool" in f.filename
        ]
        stacks.append((name, calls))


if "pool" in parts:
    threading.Thread(target=snapshot, daemon=True).start()

log_start = os.path.getsize(server_log)
started = time.monotonic()
try:
    if "connect" in parts:
        connection.connect()
        outcome = "OK connected"
    else:
        with connection.cursor() as cursor:
            cursor.execute("SELECT current_user")
            outcome = f"OK current_user is {cursor.fetchone()[0]!r}"
except Exception as e:
    outcome = f"{type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}"
    if e.__cause__ is not None:
        c = e.__cause__
        outcome += f"\n      caused by {type(c).__module__}.{type(c).__name__}: {str(c).splitlines()[0]}"
elapsed = time.monotonic() - started
what = "connection.connect()" if "connect" in parts else "the first query"
print(f"{case}: OPTIONS={options}")
print(f"  after {elapsed:.1f}s {what} gave\n    {outcome}")
time.sleep(0.5)
with open(server_log, errors="replace") as f:
    f.seek(log_start)
    new = f.read()
print(
    f"  the server logged meanwhile: {new.count('connection authorized')} sessions opened, "
    f"{new.count('statement: SET ROLE')} SET ROLE statements, "
    f"{new.count('does not exist')} 'does not exist' errors"
)
if collected.messages:
    distinct = list(dict.fromkeys(m[:150] for m in collected.messages))
    print(f"  the pool library logged {len(collected.messages)} warnings; distinct texts (first 3):")
    for message in distinct[:3]:
        print(f"    {message}")
elif "pool" in parts:
    print("  the pool library logged no warning before the caller got its answer")
for name, calls in sorted(stacks):
    names_only = [c.split(" (")[0] for c in calls]
    if "_configure_connection" not in names_only:
        continue
    start = names_only.index("_configure_connection")
    print(f"  2 s in, thread {name} is in: " + " > ".join(names_only[start:]))
    print(f"      innermost frame: {calls[-1]}")
