"""With the test settings' default alias pooled, does the wrapper that
test_connect_no_is_usable_checks connects through use the pool?

No PostgreSQL server is used: the probe only builds the wrappers and asks each
for its pool, which does not connect (Django builds the pool with open=False).

Usage: <venv>/bin/python -I probe.py <django tree at head>
"""
import inspect
import sys

tree = sys.argv[1]
sys.path.insert(0, tree)
sys.path.insert(0, tree + "/tests")

import django
from django.conf import settings

settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "app",
            "OPTIONS": {"pool": True},
        },
        "other": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "other",
            "OPTIONS": {"pool": True},
        },
    },
)
django.setup()

from django.db import connection
from backends.postgresql import tests as pg_tests

src = inspect.getsource(pg_tests.Tests.test_connect_no_is_usable_checks)
print("test body:")
print(src.rstrip())
print()
print("default wrapper pool:", type(connection.pool).__name__)
w = pg_tests.no_pool_connection()
print("no_pool_connection() OPTIONS['pool']:", w.settings_dict["OPTIONS"]["pool"])
print("no_pool_connection() pool:", w.pool)
connection.close_pool()
