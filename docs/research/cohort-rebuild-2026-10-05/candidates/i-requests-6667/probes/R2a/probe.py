"""R2a: changes made to the default CA bundle after `import requests`.

The default bundle (certifi.where()) is a writable copy of the real certifi
bundle. One TLS origin has a certificate from `ca-private`, which that bundle
does not contain. Each case changes the default trust after import in one way
and then makes a fresh verify=True request (a new Session, so a new connection).
"""
import os
import shutil
import subprocess
import sys
import tempfile

import helpers

CASE = os.environ.get("R2_CASE")

if CASE is None:
    # Parent: run every case in its own interpreter so they cannot affect each other.
    printed_banner = False
    for case in ("control", "rebind-adapters", "rebind-utils", "patch-certs-where",
                 "rewrite-file", "delete-file", "documented-env-var", "documented-verify-path"):
        env = dict(os.environ, R2_CASE=case, R2_BANNER="0" if printed_banner else "1")
        out = subprocess.run([sys.executable, "-W", "ignore", __file__], env=env,
                             capture_output=True, text=True)
        sys.stdout.write(out.stdout)
        if out.returncode:
            tail = out.stderr.strip().splitlines()[-1] if out.stderr.strip() else ""
            print(f"{case}: interpreter exited {out.returncode}: {tail}")
        printed_banner = True
    sys.exit(0)

workdir = tempfile.mkdtemp(prefix="r2-")
bundle = os.path.join(workdir, "cacert.pem")
import certifi  # noqa: E402

shutil.copy(certifi.where(), bundle)  # the real certifi bundle, no probe CA in it
private_ca = helpers.cert("ca-private.pem")

if CASE == "documented-env-var":
    os.environ["REQUESTS_CA_BUNDLE"] = private_ca

helpers.use_default_bundle(bundle)
import requests  # noqa: E402
import requests.adapters  # noqa: E402
import requests.utils  # noqa: E402

if os.environ.get("R2_BANNER") == "1":
    helpers.banner()

port = helpers.tls_origin("srv-private")
url = f"https://localhost:{port}/"
kwargs = {}

if CASE == "control":
    what = "no change after import (the origin's CA is not in the default bundle)"
elif CASE == "rebind-adapters":
    what = "requests.adapters.DEFAULT_CA_BUNDLE_PATH = <private CA> after import"
    requests.adapters.DEFAULT_CA_BUNDLE_PATH = private_ca
elif CASE == "rebind-utils":
    what = "requests.utils.DEFAULT_CA_BUNDLE_PATH = <private CA> after import"
    requests.utils.DEFAULT_CA_BUNDLE_PATH = private_ca
elif CASE == "patch-certs-where":
    what = "requests.certs.where / certifi.where patched after import"
    requests.certs.where = lambda: private_ca
    certifi.where = requests.certs.where
elif CASE == "rewrite-file":
    what = "default bundle file rewritten in place after import (private CA appended)"
    with open(bundle, "a") as handle, open(private_ca) as extra:
        handle.write(extra.read())
elif CASE == "delete-file":
    what = "default bundle file deleted after import"
    os.remove(bundle)
elif CASE == "documented-env-var":
    what = "REQUESTS_CA_BUNDLE=<private CA> (documented override)"
elif CASE == "documented-verify-path":
    what = "verify=<private CA> (documented override)"
    kwargs["verify"] = private_ca

helpers.attempt(f"{CASE}: {what}", lambda: requests.get(url, timeout=5, **kwargs))
shutil.rmtree(workdir, ignore_errors=True)
