import sys
sys.path.insert(0, "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-010/clone")
from django.conf import settings
settings.configure(SECRET_KEY="k", SECRET_KEY_FALLBACKS=[], INSTALLED_APPS=["django.contrib.auth","django.contrib.contenttypes"],
    AUTHENTICATION_BACKENDS=["b.B"], DATABASES={})
import django; django.setup()
import types
from django.contrib import auth
class DuckUser:
    pk = 1
    def get_session_auth_hash(self): return "new"
mod = types.ModuleType("b")
class B:
    def get_user(self, uid): return DuckUser()
mod.B = B; sys.modules["b"] = mod
class Sess(dict):
    def flush(self): print("flushed"); self.clear()
    def cycle_key(self): pass
class Req: pass
r = Req(); r.session = Sess({auth.SESSION_KEY: "1", auth.BACKEND_SESSION_KEY: "b.B", auth.HASH_SESSION_KEY: "old"})
from django.contrib.auth.models import User
auth._get_user_session_key = lambda req: 1
try:
    print(auth.get_user(r))
except Exception as e:
    print("EXC", type(e).__name__, e, "session after:", dict(r.session))
