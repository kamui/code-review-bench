"""What the removed default-path OSError guard changes, for a default CA file missing at import
and for one deleted after import.

Usage: python -I probe.py <site-packages> <requests-src-dir> <label> <missing-at-import|deleted-after-import>
A stand-in certifi package, placed first on the path, makes where() return a file this probe controls.
"""
import os, shutil, socket, ssl, sys, tempfile, threading

site, src, label, case = sys.argv[1:5]
tmp = tempfile.mkdtemp()
fake = os.path.join(tmp, "fakecertifi", "certifi"); os.makedirs(fake)
bundle = os.path.join(tmp, "bundle", "cacert.pem")
open(os.path.join(fake, "__init__.py"), "w").write(
    "def where():\n    return %r\n\ndef contents():\n    return open(where()).read()\n" % bundle)
sys.path[:0] = [src, os.path.dirname(fake), site]
import trustme  # noqa: E402

ca = trustme.CA()
server_cert = ca.issue_cert("localhost")
sctx = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH); server_cert.configure_cert(sctx)
lsock = socket.socket(); lsock.bind(("127.0.0.1", 0)); lsock.listen(16); port = lsock.getsockname()[1]
def serve():
    while True:
        c, _ = lsock.accept()
        try:
            t = sctx.wrap_socket(c, server_side=True); t.recv(65536)
            t.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"); t.close()
        except Exception:
            c.close()
threading.Thread(target=serve, daemon=True).start()

if case == "deleted-after-import":
    os.makedirs(os.path.dirname(bundle)); ca.cert_pem.write_to_path(bundle)

def show(name, fn):
    try:
        print("%-40s %s" % (name, fn()))
    except BaseException as e:
        print("%-40s raised %s: %s" % (name, type(e).__name__, str(e).replace(tmp, "<tmp>")[:150]))

print("== %s, %s" % (label, case))
mod = {}
def imp():
    import requests
    mod["r"] = requests
    return "import ok"
show("import requests", imp)
if "r" in mod:
    r = mod["r"]
    if case == "deleted-after-import":
        shutil.rmtree(os.path.dirname(bundle))
        print("%-40s %s" % ("(default CA file now deleted)", os.path.exists(bundle)))
    show("GET https, verify=True", lambda: r.get("https://localhost:%d/" % port, timeout=5).status_code)
    show("GET https, verify=False", lambda: r.get("https://localhost:%d/" % port, timeout=5, verify=False).status_code)
