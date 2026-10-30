import copy
import json
import sys
import threading
import warnings
from unittest.mock import patch

import django
import psycopg
from django.conf import settings

GROUP = sys.argv[1]
POOL = {"min_size": 1, "max_size": 1, "timeout": 0.3}
DB = {
    "ENGINE": "django.db.backends.postgresql", "NAME": "postgres",
    "USER": "postgres", "PASSWORD": "bench", "HOST": "127.0.0.1",
    "PORT": "55432", "CONN_MAX_AGE": 0,
}
settings.configure(DATABASES={"default": DB}, INSTALLED_APPS=[],
                   SECRET_KEY="probe", USE_TZ=False, TIME_ZONE="UTC")
django.setup()
from django.db import connections
from django.db.backends.postgresql.base import DatabaseWrapper
from django.db.backends.base.base import NO_DB_ALIAS
from django.test.utils import override_settings

HEAD = hasattr(DatabaseWrapper, "pool")


def emit(name, **data):
    print(json.dumps({"case": name, **data}, sort_keys=True, default=str), flush=True)


def config(pool=False, **options):
    d = copy.deepcopy(connections.databases["default"])
    d["OPTIONS"] = options
    if pool:
        d["OPTIONS"]["pool"] = dict(POOL)
    return d


def close(w):
    try:
        w.close()
    finally:
        if HEAD:
            w.close_pool()


def query(w, sql="SELECT 1"):
    with w.cursor() as c:
        c.execute(sql)
        return c.fetchone()


def error(fn):
    try:
        return {"result": fn()}
    except Exception as e:
        return {"error": type(e).__name__ + ": " + str(e)}


def admin():
    return psycopg.connect(host="127.0.0.1", port=55432, user="postgres",
                           password="bench", dbname="postgres", autocommit=True)


def m1():
    for mode in ("normal", "server-kill", "closed-checkout"):
        w = DatabaseWrapper(config(HEAD, isolation_level=3), alias="m1_" + mode)
        if mode == "normal":
            emit(mode, **error(lambda: query(w)))
            close(w)
            continue
        if not HEAD:
            emit(mode, available=False, reason="No built-in pool at base")
            continue
        p = w.pool
        p.open()
        p.wait()
        raw = p.getconn()
        p.putconn(raw)
        if mode == "server-kill":
            with admin() as c:
                c.execute("SELECT pg_terminate_backend(%s)", (raw.info.backend_pid,))
            emit(mode, **error(lambda: w.get_new_connection(w.get_connection_params())),
                 note="No read on killed session before isolation setter")
            if w.connection is None:
                p.putconn(raw)
            p.close()
            DatabaseWrapper._connection_pools.pop(w.alias, None)
            continue
        raw.close()
        emit(mode, injection="Closed the idle physical object, leaving it in the real pool")
        emit("setter", **error(w.connect), wrapper_has_connection=w.connection is not None,
             pool=p.get_stats())
        emit("next-checkout", **error(lambda: p.getconn()), pool=p.get_stats())
        p.close()
        DatabaseWrapper._connection_pools.pop(w.alias, None)
    emit("invalid-isolation", **error(lambda: DatabaseWrapper(
        config(HEAD, isolation_level=-1), alias="m1_invalid").connect()))
    for explicit_close in (False, True):
        records = []
        alias = "m1_threads_" + str(explicit_close)
        def thread_query():
            d = config(HEAD)
            if HEAD:
                d['OPTIONS']['pool'].update(min_size=0, max_size=2)
            w = DatabaseWrapper(d, alias=alias)
            records.append(error(lambda: query(w)))
            if explicit_close:
                w.close()
        for _ in range(3):
            t = threading.Thread(target=thread_query)
            t.start()
            t.join()
        emit('thread-exit', explicit_close=explicit_close, results=records)
        if HEAD:
            DatabaseWrapper._connection_pools.pop(alias).close()


def m2():
    class Custom(DatabaseWrapper):
        def get_new_connection(self, conn_params):
            conn_params["application_name"] = "subclass_selected"
            conn_params["dbname"] = "template1"
            return super().get_new_connection(conn_params)

    for pool in (False, True) if HEAD else (False,):
        w = Custom(config(pool, application_name="settings_selected"), alias="m2_" + str(pool))
        emit("subclass-params", pool=pool, **error(lambda: query(w,
             "SELECT current_database(), current_setting('application_name')")))
        close(w)
    class ParamsCustom(DatabaseWrapper):
        def get_connection_params(self):
            p = super().get_connection_params()
            p["application_name"] = "params_override"
            return p
    w = ParamsCustom(config(HEAD), alias="m2_params")
    emit("deterministic-params-override", **error(lambda: query(w, "SHOW application_name")))
    close(w)
    class TimezoneCustom(DatabaseWrapper):
        def ensure_timezone(self):
            with self.connection.cursor() as c:
                c.execute("SET TIME ZONE 'Asia/Tokyo'")
            return True
    w = TimezoneCustom(config(), alias="m2_timezone")
    emit("timezone-hook", **error(lambda: query(w, "SHOW TIMEZONE")))
    close(w)
    class RoleCustom(DatabaseWrapper):
        called = False
        def ensure_role(self):
            self.called = True
            with self.connection.cursor() as c:
                c.execute('SET ROLE r4v17914_hook')
            return True
    with admin() as a:
        a.execute('CREATE ROLE r4v17914_hook')
    try:
        w = RoleCustom(config(), alias="m2_role")
        result = error(lambda: query(w, 'SELECT current_user, 1'))
        emit("role-hook", called=w.called, **result)
        close(w)
    finally:
        with admin() as a:
            a.execute('DROP ROLE r4v17914_hook')
    w = DatabaseWrapper(config(), alias='m2_role_call')
    query(w)
    emit('direct-role-method', **error(lambda: w.ensure_role()))
    close(w)


def m3():
    connections.databases["default"]["OPTIONS"] = {"pool": dict(POOL)} if HEAD else {}
    created = []
    def first_thread():
        w = connections["default"]
        created.append(query(w, "SELECT pg_backend_pid(), current_setting('TimeZone')"))
        w.close()
    t = threading.Thread(target=first_thread)
    t.start()
    t.join()
    emit("creator", value=created, main_initialized=len(connections.all(initialized_only=True)))
    with override_settings(TIME_ZONE="Africa/Nairobi"):
        w = connections["default"]
        if HEAD:
            p = w.pool
            p.open()
            p.wait()
            raw = p.getconn()
            previous_pid = raw.info.backend_pid
            raw.close()
            p.putconn(raw)
            emit("replace-session", previous_pid=previous_pid)
        emit("fresh-thread-override", expected=w.timezone_name,
             **error(lambda: query(w, "SELECT pg_backend_pid(), current_setting('TimeZone')")))
        emit("local-hour", expected="03", **error(lambda: query(w,
             "SELECT to_char('2024-01-01 00:00:00+00'::timestamptz, 'HH24')")))
        if HEAD:
            emit("callback-owner", timezone=p._configure.__self__.timezone_name)
        w.close()
    with override_settings(TIME_ZONE="Africa/Nairobi"):
        emit("current-thread-control", expected=connections["default"].timezone_name,
             **error(lambda: query(connections["default"], "SHOW TIMEZONE")))
        connections["default"].close()
    w = DatabaseWrapper(config(HEAD), alias="m3_state")
    with w.cursor() as c:
        c.execute("SET TIME ZONE 'Asia/Tokyo'")
    w.close()
    emit("borrower-state", **error(lambda: query(w, "SHOW TIMEZONE")))
    close(w)
    if HEAD:
        connections["default"].close_pool()


def m4():
    if not HEAD:
        emit("close-pool", available=False, reason="No close_pool or registry at base")
        w = DatabaseWrapper(config(), alias="m4_direct")
        emit("direct-ensure-timezone", **error(w.ensure_timezone))
        return
    first = DatabaseWrapper(config(True), alias="m4_race")
    p = first.pool
    p.open()
    p.wait()
    entered = threading.Event()
    both_entered = threading.Event()
    release = threading.Event()
    arrivals = []
    original = p.close
    def paused_close(*args, **kwargs):
        arrivals.append(True)
        entered.set()
        if len(arrivals) == 2:
            both_entered.set()
        release.wait(timeout=3)
        original(*args, **kwargs)
    results = []
    def worker():
        w = DatabaseWrapper(config(True), alias="m4_race")
        results.append(error(w.ensure_timezone))
    with patch.object(p, "close", paused_close):
        ts = [threading.Thread(target=worker) for _ in range(2)]
        ts[0].start()
        entered.wait(timeout=3)
        ts[1].start()
        both_entered.wait(timeout=0.1)
        release.set()
        for t in ts: t.join()
    emit("two-closers", results=results, injection="Pause inside real pool.close", close_calls=len(arrivals))
    emit("dependency-repeated-close", first=error(p.close), second=error(p.close))
    first = DatabaseWrapper(config(True), alias="m4_checkout")
    p = first.pool
    p.open()
    p.wait()
    closed = threading.Event()
    checkout_done = threading.Event()
    original = p.close
    def paused_after_close(*args, **kwargs):
        original(*args, **kwargs)
        closed.set()
        checkout_done.wait(timeout=3)
    def closer():
        w = DatabaseWrapper(config(True), alias="m4_checkout")
        results.append(error(w.ensure_timezone))
    with patch.object(p, "close", paused_after_close):
        t = threading.Thread(target=closer)
        t.start()
        closed.wait(timeout=3)
        w = DatabaseWrapper(config(True), alias="m4_checkout")
        emit("checkout-during-close", **error(w.connect))
        checkout_done.set()
        t.join()
    emit("after-close-retry", **error(lambda: query(w)))
    close(w)
    w = DatabaseWrapper(config(True), alias="m4_lazy")
    emit("no-pool-before-close", registered=w.alias in w._connection_pools)
    count = []
    old = w.get_connection_params
    def counted():
        count.append(True)
        return old()
    w.get_connection_params = counted
    emit("lazy-cleanup", **error(w.close_pool), params_calls=len(count))


def p1():
    name = "r4v17914_p1"
    with admin() as a:
        a.execute('DROP DATABASE IF EXISTS "' + name + '" WITH (FORCE)')
        a.execute('CREATE DATABASE "' + name + '"')
    try:
        for pool in (False, True) if HEAD else (False,):
            with admin() as a:
                a.execute('DROP DATABASE IF EXISTS "' + name + '" WITH (FORCE)')
                a.execute('CREATE DATABASE "' + name + '"')
            settings_dict = config(pool)
            settings_dict["NAME"] = name
            connections.databases["default"] = settings_dict
            if hasattr(connections._connections, "default"):
                del connections["default"]
            w = connections["default"]
            original = DatabaseWrapper.get_new_connection
            def no_maintenance(self, params):
                if self.alias == NO_DB_ALIAS:
                    raise psycopg.OperationalError("injected postgres database unavailable")
                return original(self, params)
            with patch.object(DatabaseWrapper, "get_new_connection", no_maintenance):
                with warnings.catch_warnings(record=True) as caught:
                    emit("fallback-drop", pool=pool, **error(lambda: w.creation._destroy_test_db(name, 0)))
                    emit("fallback-warning", messages=[str(x.message) for x in caught])
            close(w)
        w = connections["default"]
        with admin() as a:
            a.execute('DROP DATABASE IF EXISTS "' + name + '" WITH (FORCE)')
            a.execute('CREATE DATABASE "' + name + '"')
        emit("normal-maintenance-drop", **error(lambda: w.creation._destroy_test_db(name, 0)))
        close(w)
    finally:
        with admin() as a:
            a.execute('DROP DATABASE IF EXISTS "' + name + '" WITH (FORCE)')


if __name__ == "__main__":
    emit("identity", django=django.get_version(), head=HEAD)
    {"M1": m1, "M2": m2, "M3": m3, "M4": m4, "P1": p1}[GROUP]()
