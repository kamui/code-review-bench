"""Settings of the small project the manage.py commands of probe M1 run in.

The database OPTIONS come from the environment so that one file serves every run.
"""
import json
import os

SECRET_KEY = "m1-probe"
DEBUG = True
ALLOWED_HOSTS = ["*"]
ROOT_URLCONF = "m1proj.urls"
INSTALLED_APPS = ["django.contrib.contenttypes"]
USE_TZ = True
DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"
_options = json.loads(os.environ.get("M1_OPTIONS", "{}"))
_extra = _options.pop("_settings", {})
DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.postgresql",
        "NAME": "rr_db",
        "USER": "postgres",
        "HOST": os.environ["M1_HOST"],
        "PORT": os.environ["M1_PORT"],
        "OPTIONS": _options,
        **_extra,
    }
}
