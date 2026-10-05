"""R3: CA material that one adapter sets on its own pools, under verify=True.

Two TLS origins have certificates from `ca-private`, which the default bundle
does not trust. A third has a certificate from `ca-default`, which it does.
An adapter subclass passes the private CA to its pool manager through
init_poolmanager. After it makes one verified request, a different, plain
Session (no custom adapter) contacts the second private origin.
"""
import os
import subprocess
import sys

import helpers

CASE = os.environ.get("R3_CASE")
if CASE is None:
    first = True
    for case in ("ca_cert_data", "ca_certs", "ca_cert_dir"):
        env = dict(os.environ, R3_CASE=case, R3_BANNER="1" if first else "0")
        out = subprocess.run([sys.executable, "-W", "ignore", __file__], env=env,
                             capture_output=True, text=True)
        sys.stdout.write(out.stdout)
        if out.returncode:
            print(f"{case}: interpreter exited {out.returncode}: {out.stderr.strip().splitlines()[-1:]}")
        first = False
    sys.exit(0)

helpers.use_default_bundle()
import requests  # noqa: E402
import requests.adapters  # noqa: E402
from requests.adapters import HTTPAdapter  # noqa: E402

if os.environ.get("R3_BANNER") == "1":
    helpers.banner()

private_a = helpers.tls_origin("srv-private", label="private-A")
private_b = helpers.tls_origin("srv-private", label="private-B")
public = helpers.tls_origin("srv-default", label="public")

if CASE == "ca_cert_data":
    with open(helpers.cert("ca-private.pem")) as handle:
        extra = {"ca_cert_data": handle.read()}
elif CASE == "ca_certs":
    extra = {"ca_certs": helpers.cert("ca-private.pem")}
else:
    import shutil
    import tempfile

    cadir = tempfile.mkdtemp(prefix="r3-cadir-")
    shutil.copy(helpers.cert("ca-private.pem"), cadir)
    subprocess.run(["openssl", "rehash", cadir], check=True, capture_output=True)
    extra = {"ca_cert_dir": cadir}


class PrivateCAAdapter(HTTPAdapter):
    def init_poolmanager(self, connections, maxsize, block=False, **pool_kwargs):
        pool_kwargs.update(extra)
        return super().init_poolmanager(connections, maxsize, block=block, **pool_kwargs)


def shared_store():
    context = getattr(requests.adapters, "_preloaded_ssl_context", None)
    if context is None:
        return "no process-wide context exists"
    return f"process-wide context holds {context.cert_store_stats()['x509_ca']} CA certificates"


print(f"=== adapter passes {CASE} through init_poolmanager ===")
print(f"0 {shared_store()}")
helpers.attempt("1 plain Session -> private-B, before the adapter is used (must be refused)",
                lambda: requests.Session().get(f"https://localhost:{private_b}/", timeout=5))
custom = requests.Session()
custom.mount("https://", PrivateCAAdapter())
helpers.attempt("2 adapter Session -> private-A, verify=True",
                lambda: custom.get(f"https://localhost:{private_a}/", timeout=5))
helpers.attempt("3 adapter Session -> public, verify=True",
                lambda: custom.get(f"https://localhost:{public}/", timeout=5))
print(f"4 {shared_store()}")
helpers.attempt("5 plain Session -> private-B, after the adapter was used (must still be refused)",
                lambda: requests.Session().get(f"https://localhost:{private_b}/", timeout=5))
helpers.attempt("6 requests.get -> private-B, after the adapter was used (must still be refused)",
                lambda: requests.get(f"https://localhost:{private_b}/", timeout=5))
