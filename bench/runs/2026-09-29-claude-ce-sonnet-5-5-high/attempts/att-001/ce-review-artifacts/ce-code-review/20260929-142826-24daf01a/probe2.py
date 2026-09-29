import ssl, threading, socket, requests
import requests.adapters as A
sctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); sctx.load_cert_chain("c.pem","k.pem")
srv = socket.socket(); srv.bind(("127.0.0.1",0)); srv.listen(5); port=srv.getsockname()[1]
def run():
    while True:
        try:
            c,_=srv.accept(); 
            try: sctx.wrap_socket(c,server_side=True)
            except Exception: pass
        except Exception: return
threading.Thread(target=run,daemon=True).start()
calls=[]
orig=ssl.SSLContext.load_cert_chain
def spy(self,*a,**k):
    calls.append((self is A._preloaded_ssl_context,a)); return orig(self,*a,**k)
ssl.SSLContext.load_cert_chain=spy
s=requests.Session()
try: s.get(f"https://localhost:{port}/",cert=("c.pem","k.pem"),timeout=3)
except Exception as e: print(type(e).__name__)
print(calls)
