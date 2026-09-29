import ssl, requests, requests.adapters as A
calls=[]
orig=ssl.SSLContext.load_cert_chain
def spy(self,*a,**k):
    calls.append((self is A._preloaded_ssl_context,a)); raise RuntimeError("stop")
ssl.SSLContext.load_cert_chain=spy
import socket,threading
srv=socket.socket(); srv.bind(("127.0.0.1",0)); srv.listen(5); port=srv.getsockname()[1]
threading.Thread(target=lambda:[srv.accept() for _ in range(1)],daemon=True).start()
s=requests.Session()
try: s.get(f"https://localhost:{port}/",cert=("c.pem","k.pem"),timeout=3)
except Exception as e: print("exc",type(e).__name__)
print(calls)
# finding2
from urllib3.util.ssl_ import create_urllib3_context
class Ad(requests.adapters.HTTPAdapter):
    def init_poolmanager(self,*a,**k):
        ctx=create_urllib3_context(); ctx.minimum_version=ssl.TLSVersion.TLSv1_3; self.mine=ctx
        super().init_poolmanager(*a,ssl_context=ctx,**k)
ad=Ad(); req=requests.Request("GET","https://example.com/").prepare()
c=ad._get_connection(req,True,None)
print("adapter ctx used:", c.conn_kw.get("ssl_context") is ad.mine, c.conn_kw.get("ssl_context") is A._preloaded_ssl_context)
