"""A visitor is signed in and has data in the session (a cart). The operator then rotates SECRET_KEY and
keeps the old key in SECRET_KEY_FALLBACKS. What happens to that visitor when login() is called for them
again, on each path that can call it? Run once per Django tree: PYTHONPATH=<tree> python probe.py"""
import django
from django.conf import settings

settings.configure(
    DEBUG=False, SECRET_KEY="old-secret", ALLOWED_HOSTS=["*"], ROOT_URLCONF=__name__,
    INSTALLED_APPS=[
        "django.contrib.admin", "django.contrib.auth", "django.contrib.contenttypes",
        "django.contrib.sessions", "django.contrib.messages",
    ],
    MIDDLEWARE=[
        "django.contrib.sessions.middleware.SessionMiddleware",
        "django.contrib.auth.middleware.AuthenticationMiddleware",
        "django.contrib.messages.middleware.MessageMiddleware",
    ],
    AUTHENTICATION_BACKENDS=[
        "django.contrib.auth.backends.ModelBackend",
        "django.contrib.auth.backends.RemoteUserBackend",
    ],
    TEMPLATES=[{
        "BACKEND": "django.template.backends.django.DjangoTemplates",
        "APP_DIRS": False,
        "OPTIONS": {
            "context_processors": [
                "django.template.context_processors.request",
                "django.contrib.auth.context_processors.auth",
                "django.contrib.messages.context_processors.messages",
            ],
            "loaders": [
                ("django.template.loaders.locmem.Loader", {
                    "login_plain.html": "<form method=post>{{ form.as_p }}</form>",
                    "login_greets.html": "Hello {{ user }}<form method=post>{{ form.as_p }}</form>",
                }),
                "django.template.loaders.app_directories.Loader",
            ],
        },
    }],
    DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"}},
    DEFAULT_AUTO_FIELD="django.db.models.AutoField", LOGGING_CONFIG=None,
    LOGIN_REDIRECT_URL="/", USE_TZ=True,
)
django.setup()

from django.contrib import admin
from django.contrib.auth import login
from django.contrib.auth.middleware import RemoteUserMiddleware
from django.contrib.auth.models import User
from django.contrib.auth.views import LoginView
from django.contrib.sessions.models import Session
from django.core.management import call_command
from django.http import HttpResponse
from django.test import Client, modify_settings, override_settings
from django.urls import path

BACKEND = "django.contrib.auth.backends.ModelBackend"


def home(request):
    return HttpResponse(f"signed in as: {request.user} | cart: {request.session.get('cart')}")


def first_sign_in(request):
    login(request, User.objects.get(username="pat"), backend=BACKEND)
    request.session["cart"] = "3 items"
    return HttpResponse("signed in, cart filled")


def confirm_identity(request):
    """A re-authentication view that calls login() without first reading request.user."""
    login(request, User.objects.get(username="pat"), backend=BACKEND)
    return HttpResponse("identity confirmed")


def confirm_identity_after_reading_user(request):
    str(request.user)
    login(request, User.objects.get(username="pat"), backend=BACKEND)
    return HttpResponse("identity confirmed")


urlpatterns = [
    path("", home),
    path("first-sign-in/", first_sign_in),
    path("confirm/", confirm_identity),
    path("confirm-after-reading-user/", confirm_identity_after_reading_user),
    path("accounts/login-plain/", LoginView.as_view(template_name="login_plain.html")),
    path("accounts/login-greets/", LoginView.as_view(template_name="login_greets.html")),
    path("admin/", admin.site.urls),
]
call_command("migrate", run_syncdb=True, verbosity=0)
User.objects.create_user("pat", password="pw", is_staff=True, is_superuser=True)
NAME = settings.SESSION_COOKIE_NAME
CREDENTIALS = {"username": "pat", "password": "pw"}


def signed_in_browser():
    with override_settings(SECRET_KEY="old-secret", SECRET_KEY_FALLBACKS=[]):
        browser = Client(raise_request_exception=False)
        browser.get("/first-sign-in/")
    return browser


def step(browser, label, method, url, **kwargs):
    before = browser.cookies[NAME].value
    response = getattr(browser, method)(url, **kwargs)
    after = browser.cookies[NAME].value
    cookie = "same session cookie" if after == before else ("cookie DELETED" if not after else "NEW session cookie")
    body = response.content.decode()
    shown = body[:48] if response.status_code == 200 and len(body) < 80 else ""
    print(f"      {label:44} -> HTTP {response.status_code} {shown} [{cookie}]")


def post_login_view(url):
    return lambda b: step(b, "POST sign-in form, same user", "post", url, data=CREDENTIALS)


def get_then_post(url):
    def run(browser):
        step(browser, "GET sign-in page", "get", url)
        step(browser, "POST sign-in form, same user", "post", url, data=CREDENTIALS)
    return run


def remote_user(browser):
    with modify_settings(MIDDLEWARE={"append": "django.contrib.auth.middleware.RemoteUserMiddleware"}):
        # A test client keeps the middleware it first loaded, so the header request needs a new one.
        behind_proxy = Client(raise_request_exception=False)
        behind_proxy.cookies = browser.cookies
        step(behind_proxy, "page load with REMOTE_USER: pat header", "get", "/", **{RemoteUserMiddleware.header: "pat"})


def admin_get_then_post(browser):
    step(browser, "GET admin sign-in page", "get", "/admin/login/")
    step(browser, "POST admin sign-in form, same user", "post", "/admin/login/", data={**CREDENTIALS, "next": "/"})


SCENARIOS = [
    ("1. custom view calls login() without reading request.user",
     lambda b: step(b, "POST /confirm/", "post", "/confirm/")),
    ("2. custom view reads request.user, then calls login()",
     lambda b: step(b, "POST /confirm-after-reading-user/", "post", "/confirm-after-reading-user/")),
    ("3. stock LoginView: POST is the first request (no page load before it)",
     post_login_view("/accounts/login-plain/")),
    ("4. stock LoginView: GET the page, then POST; page template does not mention the user",
     get_then_post("/accounts/login-plain/")),
    ("5. stock LoginView: GET the page, then POST; page template prints {{ user }}",
     get_then_post("/accounts/login-greets/")),
    ("6. any other page is loaded first, then scenario 3",
     lambda b: (step(b, "GET /", "get", "/"), post_login_view("/accounts/login-plain/")(b))),
    ("7. RemoteUserMiddleware signs the same user in from a header",
     remote_user),
    ("8. admin sign-in form: GET the page, then POST",
     admin_get_then_post),
    ("9. admin sign-in form: POST is the first request",
     lambda b: step(b, "POST admin sign-in form, same user", "post", "/admin/login/", data={**CREDENTIALS, "next": "/"})),
]

SETTINGS = [
    ("key NOT rotated (the day before)", dict(SECRET_KEY="old-secret", SECRET_KEY_FALLBACKS=[])),
    ("key ROTATED, old key in SECRET_KEY_FALLBACKS", dict(SECRET_KEY="new-secret", SECRET_KEY_FALLBACKS=["old-secret"])),
]

print("django", django.get_version())
summary = []
for title, run in SCENARIOS:
    print(f"\n{title}")
    row = [title.split(".")[0]]
    for label, overrides in SETTINGS:
        Session.objects.all().delete()
        browser = signed_in_browser()
        print(f"   {label}")
        with override_settings(**overrides):
            run(browser)
            outcome = browser.get("/").content.decode()
            print(f"      {'then the next page load shows':44} -> {outcome}")
        row.append("cart KEPT" if "3 items" in outcome else "cart LOST")
        row.append("signed in" if "signed in as: pat" in outcome else "SIGNED OUT")
    summary.append(row)

print("\nSummary (scenario | not rotated | rotated with fallback)")
for number, plain_cart, plain_user, rotated_cart, rotated_user in summary:
    print(f"   {number:>2} | {plain_cart}, {plain_user} | {rotated_cart}, {rotated_user}")
