from django.apps import AppConfig


class TConfig(AppConfig):
    name = "t"

    def ready(self):
        from . import checks  # noqa: F401
