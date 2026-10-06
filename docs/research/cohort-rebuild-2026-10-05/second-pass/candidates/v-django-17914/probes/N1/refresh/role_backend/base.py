from django.db.backends.postgresql.base import DatabaseWrapper as PostgreSQLWrapper


class DatabaseWrapper(PostgreSQLWrapper):
    role_calls = 0

    def ensure_role(self):
        self.role_calls += 1
        with self.connection.cursor() as cursor:
            cursor.execute('SET ROLE dossier_application')
        return True
