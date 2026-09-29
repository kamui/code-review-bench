import ssl, threading, socket, requests, requests.adapters as ad
sctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
sctx.load_cert_chain("srv.pem","srv.key")
sctx.load_verify_locations("ca.pem")
sctx.verify_mode = ssl.CERT_OPTIONAL
ls = socket.socket(); ls.bind(("127.0.0.1",0)); ls.listen(5); port = ls.getsockname()[1]
seen=[]
def serve():
    while True:
        c,_ = ls.accept()
        try:
            s = sctx.wrap_socket(c, server_side=True)
            pc = s.getpeercert()
            seen.append(dict(x[0] for x in pc["subject"])["commonName"] if pc else None)
            s.recv(4096)
            s.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            s.close()
        except Exception as e:
            seen.append("err %r"%e)
threading.Thread(target=serve,daemon=True).start()
# test-only: trust our CA in the shared preloaded context (equivalent to a CA in the certifi bundle)
ad._preloaded_ssl_context.load_verify_locations("ca.pem")
url = "https://localhost:%d/"%port
s1 = requests.Session()
print(s1.get(url, cert=("cli.pem","cli.key")).status_code)
s2 = requests.Session()   # different session, NO client cert
print(s2.get(url).status_code)
print("server saw client subjects:", seen)
