import django
from django.conf import settings
settings.configure(SECRET_KEY="new", SECRET_KEY_FALLBACKS=["old"],
    INSTALLED_APPS=["django.contrib.auth","django.contrib.contenttypes","django.contrib.sessions"],
    AUTHENTICATION_BACKENDS=["t.Backend"], DATABASES={"default":{"ENGINE":"django.db.backends.sqlite3","NAME":":memory:"}})
django.setup()
from django.contrib import auth
from django.contrib.sessions.backends.signed_cookies import SessionStore
from django.http import HttpRequest
class U:
    pk = 1
    is_authenticated = True
    def get_session_auth_hash(self): return "current"
class Backend:
    def get_user(self, uid): return U()
import sys; sys.modules["t"] = sys.modules[__name__]
r = HttpRequest(); r.session = SessionStore()
r.session[auth.SESSION_KEY] = "1"; r.session[auth.BACKEND_SESSION_KEY] = "t.Backend"; r.session[auth.HASH_SESSION_KEY] = "stale"
try:
    print(auth.get_user(r))
except Exception as e:
    print(type(e).__name__, e)
