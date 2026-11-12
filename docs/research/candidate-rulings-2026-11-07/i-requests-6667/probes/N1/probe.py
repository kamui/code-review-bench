"""One case in a fresh process: what `import requests` and five requests do when the default CA
location (certifi.where()) is in a given state.

Usage: python -I probe.py <requests-src-dir> <label> <state>
state: good | empty | nonpem | badpem | missing | unreadable | directory | nossl
A stand-in certifi package, first on sys.path, makes where() return a location this probe controls.
"""
import http.server
import os
import socket
import ssl
import stat
import subprocess
import sys
import tempfile
import threading
import traceback

src, label, state = sys.argv[1:4]
tmp = tempfile.mkdtemp()
fake = os.path.join(tmp, "fakecertifi", "certifi")
os.makedirs(fake)
default = os.path.join(tmp, "default", "cacert.pem")
os.makedirs(os.path.dirname(default))

import trustme  # noqa: E402

ca = trustme.CA()
server_cert = ca.issue_cert("localhost")
own_ca = os.path.join(tmp, "own-ca.pem")
ca.cert_pem.write_to_path(own_ca)

if state in ("good", "nossl"):
    ca.cert_pem.write_to_path(default)
elif state == "empty":
    open(default, "wb").close()
elif state == "nonpem":
    open(default, "w").write("this file holds no certificate\n")
elif state == "badpem":
    open(default, "w").write("-----BEGIN CERTIFICATE-----\nMIIBszCCAVmgAwIBAgIUZm9vYmFy\n-----END CERTIFICATE-----\n")
elif state == "unreadable":
    ca.cert_pem.write_to_path(default)
    os.chmod(default, 0)
elif state == "directory":
    default = os.path.join(tmp, "default-dir")
    os.makedirs(default)
    ca.cert_pem.write_to_path(os.path.join(default, "private-ca.pem"))
    subprocess.run(["c_rehash", default], check=True, capture_output=True)
elif state != "missing":
    raise SystemExit("unknown state " + state)

open(os.path.join(fake, "__init__.py"), "w").write(
    "def where():\n    return %r\n\ndef contents():\n    return open(where()).read()\n" % default)

sctx = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
server_cert.configure_cert(sctx)
lsock = socket.socket()
lsock.bind(("127.0.0.1", 0))
lsock.listen(16)
tls_port = lsock.getsockname()[1]


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
http_port = httpd.server_address[1]
threading.Thread(target=httpd.serve_forever, daemon=True).start()

if state == "nossl":
    for name in [m for m in sys.modules if m in ("ssl", "_ssl") or m.startswith("urllib3")]:
        del sys.modules[name]

    class BlockSSL:
        def find_spec(self, name, path=None, target=None):
            if name in ("ssl", "_ssl"):
                raise ImportError("ssl blocked by the probe")
            return None

    sys.meta_path.insert(0, BlockSSL())

sys.path[:0] = [src, os.path.dirname(fake)]

loads = []
if state != "nossl":
    import urllib3.util.ssl_ as u3ssl  # noqa: E402

    class CountingContext(u3ssl.SSLContext):
        def load_verify_locations(self, *a, **k):
            loads.append(a or k)
            return super().load_verify_locations(*a, **k)

    u3ssl.SSLContext = CountingContext


def clean(text):
    text = str(text).replace(tmp, "<tmp>")
    for port in (tls_port, http_port):
        text = text.replace(str(port), "<port>")
    return text


def show(name, fn):
    try:
        print("%-44s %s" % (name, clean(fn())))
    except BaseException as e:
        frame = traceback.extract_tb(e.__traceback__)[-1]
        where = ""
        for f in traceback.extract_tb(e.__traceback__):
            if f.filename.startswith(src):
                where = " [from %s:%d]" % (os.path.relpath(f.filename, src), f.lineno)
        cause = e.__cause__ or e.__context__
        extra = ""
        if cause is not None and not isinstance(e, ImportError):
            extra = " (cause %s)" % type(cause).__name__
        kind = type(e).__name__ if type(e).__module__ == "builtins" else "%s.%s" % (type(e).__module__, type(e).__name__)
        print("%-44s raised %s: %s%s%s" % (name, kind, clean(e)[:240], extra, where))


print("== %s, default CA location: %s" % (label, state))
mod = {}


def imp():
    import requests

    mod["r"] = requests
    return "import ok; load_verify_locations calls during import: %d" % len(loads)


show("import requests", imp)
if "r" in mod:
    r = mod["r"]
    tls = "https://localhost:%d/" % tls_port
    show("GET http://", lambda: r.get("http://127.0.0.1:%d/" % http_port, timeout=5).status_code)
    if state != "nossl":
        show("GET https, verify=False", lambda: r.get(tls, timeout=5, verify=False).status_code)
        show("GET https, verify=<own CA file>", lambda: r.get(tls, timeout=5, verify=own_ca).status_code)

        def env_bundle():
            os.environ["REQUESTS_CA_BUNDLE"] = own_ca
            try:
                return r.get(tls, timeout=5).status_code
            finally:
                del os.environ["REQUESTS_CA_BUNDLE"]

        show("GET https, REQUESTS_CA_BUNDLE=<own CA file>", env_bundle)
        show("GET https, default verify=True", lambda: r.get(tls, timeout=5).status_code)
        print("%-44s %d" % ("load_verify_locations calls in all", len(loads)))
if state == "unreadable":
    os.chmod(default, stat.S_IRUSR | stat.S_IWUSR)
