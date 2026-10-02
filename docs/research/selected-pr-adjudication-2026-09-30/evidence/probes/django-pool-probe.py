import json
from types import SimpleNamespace
from unittest.mock import patch
from django.conf import settings
settings.configure(SECRET_KEY="probe",USE_TZ=True,INSTALLED_APPS=[],DATABASES={"default":{"ENGINE":"django.db.backends.postgresql","NAME":"prior_database","OPTIONS":{},"CONN_MAX_AGE":0,"CONN_HEALTH_CHECKS":False}})
import django
django.setup()
from django.db import connections
from django.db.backends.postgresql import base
Wrapper=base.DatabaseWrapper
config=dict(connections["default"].settings_dict);config["OPTIONS"]={}
class Cursor:
    def __enter__(self):return self
    def __exit__(self,*args):pass
    def execute(self,*args):pass
raw=SimpleNamespace(info=SimpleNamespace(parameter_status=lambda k:"UTC"),cursor=lambda:Cursor(),commit=lambda:None)
class Derived(Wrapper):
    def ensure_timezone(self):self.timezone_override_called=True;return False
    def ensure_role(self):self.role_override_called=True;return False
w=Derived(config,alias="override_probe");w.connection=raw;w.timezone_override_called=False;w.role_override_called=False
w.check_database_version_supported=lambda:None
w.get_autocommit=lambda:True
w.init_connection_state()
print(json.dumps({"case":"extension_dispatch","timezone_called":w.timezone_override_called,"role_called":w.role_override_called}))
if hasattr(Wrapper,"pool"):
    cfg=dict(config);cfg["OPTIONS"]={"pool":{}}
    w=Wrapper(cfg,alias="empty_probe")
    print(json.dumps({"case":"empty_pool_options","pool_created":w.pool is not None}))
    w=Wrapper(config,alias="role_probe")
    def wrapper_cursor():raise RuntimeError("wrapper cursor reached while configuring raw connection")
    w.cursor=wrapper_cursor
    try:base.ensure_role(raw,w.ops,"example_role");result="no wrapper access"
    except RuntimeError as e:result=str(e)
    print(json.dumps({"case":"role_callback","result":result}))
    cfg=dict(config);cfg["OPTIONS"]={"pool":True}
    w=Wrapper(cfg,alias="switch_probe");pool=w.pool
    before=pool.kwargs["dbname"]
    w.close();w.settings_dict["NAME"]="test_prior_database"
    after=w.pool.kwargs["dbname"]
    print(json.dumps({"case":"pool_name_switch","before":before,"wrapper_name":w.settings_dict["NAME"],"cached_pool_name":after,"same_pool":w.pool is pool}))
    w.close_pool()
    w=Wrapper(cfg,alias="psycopg2_probe")
    with patch.object(base,"is_psycopg3",False):
        try:w.get_connection_params();result="ignored"
        except Exception as e:result=type(e).__name__+": "+str(e)
    print(json.dumps({"case":"psycopg2_option","result":result,"limit":"driver flag patched; actual parameter extraction, no psycopg2 or database server"}))
