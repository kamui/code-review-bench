"""A database-tagged system check that runs one query when PROBE_DB_CHECK=1.

The test runner runs database-tagged checks after it has created and cloned the test databases and before it forks
the parallel workers.
"""
import os

from django.core import checks
from django.db import connections


@checks.register(checks.Tags.database)
def query_check(app_configs=None, databases=None, **kwargs):
    if os.environ.get("PROBE_DB_CHECK") != "1":
        return []
    for alias in databases or []:
        with connections[alias].cursor() as cursor:
            cursor.execute("SELECT current_database(), pg_backend_pid()")
            name, backend = cursor.fetchone()
        connections[alias].close()
        with open(os.environ["PROBE_LOG"], "a") as f:
            f.write(f"parent pid={os.getpid()}: database check queried {name} on server process {backend}, then close()\n")
    return []
