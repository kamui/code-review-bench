"""How the platform and two comparable clients behave when the default CA file is missing.

    python platform.py WORK_DIR

WORK_DIR holds certs/ from make-certs.sh. Each case runs in a fresh `python -I` process.
"Missing" means: certifi.where() is repointed to a file that does not exist, and for the standard
library the OpenSSL default file is pointed at a missing file with SSL_CERT_FILE.
"""
import http.server
import os
import ssl
import subprocess
import sys
import threading

WORK = os.path.abspath(sys.argv[1])
CERTS = os.path.join(WORK, "certs")
MISSING = os.path.join(WORK, "no-such-dir", "cacert.pem")
CA = os.path.join(CERTS, "ca.pem")


class Quiet(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, *args):
        pass


def serve(tls):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Quiet)
    if tls:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(os.path.join(CERTS, "server.pem"), os.path.join(CERTS, "server.key"))
        server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server.server_address[1]


HTTP = f"http://127.0.0.1:{serve(False)}/"
HTTPS = f"https://127.0.0.1:{serve(True)}/"
REPOINT = f"import certifi; certifi.where = lambda: {MISSING!r}"

CASES = [
    ("stdlib: import ssl, urllib.request; create_default_context(); urlopen(http://)", {"SSL_CERT_FILE": MISSING},
     f"import ssl, urllib.request; ssl.create_default_context(); print('ok', urllib.request.urlopen({HTTP!r}).status)"),
    ("stdlib: urlopen(https://) with the default context", {"SSL_CERT_FILE": MISSING},
     f"import urllib.request; print('ok', urllib.request.urlopen({HTTPS!r}).status)"),
    ("stdlib: urlopen(https://) with a context given cafile=<CA file>", {"SSL_CERT_FILE": MISSING},
     f"import ssl, urllib.request; c = ssl.create_default_context(cafile={CA!r}); print('ok', urllib.request.urlopen({HTTPS!r}, context=c).status)"),
    ("urllib3: import urllib3; PoolManager().request(GET http://)", {},
     f"{REPOINT}; import urllib3; print('ok', urllib3.PoolManager().request('GET', {HTTP!r}).status)"),
    ("httpx: import httpx", {}, f"{REPOINT}; import httpx; print('ok')"),
    ("httpx: httpx.get(http://)", {}, f"{REPOINT}; import httpx; print('ok', httpx.get({HTTP!r}).status_code)"),
    ("httpx: httpx.get(http://, verify=False)", {}, f"{REPOINT}; import httpx; print('ok', httpx.get({HTTP!r}, verify=False).status_code)"),
    ("httpx: httpx.get(https://, verify=<CA file>)", {}, f"{REPOINT}; import httpx; print('ok', httpx.get({HTTPS!r}, verify={CA!r}).status_code)"),
    ("httpx: httpx.get(https://) default verify", {}, f"{REPOINT}; import httpx; print('ok', httpx.get({HTTPS!r}).status_code)"),
]

CLIENT = """
import traceback
try:
    {code}
except BaseException as exc:
    print(f'{{type(exc).__name__}}: ' + str(exc).replace(chr(10), ' ')[:200])
"""


def main():
    import certifi
    import urllib3

    try:
        import httpx
        httpx_version = httpx.__version__
    except ImportError:
        httpx_version = "not installed"
    print(f"python {sys.version.split()[0]}; urllib3 {urllib3.__version__}; httpx {httpx_version}; certifi {certifi.__version__}; {ssl.OPENSSL_VERSION}")
    for label, extra, code in CASES:
        env = {k: v for k, v in os.environ.items() if k not in ("REQUESTS_CA_BUNDLE", "CURL_CA_BUNDLE", "SSL_CERT_FILE", "SSL_CERT_DIR")}
        env.update(extra)
        env["NO_PROXY"] = "*"
        out = subprocess.run([sys.executable, "-I", "-c", CLIENT.format(code=code)], capture_output=True, text=True, env=env, cwd=WORK, timeout=60)
        print(f"- {label}: {out.stdout.strip() or out.stderr.strip()[-200:]}")


main()
