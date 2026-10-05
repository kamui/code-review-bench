import json
import os
import signal
import threading
from django.conf import settings
from django.db.backends.postgresql.base import DatabaseWrapper

pool_supported = hasattr(DatabaseWrapper, "pool")
settings.configure(USE_TZ=True, DATABASES={"default": {
    "ENGINE": "django.db.backends.postgresql", "NAME": "postgres", "USER": "dossier",
    "HOST": "127.0.0.1", "PORT": "55439",
    "OPTIONS": {"pool": {"min_size": 1, "max_size": 1, "timeout": 2}} if pool_supported else {},
}})
import django
django.setup()
from django.db import connection, connections

print("revision:", os.environ["DOSSIER_REVISION"], "pool supported:", pool_supported)
with connection.cursor() as cursor:
    cursor.execute("SELECT pg_backend_pid()")
    parent_session = cursor.fetchone()[0]
connections.close_all()
print("parent server session:", parent_session)
print("parent threads:", [t.name for t in threading.enumerate()])
read_fd, write_fd = os.pipe()
pid = os.fork()
if pid == 0:
    os.close(read_fd)
    signal.alarm(8)
    result = {"threads": [t.name for t in threading.enumerate()]}
    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT pg_backend_pid()")
            result["server_session"] = cursor.fetchone()[0]
            result["same_session_as_parent"] = result["server_session"] == parent_session
        connection.close()
    except Exception as exc:
        result["error"] = type(exc).__name__ + ": " + str(exc)
    os.write(write_fd, json.dumps(result).encode())
    os.close(write_fd)
    os._exit(0)
os.close(write_fd)
print("child:", os.read(read_fd, 10000).decode())
os.close(read_fd)
print("child wait status:", os.waitpid(pid, 0)[1])
if pool_supported:
    connection.close_pool()
