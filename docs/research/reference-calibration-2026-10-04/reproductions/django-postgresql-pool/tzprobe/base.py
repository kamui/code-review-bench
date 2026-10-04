"""A backend that subclasses the PostgreSQL wrapper and overrides ensure_timezone, as a third-party backend would."""
from django.db.backends.postgresql import base


class DatabaseWrapper(base.DatabaseWrapper):
    override_called = False

    def ensure_timezone(self):
        type(self).override_called = True
        return False
