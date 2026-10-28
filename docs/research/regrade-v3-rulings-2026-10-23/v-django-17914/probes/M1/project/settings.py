import os

SECRET_KEY = "probe"
USE_TZ = True
INSTALLED_APPS = ["django.contrib.contenttypes", "t"]
DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.postgresql",
        "NAME": "m1_app",
        "USER": "postgres",
        "HOST": os.environ["PROBE_PGHOST"],
        "PORT": os.environ["PROBE_PGPORT"],
        "OPTIONS": {"pool": {"min_size": 2, "max_size": 4, "timeout": 5}}
        if os.environ.get("PROBE_POOL") == "1"
        else {},
        "TEST": {"NAME": "test_m1_app"},
    }
}
TEST_RUNNER = os.environ.get("PROBE_RUNNER", "django.test.runner.DiscoverRunner")
