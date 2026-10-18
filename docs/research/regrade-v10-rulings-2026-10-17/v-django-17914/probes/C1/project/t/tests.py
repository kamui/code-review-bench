import os
import time

from django.db import connection
from django.test import TransactionTestCase

from .models import Item


def log(text):
    with open(os.environ["PROBE_LOG"], "a") as f:
        f.write(text + "\n")


class Mixin:
    def test_isolated(self):
        me = type(self).__name__
        pool = getattr(connection, "pool", None)
        if pool is not None:
            alive = sum(t.is_alive() for t in pool._workers)
            idle = sorted(c.info.backend_pid for c in list(pool._pool))
            pool_info = f"pool {pool.name} dbname={pool.kwargs['dbname']} worker threads alive={alive}/{len(pool._workers)} idle sessions at start={idle}"
        else:
            pool_info = "no pool"
        with connection.cursor() as cursor:
            cursor.execute("SELECT current_database(), pg_backend_pid()")
            database, backend = cursor.fetchone()
        log(f"worker pid={os.getpid()} {me}: settings NAME={connection.settings_dict['NAME']} current_database()={database} server process={backend}; {pool_info}")
        Item.objects.create(owner=me)
        time.sleep(1.5)
        owners = sorted(Item.objects.values_list("owner", flat=True))
        log(f"worker pid={os.getpid()} {me}: rows visible after 1.5 s: {owners}")
        self.assertEqual(owners, [me])


class TestA(Mixin, TransactionTestCase):
    pass


class TestB(Mixin, TransactionTestCase):
    pass


class TestC(Mixin, TransactionTestCase):
    pass


class TestD(Mixin, TransactionTestCase):
    pass
