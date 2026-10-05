"""R1, second probe: what the same request does to other threads.

No custom adapter is involved. Some threads keep sending a plain http://
request through an https:// proxy with default verify (the request of
probe.py). Other threads keep making ordinary verify=True requests, each on a
fresh Session, to two origins that must be refused: `wrong-host` (trusted CA,
certificate for another name) and `untrusted` (CA not in the default bundle).
Every such request that gets a response is a certificate check that was
skipped. This is real concurrency; counts vary between runs.

Run with urllib3 2.2.1 and with urllib3 1.26.18, at both commits.
"""
import os
import threading
import time

import helpers

helpers.use_default_bundle()
import requests  # noqa: E402
import requests.adapters  # noqa: E402

helpers.banner()

wrong_host = helpers.tls_origin("srv-wronghost", label="wrong-host")
untrusted = helpers.tls_origin("srv-private", label="untrusted")
plain = helpers.plain_origin()
proxy = helpers.tls_forward_proxy("srv-default")


def state():
    context = getattr(requests.adapters, "_preloaded_ssl_context", None)
    if context is None:
        return "no process-wide context exists"
    return (f"process-wide context: check_hostname={context.check_hostname} "
            f"verify_mode={context.verify_mode.name}")


def via_proxy():
    return requests.get(f"http://127.0.0.1:{plain}/", timeout=5,
                        proxies={"http": f"https://localhost:{proxy}"})


def plain_get(port):
    return lambda: requests.Session().get(f"https://localhost:{port}/", timeout=5)


print(f"0 at start: {state()}")
helpers.attempt("1 plain Session -> wrong-host (must be refused)", plain_get(wrong_host))
helpers.attempt("2 plain Session -> untrusted (must be refused)", plain_get(untrusted))
helpers.attempt("3 http:// URL via https:// proxy, default verify", via_proxy)
print(f"4 after that request: {state()}")
helpers.attempt("5 plain Session -> wrong-host, single thread (must be refused)", plain_get(wrong_host))
helpers.attempt("6 plain Session -> untrusted, single thread (must be refused)", plain_get(untrusted))

SECONDS = float(os.environ.get("R1_SECONDS", "20"))
stop = time.monotonic() + SECONDS
counts = {"wrong-host": [0, 0], "untrusted": [0, 0], "proxy": [0, 0]}
lock = threading.Lock()


def loop(name, call):
    while time.monotonic() < stop:
        try:
            call()
            ok = 1
        except Exception:  # noqa: BLE001
            ok = 0
        with lock:
            counts[name][0] += 1
            counts[name][1] += ok


threads = [threading.Thread(target=loop, args=("wrong-host", plain_get(wrong_host))) for _ in range(3)]
threads += [threading.Thread(target=loop, args=("untrusted", plain_get(untrusted))) for _ in range(3)]
threads += [threading.Thread(target=loop, args=("proxy", via_proxy)) for _ in range(3)]
for thread in threads:
    thread.start()
for thread in threads:
    thread.join()
print(f"7 {SECONDS:.0f} s, 3 threads per kind, default interpreter thread switching")
print(f"8 plain verify=True requests to wrong-host: {counts['wrong-host'][0]} sent, "
      f"{counts['wrong-host'][1]} WRONGLY ACCEPTED")
print(f"9 plain verify=True requests to untrusted:  {counts['untrusted'][0]} sent, "
      f"{counts['untrusted'][1]} WRONGLY ACCEPTED")
print(f"10 http:// requests through the https:// proxy: {counts['proxy'][0]} sent, "
      f"{counts['proxy'][1]} succeeded")
