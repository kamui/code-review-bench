"""K3: what does the caller see, and where does the server's message go, when a connection
cannot be opened or set up, with and without OPTIONS["pool"]?

One case per process. Usage:
  PYTHONPATH=<django checkout> python probe.py <failure> <pool> <logging> <sock dir> <port>

<failure>  bad-tz        a time zone name the server rejects (set through settings.configure(),
                         which skips the check Django makes when it loads a settings module)
           bad-db        NAME is a database that does not exist
           bad-password  a role that needs a password, with the wrong one
           server-down   a port nothing listens on
           none          nothing wrong (control)
<pool>     off | on (timeout 6 seconds) | default (OPTIONS["pool"] = True, the 30 second default)
<logging>  default       no LOGGING setting: Django's default logging configuration
           root-console  a LOGGING setting with a console handler on the root logger
           root-error    the same, with the root logger and its handler set to ERROR
           django-only   a LOGGING setting that names only the "django" logger

The script prints what the calling code gets. Everything the process writes to standard error
(where Python's logging puts a warning nobody handles) is captured by run_all.py.
"""
import sys
import time

import django
from django.conf import settings

failure, pool, logging_mode, host, port = sys.argv[1:6]
options = {}
if pool == "on":
    options["pool"] = {"timeout": 6}
elif pool == "default":
    options["pool"] = True
database = {
    "ENGINE": "django.db.backends.postgresql",
    "NAME": "k3_no_such_db" if failure == "bad-db" else "postgres",
    "USER": "k3_pwuser" if failure == "bad-password" else "postgres",
    "HOST": host,
    "PORT": "54439" if failure == "server-down" else port,
    "OPTIONS": options,
}
if failure == "bad-password":
    database["PASSWORD"] = "wrong-password"
extra = {}
if logging_mode == "root-console":
    extra["LOGGING"] = {
        "version": 1,
        "disable_existing_loggers": False,
        "handlers": {"console": {"class": "logging.StreamHandler"}},
        "root": {"handlers": ["console"], "level": "WARNING"},
    }
elif logging_mode == "root-error":
    extra["LOGGING"] = {
        "version": 1,
        "disable_existing_loggers": False,
        "handlers": {"console": {"class": "logging.StreamHandler", "level": "ERROR"}},
        "root": {"handlers": ["console"], "level": "ERROR"},
    }
elif logging_mode == "django-only":
    extra["LOGGING"] = {
        "version": 1,
        "disable_existing_loggers": False,
        "handlers": {"console": {"class": "logging.StreamHandler"}},
        "loggers": {"django": {"handlers": ["console"], "level": "INFO"}},
    }
settings.configure(
    DATABASES={"default": database},
    INSTALLED_APPS=[],
    USE_TZ=False,
    TIME_ZONE="Mars/Olympus_Mons" if failure == "bad-tz" else "UTC",
    **extra,
)
django.setup()
from django.db import connection


def describe(exc):
    first = str(exc).splitlines()[0] if str(exc) else ""
    return f"{type(exc).__module__}.{type(exc).__name__}: {first}"


started = time.monotonic()
try:
    with connection.cursor() as cursor:
        cursor.execute("SELECT current_user, current_setting('TimeZone')")
        print(f"the first query returned {cursor.fetchone()}")
except Exception as exc:
    elapsed = time.monotonic() - started
    print(f"after {elapsed:.1f}s the first query raised")
    print(f"    {describe(exc)}")
    seen, link, depth = {id(exc)}, exc, 1
    while True:
        nxt = link.__cause__ or link.__context__
        if nxt is None or id(nxt) in seen:
            break
        kind = "__cause__" if link.__cause__ is not None else "__context__"
        print(f"    {'  ' * depth}its {kind}: {describe(nxt)}")
        seen.add(id(nxt))
        link, depth = nxt, depth + 1
    # Is the server's own wording anywhere in what the caller holds?
    held = []
    link, seen = exc, set()
    while link is not None and id(link) not in seen:
        seen.add(id(link))
        held.append(str(link))
        held.append(repr(getattr(link, "diag", None) and link.diag.message_primary))
        link = link.__cause__ or link.__context__
    needles = {
        "bad-tz": "invalid value for parameter",
        "bad-db": "does not exist",
        "bad-password": "password authentication failed",
        "server-down": "No such file or directory",
    }
    needle = needles.get(failure)
    if needle:
        found = any(needle in text for text in held)
        print(f"    the exception chain {'holds' if found else 'does not hold'} the text {needle!r}")
else:
    print(f"    after {time.monotonic() - started:.1f}s")
finally:
    if pool != "off":
        try:
            connection.close_pool()
        except Exception:
            pass
