"""R7a: an adapter's hostname settings and the process-wide TLS context.

Origins: `public` has a good certificate for localhost from a trusted CA;
`wrong-host` has a certificate from the same trusted CA but for the name
other.example; `untrusted` has a certificate for localhost from a CA the
default bundle does not trust. A plain Session must refuse the last two.

Part 1 (single thread): an adapter that passes assert_hostname (or
assert_fingerprint) through init_poolmanager makes one verified request.
Then plain Sessions contact the bad origins again.

Part 2 (single thread): after that, a plain http:// request goes through an
https:// proxy (the request that raises ValueError in probe R1).

Part 3 (threads): plain Sessions keep contacting the two bad origins while
other threads keep sending http:// requests through the https:// proxy. Every
plain-Session request that gets a response is a certificate check that was
skipped. This part is real concurrency; the count varies between runs.
"""
import hashlib
import os
import ssl
import subprocess
import sys
import threading
import time

import helpers

CASE = os.environ.get("R7_CASE")
if CASE is None:
    for index, case in enumerate(("assert_hostname", "assert_fingerprint")):
        env = dict(os.environ, R7_CASE=case, R7_BANNER="1" if index == 0 else "0")
        out = subprocess.run([sys.executable, "-W", "ignore", __file__], env=env,
                             capture_output=True, text=True)
        sys.stdout.write(out.stdout)
        if out.returncode:
            print(f"{case}: interpreter exited {out.returncode}: {out.stderr.strip().splitlines()[-3:]}")
    sys.exit(0)

helpers.use_default_bundle()
import requests  # noqa: E402
import requests.adapters  # noqa: E402
from requests.adapters import HTTPAdapter  # noqa: E402

if os.environ.get("R7_BANNER") == "1":
    helpers.banner()

public = helpers.tls_origin("srv-default", label="public")
wrong_host = helpers.tls_origin("srv-wronghost", label="wrong-host")
untrusted = helpers.tls_origin("srv-private", label="untrusted")
plain = helpers.plain_origin()
proxy = helpers.tls_forward_proxy("srv-default")

if CASE == "assert_hostname":
    setting = {"assert_hostname": "localhost"}
else:
    with open(helpers.cert("srv-default.pem")) as handle:
        der = ssl.PEM_cert_to_DER_cert(handle.read())
    setting = {"assert_fingerprint": hashlib.sha256(der).hexdigest()}


class PinningAdapter(HTTPAdapter):
    def init_poolmanager(self, connections, maxsize, block=False, **pool_kwargs):
        pool_kwargs.update(setting)
        return super().init_poolmanager(connections, maxsize, block=block, **pool_kwargs)


def state():
    context = getattr(requests.adapters, "_preloaded_ssl_context", None)
    if context is None:
        return "no process-wide context exists"
    return (f"process-wide context: check_hostname={context.check_hostname} "
            f"verify_mode={context.verify_mode.name}")


def plain_get(port):
    return lambda: requests.Session().get(f"https://localhost:{port}/", timeout=5)


def via_proxy():
    return requests.get(f"http://127.0.0.1:{plain}/", timeout=5,
                        proxies={"http": f"https://localhost:{proxy}"})


print(f"=== adapter passes {CASE} through init_poolmanager ===")
print(f"1.0 {state()}")
helpers.attempt("1.1 plain Session -> wrong-host (must be refused)", plain_get(wrong_host))
helpers.attempt("1.2 plain Session -> untrusted (must be refused)", plain_get(untrusted))
pinned = requests.Session()
pinned.mount("https://", PinningAdapter())
helpers.attempt(f"1.3 adapter Session with {CASE} -> public, verify=True",
                lambda: pinned.get(f"https://localhost:{public}/", timeout=5))
print(f"1.4 {state()}")
helpers.attempt("1.5 plain Session -> wrong-host (must be refused)", plain_get(wrong_host))
helpers.attempt("1.6 plain Session -> untrusted (must be refused)", plain_get(untrusted))
helpers.attempt("1.7 plain Session -> public (must work)", plain_get(public))

helpers.attempt("2.1 http:// URL via https:// proxy, default verify", via_proxy)
print(f"2.2 {state()}")
helpers.attempt("2.3 plain Session -> wrong-host (must be refused)", plain_get(wrong_host))
helpers.attempt("2.4 plain Session -> untrusted (must be refused)", plain_get(untrusted))
print(f"2.5 {state()}")

SECONDS = float(os.environ.get("R7_SECONDS", "20"))
stop = time.monotonic() + SECONDS
counts = {"wrong-host": [0, 0], "untrusted": [0, 0], "proxy": [0, 0]}
lock = threading.Lock()


def victim(name, port):
    while time.monotonic() < stop:
        try:
            requests.Session().get(f"https://localhost:{port}/", timeout=5)
            accepted = 1
        except Exception:  # noqa: BLE001
            accepted = 0
        with lock:
            counts[name][0] += 1
            counts[name][1] += accepted


def proxy_user():
    while time.monotonic() < stop:
        try:
            via_proxy()
            ok = 1
        except Exception:  # noqa: BLE001
            ok = 0
        with lock:
            counts["proxy"][0] += 1
            counts["proxy"][1] += ok


threads = [threading.Thread(target=victim, args=("wrong-host", wrong_host)) for _ in range(3)]
threads += [threading.Thread(target=victim, args=("untrusted", untrusted)) for _ in range(3)]
threads += [threading.Thread(target=proxy_user) for _ in range(3)]
for thread in threads:
    thread.start()
for thread in threads:
    thread.join()
print(f"3.0 {SECONDS:.0f} s, 3 threads per kind, default interpreter thread switching")
print(f"3.1 plain verify=True requests to wrong-host: {counts['wrong-host'][0]} sent, "
      f"{counts['wrong-host'][1]} WRONGLY ACCEPTED")
print(f"3.2 plain verify=True requests to untrusted:  {counts['untrusted'][0]} sent, "
      f"{counts['untrusted'][1]} WRONGLY ACCEPTED")
print(f"3.3 http:// requests through the https:// proxy: {counts['proxy'][0]} sent, "
      f"{counts['proxy'][1]} succeeded")
