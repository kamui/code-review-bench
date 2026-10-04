"""After a SECRET_KEY rotation with the old key as a fallback, where is a signed-in visitor's session data once the
session is lost to them: erased, or stored on the server under a key the browser does not hold?

Run once per session backend: PYTHONPATH=<django tree> python probe.py django.contrib.sessions.backends.<db|signed_cookies>"""
import sys
import django
from django.conf import settings

ENGINE = sys.argv[1]
settings.configure(
    DEBUG=False, SECRET_KEY="old-secret", ALLOWED_HOSTS=["*"], ROOT_URLCONF=__name__, SESSION_ENGINE=ENGINE,
    INSTALLED_APPS=["django.contrib.auth", "django.contrib.contenttypes", "django.contrib.sessions"],
    MIDDLEWARE=["django.contrib.sessions.middleware.SessionMiddleware", "django.contrib.auth.middleware.AuthenticationMiddleware"],
    DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"}},
    DEFAULT_AUTO_FIELD="django.db.models.AutoField", LOGGING_CONFIG=None,
)
django.setup()

from django.contrib.auth import login
from django.contrib.auth.models import User
from django.contrib.sessions.models import Session
from django.core.management import call_command
from django.http import HttpResponse
from django.test import Client, override_settings
from django.urls import path


def home(request):
    return HttpResponse(f"signed in as: {request.user} | cart: {request.session.get('cart')}")


def broken(request):
    str(request.user)
    return HttpResponse("server error", status=500)


def sign_in(request):
    login(request, User.objects.get(username="pat"))
    request.session["cart"] = "3 items"
    return HttpResponse("signed in")


urlpatterns = [path("", home), path("broken/", broken), path("login/", sign_in)]
call_command("migrate", run_syncdb=True, verbosity=0)
User.objects.create_user("pat", password="pw")
NAME = settings.SESSION_COOKIE_NAME


def name(key):
    return "the key from sign-in" if key == ORIGINAL else "a new key"


def rows(label):
    print(f"  server rows {label}:", [(name(s.session_key), s.get_decoded().get("cart")) for s in Session.objects.all()] or "none")


def browser_state(label, client):
    cookie = client.cookies.get(NAME)
    print(f"  browser cookie {label}:", "none" if cookie is None or not cookie.value else name(cookie.value))


def fresh():
    Session.objects.all().delete()
    with override_settings(SECRET_KEY="old-secret", SECRET_KEY_FALLBACKS=[]):
        client = Client(raise_request_exception=False)
        client.get("/login/")
    global ORIGINAL
    ORIGINAL = client.cookies[NAME].value
    return client


print(django.get_version(), ENGINE)
rotated = override_settings(SECRET_KEY="new-secret", SECRET_KEY_FALLBACKS=["old-secret"])

print("B. request 2 carries the cookie from before the rotation after request 1 finished; its response reaches the browser last")
client = fresh(); rows("before"); old = client.cookies[NAME].value
with rotated:
    one = Client(raise_request_exception=False); one.cookies[NAME] = old; print("  request 1:", one.get("/").content.decode())
    client.cookies[NAME] = old; print("  request 2:", client.get("/").content.decode())
    rows("after"); browser_state("after", client)
    print("  next page:", client.get("/").content.decode())

print("C. the first request after the rotation ends in HTTP 500")
client = fresh(); rows("before")
with rotated:
    client.get("/broken/"); rows("after the 500"); browser_state("after the 500", client)
    print("  next page:", client.get("/").content.decode()); rows("after the next page"); browser_state("after the next page", client)
