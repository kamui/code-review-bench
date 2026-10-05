import os

from django.apps import AppConfig
from django.db.models.signals import post_migrate


def count_items(sender, **kwargs):
    """A post_migrate handler that ignores the `using` argument, as many do."""
    if os.environ.get("PROBE_POST_MIGRATE") != "1" or kwargs.get("using") != "other":
        return
    from django.db import connection

    from .models import Item

    Item.objects.count()
    with connection.cursor() as cursor:
        cursor.execute("SELECT current_database()")
        name = cursor.fetchone()[0]
    with open(os.environ["PROBE_LOG"], "a") as f:
        f.write(
            f"process pid={os.getpid()} post_migrate handler for alias 'other' "
            f"queried the default alias on {name}\n"
        )


class TConfig(AppConfig):
    name = "t"

    def ready(self):
        from . import checks  # noqa: F401

        post_migrate.connect(count_items, sender=self)
