import inspect
import os
from importlib.metadata import version, requires
from django.conf import settings

settings.configure(USE_TZ=True, DATABASES={"default": {
    "ENGINE": "django.db.backends.postgresql", "NAME": "postgres", "USER": "dossier",
    "HOST": "127.0.0.1", "PORT": "55439",
}})
import django
django.setup()
from django.db import connections
from django.db.backends.postgresql.base import DatabaseWrapper
from psycopg_pool import ConnectionPool

print("revision:", os.environ["DOSSIER_REVISION"])
print("psycopg:", version("psycopg"), "psycopg-pool:", version("psycopg-pool"))
print("psycopg pool extra:", [r for r in requires("psycopg") if "pool" in r])
print("constructor:", inspect.signature(ConnectionPool))
print("check_connection present:", hasattr(ConnectionPool, "check_connection"))
for pool_enabled, health in [(False, False), (True, False), (True, True)]:
    config = connections["default"].settings_dict.copy()
    config["CONN_HEALTH_CHECKS"] = health
    config["OPTIONS"] = {"pool": True} if pool_enabled else {}
    wrapper = DatabaseWrapper(config, alias="dependency_probe")
    try:
        with wrapper.cursor() as cursor:
            cursor.execute("SELECT 1")
            print("pool:", pool_enabled, "health:", health, "query:", cursor.fetchone())
    except Exception as exc:
        print("pool:", pool_enabled, "health:", health, type(exc).__module__ + "." + type(exc).__name__ + ":", str(exc))
    finally:
        try:
            wrapper.close()
            if hasattr(wrapper, "close_pool") and wrapper.alias in wrapper._connection_pools:
                wrapper.close_pool()
        except Exception as exc:
            print("cleanup:", type(exc).__name__ + ":", str(exc))
