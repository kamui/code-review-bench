import json
from types import SimpleNamespace
from unittest.mock import patch
from django.conf import settings
settings.configure(SECRET_KEY="old-secret", SECRET_KEY_FALLBACKS=[], INSTALLED_APPS=["django.contrib.auth","django.contrib.contenttypes"],DATABASES={"default":{"ENGINE":"django.db.backends.sqlite3","NAME":":memory:"}},AUTHENTICATION_BACKENDS=["django.contrib.auth.backends.ModelBackend"])
import django
django.setup()
from django.contrib import auth
from django.contrib.auth.models import User
from django.test import override_settings
class Session(dict):
    def flush(self): self.clear(); self.flushed=True
    def cycle_key(self): self.cycled=True
    flushed=False
    cycled=False
def run_case(user, stored_hash):
    session=Session({auth.SESSION_KEY:"1",auth.BACKEND_SESSION_KEY:settings.AUTHENTICATION_BACKENDS[0],auth.HASH_SESSION_KEY:stored_hash})
    backend=SimpleNamespace(get_user=lambda pk:user)
    with patch.object(auth,"load_backend",return_value=backend), patch.object(auth,"_get_user_session_key",return_value=1):
        try:
            result=auth.get_user(SimpleNamespace(session=session))
            return {"authenticated":result is user,"flushed":session.flushed,"cycled":session.cycled}
        except Exception as e:return {"error":type(e).__name__,"message":str(e),"flushed":session.flushed}
user=SimpleNamespace(get_session_auth_hash=lambda:"new-password-hash")
print(json.dumps({"case":"legacy_protocol_password_change","result":run_case(user,"old-password-hash")}))
user=User(password="password-value")
original=user.get_session_auth_hash
user.get_session_auth_hash=lambda:original()+"custom-state"
old_hash=user.get_session_auth_hash()
with override_settings(SECRET_KEY="new-secret",SECRET_KEY_FALLBACKS=["old-secret"]):
    result=run_case(user,old_hash)
    hashes=list(user.get_session_auth_fallback_hash()) if hasattr(user,"get_session_auth_fallback_hash") else []
    print(json.dumps({"case":"custom_hash_rotation","fallback_matches":old_hash in hashes,"result":result}))
user=User(password="password-value")
old_hash=user.get_session_auth_hash()
with override_settings(SECRET_KEY="new-secret",SECRET_KEY_FALLBACKS=["old-secret"]):
    print(json.dumps({"case":"default_hash_rotation","result":run_case(user,old_hash)}))
