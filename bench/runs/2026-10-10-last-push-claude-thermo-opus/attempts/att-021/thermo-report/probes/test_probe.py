"""Scratch probes for psf/requests#6667 (head 4089f3dc). Not part of the clone."""
import importlib
import socket
import ssl
import threading
import time

import pytest
import trustme
import urllib3.connection
from urllib3.util.ssl_ import create_urllib3_context

import requests
from requests import adapters
from requests.adapters import HTTPAdapter
from requests.utils import DEFAULT_CA_BUNDLE_PATH, extract_zipped_paths


@pytest.fixture(autouse=True)
def fresh_preloaded_context(monkeypatch):
    """Rebuild the module global exactly as adapters.py does, so probes don't
    contaminate each other (the global is looked up at call time)."""
    ctx = create_urllib3_context()
    ctx.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
    monkeypatch.setattr(adapters, "_preloaded_ssl_context", ctx)
    return ctx


def _prep(url):
    return requests.Request("GET", url).prepare()


def test_adapter_ssl_context_is_overridden():
    custom = ssl.create_default_context()
    custom.minimum_version = ssl.TLSVersion.TLSv1_3

    class CtxAdapter(HTTPAdapter):
        def init_poolmanager(self, *args, **kwargs):
            kwargs["ssl_context"] = custom
            super().init_poolmanager(*args, **kwargs)

    a = CtxAdapter()
    assert a.poolmanager.connection_pool_kw["ssl_context"] is custom
    pool = a._get_connection(_prep("https://example.invalid/"), True)
    used = pool.conn_kw["ssl_context"]
    print("\nPROBE1 pool ssl_context is custom:", used is custom,
          "| is _preloaded:", used is adapters._preloaded_ssl_context)
    assert used is adapters._preloaded_ssl_context and used is not custom


def test_adapter_ssl_version_kwargs_are_dead():
    class MinTls13Adapter(HTTPAdapter):
        def init_poolmanager(self, connections, maxsize, block=False):
            super().init_poolmanager(
                connections, maxsize, block=block,
                ssl_minimum_version=ssl.TLSVersion.TLSv1_3,
            )

    a = MinTls13Adapter()
    pool = a._get_connection(_prep("https://example.invalid/"), True)
    ctx = pool.conn_kw["ssl_context"]
    print("\nPROBE2 pool ssl_minimum_version kw:", pool.conn_kw.get("ssl_minimum_version"),
          "| context actually handed to urllib3 has minimum_version:", ctx.minimum_version)
    assert ctx is adapters._preloaded_ssl_context
    assert ctx.minimum_version != ssl.TLSVersion.TLSv1_3


class _MTLSServer(threading.Thread):
    def __init__(self, server_ctx, n):
        super().__init__(daemon=True)
        self.sock = socket.socket()
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(5)
        self.port = self.sock.getsockname()[1]
        self.ctx = server_ctx
        self.n = n
        self.peer_certs = []

    def run(self):
        for _ in range(self.n):
            raw, _ = self.sock.accept()
            try:
                tls = self.ctx.wrap_socket(raw, server_side=True)
                self.peer_certs.append(tls.getpeercert())
                tls.recv(65536)
                tls.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                tls.close()
            except Exception as e:  # pragma: no cover
                self.peer_certs.append(e)
        self.sock.close()


def test_client_cert_leaks_through_shared_context(tmp_path, fresh_preloaded_context):
    ca = trustme.CA()
    server_cert = ca.issue_cert("localhost")
    client_cert = ca.issue_cert("client@example.test")
    server_ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_cert.configure_cert(server_ctx)
    ca.configure_trust(server_ctx)
    server_ctx.verify_mode = ssl.CERT_OPTIONAL
    # test-only: make verify=True trust the throwaway CA
    ca.configure_trust(fresh_preloaded_context)

    pem = tmp_path / "client.pem"
    key = tmp_path / "client.key"
    client_cert.cert_chain_pems[0].write_to_path(str(pem))
    client_cert.private_key_pem.write_to_path(str(key))

    srv = _MTLSServer(server_ctx, 3)
    srv.start()
    url = f"https://localhost:{srv.port}/"

    requests.get(url)                                  # 1: anonymous
    requests.get(url, cert=(str(pem), str(key)))       # 2: authenticated session
    requests.Session().get(url)                        # 3: unrelated session, NO cert
    srv.join(5)
    summary = [bool(c) for c in srv.peer_certs]
    print("\nPROBE3 server saw client certificate per request:", summary)
    print("PROBE3 third (cert-less, separate Session) peer cert:", srv.peer_certs[2])
    assert summary == [False, True, True]


def test_http_request_via_https_proxy():
    lsock = socket.socket()
    lsock.bind(("127.0.0.1", 0))
    lsock.listen(1)
    port = lsock.getsockname()[1]
    try:
        with pytest.raises(Exception) as ei:
            requests.get(
                "http://example.invalid/",
                proxies={"http": f"https://127.0.0.1:{port}"},
                timeout=3,
            )
    finally:
        lsock.close()
    print("\nPROBE4 http-over-https-proxy raised:", type(ei.value).__module__,
          type(ei.value).__name__, "-", ei.value)
    assert type(ei.value) is ValueError


def test_https_proxy_tunnel_trust_anchors(monkeypatch):
    calls = []

    class Stop(Exception):
        pass

    def recorder(sock, **kw):
        calls.append({k: kw.get(k) for k in ("cert_reqs", "ca_certs", "ca_cert_dir", "ssl_context", "server_hostname")})
        raise Stop()

    monkeypatch.setattr(urllib3.connection, "_ssl_wrap_socket_and_match_hostname", recorder)
    lsock = socket.socket()
    lsock.bind(("127.0.0.1", 0))
    lsock.listen(1)
    port = lsock.getsockname()[1]
    try:
        with pytest.raises(Exception):
            requests.get(
                "https://example.invalid/",
                proxies={"https": f"https://127.0.0.1:{port}"},
                timeout=3,
            )
    finally:
        lsock.close()
    print("\nPROBE5 TLS-to-proxy wrap kwargs:", calls[0])
    assert calls[0]["ca_certs"] is None and calls[0]["ssl_context"] is None


def test_legacy_get_connection_plus_cert_verify():
    a = HTTPAdapter()
    url = "https://example.invalid/"
    conn = a.get_connection(url)
    a.cert_verify(conn, url, True, None)
    print("\nPROBE6 get_connection()+cert_verify(verify=True): cert_reqs=%r ca_certs=%r ca_cert_dir=%r ssl_context=%r"
          % (conn.cert_reqs, conn.ca_certs, conn.ca_cert_dir, conn.conn_kw.get("ssl_context")))
    assert conn.ca_certs is None and conn.conn_kw.get("ssl_context") is None


def test_default_bundle_patch_is_ignored(monkeypatch):
    monkeypatch.setattr(adapters, "DEFAULT_CA_BUNDLE_PATH", "/nonexistent/ca.pem")
    a = HTTPAdapter()
    url = "https://example.invalid/"
    pool = a._get_connection(_prep(url), True)
    a.cert_verify(pool, url, True, None)  # raised OSError before the PR
    print("\nPROBE7 patched DEFAULT_CA_BUNDLE_PATH ignored; cert_verify raised nothing; ca_certs=%r" % pool.ca_certs)


def test_import_time_cost_and_failure(monkeypatch):
    t = time.perf_counter()
    ctx = create_urllib3_context()
    ctx.load_verify_locations(extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))
    print("\nPROBE8 building the preloaded context costs %.1f ms" % ((time.perf_counter() - t) * 1e3))

    import requests.utils
    good = requests.utils.DEFAULT_CA_BUNDLE_PATH
    requests.utils.DEFAULT_CA_BUNDLE_PATH = "/nonexistent/ca.pem"
    try:
        with pytest.raises(Exception) as ei:
            importlib.reload(adapters)
    finally:
        requests.utils.DEFAULT_CA_BUNDLE_PATH = good
        importlib.reload(adapters)
    print("PROBE8 importing requests.adapters with a missing default bundle raised:",
          type(ei.value).__name__, "-", ei.value)
