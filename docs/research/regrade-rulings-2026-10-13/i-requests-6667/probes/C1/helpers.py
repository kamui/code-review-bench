"""Shared helpers for the i-requests-6667 probes.

Every probe runs twice with the same interpreter and the same installed
dependencies; only PYTHONPATH changes, to put the `requests` source of the
commit before the change or of its head first.

PROBE_CERTS names the directory written by make_certs.sh.
"""
import http.client
import os
import ssl
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

CERTS = os.environ["PROBE_CERTS"]


def cert(name):
    return os.path.join(CERTS, name)


def use_default_bundle(path=None):
    """Make certifi.where() return `path` before requests is imported.

    This stands in for "a server whose certificate chains to a CA that the
    default bundle trusts". The bundle is the real certifi bundle plus the
    throwaway CA `ca-default`. It is applied the same way at both commits.
    """
    assert "requests" not in sys.modules, "call before importing requests"
    import certifi
    import certifi.core

    target = path or cert("bundle-default.pem")
    certifi.where = lambda: target
    certifi.core.where = certifi.where
    return target


class _Quiet(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def _send(self, code, body):
        data = body.encode()
        self.send_response(code)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)


class _OriginHandler(_Quiet):
    def do_GET(self):
        version = getattr(self.connection, "version", lambda: None)()
        self._send(200, f"origin={self.server.label} tls={version}")


class _ProxyHandler(_Quiet):
    """A forwarding proxy: the client sends `GET http://host/path` to it."""

    def do_GET(self):
        target = urlsplit(self.path)
        if not target.netloc:
            self._send(400, "not a proxy request")
            return
        upstream = http.client.HTTPConnection(target.hostname, target.port or 80, timeout=5)
        upstream.request("GET", target.path or "/")
        response = upstream.getresponse()
        body = response.read().decode()
        upstream.close()
        self._send(response.status, f"via-proxy[{body}]")


class _Server(ThreadingHTTPServer):
    daemon_threads = True
    tls_context = None
    label = ""

    def process_request_thread(self, request, client_address):
        if self.tls_context is not None:
            try:
                request.settimeout(5)
                request = self.tls_context.wrap_socket(request, server_side=True)
            except Exception:
                self.shutdown_request(request)
                return
        super().process_request_thread(request, client_address)

    def handle_error(self, request, client_address):
        pass


def _start(handler, label, certname=None, minimum=None, maximum=None):
    server = _Server(("127.0.0.1", 0), handler)
    server.label = label
    if certname:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert(certname + ".pem"), cert(certname + ".key"))
        if minimum:
            context.minimum_version = minimum
        if maximum:
            context.maximum_version = maximum
        server.tls_context = context
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server.server_address[1]


def plain_origin(label="plain"):
    return _start(_OriginHandler, label)


def tls_origin(certname, label=None, minimum=None, maximum=None):
    return _start(_OriginHandler, label or certname, certname, minimum, maximum)


def tls_forward_proxy(certname):
    return _start(_ProxyHandler, "proxy", certname)


def banner():
    import requests
    import urllib3

    print(f"python {sys.version.split()[0]}  {ssl.OPENSSL_VERSION}")
    print(f"urllib3 {urllib3.__version__}")
    src = os.path.dirname(requests.__file__)
    print(f"requests {requests.__version__} from {src.replace(os.environ.get('PROBE_SCRATCH', '~~'), '<scratch>')}")
    print(f"commit under test: {os.environ.get('PROBE_LABEL', '?')}")
    print("-" * 60)


def attempt(label, call):
    """Run `call`, print one line saying what the caller would see."""
    import requests

    try:
        response = call()
    except BaseException as exc:  # noqa: BLE001 - the probe reports every outcome
        kind = type(exc)
        caught = isinstance(exc, requests.RequestException)
        text = str(exc).replace("\n", " ")
        if len(text) > 300:
            text = text[:300] + "..."
        print(f"{label}: RAISED {kind.__module__}.{kind.__name__} "
              f"(a requests exception: {'yes' if caught else 'NO'}): {text}")
        return exc
    print(f"{label}: OK {response.status_code} {response.text}")
    return response
