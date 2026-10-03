import django
from django.conf import settings
settings.configure(DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": "/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-002/clone-work/ce-review-artifacts/ce-code-review/20261002-154306-f793a808/scratch/correctness/t1.sqlite3"}})
django.setup()
from django.db import connection, transaction
connection.set_autocommit(False)
with transaction.atomic():
    connection.close()
print("after exit: connection", connection.connection, "in_atomic_block", connection.in_atomic_block, "closed_in_transaction", connection.closed_in_transaction)
try:
    with connection.cursor() as c:
        c.execute("SELECT 1")
        print("reconnected OK", c.fetchone())
except Exception as e:
    print("FAILED:", type(e).__name__, e)
