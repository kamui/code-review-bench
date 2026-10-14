import os

from .base import *

ALLOWED_HOSTS = ["*"]

DEBUG = True

X_FRAME_OPTIONS = "SAMEORIGIN"

# CSRF settings for development with ngrok
CSRF_TRUSTED_ORIGINS = [
    "https://anomaly.ngrok.app",
]

DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.postgresql_psycopg2",
        "NAME": os.environ.get("POSTGRES_DB"),
        "USER": os.environ.get("POSTGRES_USER"),
        "PASSWORD": os.environ.get("POSTGRES_PASSWORD"),
        "HOST": os.environ.get("POSTGRES_HOST"),
        "PORT": os.environ.get("POSTGRES_PORT"),
        "OPTIONS": {
            "pool": True,
        },
    },
}

if DEBUG:
    INSTALLED_APPS += [
        "django_browser_reload",
    ]

    MIDDLEWARE += [
        "django_browser_reload.middleware.BrowserReloadMiddleware",
    ]
