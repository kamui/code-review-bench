"""An optional database-tagged system check that runs one query.

Django runs database-tagged checks from the test runner after the test
databases are created and cloned, and before the worker processes are forked.
Django's own MySQL backend does this kind of thing (it reads the server's SQL
mode in a database check).
"""
import os

from django.core import checks
from django.db import connections


@checks.register(checks.Tags.database)
def server_version_check(app_configs=None, databases=None, **kwargs):
    if os.environ.get("PROBE_DB_CHECK") != "1":
        return []
    for alias in databases or []:
        connection = connections[alias]
        with connection.temporary_connection() as cursor:
            cursor.execute("SELECT current_database()")
            name = cursor.fetchone()[0]
        with open(os.environ["PROBE_LOG"], "a") as f:
            f.write(f"parent pid={os.getpid()} database check ran a query on {name}\n")
    return []
