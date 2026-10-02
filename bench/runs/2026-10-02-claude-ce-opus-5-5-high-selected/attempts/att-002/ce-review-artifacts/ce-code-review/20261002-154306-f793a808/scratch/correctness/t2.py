import django
from django.conf import settings
settings.configure(USE_TZ=True, DATABASES={})
django.setup()
from django.db.backends.postgresql.base import DatabaseWrapper
base = {"ENGINE": "django.db.backends.postgresql", "NAME": "prod", "USER": "", "PASSWORD": "", "HOST": "", "PORT": "",
        "OPTIONS": {"pool": True}, "TIME_ZONE": None, "CONN_MAX_AGE": 0, "CONN_HEALTH_CHECKS": False, "AUTOCOMMIT": True, "ATOMIC_REQUESTS": False, "TEST": {}}
w = DatabaseWrapper(dict(base), alias="default")
# postgres-db-inaccessible fallback wrapper as built in DatabaseWrapper._nodb_cursor
fb = DatabaseWrapper({**w.settings_dict, "NAME": "prod"}, alias=w.alias)
print("fallback pool dbname:", fb.pool.kwargs["dbname"], "bool(pool):", bool(fb.pool))
# create_test_db then switches NAME
w.settings_dict["NAME"] = "test_prod"
print("wrapper NAME:", w.settings_dict["NAME"], "-> pool dbname:", w.pool.kwargs["dbname"], "same pool:", w.pool is fb.pool)
print("get_connection_params dbname:", w.get_connection_params()["dbname"])
w.close_pool()
