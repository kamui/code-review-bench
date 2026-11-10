"""Probe for N1: which wrapper's time zone and role a pooled session gets.

Usage: <venv>/bin/python -I probe.py <django tree> <case> <port>

Each case runs in a fresh process against a real PostgreSQL server on
127.0.0.1:<port>, database probe_app, login role postgres. The role
probe_role exists and the login can select it. At the base there is no pool
option, so the same steps run on direct connections.

Cases:
  v16          GT-v16's case: a thread builds the pool in UTC, closes and exits;
               the main thread, with no wrapper used yet, overrides TIME_ZONE and
               the pool replaces its one session.
  v16-idle     The same, but the pool hands out the session it already has.
  stale-a      Thread A and the main thread B both used the alias; B overrides
               TIME_ZONE; A queries again before B (A's wrapper rebuilds the pool).
  stale-b      The same with B querying before A.
  stale-copy   One thread: a copy() of the main wrapper is used, then the main
               wrapper overrides TIME_ZONE; the copy queries before the main wrapper.
  copy-role-1  A copy() with OPTIONS["assume_role"] = probe_role queries first;
               then the alias's ordinary wrapper queries.
  copy-role-2  The ordinary wrapper queries first; then the copy with the role.
  copy-tz-1    USE_TZ=True. A copy() with TIME_ZONE = Asia/Tokyo queries first;
               then the ordinary wrapper (TIME_ZONE unset, so UTC).
  copy-tz-2    USE_TZ=True. The ordinary wrapper first, then the copy.
  alias-tz     USE_TZ=True. The documented way to give one database another time
               zone: a second alias "other" with TIME_ZONE = Asia/Tokyo; default,
               other, default.
  alias-role   The same with a second alias whose OPTIONS["assume_role"] is probe_role.
  one-thread   The documented use with one thread: the main thread uses the alias,
               overrides TIME_ZONE and queries again.
  v10          GT-v10's case, as a control: a borrower runs SET TIME ZONE
               'Asia/Tokyo' and closes; the next query on the alias.
"""
import json
import sys
import threading

tree, case, port = sys.argv[1], sys.argv[2], sys.argv[3]
sys.path.insert(0, tree)

import django  # noqa: E402
from django.conf import settings  # noqa: E402

USE_TZ = case.startswith("copy-tz") or case == "alias-tz"
POOL = {"min_size": 1, "max_size": 2, "timeout": 2}
if case in ("v16", "v10"):
    POOL = {"min_size": 1, "max_size": 1, "timeout": 2}

settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "probe_app",
            "USER": "postgres",
            "HOST": "127.0.0.1",
            "PORT": port,
            "CONN_MAX_AGE": 0,
            "OPTIONS": {},
        }
    },
    INSTALLED_APPS=[],
    SECRET_KEY="probe",
    USE_TZ=USE_TZ,
    TIME_ZONE="UTC",
)
django.setup()

import django.test.signals  # noqa: E402,F401  registers the setting_changed receivers
from django.db import connections  # noqa: E402
from django.db.backends.postgresql.base import DatabaseWrapper  # noqa: E402
from django.test.utils import override_settings  # noqa: E402

HEAD = hasattr(DatabaseWrapper, "pool")
if HEAD:
    connections.databases["default"]["OPTIONS"]["pool"] = dict(POOL)
if case.startswith("alias-"):
    other = dict(connections.databases["default"], OPTIONS=dict(connections.databases["default"]["OPTIONS"]))
    if case == "alias-tz":
        other["TIME_ZONE"] = "Asia/Tokyo"
    else:
        other["OPTIONS"]["assume_role"] = "probe_role"
    connections.databases["other"] = other

SESSION_SQL = (
    "SELECT pg_backend_pid(), current_setting('TimeZone'), current_user::text, "
    "to_char('2024-01-01 00:00:00+00'::timestamptz, 'HH24')"
)


def emit(step, **data):
    print(json.dumps({"case": case, "step": step, **data}, sort_keys=True, default=str), flush=True)


def session(w):
    try:
        with w.cursor() as c:
            c.execute(SESSION_SQL)
            pid, tz, user, hour = c.fetchone()
        return {"pid": pid, "timezone": tz, "current_user": user, "hour_of_00utc": hour}
    except Exception as e:  # report the failure as the result
        return {"error": f"{type(e).__name__}: {e}"}


def owner():
    """Which wrapper the alias's current pool sets new sessions up from."""
    if not HEAD:
        return None
    pools = [p for k, p in DatabaseWrapper._connection_pools.items()
             if k == "default" or (isinstance(k, tuple) and k[0] == "default")]
    if not pools:
        return "no pool"
    names = []
    for pool in pools:
        cb = pool._configure
        names.append(getattr(getattr(cb, "__self__", None), "_probe_name", repr(cb)))
    return names[0] if len(names) == 1 else names


def finish():
    for w in connections.all(initialized_only=True):
        w.close()
    if HEAD:
        for p in list(DatabaseWrapper._connection_pools.values()):
            p.close()
        DatabaseWrapper._connection_pools.clear()


def other_thread(steps):
    """Run steps in a thread that stays alive between them; return a driver."""
    go, done, out = threading.Event(), threading.Event(), {}
    queue = list(steps)

    def run():
        while queue:
            go.wait()
            go.clear()
            name, fn = queue.pop(0)
            out[name] = fn()
            done.set()

    t = threading.Thread(target=run, daemon=True)
    t.start()

    def step(name):
        done.clear()
        go.set()
        done.wait(timeout=30)
        return out.get(name)

    return t, step


def a_query_and_close():
    w = connections["default"]
    w._probe_name = "A (other thread)"
    r = session(w)
    r["cached_timezone_name"] = w.__dict__.get("timezone_name")
    w.close()
    return r


def case_v16():
    created = []

    def creator():
        w = connections["default"]
        w._probe_name = "creator (exited thread)"
        created.append(session(w))
        w.close()

    t = threading.Thread(target=creator)
    t.start()
    t.join()
    emit("creator", result=created[0], creator_alive=t.is_alive(),
         main_initialized=len(connections.all(initialized_only=True)))
    with override_settings(TIME_ZONE="Africa/Nairobi"):
        w = connections["default"]
        w._probe_name = "main"
        if HEAD and case == "v16":
            p = w.pool
            p.open()
            p.wait()
            raw = p.getconn()
            previous = raw.info.backend_pid
            raw.close()
            p.putconn(raw)
            emit("replace-session", previous_pid=previous)
        emit("main-under-override", expected=w.timezone_name, result=session(w), pool_owner=owner())
        w.close()


def case_stale():
    t, step = other_thread([("a1", a_query_and_close), ("a2", a_query_and_close)])
    emit("A-before", result=step("a1"), pool_owner=owner())
    b = connections["default"]
    b._probe_name = "B (main thread)"
    emit("B-before", result=session(b), pool_owner=owner())
    b.close()
    with override_settings(TIME_ZONE="Africa/Nairobi"):
        emit("after-override", pool_owner=owner(),
             B_cached=b.__dict__.get("timezone_name"))
        if case == "stale-a":
            emit("A-under-override", result=step("a2"), pool_owner=owner())
            emit("B-under-override", expected="Africa/Nairobi", result=session(b), pool_owner=owner())
            b.close()
        else:
            emit("B-under-override", expected="Africa/Nairobi", result=session(b), pool_owner=owner())
            b.close()
            emit("A-under-override", result=step("a2"), pool_owner=owner())


def case_stale_copy():
    main = connections["default"]
    main._probe_name = "main wrapper"
    c = main.copy()
    c._probe_name = "copy"
    emit("copy-before", result=session(c), cached=c.__dict__.get("timezone_name"), pool_owner=owner())
    c.close()
    emit("main-before", result=session(main), pool_owner=owner())
    main.close()
    with override_settings(TIME_ZONE="Africa/Nairobi"):
        emit("after-override", copy_cached=c.__dict__.get("timezone_name"),
             main_cached=main.__dict__.get("timezone_name"), pool_owner=owner())
        emit("copy-under-override", result=session(c), pool_owner=owner())
        c.close()
        emit("main-under-override", expected="Africa/Nairobi", result=session(main), pool_owner=owner())
        main.close()


def case_copy():
    main = connections["default"]
    main._probe_name = "ordinary wrapper"
    c = main.copy()
    c._probe_name = "copy"
    if case.startswith("copy-role"):
        c.settings_dict["OPTIONS"]["assume_role"] = "probe_role"
        want = {"copy": "probe_role", "ordinary wrapper": "postgres"}
    else:
        c.settings_dict["TIME_ZONE"] = "Asia/Tokyo"
        want = {"copy": "Asia/Tokyo", "ordinary wrapper": "UTC"}
    order = [c, main] if case.endswith("-1") else [main, c]
    for w in order:
        emit(w._probe_name, expected=want[w._probe_name], result=session(w), pool_owner=owner())
        w.close()


def case_alias():
    want = {"default": "UTC", "other": "Asia/Tokyo"} if case == "alias-tz" else {
        "default": "postgres", "other": "probe_role"}
    for alias in ("default", "other", "default"):
        w = connections[alias]
        w._probe_name = alias
        emit(alias, expected=want[alias], result=session(w))
        w.close()


def case_one_thread():
    w = connections["default"]
    w._probe_name = "main"
    emit("before-override", result=session(w), pool_owner=owner())
    w.close()
    with override_settings(TIME_ZONE="Africa/Nairobi"):
        emit("main-under-override", expected="Africa/Nairobi", result=session(w), pool_owner=owner())
        w.close()


def case_v10():
    w = connections["default"]
    w._probe_name = "main"
    with w.cursor() as c:
        c.execute("SET TIME ZONE 'Asia/Tokyo'")
        c.execute("SELECT pg_backend_pid()")
        first = c.fetchone()[0]
    w.close()
    emit("next-borrower", expected="UTC", first_borrower_pid=first, result=session(w), pool_owner=owner())


CASES = {
    "alias-tz": case_alias, "alias-role": case_alias, "v10": case_v10,
    "one-thread": case_one_thread,
    "v16": case_v16, "v16-idle": case_v16,
    "stale-a": case_stale, "stale-b": case_stale,
    "stale-copy": case_stale_copy,
    "copy-role-1": case_copy, "copy-role-2": case_copy,
    "copy-tz-1": case_copy, "copy-tz-2": case_copy,
}

emit("identity", head=HEAD, tree=tree.rsplit("/", 1)[-1], use_tz=USE_TZ, pool=POOL if HEAD else None)
try:
    CASES[case]()
finally:
    finish()
