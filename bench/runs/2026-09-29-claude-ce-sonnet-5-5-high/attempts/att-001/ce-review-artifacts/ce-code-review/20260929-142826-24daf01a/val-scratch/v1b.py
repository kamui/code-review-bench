import ssl, requests, requests.adapters as A, socket, threading
calls=[]
orig=ssl.SSLContext.load_cert_chain
def spy(self,*a,**k):
    calls.append(self is A._preloaded_ssl_context); return orig(self,*a,**k)
ssl.SSLContext.load_cert_chain=spy
srv=socket.socket(); srv.bind(("127.0.0.1",0)); srv.listen(5); port=srv.getsockname()[1]
def run():
    while True:
        c,_=srv.accept(); c.close()
threading.Thread(target=run,daemon=True).start()
try: requests.Session().get(f"https://localhost:{port}/",cert=("c.pem","k.pem"),timeout=3)
except Exception as e: print("exc",type(e).__name__)
print("load_cert_chain on shared ctx:",calls)
