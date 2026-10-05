"""R4: TLS version settings an adapter passes through init_poolmanager.

Both origins have a certificate from `ca-default`, which the default bundle
trusts, so plain verify=True works against them. One origin speaks TLS 1.2
only; the other speaks TLS 1.2 and 1.3. Each origin reports the TLS version
that was negotiated.
"""
import ssl

import helpers

helpers.use_default_bundle()
import requests  # noqa: E402
from requests.adapters import HTTPAdapter  # noqa: E402
from urllib3.poolmanager import PoolManager  # noqa: E402

helpers.banner()

only12 = helpers.tls_origin("srv-default", label="tls1.2-only", maximum=ssl.TLSVersion.TLSv1_2)
modern = helpers.tls_origin("srv-default", label="tls1.2+1.3")


def adapter_with(**tls_settings):
    class VersionAdapter(HTTPAdapter):
        def init_poolmanager(self, connections, maxsize, block=False, **pool_kwargs):
            pool_kwargs.update(tls_settings)
            return super().init_poolmanager(connections, maxsize, block=block, **pool_kwargs)

    session = requests.Session()
    session.mount("https://", VersionAdapter())
    return session


class DocsStyleAdapter(HTTPAdapter):
    """The shape of docs/user/advanced.rst 'Example: Specific SSL Version',
    with TLS 1.2 in place of the documented SSLv3 (which OpenSSL no longer has)."""

    def init_poolmanager(self, connections, maxsize, block=False):
        self.poolmanager = PoolManager(
            num_pools=connections, maxsize=maxsize, block=block,
            ssl_version=ssl.PROTOCOL_TLSv1_2)


def get(session, port, **kwargs):
    return lambda: session.get(f"https://localhost:{port}/", timeout=5, **kwargs)


helpers.attempt("0 plain Session -> tls1.2+1.3 origin (control)", get(requests.Session(), modern))

tls13_only = adapter_with(ssl_minimum_version=ssl.TLSVersion.TLSv1_3)
helpers.attempt("1 adapter requires TLS >= 1.3 -> tls1.2-only origin, verify=True (adapter says: refuse)",
                get(tls13_only, only12))

cap12 = adapter_with(ssl_maximum_version=ssl.TLSVersion.TLSv1_2)
helpers.attempt("2 adapter caps TLS at 1.2 -> tls1.2+1.3 origin, verify=True (adapter says: TLSv1.2)",
                get(cap12, modern))

docs = requests.Session()
docs.mount("https://", DocsStyleAdapter())
helpers.attempt("3 docs-style adapter ssl_version=PROTOCOL_TLSv1_2 -> tls1.2+1.3 origin, verify=True (adapter says: TLSv1.2)",
                get(docs, modern))

helpers.attempt("4 same adapter as 1, verify=<CA file> -> tls1.2-only origin (adapter says: refuse)",
                get(adapter_with(ssl_minimum_version=ssl.TLSVersion.TLSv1_3), only12,
                    verify=helpers.cert("ca-default.pem")))
helpers.attempt("5 same adapter as 2, verify=False -> tls1.2+1.3 origin (adapter says: TLSv1.2)",
                get(adapter_with(ssl_maximum_version=ssl.TLSVersion.TLSv1_2), modern, verify=False))
