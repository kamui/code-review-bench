import ssl, requests
from requests.adapters import HTTPAdapter
import requests.adapters as A
class MyAd(HTTPAdapter):
    def init_poolmanager(self,*a,**k):
        ctx = ssl.create_default_context(); self.mine=ctx
        super().init_poolmanager(*a, ssl_context=ctx, **k)
ad=MyAd()
r=requests.Request("GET","https://example.com").prepare()
conn=ad._get_connection(r,True,None)
print("custom ctx used:", conn.conn_kw.get("ssl_context") is ad.mine, "preloaded used:", conn.conn_kw.get("ssl_context") is A._preloaded_ssl_context)
# shared across cert
ad2=HTTPAdapter()
c=ad2._get_connection(r,True,("a.pem","a.key"))
print("client-cert conn shares module ctx:", c.conn_kw.get("ssl_context") is A._preloaded_ssl_context, c.conn_kw.get("cert_file"))
# CA bundle env / dir
c=ad2._get_connection(r,"/tmp",None); print(c.conn_kw)
