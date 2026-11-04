"""A TLS-1.2-only server. Two adapters that require TLS 1.3, each a different way:
  ctx-route: init_poolmanager(..., ssl_context=ctx) with ctx.minimum_version = TLSv1_3 (the comment's own setup)
  kw-route:  init_poolmanager(..., ssl_minimum_version=TLSv1_3)        (GT-i8's trigger)
A refusal means the adapter's version setting was applied; 200 means it was ignored.
Usage: python -I probe.py <site-packages> <requests-src-dir> <label>"""
import os, socket, ssl, sys, tempfile, threading

site, src, label = sys.argv[1:4]
sys.path[:0] = [src, site]
import trustme  # noqa: E402

tmp = tempfile.mkdtemp()
ca = trustme.CA(); server_cert = ca.issue_cert("localhost")
ca_path = os.path.join(tmp, "ca.pem"); ca.cert_pem.write_to_path(ca_path)
sctx = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH); server_cert.configure_cert(sctx)
sctx.maximum_version = ssl.TLSVersion.TLSv1_2
lsock = socket.socket(); lsock.bind(("127.0.0.1", 0)); lsock.listen(16); port = lsock.getsockname()[1]
def serve():
    while True:
        c, _ = lsock.accept()
        try:
            t = sctx.wrap_socket(c, server_side=True); t.recv(65536)
            body = t.version().encode()
            t.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s" % (len(body), body)); t.close()
        except Exception:
            c.close()
threading.Thread(target=serve, daemon=True).start()
url = "https://localhost:%d/" % port

import requests  # noqa: E402
import requests.adapters as ad  # noqa: E402
from requests.adapters import HTTPAdapter  # noqa: E402
from urllib3.poolmanager import PoolManager  # noqa: E402
from urllib3.util.ssl_ import create_urllib3_context  # noqa: E402
if hasattr(ad, "_preloaded_ssl_context"):
    ad._preloaded_ssl_context.load_verify_locations(ca_path)
else:
    ad.DEFAULT_CA_BUNDLE_PATH = ca_path

class CtxRoute(HTTPAdapter):
    def init_poolmanager(self, connections, maxsize, block=False, **kw):
        ctx = create_urllib3_context()
        ctx.minimum_version = ssl.TLSVersion.TLSv1_3
        ctx.load_verify_locations(ca_path)
        self.poolmanager = PoolManager(num_pools=connections, maxsize=maxsize, block=block, ssl_context=ctx)

class KwRoute(HTTPAdapter):
    def init_poolmanager(self, connections, maxsize, block=False, **kw):
        self.poolmanager = PoolManager(num_pools=connections, maxsize=maxsize, block=block,
                                       ssl_minimum_version=ssl.TLSVersion.TLSv1_3)

print("== %s" % label)
for name, cls in (("ctx-route (the comment's setup)", CtxRoute), ("kw-route (GT-i8's trigger)", KwRoute)):
    s = requests.Session(); s.mount("https://", cls())
    try:
        r = s.get(url, timeout=5)
        out = "%d over %s: the TLS 1.3 minimum was ignored" % (r.status_code, r.text)
    except Exception as e:
        out = "refused (%s): the TLS 1.3 minimum was applied" % ("PROTOCOL_VERSION" if "PROTOCOL_VERSION" in str(e) or "protocol version" in str(e) else type(e).__name__)
    print("  %-34s verify=True: %s" % (name, out))
