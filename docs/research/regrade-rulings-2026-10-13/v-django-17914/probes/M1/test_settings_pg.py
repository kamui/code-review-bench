"""Settings for running Django's own test suite against the private PostgreSQL server of the probes."""
import os

DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.postgresql",
        "NAME": "rr_db",
        "USER": "postgres",
        "HOST": os.environ["M1_HOST"],
        "PORT": os.environ["M1_PORT"],
    },
    "other": {
        "ENGINE": "django.db.backends.postgresql",
        "NAME": "rr_db_other",
        "USER": "postgres",
        "HOST": os.environ["M1_HOST"],
        "PORT": os.environ["M1_PORT"],
    },
}
SECRET_KEY = "django_tests_secret_key"
PASSWORD_HASHERS = ["django.contrib.auth.hashers.MD5PasswordHasher"]
DEFAULT_AUTO_FIELD = "django.db.models.AutoField"
USE_TZ = False
