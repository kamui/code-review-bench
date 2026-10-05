import argparse
import importlib.metadata
import json
import logging
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument("clone")
parser.add_argument("--suppress-hash-upgrade", action="store_true")
args = parser.parse_args()
sys.path.insert(0, args.clone)

import django
from django.conf import settings

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
    USE_TZ=True,
)
django.setup()

from django.contrib.auth import HASH_SESSION_KEY, get_user, login
from django.contrib.auth.models import User
from django.contrib.sessions.middleware import SessionMiddleware
from django.core.cache import cache
from django.core.management import call_command
from django.http import HttpResponse
from django.test import RequestFactory, override_settings

logging.basicConfig(level=logging.WARNING, stream=sys.stdout)
call_command("migrate", verbosity=0)
user = User.objects.create_user(username="pat", password="probe-password")
factory = RequestFactory()
old = ("old-probe-key", [])
new = ("new-probe-key", ["old-probe-key"])
prepared_old = ("old-probe-key", ["new-probe-key"])
hashes = {}
for name, config in [("old", old), ("new", new)]:
    with override_settings(SECRET_KEY=config[0], SECRET_KEY_FALLBACKS=config[1]):
        hashes[name] = user.get_session_auth_hash()


def request(cookie, config, backend, action="read"):
    with override_settings(
        SECRET_KEY=config[0], SECRET_KEY_FALLBACKS=config[1], SESSION_ENGINE=backend
    ):
        req = factory.get("/")
        if cookie:
            req.COOKIES[settings.SESSION_COOKIE_NAME] = cookie
        middleware = SessionMiddleware(lambda request: HttpResponse())
        middleware.process_request(req)
        loaded_hash = req.session.get(HASH_SESSION_KEY)
        loaded_cart = req.session.get("cart")
        if action == "login":
            login(req, user, backend="django.contrib.auth.backends.ModelBackend")
            req.session["cart"] = "3 items"
            current = user
        else:
            current = get_user(req)
            if args.suppress_hash_upgrade and loaded_hash and current.is_authenticated:
                req.session[HASH_SESSION_KEY] = loaded_hash
        body = {
            "signed_in": current.is_authenticated,
            "cart": req.session.get("cart"),
        }
        response = middleware.process_response(req, HttpResponse(json.dumps(body)))
        outgoing = response.cookies.get(settings.SESSION_COOKIE_NAME)
        next_cookie = outgoing.value if outgoing else cookie
        outcome = {
            "response": response.content.decode(),
            "status": response.status_code,
            "loaded_cart": loaded_cart,
            "loaded_hash": next((k for k, v in hashes.items() if v == loaded_hash), None),
            "stored_hash": next(
                (k for k, v in hashes.items() if v == req.session.get(HASH_SESSION_KEY)),
                None,
            ),
            "cookie": "deleted" if outgoing and not outgoing.value else (
                "replaced" if outgoing and next_cookie != cookie else "unchanged"
            ),
        }
        return next_cookie, outcome


print("commit:", subprocess.check_output(
    ["git", "-C", args.clone, "rev-parse", "HEAD"], text=True
).strip())
print("Python:", sys.version.split()[0], "Django:", django.get_version())
print("dependencies:", {p: importlib.metadata.version(p) for p in ["asgiref", "sqlparse"]})
print("hash upgrade suppressed:", args.suppress_hash_upgrade)
print("Nodes are sequential settings contexts sharing the real session backend.")
for backend in ["django.contrib.sessions.backends.cache", "django.contrib.sessions.backends.db"]:
    for name, nodes in [
        ("no rotation", [old, old]),
        ("one-phase rolling rotation", [new, old, new]),
        ("both keys distributed first", [new, prepared_old, new, prepared_old]),
        ("all nodes rotated", [new, new]),
    ]:
        cache.clear()
        print("\nCASE", backend, name)
        cookie, initial = request(None, old, backend, "login")
        print("initial old-node login:", json.dumps(initial, sort_keys=True))
        for index, node in enumerate(nodes, 1):
            cookie, result = request(cookie, node, backend)
            print("read", index, "node", "new" if node[0] == new[0] else "old",
                  "fallbacks", len(node[1]), json.dumps(result, sort_keys=True))
    print("\nCASE", backend, "fresh login on new node, then read on old node")
    cookie, initial = request(None, new, backend, "login")
    print("new-node login:", json.dumps(initial, sort_keys=True))
    cookie, result = request(cookie, old, backend)
    print("old-node read:", json.dumps(result, sort_keys=True))
