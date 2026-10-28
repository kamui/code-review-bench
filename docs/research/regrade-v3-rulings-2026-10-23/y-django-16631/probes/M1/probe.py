"""After SECRET_KEY is rotated with the old key in SECRET_KEY_FALLBACKS, two requests from one
signed-in browser carry the old session cookie at the same time. What happens to each, for each
session backend?

Usage: python probe.py <backend> <workdir> [save-every]
  backend: db | cache | cached_db | file | signed_cookies
  workdir: an empty scratch directory for the SQLite file and session files
  save-every: run with SESSION_SAVE_EVERY_REQUEST = True

Requests run in two threads. Events in the views fix the order: request 2 touches its session,
then request 1 runs to completion, then request 2 finishes.
"""
import sys
import threading

backend, workdir = sys.argv[1], sys.argv[2]
save_every = len(sys.argv) > 3 and sys.argv[3] == "save-every"

import django
from django.conf import settings

ENGINES = {
    "db": "django.contrib.sessions.backends.db",
    "cache": "django.contrib.sessions.backends.cache",
    "cached_db": "django.contrib.sessions.backends.cached_db",
    "file": "django.contrib.sessions.backends.file",
    "signed_cookies": "django.contrib.sessions.backends.signed_cookies",
}

settings.configure(
    DEBUG=False,
    SECRET_KEY="old-secret",
    ALLOWED_HOSTS=["*"],
    ROOT_URLCONF=__name__,
    INSTALLED_APPS=[
        "django.contrib.auth",
        "django.contrib.contenttypes",
        "django.contrib.sessions",
    ],
    MIDDLEWARE=[
        "django.contrib.sessions.middleware.SessionMiddleware",
        "django.contrib.auth.middleware.AuthenticationMiddleware",
    ],
    DATABASES={
        "default": {
            "ENGINE": "django.db.backends.sqlite3",
            "NAME": f"{workdir}/db.sqlite3",
        }
    },
    CACHES={"default": {"BACKEND": "django.core.cache.backends.locmem.LocMemCache"}},
    SESSION_ENGINE=ENGINES[backend],
    SESSION_FILE_PATH=workdir,
    SESSION_SAVE_EVERY_REQUEST=save_every,
    DEFAULT_AUTO_FIELD="django.db.models.AutoField",
    LOGGING_CONFIG=None,
)
django.setup()

from django.contrib.auth import login, logout
from django.contrib.auth.models import User
from django.core.management import call_command
from django.http import HttpResponse
from django.test import Client, override_settings
from django.urls import path

loaded = threading.Event()
go = threading.Event()


def whoami(request):
    return HttpResponse(f"user={request.user} cart={request.session.get('cart')}")


def add_to_cart(request):
    """Touches the session first, waits, then writes the cart. Never reads request.user."""
    cart = list(request.session.get("cart", []))
    loaded.set()
    go.wait(10)
    request.session["cart"] = cart + ["item added by request 2"]
    return HttpResponse(f"added; cart now {len(cart) + 1} items")


def add_then_whoami(request):
    """Touches the session first, waits, then reads request.user and writes the cart."""
    cart = list(request.session.get("cart", []))
    loaded.set()
    go.wait(10)
    user = str(request.user)
    request.session["cart"] = cart + ["item added by request 2"]
    return HttpResponse(f"user={user}; added")


def late_add_to_cart(request):
    """Touches the session only after request 1 has finished. Never reads request.user."""
    loaded.set()
    go.wait(10)
    cart = list(request.session.get("cart", []))
    request.session["cart"] = cart + ["item added by request 2"]
    return HttpResponse(f"added; cart now {len(cart) + 1} items")


def read_cart(request):
    """Touches the session first, waits, then answers. Never writes the session or reads request.user."""
    cart = list(request.session.get("cart", []))
    loaded.set()
    go.wait(10)
    return HttpResponse(f"read; cart has {len(cart)} items")


def broken(request):
    str(request.user)
    return HttpResponse("server error after reading who is signed in", status=500)


def sign_in(request):
    login(request, User.objects.get(username="pat"))
    request.session["cart"] = ["item 1", "item 2", "item 3"]
    return HttpResponse("signed in")


def sign_in_from_anonymous(request):
    login(request, User.objects.get(username="pat"))
    return HttpResponse("signed in")


def anonymous_cart(request):
    request.session["cart"] = ["item 1", "item 2", "item 3"]
    return HttpResponse("anonymous cart")


def sign_out(request):
    logout(request)
    return HttpResponse("signed out")


urlpatterns = [
    path("", whoami),
    path("add/", add_to_cart),
    path("add-then-whoami/", add_then_whoami),
    path("late-add/", late_add_to_cart),
    path("read/", read_cart),
    path("broken/", broken),
    path("login/", sign_in),
    path("logout/", sign_out),
    path("login-from-anonymous/", sign_in_from_anonymous),
    path("anonymous-cart/", anonymous_cart),
]

call_command("migrate", run_syncdb=True, verbosity=0)
User.objects.create_user("pat", password="pw")
NAME = settings.SESSION_COOKIE_NAME
ROTATED = dict(SECRET_KEY="new-secret", SECRET_KEY_FALLBACKS=["old-secret"])
UNROTATED = dict(SECRET_KEY="old-secret", SECRET_KEY_FALLBACKS=[])


def cookie_state(response):
    c = response.cookies.get(NAME)
    if c is None:
        return "no cookie change"
    return "cookie DELETED" if not c.value else "new cookie"


def show(label, response):
    body = response.content.decode()[:70].replace("\n", " ")
    print(f"    {label:44} HTTP {response.status_code} | {body} | {cookie_state(response)}")


def client_with(value):
    c = Client(raise_request_exception=False)
    if value:
        c.cookies[NAME] = value
    return c


def signed_in_cookie():
    with override_settings(**UNROTATED):
        c = Client(raise_request_exception=False)
        c.get("/login/")
    return c.cookies[NAME].value


def jar_after(old, *responses):
    value = old
    for r in responses:
        c = r.cookies.get(NAME)
        if c is not None:
            value = c.value
    return value


def parallel(old, path1, path2, keys):
    """Request 2 (path2) starts first and touches its session; request 1 (path1) then runs to
    completion; then request 2 finishes. Both carry the cookie `old`."""
    loaded.clear()
    go.clear()
    box = {}
    with override_settings(**keys):
        t = threading.Thread(target=lambda: box.setdefault("r2", client_with(old).get(path2)))
        t.start()
        loaded.wait(10)
        r1 = client_with(old).get(path1)
        go.set()
        t.join(20)
    return r1, box["r2"]


def next_load(label, cookie, keys):
    with override_settings(**keys):
        show(label, client_with(cookie).get("/"))


print(f"django {django.get_version()} | backend {backend} | SESSION_SAVE_EVERY_REQUEST={save_every}")

print("S0  control, no rotation: request 2 writes the cart while request 1 reads who is signed in")
old = signed_in_cookie()
r1, r2 = parallel(old, "/", "/add/", UNROTATED)
show("request 1 GET / (reads request.user)", r1)
show("request 2 GET /add/ (writes cart)", r2)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), UNROTATED)

print("S0L control, no rotation: request 2 writes the cart while request 1 signs out")
old = signed_in_cookie()
r1, r2 = parallel(old, "/logout/", "/add/", UNROTATED)
show("request 1 GET /logout/", r1)
show("request 2 GET /add/ (writes cart)", r2)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), UNROTATED)

print("S1  rotation: request 2 touched its session before request 1 read request.user, writes after")
old = signed_in_cookie()
r1, r2 = parallel(old, "/", "/add/", ROTATED)
show("request 1 GET / (reads request.user)", r1)
show("request 2 GET /add/ (writes cart)", r2)
next_load("next page load, response 1 arrived last", jar_after(old, r2, r1), ROTATED)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), ROTATED)

print("S2  rotation: as S1, but request 2 reads request.user itself before writing")
old = signed_in_cookie()
r1, r2 = parallel(old, "/", "/add-then-whoami/", ROTATED)
show("request 1 GET / (reads request.user)", r1)
show("request 2 GET /add-then-whoami/", r2)
next_load("next page load, response 1 arrived last", jar_after(old, r2, r1), ROTATED)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), ROTATED)

print("S3  rotation: request 2 touches its session only after request 1 finished (GT-y2 B)")
old = signed_in_cookie()
r1, r2 = parallel(old, "/", "/late-add/", ROTATED)
show("request 1 GET / (reads request.user)", r1)
show("request 2 GET /late-add/ (writes cart)", r2)
next_load("next page load, response 1 arrived last", jar_after(old, r2, r1), ROTATED)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), ROTATED)

print("S4  rotation: request 2 reads only, after request 1 finished (GT-y2 B, read-only)")
old = signed_in_cookie()
with override_settings(**ROTATED):
    r1 = client_with(old).get("/")
    r2 = client_with(old).get("/")
show("request 1 GET /", r1)
show("request 2 GET / (old cookie)", r2)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), ROTATED)

print("S5  rotation: the first request ends in HTTP 500 (GT-y2 C)")
old = signed_in_cookie()
with override_settings(**ROTATED):
    r1 = client_with(old).get("/broken/")
show("request 1 GET /broken/", r1)
next_load("next page load", jar_after(old, r1), ROTATED)

print("S6  rotation, one request at a time (the documented rotation)")
old = signed_in_cookie()
with override_settings(**ROTATED):
    r1 = client_with(old).get("/")
show("request 1 GET /", r1)
next_load("next page load", jar_after(old, r1), ROTATED)

print("S7  rotation: request 2 only reads the session, touched before request 1 read request.user")
old = signed_in_cookie()
r1, r2 = parallel(old, "/", "/read/", ROTATED)
show("request 1 GET / (reads request.user)", r1)
show("request 2 GET /read/ (reads the cart only)", r2)
next_load("next page load, response 1 arrived last", jar_after(old, r2, r1), ROTATED)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), ROTATED)

print("S8  control, no rotation: an anonymous visitor with a cart signs in (login() cycles the key) while request 2 writes the cart")
with override_settings(**UNROTATED):
    anon = Client(raise_request_exception=False)
    anon.get("/anonymous-cart/")
old = anon.cookies[NAME].value
r1, r2 = parallel(old, "/login-from-anonymous/", "/add/", UNROTATED)
show("request 1 GET /login-from-anonymous/", r1)
show("request 2 GET /add/ (writes cart)", r2)
next_load("next page load, response 1 arrived last", jar_after(old, r2, r1), UNROTATED)
next_load("next page load, response 2 arrived last", jar_after(old, r1, r2), UNROTATED)
