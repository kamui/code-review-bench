"""How the platform and comparable clients treat a default CA file that exists, is readable and holds
no certificate. One case per process.

Usage: python -I platform.py <stdlib|urllib3|httpx> <empty|nonpem>
stdlib: SSL_CERT_FILE names the file (set before ssl is imported). urllib3: its own import and requests.
httpx: a stand-in certifi package, first on sys.path, makes certifi.where() return the file.
"""
import http.server
import os
import socket
import sys
import tempfile
import threading

tool, state = sys.argv[1:3]
tmp = tempfile.mkdtemp()
bundle = os.path.join(tmp, "cacert.pem")
if state == "empty":
    open(bundle, "wb").close()
else:
    open(bundle, "w").write("this file holds no certificate\n")
if tool == "stdlib":
    os.environ["SSL_CERT_FILE"] = bundle
if tool == "httpx":
    fake = os.path.join(tmp, "fakecertifi", "certifi")
    os.makedirs(fake)
    open(os.path.join(fake, "__init__.py"), "w").write(
        "def where():\n    return %r\n\ndef contents():\n    return open(where()).read()\n" % bundle)
    sys.path.insert(0, os.path.dirname(fake))

import ssl  # noqa: E402

import trustme  # noqa: E402

ca = trustme.CA()
server_cert = ca.issue_cert("localhost")
own_ca = os.path.join(tmp, "own-ca.pem")
ca.cert_pem.write_to_path(own_ca)
sctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
server_cert.configure_cert(sctx)
lsock = socket.socket()
lsock.bind(("127.0.0.1", 0))
lsock.listen(16)
tls = "https://localhost:%d/" % lsock.getsockname()[1]


def serve_tls():
    while True:
        c, _ = lsock.accept()
        try:
            t = sctx.wrap_socket(c, server_side=True)
            t.recv(65536)
            t.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            t.close()
        except Exception:
            c.close()


threading.Thread(target=serve_tls, daemon=True).start()


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, *a):
        pass


httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
plain = "http://127.0.0.1:%d/" % httpd.server_address[1]
threading.Thread(target=httpd.serve_forever, daemon=True).start()


def show(name, fn):
    try:
        print("%-46s %s" % (name, fn()))
    except BaseException as e:
        print("%-46s raised %s: %s" % (name, type(e).__name__, str(e).replace(tmp, "<tmp>")[:200]))


print("== %s, default CA file: %s" % (tool, state))
if tool == "stdlib":
    import urllib.request

    show("ssl.create_default_context()", lambda: "ok, %d CA certs" % ssl.create_default_context().cert_store_stats()["x509_ca"])
    show("urllib.request http://", lambda: urllib.request.urlopen(plain, timeout=5).status)
    show("urllib.request https, default context", lambda: urllib.request.urlopen(tls, timeout=5).status)
    show("urllib.request https, cafile=<own CA file>",
         lambda: urllib.request.urlopen(tls, timeout=5, context=ssl.create_default_context(cafile=own_ca)).status)
elif tool == "urllib3":
    import urllib3

    show("import urllib3", lambda: "ok " + urllib3.__version__)
    show("urllib3 http://", lambda: urllib3.request("GET", plain, timeout=5).status)
    show("urllib3 https, ca_certs=<own CA file>",
         lambda: urllib3.PoolManager(ca_certs=own_ca).request("GET", tls, timeout=5).status)
else:
    mod = {}

    def imp():
        import httpx

        mod["h"] = httpx
        return "ok " + httpx.__version__

    show("import httpx", imp)
    if "h" in mod:
        h = mod["h"]
        show("httpx.get http://", lambda: h.get(plain, timeout=5).status_code)
        show("httpx.get https, verify=False", lambda: h.get(tls, timeout=5, verify=False).status_code)
        show("httpx.get https, verify=<own CA file>", lambda: h.get(tls, timeout=5, verify=own_ca).status_code)
        show("httpx.get https, default verify", lambda: h.get(tls, timeout=5).status_code)
