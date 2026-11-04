"""Evaluating connection.features.is_postgresql_15 before the test database is
set up: which database does the alias's pool keep after NAME is switched the way
create_test_db() switches it?

No PostgreSQL server is used. HOST is a closed local port, so the connection
attempt fails after the pool is built; the pool object and its connection
arguments are still inspected.

Usage: <venv>/bin/python -I probe.py <django tree at head>
"""
import logging
import sys

logging.getLogger("psycopg.pool").setLevel(logging.CRITICAL)

tree = sys.argv[1]
sys.path.insert(0, tree)

import django
from django.conf import settings

settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "app",
            "HOST": "127.0.0.1",
            "PORT": "1",
            "OPTIONS": {"pool": {"timeout": 1}},
        }
    },
)
django.setup()

from django.db import connections

connection = connections["default"]

try:
    connection.features.is_postgresql_15
except Exception as e:
    print(f"is_postgresql_15: {type(e).__name__}: {str(e).splitlines()[0]}")
pool = type(connection)._connection_pools.get("default")
print("pool registered for 'default' after the feature check:", pool is not None)
print("pool connects to dbname:", pool.kwargs["dbname"])

# What create_test_db() does to the wrapper before migrating (base/creation.py).
connection.close()
settings.DATABASES["default"]["NAME"] = "test_app"
connection.settings_dict["NAME"] = "test_app"

print("wrapper NAME now:", connection.settings_dict["NAME"])
print("get_connection_params() dbname now:", connection.get_connection_params()["dbname"])
print("same pool object:", connection.pool is pool)
print("pool still connects to dbname:", connection.pool.kwargs["dbname"])
connection.close_pool()
