import ssl, requests, requests.adapters as A
from requests.adapters import HTTPAdapter
from urllib3.util.ssl_ import create_urllib3_context
custom = create_urllib3_context(); custom.minimum_version = ssl.TLSVersion.TLSv1_3
class Ad(HTTPAdapter):
    def init_poolmanager(self, *a, **k):
        k["ssl_context"] = custom
        super().init_poolmanager(*a, **k)
ad = Ad()
req = requests.Request("GET","https://example.com/").prepare()
hp, pk = A._urllib3_request_context(req, True, None)
pool = ad.poolmanager.connection_from_host(**hp, pool_kwargs=pk)
print("adapter ctx used:", pool.conn_kw.get("ssl_context") is custom, "| min version:", pool.conn_kw["ssl_context"].minimum_version)
