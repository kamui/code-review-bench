import ssl, requests
from requests.adapters import HTTPAdapter
class A(HTTPAdapter):
    def init_poolmanager(self, *a, **k):
        ctx = ssl.create_default_context()
        ctx.minimum_version = ssl.TLSVersion.TLSv1_3
        self.ctx = ctx
        super().init_poolmanager(*a, ssl_context=ctx, **k)
a = A()
req = requests.Request("GET","https://example.com/").prepare()
pool = a._get_connection(req, True)
print("verify=True uses adapter ctx:", pool.conn_kw.get("ssl_context") is a.ctx)
pool = a._get_connection(req, "/etc/ssl/certs/ca-certificates.crt")
print("verify=str uses adapter ctx:", pool.conn_kw.get("ssl_context") is a.ctx)
pool = a._get_connection(req, True, cert=("c","k"))
print("shared global ctx w/ cert:", pool.conn_kw.get("ssl_context") is requests.adapters._preloaded_ssl_context, pool.conn_kw.get("cert_file"))
