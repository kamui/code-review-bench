"""Settings that turn the pool on with a non-zero CONN_MAX_AGE from the start:
what does the first query raise?

No PostgreSQL server is used. The host is a closed local port, so a connection
attempt that got as far as the network would fail with "connection refused".

Usage: <venv>/bin/python -I probe.py <django tree> <label> <CONN_MAX_AGE>
"""
import sys

tree, label, age = sys.argv[1], sys.argv[2], int(sys.argv[3])
sys.path.insert(0, tree)

import logging

logging.getLogger("psycopg.pool").setLevel(logging.CRITICAL)

import django
from django.conf import settings

settings.configure(
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.postgresql",
            "NAME": "app",
            "HOST": "127.0.0.1",
            "PORT": "1",
            "CONN_MAX_AGE": age,
            "OPTIONS": {"pool": {"timeout": 1}},
        }
    },
)
django.setup()

from django.db import connection

try:
    with connection.cursor() as cursor:
        cursor.execute("SELECT 1")
    print(f"[{label}] CONN_MAX_AGE={age}: query ran")
except Exception as e:
    print(f"[{label}] CONN_MAX_AGE={age}: {type(e).__module__}.{type(e).__name__}: {str(e).splitlines()[0]}")
