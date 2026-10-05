"""R1: a plain http:// request sent through an https:// proxy, default verify.

A plain-HTTP origin and a TLS forwarding proxy run on spare local ports.
The proxy's certificate chains to `ca-private`, which the default bundle does
not trust, and once more to `ca-default`, which it does.
"""
import os

import helpers

helpers.use_default_bundle()
import requests  # noqa: E402
import requests.adapters  # noqa: E402

helpers.banner()

origin = helpers.plain_origin()
untrusted_https = helpers.tls_origin("srv-private", label="untrusted-https")
proxy_untrusted = helpers.tls_forward_proxy("srv-private")
proxy_trusted = helpers.tls_forward_proxy("srv-default")
url = f"http://127.0.0.1:{origin}/"


def via(port, **kwargs):
    proxies = {"http": f"https://localhost:{port}"}
    return lambda: requests.get(url, proxies=proxies, timeout=5, **kwargs)


helpers.attempt("1 direct, no proxy", lambda: requests.get(url, timeout=5))
helpers.attempt("2 http URL via https proxy (proxy cert from a trusted CA), default verify", via(proxy_trusted))
helpers.attempt("3 http URL via https proxy (proxy cert from an untrusted CA), default verify", via(proxy_untrusted))
helpers.attempt("4 same as 2 with verify=False", via(proxy_trusted, verify=False))
helpers.attempt("5 same as 2 with verify=<CA file>", via(proxy_trusted, verify=helpers.cert("ca-default.pem")))

os.environ["http_proxy"] = f"https://localhost:{proxy_trusted}"
helpers.attempt("6 proxy taken from the http_proxy environment variable", lambda: requests.get(url, timeout=5))
del os.environ["http_proxy"]

session = requests.Session()
session.proxies = {"http": f"https://localhost:{proxy_trusted}"}
helpers.attempt("7 Session with proxies set", lambda: session.get(url, timeout=5))

try:
    requests.get(url, proxies={"http": f"https://localhost:{proxy_trusted}"}, timeout=5)
    print("8 `except requests.RequestException` handler: not needed, request succeeded")
except requests.RequestException as exc:
    print(f"8 `except requests.RequestException` handler: caught {type(exc).__name__}")
except Exception as exc:  # noqa: BLE001
    print(f"8 `except requests.RequestException` handler: MISSED, {type(exc).__name__} escaped it")


class NoVerifyAdapter(requests.adapters.HTTPAdapter):
    """Second route to the same pairing: a subclass that turns verification off
    in cert_verify (the shape of exchangelib's NoVerifyHTTPAdapter)."""

    def cert_verify(self, conn, url, verify, cert):
        super().cert_verify(conn=conn, url=url, verify=False, cert=cert)


no_verify = requests.Session()
no_verify.mount("https://", NoVerifyAdapter())
helpers.attempt("9 adapter that disables verification in cert_verify -> https origin with an untrusted certificate",
                lambda: no_verify.get(f"https://localhost:{untrusted_https}/", timeout=5))
