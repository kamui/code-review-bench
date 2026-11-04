"""An HTTPAdapter subclass that sets conn.ca_certs for verify=True: does its CA reach other sessions?

Usage: python -I probe.py <site-packages> <requests-src-dir> <label> <conn-ca-certs|init-poolmanager>
"""
import os, socket, ssl, sys, tempfile, threading

site, src, label = sys.argv[1:4]
sys.path[:0] = [src, site]
import trustme  # noqa: E402

tmp = tempfile.mkdtemp()
corp = trustme.CA()                      # a private CA, not in certifi
server_cert = corp.issue_cert("localhost")
corp_path = os.path.join(tmp, "corp-ca.pem"); corp.cert_pem.write_to_path(corp_path)

sctx = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
server_cert.configure_cert(sctx)
lsock = socket.socket(); lsock.bind(("127.0.0.1", 0)); lsock.listen(16)
port = lsock.getsockname()[1]

def serve():
    while True:
        c, _ = lsock.accept()
        try:
            t = sctx.wrap_socket(c, server_side=True)
            t.recv(65536)
            t.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            t.close()
        except Exception:
            c.close()

threading.Thread(target=serve, daemon=True).start()
url = "https://localhost:%d/" % port

import requests  # noqa: E402
import requests.adapters as ad  # noqa: E402
from requests.adapters import HTTPAdapter  # noqa: E402

from urllib3.poolmanager import PoolManager  # noqa: E402

class InitPoolCAAdapter(HTTPAdapter):
    """GT-i7's trigger: CA material passed to the pool manager through init_poolmanager."""
    def init_poolmanager(self, connections, maxsize, block=False, **kw):
        self.poolmanager = PoolManager(num_pools=connections, maxsize=maxsize, block=block, ca_certs=corp_path)

class CorpCAAdapter(HTTPAdapter):
    """The shape the claim names: a subclass that sets conn.ca_certs for verify=True."""
    def cert_verify(self, conn, url, verify, cert):
        super().cert_verify(conn, url, verify, cert)
        if verify is True:
            conn.ca_certs = corp_path

def shared_count():
    c = getattr(ad, "_preloaded_ssl_context", None)
    return "n/a (no shared context)" if c is None else str(c.cert_store_stats()["x509_ca"])

def get(name, session):
    try:
        r = session.get(url, timeout=5)
        out = "%d" % r.status_code
    except Exception as e:
        out = "raised %s (%s)" % (type(e).__name__, "CERTIFICATE_VERIFY_FAILED" if "CERTIFICATE_VERIFY_FAILED" in str(e) else str(e)[:80])
    print("%-44s %-48s shared-context CA count: %s" % (name, out, shared_count()))

route = sys.argv[4]
print("== %s, %s: %s" % (label, route, "shared context" if hasattr(ad, "_preloaded_ssl_context") else "no shared context"))
cls = {"conn-ca-certs": CorpCAAdapter, "init-poolmanager": InitPoolCAAdapter}[route]
get("1 plain Session, verify=True", requests.Session())
corp_s = requests.Session(); corp_s.mount("https://", cls())
get("2 Session with %s, verify=True" % cls.__name__, corp_s)
get("3 new plain Session, verify=True", requests.Session())
get("4 requests.get(), verify=True", requests)
