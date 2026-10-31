# Django at 2396933 (head of django/django#16631): does cycle_key() change the session key of the signed-cookie backend?
import sys, time
import django
from django.conf import settings
settings.configure(SECRET_KEY="new-key", SECRET_KEY_FALLBACKS=["old-key"], INSTALLED_APPS=["django.contrib.sessions"],
                   SESSION_ENGINE="django.contrib.sessions.backends.signed_cookies")
django.setup()
from django.contrib.sessions.backends.signed_cookies import SessionStore
s = SessionStore()
s["k"] = "v"
s.save()
k0 = s.session_key
time.sleep(1.1)
s.cycle_key()
k1 = s.session_key
print("django", django.get_version(), "python", sys.version.split()[0])
print("key changed after cycle_key (1.1 s later):", k0 != k1)
old = SessionStore(session_key=k0)
print("old cookie still loads its data:", old.load() == {"k": "v"})
# Same second: no sleep.
s2 = SessionStore(); s2["k"] = "v"; s2.save(); a = s2.session_key; s2.cycle_key(); print("key changed within the same second:", a != s2.session_key)
