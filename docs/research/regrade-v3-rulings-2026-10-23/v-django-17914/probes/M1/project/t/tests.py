from django.db import connection
from django.test import TestCase


class T(TestCase):
    def test_query(self):
        with connection.cursor() as cur:
            cur.execute("SELECT current_database()")
            print("\n[test] current_database() =", cur.fetchone()[0])
