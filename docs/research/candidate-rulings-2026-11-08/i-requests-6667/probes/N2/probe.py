"""Whose SSLContext a verify=True request uses when an HTTPAdapter subclass supplies one through
proxy_manager_for (this group) or init_poolmanager (GT-i1).

Usage: python -I probe.py <tree> <label> <material dir>

<tree> holds src/requests. A stand-in certifi makes the default CA file hold one local CA ("default"). A second,
private CA signs the "private" server; the default CA file does not hold it. Each adapter builds its context with
urllib3's create_urllib3_context() and either trusts the private CA, or trusts the default CA and caps TLS at 1.2.
Requests go through a local http:// proxy (CONNECT) when the case says so. For each case the probe prints what the
request returned (the server answers with the TLS version) and whether the destination pool holds the adapter's
context, requests' shared context or another one.
"""
import json
import os
import ssl
import subprocess
import sys

tree, label, directory = sys.argv[1:4]
import trustme  # noqa: E402

os.makedirs(directory, exist_ok=True)
for name in ("default", "private"):
    ca = trustme.CA()
    ca.cert_pem.write_to_path(os.path.join(directory, f"{name}-ca.pem"))
    leaf = ca.issue_cert("localhost", "127.0.0.1")
    leaf.private_key_pem.write_to_path(os.path.join(directory, f"{name}.key"))
    leaf.cert_chain_pems[0].write_to_path(os.path.join(directory, f"{name}.crt"))
fake = os.path.join(directory, "standin", "certifi")
os.makedirs(fake, exist_ok=True)
with open(os.path.join(fake, "__init__.py"), "w") as f:
    f.write("def where():\n    return %r\n\ndef contents():\n    return open(where()).read()\n" % os.path.join(directory, "default-ca.pem"))
sys.path.insert(0, os.path.join(directory, "standin"))
sys.path.insert(0, os.path.join(tree, "src"))
import requests  # noqa: E402
import requests.adapters  # noqa: E402
import urllib3  # noqa: E402
from urllib3.util.ssl_ import create_urllib3_context  # noqa: E402

assert requests.__file__.startswith(os.path.join(tree, "src")), requests.__file__
process = subprocess.Popen([sys.executable, "-I", os.path.join(os.path.dirname(os.path.abspath(__file__)), "servers.py"), directory],
                           stdout=subprocess.PIPE, text=True)
ports = json.loads(process.stdout.readline())
proxy = f"http://127.0.0.1:{ports['proxy']}"
PROXIES = {"https": proxy}


def private_context():
    context = create_urllib3_context()
    context.load_verify_locations(os.path.join(directory, "private-ca.pem"))
    return context


def capped_context():
    context = create_urllib3_context()
    context.load_verify_locations(os.path.join(directory, "default-ca.pem"))
    context.maximum_version = ssl.TLSVersion.TLSv1_2
    return context


class ProxyAdapter(requests.adapters.HTTPAdapter):
    """Supplies its context to the proxy manager, as proxy_manager_for's docstring allows for subclasses."""

    def __init__(self, make, **kwargs):
        self.context = make()
        super().__init__(**kwargs)

    def proxy_manager_for(self, proxy, **proxy_kwargs):
        proxy_kwargs["ssl_context"] = self.context
        return super().proxy_manager_for(proxy, **proxy_kwargs)


class PoolAdapter(requests.adapters.HTTPAdapter):
    """GT-i1's trigger: supplies its context to the pool manager through init_poolmanager."""

    def __init__(self, make, **kwargs):
        self.context = make()
        super().__init__(**kwargs)

    def init_poolmanager(self, *args, **kwargs):
        kwargs["ssl_context"] = self.context
        return super().init_poolmanager(*args, **kwargs)


class BothAdapter(PoolAdapter):
    """Supplies its context through both hooks."""

    def proxy_manager_for(self, proxy, **proxy_kwargs):
        proxy_kwargs["ssl_context"] = self.context
        return super().proxy_manager_for(proxy, **proxy_kwargs)


def tunnels():
    return int(requests.get(f"{proxy}/count", proxies={"http": None, "https": None}).text)


def whose(adapter, url, proxies):
    getter = getattr(adapter, "_get_connection", None) or getattr(adapter, "get_connection_with_tls_context")
    request = requests.Request("GET", url).prepare()
    pool = getter(request, True, proxies=proxies)
    held = pool.conn_kw.get("ssl_context")
    if held is not None and held is adapter.context:
        return "the adapter's context"
    if held is not None and held is getattr(requests.adapters, "_preloaded_ssl_context", object()):
        return "requests' shared context"
    return "no context (urllib3 builds one)" if held is None else "another context"


def case(name, make, adapter_class, server, through_proxy, verify=True):
    session = requests.Session()
    session.trust_env = False
    if adapter_class is requests.adapters.HTTPAdapter:
        adapter = adapter_class()
        adapter.context = None
    else:
        adapter = adapter_class(make)
    session.mount("https://", adapter)
    url = f"https://localhost:{ports[server]}/"
    proxies = PROXIES if through_proxy else {}
    before = tunnels()
    try:
        response = session.get(url, proxies=proxies, verify=verify, timeout=20)
        result = f"{response.status_code} {response.text}"
    except Exception as error:  # noqa: BLE001
        text = f"{type(error).__name__}: {error}"
        reason = "CERTIFICATE_VERIFY_FAILED" if "CERTIFICATE_VERIFY_FAILED" in text else text[:160]
        result = f"raises {type(error).__name__} ({reason})"
    through = tunnels() - before
    pool = whose(adapter, url, proxies) if verify is True else "-"
    print(f"  {name}\n    -> {result}; tunnels through the proxy: {through}; destination pool holds: {pool}", flush=True)


print(f"== {label}: requests {requests.__version__}, urllib3 {urllib3.__version__}, Python {sys.version.split()[0]}", flush=True)
try:
    case("1. proxy_manager_for adapter, context trusts the private CA; through the proxy; verify=True", private_context, ProxyAdapter, "private", True)
    case("2. proxy_manager_for adapter, default CA, TLS capped at 1.2; through the proxy; verify=True", capped_context, ProxyAdapter, "default", True)
    case("3. init_poolmanager adapter (GT-i1), context trusts the private CA; direct; verify=True", private_context, PoolAdapter, "private", False)
    case("4. init_poolmanager adapter (GT-i1), default CA, TLS capped at 1.2; direct; verify=True", capped_context, PoolAdapter, "default", False)
    case("5. adapter using both hooks, context trusts the private CA; through the proxy; verify=True", private_context, BothAdapter, "private", True)
    case("6. adapter using both hooks, context trusts the private CA; direct; verify=True", private_context, BothAdapter, "private", False)
    case("7. proxy_manager_for adapter; through the proxy; verify=<private CA file> (control)", private_context, ProxyAdapter, "private", True,
         verify=os.path.join(directory, "private-ca.pem"))
    case("8. plain HTTPAdapter; through the proxy; verify=True to the private server (control)", private_context, requests.adapters.HTTPAdapter, "private", True)
finally:
    process.kill()
