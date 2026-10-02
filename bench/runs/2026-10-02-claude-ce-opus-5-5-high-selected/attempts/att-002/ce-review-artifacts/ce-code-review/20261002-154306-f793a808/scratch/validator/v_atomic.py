import os, sys, django
from django.conf import settings
here = os.path.dirname(os.path.abspath(__file__))
settings.configure(DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": os.path.join(here, "v.sqlite3")}})
django.setup()
from django.db import connection, transaction
print("django from:", os.path.dirname(django.__file__))
connection.set_autocommit(False)
with transaction.atomic():
    connection.close()
print("after block: connection=%r in_atomic_block=%r closed_in_transaction=%r atomic_blocks=%r" % (
    connection.connection, connection.in_atomic_block, connection.closed_in_transaction, connection.atomic_blocks))
for step in ("cursor", "close+cursor", "close_if_unusable_or_obsolete+cursor"):
    try:
        if step.startswith("close+"): connection.close()
        if step.startswith("close_if"): connection.close_if_unusable_or_obsolete()
        with connection.cursor() as c:
            c.execute("SELECT 1"); print(step, "-> ok", c.fetchone())
    except Exception as e:
        print(step, "->", type(e).__name__, e)
