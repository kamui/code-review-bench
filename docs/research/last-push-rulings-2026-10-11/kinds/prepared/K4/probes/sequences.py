"""K4: every short path of one visitor through servers that hold different signing keys.

Usage: python sequences.py <django checkout> > outcomes-<label>.json

Four kinds of server, by settings:
  O  SECRET_KEY = old, SECRET_KEY_FALLBACKS = []          not yet rotated
  N  SECRET_KEY = new, SECRET_KEY_FALLBACKS = [old]       rotated as the documentation says
  P  SECRET_KEY = old, SECRET_KEY_FALLBACKS = [new]       given the new key ahead of the switch
  X  SECRET_KEY = new, SECRET_KEY_FALLBACKS = []          rotated, old key already removed
The servers are settings contexts used one after another in one process. They share one real
session store, as servers behind a load balancer share a cache or a database.

A visitor signs in on one server (O or N), puts a value in the session, and then makes one,
two or three more requests, each on any of the four kinds. For each request the script
records whether the visitor is signed in and whether the session value is still there.
Stock User, ModelBackend, SessionMiddleware, login() and get_user(); nothing is patched.
"""
import itertools
import json
import logging
import shutil
import sys
import tempfile

checkout = sys.argv[1]
sys.path.insert(0, checkout)

import django
from django.conf import settings

session_files = tempfile.mkdtemp(prefix="k4-sessions-")
settings.configure(
    SECRET_KEY="old-probe-key",
    SECRET_KEY_FALLBACKS=[],
    INSTALLED_APPS=[
        "django.contrib.auth",
        "django.contrib.contenttypes",
        "django.contrib.sessions",
    ],
    DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"}},
    CACHES={"default": {"BACKEND": "django.core.cache.backends.locmem.LocMemCache"}},
    PASSWORD_HASHERS=["django.contrib.auth.hashers.MD5PasswordHasher"],
    SESSION_FILE_PATH=session_files,
    USE_TZ=True,
)
django.setup()

from django.contrib.auth import get_user, login
from django.contrib.auth.models import User
from django.contrib.sessions.middleware import SessionMiddleware
from django.contrib.sessions.models import Session
from django.core.cache import cache
from django.core.management import call_command
from django.http import HttpResponse
from django.test import RequestFactory, override_settings

logging.disable(logging.CRITICAL)
call_command("migrate", verbosity=0)
user = User.objects.create_user(username="pat", password="probe-password")
factory = RequestFactory()
SERVERS = {
    "O": ("old-probe-key", []),
    "N": ("new-probe-key", ["old-probe-key"]),
    "P": ("old-probe-key", ["new-probe-key"]),
    "X": ("new-probe-key", []),
}
BACKENDS = ["cache", "db", "cached_db", "file", "signed_cookies"]


def request(cookie, server, backend, action):
    key, fallbacks = SERVERS[server]
    with override_settings(
        SECRET_KEY=key,
        SECRET_KEY_FALLBACKS=fallbacks,
        SESSION_ENGINE=f"django.contrib.sessions.backends.{backend}",
    ):
        req = factory.get("/")
        if cookie:
            req.COOKIES[settings.SESSION_COOKIE_NAME] = cookie
        middleware = SessionMiddleware(lambda request: HttpResponse())
        middleware.process_request(req)
        if action == "login":
            login(req, user, backend="django.contrib.auth.backends.ModelBackend")
            req.session["cart"] = "3 items"
            signed_in = True
        else:
            signed_in = get_user(req).is_authenticated
        cart = req.session.get("cart")
        response = middleware.process_response(req, HttpResponse())
        outgoing = response.cookies.get(settings.SESSION_COOKIE_NAME)
        return (outgoing.value if outgoing else cookie), {"signed_in": signed_in, "cart": cart is not None}


outcomes = {}
for backend in BACKENDS:
    for start in ["O", "N"]:
        for length in (1, 2, 3):
            for path in itertools.product("ONPX", repeat=length):
                cache.clear()
                Session.objects.all().delete()
                cookie, _ = request(None, start, backend, "login")
                steps = []
                for server in path:
                    cookie, seen = request(cookie, server, backend, "read")
                    steps.append(seen)
                outcomes[f"{backend} | sign in on {start} | then {' '.join(path)}"] = steps
shutil.rmtree(session_files, ignore_errors=True)
json.dump({"django": django.get_version(), "outcomes": outcomes}, sys.stdout, indent=0, sort_keys=True)
