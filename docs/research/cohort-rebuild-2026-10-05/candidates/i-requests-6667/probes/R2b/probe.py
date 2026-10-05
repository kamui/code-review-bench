"""R2b: truststore.inject_into_ssl() called after `import requests`.

truststore makes Python verify certificates against the operating system's
trust store. On Linux that store is whatever OpenSSL's default paths name, so
SSL_CERT_FILE points it at `ca-private` here. One TLS origin has a certificate
from `ca-private`; the certifi bundle does not contain that CA.
"""
import os
import subprocess
import sys
import traceback

import helpers

CASE = os.environ.get("R2B_CASE")
NEWER = os.path.join(os.environ["PROBE_SCRATCH"], "alt", "truststore-latest")

if CASE is None:
    cases = ("inject-before-import", "inject-after-import", "inject-after-import+verify-path",
             "inject-after-import+verify-false", "inject-after-import+newer-truststore")
    for index, case in enumerate(cases):
        env = dict(os.environ, R2B_CASE=case, R2B_BANNER="1" if index == 0 else "0")
        out = subprocess.run([sys.executable, "-W", "ignore", __file__], env=env,
                             capture_output=True, text=True)
        sys.stdout.write(out.stdout)
        if out.returncode:
            print(f"{case}: interpreter exited {out.returncode}: {out.stderr.strip().splitlines()[-1:]}")
    sys.exit(0)

os.environ["SSL_CERT_FILE"] = helpers.cert("ca-private.pem")
if CASE.endswith("newer-truststore"):
    sys.path.insert(0, NEWER)
import truststore  # noqa: E402

if CASE.endswith("newer-truststore"):
    version = next(d for d in os.listdir(NEWER) if d.endswith(".dist-info")).split("-")[1].replace(".dist", "")
else:
    from importlib.metadata import version as installed_version

    version = installed_version("truststore")

if CASE == "inject-before-import":
    truststore.inject_into_ssl()
import requests  # noqa: E402

if os.environ.get("R2B_BANNER") == "1":
    helpers.banner()
if CASE != "inject-before-import":
    truststore.inject_into_ssl()

port = helpers.tls_origin("srv-private")
kwargs = {}
if CASE.endswith("verify-path"):
    kwargs["verify"] = helpers.cert("ca-private.pem")
if CASE.endswith("verify-false"):
    kwargs["verify"] = False

result = helpers.attempt(f"{CASE} (truststore {version})",
                         lambda: requests.get(f"https://localhost:{port}/", timeout=5, **kwargs))
if isinstance(result, RecursionError):
    frames = traceback.extract_tb(result.__traceback__)
    entry = next(i for i, f in enumerate(frames) if f.filename.endswith("ssl.py"))
    for frame in frames[max(0, entry - 2):entry + 3]:
        short = frame.filename.split("site-packages/")[-1].split("/src/")[-1]
        short = short if "/" in short and not short.startswith("/") else os.path.basename(short)
        print(f"    {short}:{frame.lineno} in {frame.name}: {frame.line}")
    print(f"    ... the last frame repeats {len(frames) - entry - 1} more times")
