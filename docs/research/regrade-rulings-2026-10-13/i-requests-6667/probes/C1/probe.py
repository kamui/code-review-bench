"""Credit groups C1 and C2 of i-requests-6667: what an adapter's TLS settings do on a verified request.

Run once per commit with the same interpreter and dependencies; only PYTHONPATH changes.
Two local TLS origins hold a certificate the default bundle trusts. One speaks TLS 1.2 only,
the other TLS 1.2 and 1.3. A third holds a certificate from a private CA. Each origin replies
with the TLS version it negotiated. For every request the probe prints what the caller got,
which context the connection used with the versions that context allows, and what the pool
was given.

To see which context a connection uses, the probe wraps ssl.SSLContext.wrap_socket and records
the context of each client-side call. The wrapper changes nothing and is the same at both commits.
"""
import ssl

import helpers

BUNDLE = helpers.use_default_bundle()
import requests  # noqa: E402
import requests.adapters as adapters_module  # noqa: E402
import urllib3  # noqa: E402
from requests.adapters import HTTPAdapter  # noqa: E402
from urllib3.poolmanager import PoolManager  # noqa: E402
from urllib3.util.ssl_ import create_urllib3_context  # noqa: E402

helpers.banner()

observed = []
_wrap_socket = ssl.SSLContext.wrap_socket


def _recording_wrap_socket(self, sock, *args, **kwargs):
    client_side = not kwargs.get("server_side", args[0] if args else False)
    if client_side:
        observed.append({"context": self})
    wrapped = _wrap_socket(self, sock, *args, **kwargs)
    if client_side:
        observed[-1]["cipher"] = wrapped.cipher()[0]
    return wrapped


ssl.SSLContext.wrap_socket = _recording_wrap_socket

SHARED = getattr(adapters_module, "_preloaded_ssl_context", None)
print("requests.adapters._preloaded_ssl_context (the context built at import): "
      + ("absent at this commit" if SHARED is None else f"present, allows {SHARED.minimum_version.name} "
         f"({int(SHARED.minimum_version)}) to {SHARED.maximum_version.name}"))

only12 = helpers.tls_origin("srv-default", label="tls1.2-only", maximum=ssl.TLSVersion.TLSv1_2)
modern = helpers.tls_origin("srv-default", label="tls1.2+1.3")
private = helpers.tls_origin("srv-private", label="private-ca", maximum=ssl.TLSVersion.TLSv1_2)


def which(context, own=None):
    if context is None:
        return "none"
    if SHARED is not None and context is SHARED:
        name = "the shared context requests built at import"
    elif own is not None and context is own:
        name = "the adapter's own context"
    else:
        name = "a context urllib3 built for this pool"
    return (f"{name}; allows {context.minimum_version.name} ({int(context.minimum_version)})"
            f" to {context.maximum_version.name}")


def pool_settings(adapter, own=None):
    manager = adapter.poolmanager
    lines = []
    for key in manager.pools.keys():
        pool = manager.pools[key]
        if pool.scheme != "https":
            continue

        def setting(name):
            value = getattr(pool, name, None)
            return pool.conn_kw.get(name) if value is None else value

        given = pool.conn_kw.get("ssl_context")
        given_name = "none" if given is None else which(given, own).split(";")[0]
        lines.append(f"ssl_context={given_name}, ssl_minimum_version={setting('ssl_minimum_version')!r}, "
                     f"ssl_maximum_version={setting('ssl_maximum_version')!r}, ssl_version={setting('ssl_version')!r}")
    return lines or ["no https pool"]


def attempt(label, session, port, own=None, **kwargs):
    print()
    print(label)
    adapter = session.get_adapter("https://localhost/")
    del observed[:]
    try:
        response = session.get(f"https://localhost:{port}/", timeout=5, **kwargs)
        print(f"  caller gets: {response.status_code} {response.text}; cipher {observed[-1].get('cipher', '?')}")
    except BaseException as exc:  # noqa: BLE001 - the probe reports every outcome
        text = str(exc).replace("\n", " ")
        text = text[text.find("(Caused by"):] if "(Caused by" in text else text
        print(f"  caller gets: RAISED {type(exc).__module__}.{type(exc).__name__}: {text[:260]}")
    print(f"  TLS handshakes started: {len(observed)}")
    for seen in observed:
        print(f"  context the connection used: {which(seen['context'], own)}")
    for line in pool_settings(adapter, own):
        print(f"  pool was given: {line}")


def adapter_with(**tls_settings):
    class SettingsAdapter(HTTPAdapter):
        def init_poolmanager(self, connections, maxsize, block=False, **pool_kwargs):
            pool_kwargs.update(tls_settings)
            return super().init_poolmanager(connections, maxsize, block=block, **pool_kwargs)

    session = requests.Session()
    session.mount("https://", SettingsAdapter())
    return session


class DocsStyleAdapter(HTTPAdapter):
    """The shape of docs/user/advanced.rst 'Example: Specific SSL Version',
    with TLS 1.2 in place of the documented SSLv3 (which OpenSSL no longer has)."""

    def init_poolmanager(self, connections, maxsize, block=False):
        self.poolmanager = PoolManager(
            num_pools=connections, maxsize=maxsize, block=block,
            ssl_version=ssl.PROTOCOL_TLSv1_2)


print()
print("=== A. Version settings passed through init_poolmanager, no context from the adapter ===")
attempt("A0 plain Session -> tls1.2+1.3 origin, verify=True (control)", requests.Session(), modern)
attempt("A1 adapter sets ssl_minimum_version=TLSv1_3 -> tls1.2-only origin, verify=True (the setting says: refuse)",
        adapter_with(ssl_minimum_version=ssl.TLSVersion.TLSv1_3), only12)
docs = requests.Session()
docs.mount("https://", DocsStyleAdapter())
attempt("A2 documented-shape adapter sets ssl_version=PROTOCOL_TLSv1_2 -> tls1.2+1.3 origin, verify=True (the setting says: TLSv1.2)",
        docs, modern)
attempt("A3 adapter sets ssl_maximum_version=TLSv1_2 -> tls1.2+1.3 origin, verify=True (the setting says: TLSv1.2)",
        adapter_with(ssl_maximum_version=ssl.TLSVersion.TLSv1_2), modern)
attempt("A4 same adapter as A1, verify=<CA file> -> tls1.2-only origin (the setting says: refuse)",
        adapter_with(ssl_minimum_version=ssl.TLSVersion.TLSv1_3), only12, verify=helpers.cert("ca-default.pem"))
attempt("A5 same adapter as A3, verify=False -> tls1.2+1.3 origin (the setting says: TLSv1.2)",
        adapter_with(ssl_maximum_version=ssl.TLSVersion.TLSv1_2), modern, verify=False)

print()
print("=== B. The adapter passes its own ssl_context through init_poolmanager ===")
own13 = create_urllib3_context()
own13.load_verify_locations(BUNDLE)
own13.minimum_version = ssl.TLSVersion.TLSv1_3
attempt("B1 own context with minimum_version=TLSv1_3 -> tls1.2-only origin, verify=True (the context says: refuse)",
        adapter_with(ssl_context=own13), only12, own=own13)
own_ca = create_urllib3_context()
own_ca.load_verify_locations(helpers.cert("ca-private.pem"))
attempt("B2 own context trusting a private CA -> private-ca origin, verify=True (the context says: trusted)",
        adapter_with(ssl_context=own_ca), private, own=own_ca)
own_cipher = create_urllib3_context()
own_cipher.load_verify_locations(BUNDLE)
own_cipher.set_ciphers("ECDHE-RSA-AES128-GCM-SHA256")
attempt("B3 own context limited to cipher ECDHE-RSA-AES128-GCM-SHA256 -> tls1.2-only origin, verify=True",
        adapter_with(ssl_context=own_cipher), only12, own=own_cipher)

print()
print("=== C. urllib3 alone, no requests code: version settings with and without a context ===")


def direct(label, port, **manager_kwargs):
    manager = PoolManager(cert_reqs="CERT_REQUIRED", retries=False, **manager_kwargs)
    try:
        response = manager.request("GET", f"https://localhost:{port}/", timeout=5)
        print(f"{label}: {response.status} {response.data.decode()}")
    except BaseException as exc:  # noqa: BLE001
        print(f"{label}: RAISED {type(exc).__module__}.{type(exc).__name__}: {str(exc)[:160]}")


def default_context():
    context = create_urllib3_context()
    context.load_verify_locations(BUNDLE)
    return context


direct("C1 ssl_minimum_version=TLSv1_3, no ssl_context -> tls1.2-only origin", only12,
       ca_certs=BUNDLE, ssl_minimum_version=ssl.TLSVersion.TLSv1_3)
direct("C2 ssl_minimum_version=TLSv1_3 and a ready-made ssl_context -> tls1.2-only origin", only12,
       ssl_minimum_version=ssl.TLSVersion.TLSv1_3, ssl_context=default_context())
direct("C3 ssl_version=PROTOCOL_TLSv1_2, no ssl_context -> tls1.2+1.3 origin", modern,
       ca_certs=BUNDLE, ssl_version=ssl.PROTOCOL_TLSv1_2)
direct("C4 ssl_version=PROTOCOL_TLSv1_2 and a ready-made ssl_context -> tls1.2+1.3 origin", modern,
       ssl_version=ssl.PROTOCOL_TLSv1_2, ssl_context=default_context())
direct("C5 ssl_maximum_version=TLSv1_2, no ssl_context -> tls1.2+1.3 origin", modern,
       ca_certs=BUNDLE, ssl_maximum_version=ssl.TLSVersion.TLSv1_2)
direct("C6 ssl_maximum_version=TLSv1_2 and a ready-made ssl_context -> tls1.2+1.3 origin", modern,
       ssl_maximum_version=ssl.TLSVersion.TLSv1_2, ssl_context=default_context())
