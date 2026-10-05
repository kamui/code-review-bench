import os
import time

from django.db import connection
from django.test import TransactionTestCase

from .models import Item


def log(text):
    with open(os.environ["PROBE_LOG"], "a") as f:
        f.write(text + "\n")


class IsolationMixin:
    """Each class writes its own rows, waits, and expects to see only its rows.

    Parallel workers are supposed to work on separate cloned databases.
    """

    # With two aliases configured, ask the runner to set up both.
    databases = "__all__" if os.environ.get("PROBE_TWO_DBS") == "1" else {"default"}

    def test_isolated(self):
        me = f"{type(self).__name__}"
        with connection.cursor() as cursor:
            cursor.execute("SELECT current_database(), pg_backend_pid()")
            database, backend = cursor.fetchone()
        pool = getattr(connection, "pool", None)
        pool_info = "no pool"
        if pool is not None:
            alive = sum(t.is_alive() for t in pool._workers)
            pool_info = (
                f"pool dbname={pool.kwargs['dbname']} "
                f"worker threads alive={alive}/{len(pool._workers)}"
            )
        log(
            f"worker pid={os.getpid()} {me}: settings NAME={connection.settings_dict['NAME']} "
            f"current_database()={database} backend={backend} {pool_info}"
        )
        Item.objects.create(owner=me)
        time.sleep(1.5)
        owners = sorted(Item.objects.values_list("owner", flat=True))
        log(f"worker pid={os.getpid()} {me}: rows visible after 1.5s: {owners}")
        self.assertEqual(owners, [me])


class TestA(IsolationMixin, TransactionTestCase):
    pass


class TestB(IsolationMixin, TransactionTestCase):
    pass


class TestC(IsolationMixin, TransactionTestCase):
    pass


class TestD(IsolationMixin, TransactionTestCase):
    pass
