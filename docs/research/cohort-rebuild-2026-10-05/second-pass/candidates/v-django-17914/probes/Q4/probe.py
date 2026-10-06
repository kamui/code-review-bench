import os
from django.conf import settings

settings.configure(DATABASES={"default": {
    "ENGINE": "django.db.backends.sqlite3", "NAME": os.environ["DOSSIER_SQLITE_PATH"],
}})
import django
django.setup()
from django.db import connection, transaction

print("revision:", os.environ["DOSSIER_REVISION"])

def state(label):
    print(label, "connection is None:", connection.connection is None,
          "in_atomic_block:", connection.in_atomic_block,
          "closed_in_transaction:", connection.closed_in_transaction)

def query(label):
    try:
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
            print(label, "OK", cursor.fetchone())
    except Exception as exc:
        print(label, type(exc).__module__ + "." + type(exc).__name__ + ":", str(exc))

for autocommit in [True, False]:
    connection.connect()
    connection.set_autocommit(autocommit)
    print("autocommit:", autocommit)
    with transaction.atomic():
        connection.close()
        state("after close inside block")
        query("query inside block")
    state("after block")
    query("first query after block")
    query("second query after block")
    connection.close()
    query("query after extra close")
    connection.connect()
    query("query after direct connect")
    connection.set_autocommit(True)
    connection.close()
