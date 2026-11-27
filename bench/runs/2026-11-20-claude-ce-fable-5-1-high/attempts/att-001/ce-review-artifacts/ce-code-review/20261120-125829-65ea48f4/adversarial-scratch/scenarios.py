import os
import socket
import ssl
import subprocess
import sys
import threading

import requests
import requests.adapters as A
import urllib3.connection as U3C
from requests.adapters import HTTPAdapter
from urllib3.util.ssl_ import create_urllib3_context

HEAD = hasattr(A, "_preloaded_ssl_context")
SHARED = getattr(A, "_preloaded_ssl_context", None)
print("MODE:", "HEAD" if HEAD else "BASE", "->", requests.__file__)
HERE = os.path.dirname(os.path.abspath(__file__))


def prep(url):
    return requests.Request("GET", url).prepare()


def is_shared(ctx):
    return HEAD and ctx is SHARED


# ---------------------------------------------------------------- S4: adapter-supplied ssl_context
custom = create_urllib3_context()


class CustomCtxAdapter(HTTPAdapter):
    def init_poolmanager(self, *a, **kw):
        kw["ssl_context"] = custom
        super().init_poolmanager(*a, **kw)


ad = CustomCtxAdapter()
pool = ad._get_connection(prep("https://example.com/"), verify=True)
print("S4 verify=True pool ssl_context is adapter's custom ctx:", pool.conn_kw.get("ssl_context") is custom,
      "| is requests' shared ctx:", is_shared(pool.conn_kw.get("ssl_context")))

# ---------------------------------------------------------------- S3: https target via https proxy (tunnel)
captured = {}


def fake_wrap(sock, **kw):
    captured.update(kw)
    raise RuntimeError("captured")


orig = U3C._ssl_wrap_socket_and_match_hostname
U3C._ssl_wrap_socket_and_match_hostname = fake_wrap
try:
    ad = HTTPAdapter()
    p = prep("https://example.com/")
    pool = ad._get_connection(p, verify=True, proxies={"https": "https://proxy.invalid:8443"})
    ad.cert_verify(pool, p.url, True, None)
    print("S3 pool.ca_certs:", pool.ca_certs, "| pool ssl_context is shared:", is_shared(pool.conn_kw.get("ssl_context")))
    c = pool._new_conn()
    c.set_tunnel(scheme="https", host="example.com", port=443, headers=pool.proxy_headers)
    a, b = socket.socketpair()
    a.settimeout(1)
    c._new_conn = lambda: a
    try:
        c.connect()
    except RuntimeError:
        pass
    print("S3 proxy-TLS call: ssl_context=", "shared" if is_shared(captured.get("ssl_context")) else captured.get("ssl_context"),
          "| ca_certs=", captured.get("ca_certs"), "| cert_reqs=", captured.get("cert_reqs"))
finally:
    U3C._ssl_wrap_socket_and_match_hostname = orig

# ---------------------------------------------------------------- S2: http target via https proxy (forwarding)
ad = HTTPAdapter()
p = prep("http://example.com/")
pool = ad._get_connection(p, verify=True, proxies={"http": "https://proxy.invalid:8443"})
ad.cert_verify(pool, p.url, True, None)
print("S2 pool:", type(pool).__name__, "| pool.cert_reqs:", pool.cert_reqs, "| pool ssl_context is shared:", is_shared(pool.conn_kw.get("ssl_context")))
c = pool._new_conn()
a, b = socket.socketpair()
a.settimeout(1)
c._new_conn = lambda: a
try:
    c.connect()
    print("S2 connect: no exception")
except Exception as e:  # noqa: BLE001
    print("S2 connect raised:", type(e).__name__, "-", e)
if HEAD:
    print("S2 shared verify_mode afterwards:", SHARED.verify_mode)

# ---------------------------------------------------------------- S1: client cert / CA pollution via shared ctx (loopback mTLS)
import trustme  # noqa: E402

ca = trustme.CA()
srv = ca.issue_cert("localhost", "127.0.0.1")
cli = ca.issue_cert("client-identity@example.com")
ca_path = os.path.join(HERE, "ca.pem")
srv_path = os.path.join(HERE, "server.pem")
cli_cert = os.path.join(HERE, "client.pem")
cli_key = os.path.join(HERE, "client.key")
ca.cert_pem.write_to_path(ca_path)
srv.private_key_and_cert_chain_pem.write_to_path(srv_path)
cli.cert_chain_pems[0].write_to_path(cli_cert)
cli.private_key_pem.write_to_path(cli_key)

sctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
sctx.load_cert_chain(srv_path)
sctx.verify_mode = ssl.CERT_REQUIRED
sctx.load_verify_locations(ca_path)
lsock = socket.socket()
lsock.bind(("127.0.0.1", 0))
lsock.listen(5)
lsock.settimeout(5)
port = lsock.getsockname()[1]
seen = []


def serve(n):
    for _ in range(n):
        try:
            s, _ = lsock.accept()
        except OSError:
            return
        try:
            t = sctx.wrap_socket(s, server_side=True)
            t.settimeout(2)
            t.recv(65536)
            pc = t.getpeercert()
            seen.append(("ok", pc.get("subject") if pc else None))
            t.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            t.close()
        except Exception as e:  # noqa: BLE001
            seen.append(("handshake-failed", type(e).__name__))
            s.close()


th = threading.Thread(target=serve, args=(3,), daemon=True)
th.start()
url = f"https://localhost:{port}/"


class InternalCAAdapter(HTTPAdapter):
    """Common third-party pattern: keep verify=True but trust an internal CA."""

    def cert_verify(self, conn, url, verify, cert):
        super().cert_verify(conn, url, verify, cert)
        if verify is True:
            conn.ca_certs = ca_path


s1 = requests.Session()
s1.mount("https://", InternalCAAdapter())
try:
    r = s1.get(url, verify=True, cert=(cli_cert, cli_key), timeout=5)
    print("S1 req1 (InternalCAAdapter, verify=True, cert=client):", r.status_code)
except Exception as e:  # noqa: BLE001
    print("S1 req1 failed:", type(e).__name__, str(e)[:160])

# Completely unrelated session: default adapter, verify=True, NO client cert.
s2 = requests.Session()
try:
    r = s2.get(url, verify=True, timeout=5)
    print("S1 req2 (plain Session, verify=True, no cert):", r.status_code, "<-- succeeded: server got a client cert from somewhere")
except Exception as e:  # noqa: BLE001
    print("S1 req2 (plain Session, verify=True, no cert) failed:", type(e).__name__, str(e)[:160])

# Control: same server, explicit CA bundle (fresh per-connection context), no client cert.
try:
    r = requests.get(url, verify=ca_path, timeout=5)
    print("S1 req3 (verify=ca_path, no cert):", r.status_code)
except Exception as e:  # noqa: BLE001
    print("S1 req3 (verify=ca_path, no cert) failed:", type(e).__name__, str(e)[:120])

th.join(5)
print("S1 server saw:", seen)
if HEAD:
    print("S1 shared ctx now trusts injected CA:", any(
        c.get("subject") and "trustme" in str(c.get("subject")) for c in SHARED.get_ca_certs()))

# ---------------------------------------------------------------- S5: import-time failure contract
code = "import certifi; certifi.where = lambda: '/nonexistent/cacert.pem'; import requests; print('import ok')"
r = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, env=os.environ)
print("S5 import with missing bundle -> rc", r.returncode, "|", (r.stdout + r.stderr).strip().splitlines()[-1][:160])
