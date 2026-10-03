"""Offline lifecycle probes; no PostgreSQL connection or clone writes."""
import copy
import json
import threading
from types import SimpleNamespace
from unittest import mock

from django.conf import settings

settings.configure(
    DATABASES={"default": {"ENGINE": "django.db.backends.postgresql", "NAME": "application", "OPTIONS": {"pool": True}}},
    USE_TZ=True,
    TIME_ZONE="UTC",
    INSTALLED_APPS=[],
)

import django
django.setup()

from django.db import connections
from django.db.backends.postgresql.base import DatabaseWrapper
from django.db.transaction import atomic
from psycopg_pool import ConnectionPool, PoolTimeout

TEMPLATE = copy.deepcopy(connections["default"].settings_dict)


def wrapper(alias, pool=True):
    config = copy.deepcopy(TEMPLATE)
    config["OPTIONS"]["pool"] = pool
    return DatabaseWrapper(config, alias)


class FakePool:
    instances = []
    check_connection = staticmethod(lambda conn: None)

    def __init__(self, **kwargs):
        self.kwargs = kwargs["kwargs"]
        self.configure = kwargs["configure"]
        self.closed = False
        self.returned = []
        self.__class__.instances.append(self)

    def close(self):
        self.closed = True

    def putconn(self, conn):
        self.returned.append(conn)


def emit(name, **values):
    print(json.dumps({"probe": name, **values}, sort_keys=True))


with mock.patch("psycopg_pool.ConnectionPool", FakePool):
    # The documented mapping contract includes an empty dict.
    observed = {}
    for idx, option in enumerate([False, True, {}, {"min_size": 0}]):
        conn = wrapper("option_%s" % idx, option)
        observed[repr(option)] = conn.pool is not None
    emit("option_contract", enabled=observed)

    # Closing an unused wrapper should have no resource-acquisition effect.
    conn = wrapper("unused")
    before = len(FakePool.instances)
    conn.ensure_timezone()
    emit("timezone_on_unused_wrapper", allocations=len(FakePool.instances) - before,
         registered=conn.alias in conn._connection_pools)

    # Return a checked-out connection after the associated pool was invalidated.
    conn = wrapper("leased")
    owner = conn.pool
    raw = SimpleNamespace(_pool=owner)
    conn.connection = raw
    conn.close_pool()
    before = len(FakePool.instances)
    conn.close()
    emit("return_after_invalidation", allocations=len(FakePool.instances) - before,
         returned_to_original=owner.returned == [raw],
         replacement_registered=conn.alias in conn._connection_pools)

    # Run real test-database creation orchestration with only I/O mocked.
    conn = connections["default"]
    original_pool = conn.pool
    migrations = []

    def record_command(name, **kwargs):
        migrations.append({"command": name, "wrapper_db": conn.settings_dict["NAME"],
                           "pool_db": conn.pool.kwargs["dbname"],
                           "original_pool": conn.pool is original_pool})

    with mock.patch.object(conn.creation, "_create_test_db"), \
         mock.patch("django.core.management.call_command", side_effect=record_command), \
         mock.patch.object(conn, "ensure_connection"):
        conn.creation.create_test_db(verbosity=0, serialize=False)
    emit("test_db_transition", commands=migrations)

    # A normal teardown close is safe only if registry removal is atomic.
    conn = wrapper("concurrent")
    pool = conn.pool
    barrier = threading.Barrier(2)
    pool.close = lambda: barrier.wait(timeout=2)
    errors = []

    def close_same_pool():
        try:
            conn.close_pool()
        except Exception as exc:
            errors.append(type(exc).__name__ + ": " + str(exc))

    threads = [threading.Thread(target=close_same_pool) for _ in range(2)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(timeout=3)
    emit("concurrent_close", errors=errors)

    # Exercise the new generic guard using an actual atomic enter/exit cycle.
    conn = wrapper("atomic")
    conn.connection = SimpleNamespace(_pool=conn.pool, autocommit=True)
    conn.autocommit = True
    error = None
    with mock.patch("django.db.transaction.get_connection", return_value=conn):
        with atomic():
            conn.close()
            try:
                conn.ensure_connection()
            except Exception as exc:
                error = type(exc).__name__ + ": " + str(exc)
    emit("closed_atomic_guard", error=error, in_atomic_block=conn.in_atomic_block)

DatabaseWrapper._connection_pools.clear()

# Check the callback's real helper chain; mock only the physical driver object
# and stop at the forbidden wrapper acquisition boundary.
conn = wrapper("callback")
conn.settings_dict["OPTIONS"]["assume_role"] = "app_role"
raw = mock.MagicMock()
raw.info.parameter_status.return_value = "UTC"
with mock.patch.object(conn, "ensure_connection", side_effect=RuntimeError("wrapper acquisition reached")) as acquisition:
    try:
        conn._configure_connection(raw)
    except RuntimeError as exc:
        emit("role_callback_boundary", error=str(exc), wrapper_acquisitions=acquisition.call_count)

# Exercise an actual installed psycopg_pool worker with a fake physical driver.
# The absence of a database means that timeout is purely callback re-entry.
class PhysicalConnection:
    def __init__(self):
        self.info = SimpleNamespace(parameter_status=lambda name: "UTC")
        self.pgconn = SimpleNamespace(transaction_status=0)

    @classmethod
    def connect(cls, *args, **kwargs):
        return cls()

    def cursor(self):
        return mock.MagicMock()

    def close(self):
        pass


conn = wrapper("worker", {"connection_class": PhysicalConnection, "min_size": 1,
                          "max_size": 1, "timeout": 0.1, "num_workers": 1,
                          "reconnect_timeout": 0.2})
conn.settings_dict["OPTIONS"]["assume_role"] = "app_role"
pool = conn.pool
pool.open()
try:
    conn.connect()
except PoolTimeout as exc:
    emit("real_pool_role_worker", error=type(exc).__name__, connection_assigned=conn.connection is not None,
         requests=pool.get_stats().get("requests_num"))
finally:
    pool.close(timeout=1)
    DatabaseWrapper._connection_pools.clear()

conn = wrapper("worker_baseline", {"connection_class": PhysicalConnection,
                                  "min_size": 1, "max_size": 1, "timeout": 0.1})
pool = conn.pool
pool.open()
try:
    raw = pool.getconn()
    emit("real_pool_worker_without_role", acquired=isinstance(raw, PhysicalConnection))
    pool.putconn(raw)
finally:
    pool.close(timeout=1)
    DatabaseWrapper._connection_pools.clear()

# If the retained wrapper already has a connection, callback re-entry instead
# hits Django's existing thread ownership enforcement.
conn = wrapper("connected_callback")
conn.settings_dict["OPTIONS"]["assume_role"] = "app_role"
conn.connection = mock.MagicMock()
conn.connection.info.parameter_status.return_value = "UTC"
errors = []

def configure_in_worker():
    try:
        conn._configure_connection(conn.connection)
    except Exception as exc:
        errors.append(type(exc).__name__ + ": " + str(exc))

with mock.patch.object(conn, "create_cursor", return_value=mock.MagicMock()):
    thread = threading.Thread(target=configure_in_worker)
    thread.start()
    thread.join(timeout=2)
emit("connected_role_callback_thread", errors=errors)
conn.close_pool()

conn = wrapper("driver_policy")
with mock.patch("django.db.backends.postgresql.base.is_psycopg3", False):
    try:
        conn.get_connection_params()
    except Exception as exc:
        emit("psycopg2_policy_branch", error=type(exc).__name__ + ": " + str(exc))
