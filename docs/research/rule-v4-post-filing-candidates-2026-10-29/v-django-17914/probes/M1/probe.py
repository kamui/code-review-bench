"""probes/M1: a PostgreSQL DatabaseWrapper subclass that overrides ensure_timezone().

Usage: python -I probe.py <django source dir> <label>

Cases (each on its own alias, so no pool is shared between cases):
  plain           the stock DatabaseWrapper, settings TIME_ZONE=UTC, USE_TZ=False
  tokyo           a subclass whose ensure_timezone() sets the session time zone
                  to Asia/Tokyo (its own choice), same settings
  tokyo-pool      the same subclass with OPTIONS["pool"] = True (head only; the
                  commit before the change has no pool option)
  skip-emulated   GT-v5's case, emulated: a subclass whose ensure_timezone()
                  skips time zone SQL, logging in as m1_qdb, a role for which
                  set_config raises (see fixture.sql). USE_TZ=True, so Django
                  wants 'UTC' and the role's session starts in Asia/Tokyo.
  skip-emulated-pool  the same with OPTIONS["pool"] = True (head only)
  signal          tokyo, connected, then override_settings(TIME_ZONE=...) on an
                  initialized wrapper: does Django call the override then?
  documented      the stock wrapper with settings TIME_ZONE=Asia/Tokyo (the
                  documented way to choose the session time zone with USE_TZ=False)
"""
import copy
import sys
import traceback

src, label = sys.argv[1], sys.argv[2]
sys.path.insert(0, src)

import django  # noqa: E402
from django.conf import settings  # noqa: E402

BASE = {
    "ENGINE": "django.db.backends.postgresql",
    "HOST": "127.0.0.1",
    "PORT": "55432",
    "USER": "postgres",
    "PASSWORD": "bench",
    "NAME": "postgres",
}
settings.configure(
    DATABASES={"default": dict(BASE)},
    USE_TZ=False,
    TIME_ZONE="UTC",
)
django.setup()

from django.db import connections  # noqa: E402
from django.db.backends.postgresql import base as pg  # noqa: E402
from django.test.utils import override_settings  # noqa: E402

HAS_POOL = hasattr(pg.DatabaseWrapper, "pool")
CALLS = []


class TokyoWrapper(pg.DatabaseWrapper):
    session_time_zone = "Asia/Tokyo"

    def ensure_timezone(self):
        CALLS.append("tokyo")
        if self.connection is None:
            return False
        with self.connection.cursor() as cursor:
            cursor.execute("SELECT set_config('TimeZone', %s, false)", [self.session_time_zone])
        return True


class SkipWrapper(pg.DatabaseWrapper):
    def ensure_timezone(self):
        CALLS.append("skip")
        return False


def settings_dict(**over):
    d = copy.deepcopy(connections.settings["default"])
    for k, v in over.items():
        if k == "OPTIONS":
            d["OPTIONS"] = dict(d.get("OPTIONS") or {}, **v)
        else:
            d[k] = v
    return d


def query(w):
    with w.cursor() as c:
        c.execute(
            "SELECT current_setting('TimeZone'), "
            "extract(hour from TIMESTAMPTZ '2024-01-01 00:00:00+00')::int, "
            "current_user"
        )
        return c.fetchone()


def run(name, factory, needs_pool=False):
    if needs_pool and not HAS_POOL:
        print(f"{name}: not run (no pool option at this commit)")
        return
    CALLS.clear()
    w = factory()
    try:
        tz, hour, user = query(w)
        print(f"{name}: session TimeZone={tz} local hour of 00:00 UTC={hour} "
              f"user={user} override calls={len(CALLS)}")
    except Exception as e:  # report, do not hide
        print(f"{name}: {type(e).__module__}.{type(e).__name__}: {str(e).strip()} "
              f"override calls={len(CALLS)}")
    finally:
        try:
            w.close()
            if HAS_POOL:
                w.close_pool()
        except Exception as e:
            print(f"  cleanup: {type(e).__name__}: {e}")


print(f"label={label} django={django.get_version()} pool option={'yes' if HAS_POOL else 'no'}")
print(f"settings: TIME_ZONE={settings.TIME_ZONE} USE_TZ={settings.USE_TZ}")

run("plain", lambda: pg.DatabaseWrapper(settings_dict(), "m1_plain"))
run("tokyo", lambda: TokyoWrapper(settings_dict(), "m1_tokyo"))
run("tokyo-pool", lambda: TokyoWrapper(
    settings_dict(OPTIONS={"pool": {"min_size": 1, "max_size": 2, "timeout": 5}}),
    "m1_tokyo_pool"), needs_pool=True)

with override_settings(USE_TZ=True):
    run("skip-emulated", lambda: SkipWrapper(
        settings_dict(USER="m1_qdb"), "m1_skip"))
    run("skip-emulated-pool", lambda: SkipWrapper(
        settings_dict(USER="m1_qdb",
                      OPTIONS={"pool": {"min_size": 1, "max_size": 2, "timeout": 3}}),
        "m1_skip_pool"), needs_pool=True)
    # Control for the emulation: the stock wrapper as m1_qdb must hit the shim.
    run("stock-on-emulated", lambda: pg.DatabaseWrapper(
        settings_dict(USER="m1_qdb"), "m1_stock_qdb"))

# signal: the override on an initialized wrapper registered in `connections`.
CALLS.clear()
connections["default"] = TokyoWrapper(settings_dict(), "default")
w = connections["default"]
tz0 = query(w)[0]
calls_connect = len(CALLS)
with override_settings(TIME_ZONE="Europe/Paris"):
    calls_signal = len(CALLS) - calls_connect
print(f"signal: TimeZone after connect={tz0} override calls at connect={calls_connect} "
      f"override calls from the TIME_ZONE override signal={calls_signal}")
w.close()

with override_settings(TIME_ZONE="Asia/Tokyo"):
    run("documented", lambda: pg.DatabaseWrapper(settings_dict(), "m1_documented"))
