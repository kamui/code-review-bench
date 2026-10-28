from django.db import connection
from django.test.runner import DiscoverRunner


class RecordingRunner(DiscoverRunner):
    """A custom runner that records the run in the application database after teardown."""

    def teardown_databases(self, old_config, **kwargs):
        super().teardown_databases(old_config, **kwargs)
        with connection.cursor() as cur:
            cur.execute("SELECT current_database()")
            print(f"[runner] after teardown_databases: NAME={connection.settings_dict['NAME']} current_database()={cur.fetchone()[0]}")
        connection.close()
