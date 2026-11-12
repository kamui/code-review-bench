"""Through requests: what a request whose client-certificate load fails does to other verified requests.

Usage: python -I probe.py <tree> <label> <material dir> sequential|threaded [ROUNDS [KIND]]

<tree> holds src/requests. A stand-in certifi makes the default CA file the local CA, so verify=True (the
default) is used throughout. The server asks for a client certificate and answers with the name it received.

sequential: one request at a time, in a fixed order, each with requests.get (a new Session), each on a new
TLS connection.

threaded: thread "good" sends ROUNDS requests with cert=(client A's certificate, client A's key) and counts what
the server received. At the same time thread "other" keeps sending requests of one kind until "good" is done:
  - mismatch: cert=(client B's certificate, client A's key), whose load fails with KEY_VALUES_MISMATCH
  - nokey: cert=(client B's certificate, a file holding no key), whose load fails with PEM lib
  - certb: cert=(client B's certificate, client B's key), which loads (GT-i4's concurrent form, for comparison)
  - none: no cert= (for comparison)
KIND runs one of these; without it all four run in turn. faulthandler prints the threads' stacks if the process
crashes.
"""
import collections
import faulthandler
import os
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import describe, make_material, start_server, use_requests  # noqa: E402

faulthandler.enable()
tree, label, directory, mode = sys.argv[1:5]
rounds = int(sys.argv[5]) if len(sys.argv) > 5 else 400
kinds = sys.argv[6:7] or ["mismatch", "nokey", "certb", "none"]
m = make_material(directory)
requests = use_requests(tree, directory)
import urllib3  # noqa: E402

process, port = start_server(directory)
url = f"https://localhost:{port}/"
A = (m["client-a.crt"], m["client-a.key"])
OTHER = {
    "mismatch": (m["client-b.crt"], m["client-a.key"]),
    "nokey": (m["client-b.crt"], m["garbage.pem"]),
    "certb": (m["client-b.crt"], m["client-b.key"]),
    "none": None,
}


def send(cert):
    try:
        return "server received " + requests.get(url, cert=cert, timeout=20).text
    except Exception as error:  # noqa: BLE001
        return "raises " + describe(error)


print(f"== {label} {mode} {' '.join(sys.argv[6:7])}: requests {requests.__version__}, urllib3 {urllib3.__version__}, Python {sys.version.split()[0]}", flush=True)
try:
    if mode == "sequential":
        steps = [
            ("no cert=", None),
            ("cert=(A cert, A key)", A),
            ("no cert=", None),
            ("cert=(B cert, A key): key does not match", OTHER["mismatch"]),
            ("no cert=", None),
            ("cert=(A cert, A key)", A),
            ("cert=(B cert, file with no key)", OTHER["nokey"]),
            ("no cert=", None),
            ("cert=(A cert, A key)", A),
            ("cert=(B cert, missing key file)", (m["client-b.crt"], os.path.join(directory, "no-such.key"))),
            ("no cert=", None),
        ]
        for number, (what, cert) in enumerate(steps, 1):
            print(f"  {number:2}. {what:44} -> {send(cert)}", flush=True)
    elif mode == "threaded":
        for kind in kinds:
            seen, others = collections.Counter(), collections.Counter()
            done = threading.Event()

            def good():
                for _ in range(rounds):
                    seen[send(A)] += 1
                done.set()

            def other():
                while not done.is_set():
                    others[send(OTHER[kind]).split(":")[0]] += 1

            threads = [threading.Thread(target=good), threading.Thread(target=other)]
            started = time.monotonic()
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join()
            print(f"  other={kind}: {rounds} cert=A requests in {time.monotonic() - started:.1f}s; the other thread sent {sum(others.values())} ({dict(sorted(others.items()))})")
            for outcome, count in sorted(seen.items(), key=lambda item: (-item[1], item[0])):
                print(f"    {count:5}  {outcome}")
            sys.stdout.flush()
    else:
        raise SystemExit(__doc__)
finally:
    process.kill()
