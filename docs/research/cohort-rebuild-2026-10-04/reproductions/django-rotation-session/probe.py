"""After an operator rotates SECRET_KEY and keeps the old one in SECRET_KEY_FALLBACKS, what happens to a
signed-in visitor, before and after the pull request?"""
import django
from django.conf import settings

settings.configure(
    DEBUG=False, SECRET_KEY="old-secret", ALLOWED_HOSTS=["*"], ROOT_URLCONF=__name__,
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
    return HttpResponse("server error after reading who is signed in", status=500)


def sign_in(request):
    login(request, User.objects.get(username="pat"))
    request.session["cart"] = "3 items"
    return HttpResponse("signed in")


urlpatterns = [path("", home), path("broken/", broken), path("login/", sign_in)]
call_command("migrate", run_syncdb=True, verbosity=0)
User.objects.create_user("pat", password="pw")
NAME = settings.SESSION_COOKIE_NAME


def show(label, response):
    cookie = response.cookies.get(NAME)
    sent = "no session cookie sent" if cookie is None else ("cookie DELETED" if not cookie.value else "new cookie sent")
    print(f"  {label:52} -> HTTP {response.status_code}: {response.content.decode()[:60]} [{sent}]")
    return cookie


def signed_in_browser():
    with override_settings(SECRET_KEY="old-secret", SECRET_KEY_FALLBACKS=[]):
        browser = Client(raise_request_exception=False)
        browser.get("/login/")
    return browser


def with_cookie(value):
    client = Client(raise_request_exception=False)
    if value:
        client.cookies[NAME] = value
    return client


print("django", django.get_version())
rotated = override_settings(SECRET_KEY="new-secret", SECRET_KEY_FALLBACKS=["old-secret"])

print("A. one request at a time after the rotation")
browser = signed_in_browser()
with rotated:
    show("first page load", browser.get("/"))
    show("second page load", browser.get("/"))

print("B. two requests sent together, both carrying the cookie from before the rotation")
browser = signed_in_browser()
old = browser.cookies[NAME].value
with rotated:
    first = show("request 1 (old cookie)", with_cookie(old).get("/"))
    second = show("request 2 (old cookie, already on its way)", with_cookie(old).get("/"))
    print("     session rows in the database:", Session.objects.count())
    for label, earlier, later in (("response 2 arrived last", first, second), ("response 1 arrived last", second, first)):
        jar = old
        for cookie in (earlier, later):
            jar = jar if cookie is None else cookie.value
        show(f"next page load if {label}", with_cookie(jar).get("/"))

print("C. the first request after the rotation ends in a server error")
browser = signed_in_browser()
with rotated:
    show("page that fails with HTTP 500", browser.get("/broken/"))
    show("next page load", browser.get("/"))
