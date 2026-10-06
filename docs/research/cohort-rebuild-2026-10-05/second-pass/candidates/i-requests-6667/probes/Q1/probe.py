import argparse
from pathlib import Path
import ssl
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "N1"))
from probe import serve

parser = argparse.ArgumentParser()
parser.add_argument("--scratch", type=Path, required=True)
arguments = parser.parse_args()
server, certificate = serve(arguments.scratch)
import certifi
certifi.where = lambda: certificate
import requests
import urllib3
print("python", sys.version.replace("\n", " "))
print("urllib3", urllib3.__version__, "openssl", ssl.OPENSSL_VERSION)


class NoVerifyAdapter(requests.adapters.HTTPAdapter):
    def cert_verify(self, conn, url, verify, cert):
        super().cert_verify(conn, url, False, cert)


context = getattr(requests.adapters, "_preloaded_ssl_context", None)
print("shared_before", getattr(context, "verify_mode", None), getattr(context, "check_hostname", None))
with requests.Session() as session:
    session.trust_env = False
    session.mount("https://", NoVerifyAdapter())
    try:
        print("request_status", session.get(f"https://127.0.0.1:{server.server_port}/", timeout=4).status_code)
    except Exception as error:
        print("request_error", type(error).__name__, str(error))
print("shared_after", getattr(context, "verify_mode", None), getattr(context, "check_hostname", None))
server.shutdown()
server.server_close()
