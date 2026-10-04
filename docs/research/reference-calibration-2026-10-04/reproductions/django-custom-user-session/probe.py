"""What does a visitor with a stale session get, before and after the pull request?"""
import django
from django.conf import settings

settings.configure(
    DEBUG=False, SECRET_KEY="probe-secret", ALLOWED_HOSTS=["*"], ROOT_URLCONF=__name__,
    INSTALLED_APPS=["django.contrib.auth", "django.contrib.contenttypes", "django.contrib.sessions", "accounts"],
    MIDDLEWARE=["django.contrib.sessions.middleware.SessionMiddleware", "django.contrib.auth.middleware.AuthenticationMiddleware"],
    AUTH_USER_MODEL="accounts.Account", DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"}},
    DEFAULT_AUTO_FIELD="django.db.models.AutoField", LOGGING_CONFIG=None,
)
django.setup()

from django.contrib.auth import login, logout
from django.core.management import call_command
from django.http import HttpResponse
from django.test import Client
from django.urls import path

from accounts.models import Account


def home(request):
    return HttpResponse(f"signed in as: {request.user}")


def public(request):
    return HttpResponse("public page, does not look at who is signed in")


def sign_in(request):
    login(request, Account.objects.get(username="pat"), backend="django.contrib.auth.backends.ModelBackend")
    return HttpResponse("signed in")


def sign_out(request):
    logout(request)
    return HttpResponse("signed out")


urlpatterns = [path("", home), path("public/", public), path("login/", sign_in), path("logout/", sign_out)]

call_command("migrate", run_syncdb=True, verbosity=0)
Account.objects.create(username="pat", password="old-password-hash")


def show(label, response):
    body = response.content.decode().strip().replace("\n", " ")
    print(f"  {label:44} -> HTTP {response.status_code}: {body[:90]}")


print("django", django.get_version(), "| fallback secrets configured:", bool(settings.SECRET_KEY_FALLBACKS))
phone = Client(raise_request_exception=False)
show("phone signs in", phone.get("/login/"))
show("phone loads home page", phone.get("/"))
Account.objects.filter(username="pat").update(password="new-password-hash")
print("  -- pat changes password on another device --")
show("phone loads home page (1st time)", phone.get("/"))
show("phone loads home page (2nd time)", phone.get("/"))
show("phone loads a public page", phone.get("/public/"))
show("phone tries to sign out", phone.get("/logout/"))
show("phone loads home page after sign-out try", phone.get("/"))
show("phone signs in again", phone.get("/login/"))
show("phone loads home page after signing in", phone.get("/"))
