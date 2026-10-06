import importlib.metadata
import json
import sys

sys.path.insert(0, sys.argv[1])
from django.conf import settings

settings.configure(SECRET_KEY="old-probe-key", SECRET_KEY_FALLBACKS=[])
from django.core.signing import BadSignature, Signer
from django.test import override_settings
from itsdangerous import BadSignature as ItsBadSignature
from itsdangerous import URLSafeSerializer

old = "old-probe-key"
new = "new-probe-key"
payload = {"cart": "3 items"}
print("Python:", sys.version.split()[0])
print("itsdangerous:", importlib.metadata.version("itsdangerous"))
for prepared in (False, True):
    old_keys = [old, new] if prepared else [old]
    new_keys = [old, new]
    with override_settings(SECRET_KEY=new, SECRET_KEY_FALLBACKS=[old]):
        django_token = Signer().sign_object(payload)
    with override_settings(SECRET_KEY=old, SECRET_KEY_FALLBACKS=[new] if prepared else []):
        try:
            django_result = Signer().unsign_object(django_token)
        except BadSignature as error:
            django_result = {"error": type(error).__name__, "text": str(error)}
    its_token = URLSafeSerializer(new_keys).dumps(payload)
    try:
        its_result = URLSafeSerializer(old_keys).loads(its_token)
    except ItsBadSignature as error:
        its_result = {"error": type(error).__name__, "text": str(error)}
    print(json.dumps({"both_keys_distributed_first": prepared, "django": django_result, "itsdangerous": its_result}, sort_keys=True))
