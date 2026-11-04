"""Does a failed load_cert_chain leave state in requests' shared SSLContext?

Usage: python -I probe.py <site-packages> <requests-src-dir> <label> <case>
case: control | mismatch | badkey | encrypted
Runs in its own process per case so the shared context starts fresh.
"""
import os, socket, ssl, sys, tempfile, threading

site, src, label, case = sys.argv[1:5]
sys.path[:0] = [src, site]
import trustme  # noqa: E402

tmp = tempfile.mkdtemp()
ca = trustme.CA()
server_cert = ca.issue_cert("localhost", "127.0.0.1")
A = ca.issue_cert("client-a@example.org", common_name="client-A")
B = ca.issue_cert("client-b@example.org", common_name="client-B")

def write(name, blob):
    p = os.path.join(tmp, name)
    blob.write_to_path(p)
    return p

ca_path = write("ca.pem", ca.cert_pem)
a_cert = write("a.crt", A.cert_chain_pems[0]); a_key = write("a.key", A.private_key_pem)
b_cert = write("b.crt", B.cert_chain_pems[0]); b_key = write("b.key", B.private_key_pem)
garbage_key = os.path.join(tmp, "garbage.key")
open(garbage_key, "w").write("-----BEGIN PRIVATE KEY-----\nnot a key\n-----END PRIVATE KEY-----\n")
from cryptography.hazmat.primitives import serialization  # noqa: E402
enc_key = os.path.join(tmp, "b-enc.key")
pk = serialization.load_pem_private_key(B.private_key_pem.bytes(), password=None)
open(enc_key, "wb").write(pk.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
                                           serialization.BestAvailableEncryption(b"secret")))

BAD = {"mismatch": (b_cert, a_key), "badkey": (b_cert, garbage_key), "encrypted": (b_cert, enc_key)}

# A TLS server that asks for an optional client certificate and answers with the CN it saw.
sctx = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
server_cert.configure_cert(sctx)
sctx.load_verify_locations(ca_path)
sctx.verify_mode = ssl.CERT_OPTIONAL
lsock = socket.socket(); lsock.bind(("127.0.0.1", 0)); lsock.listen(16)
port = lsock.getsockname()[1]
seen = []

def serve():
    while True:
        c, _ = lsock.accept()
        try:
            t = sctx.wrap_socket(c, server_side=True)
        except Exception as e:  # handshake failure on the server side
            seen.append("server-handshake-error: %s" % type(e).__name__ + " " + str(e).split("(_ssl")[0][:90])
            c.close(); continue
        pc = t.getpeercert()
        cn = None
        if pc:
            cn = dict(x[0] for x in pc["subject"]).get("commonName")
        seen.append("client-cert=%s" % cn)
        try:
            t.recv(65536)
            body = str(cn).encode()
            t.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s" % (len(body), body))
        finally:
            t.close()

threading.Thread(target=serve, daemon=True).start()
url = "https://localhost:%d/" % port

import requests  # noqa: E402
import requests.adapters as ad  # noqa: E402
if hasattr(ad, "_preloaded_ssl_context"):
    ad._preloaded_ssl_context.load_verify_locations(ca_path)  # stand-in for a publicly trusted server
else:
    ad.DEFAULT_CA_BUNDLE_PATH = ca_path

def step(name, **kw):
    before = len(seen)
    try:
        with requests.Session() as s:
            r = s.get(url, timeout=5, **kw)
        out = "200 body=%s" % r.text
    except Exception as e:
        out = "raised %s: %s" % (type(e).__name__, str(e)[:110])
    import time; time.sleep(0.05)
    srv = seen[before:] or ["(no server record)"]
    print("%-34s client: %-60s server: %s" % (name, out, "; ".join(srv)))

print("== %s, case %s, requests %s, %s, %s" % (label, case, requests.__version__, ssl.OPENSSL_VERSION,
                                               "shared context" if hasattr(ad, "_preloaded_ssl_context") else "no shared context"))
step("1 no cert")
step("2 cert=A", cert=(a_cert, a_key))
if case != "control":
    step("3 cert=%s (fails to load)" % case, cert=BAD[case])
step("4 no cert, new Session")
step("5 cert=A again, new Session", cert=(a_cert, a_key))
step("6 no cert, new Session")

if case != "control" and label == "head":
    # The same sequence on a bare SSLContext, outside requests and urllib3.
    ctx = ssl.create_default_context(cafile=ca_path)
    ctx.load_cert_chain(a_cert, a_key)
    try:
        ctx.load_cert_chain(*BAD[case])
        print("bare ssl: second load_cert_chain did not raise")
    except Exception as e:
        print("bare ssl: second load_cert_chain raised %s: %s" % (type(e).__name__, str(e)[:80]))
    before = len(seen)
    try:
        with socket.create_connection(("127.0.0.1", port)) as raw, ctx.wrap_socket(raw, server_hostname="localhost") as t:
            t.sendall(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"); resp = t.recv(4096).split(b"\r\n\r\n")[-1].decode()
        out = "handshake ok, body=%s" % resp
    except Exception as e:
        out = "raised %s: %s" % (type(e).__name__, str(e)[:90])
    import time; time.sleep(0.05)
    print("bare ssl: later handshake on that context: %s; server: %s" % (out, "; ".join(seen[before:]) or "(none)"))
