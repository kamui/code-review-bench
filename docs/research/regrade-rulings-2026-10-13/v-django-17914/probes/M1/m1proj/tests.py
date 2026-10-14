from django.db import connection
from django.test import TestCase


class OneQuery(TestCase):
    def test_select(self):
        with connection.cursor() as cursor:
            cursor.execute("SELECT 1")
            self.assertEqual(cursor.fetchone(), (1,))
