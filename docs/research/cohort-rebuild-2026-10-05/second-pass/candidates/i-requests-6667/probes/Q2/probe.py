import argparse
from pathlib import Path
import ssl
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "N1"))
from probe import serve

parser = argparse.ArgumentParser()
parser.add_argument("--scratch", type=Path, required=True)
arguments = parser.parse_args()
server, certificate = serve(arguments.scratch, verify_client=True)
import certifi
certifi.where = lambda: certificate
import requests
import urllib3
print("python", sys.version.replace("\n", " "))
print("urllib3", urllib3.__version__, "openssl", ssl.OPENSSL_VERSION)
url = f"https://127.0.0.1:{server.server_port}/"
for case, client_certificate in [("first-with-client-cert", (certificate, str(arguments.scratch / "key.pem"))),
                                 ("new-session-without-client-cert", None)]:
    with requests.Session() as session:
        session.trust_env = False
        try:
            response = session.get(url, cert=client_certificate, timeout=4)
            print(case, response.status_code, response.text)
        except Exception as error:
            print(case, type(error).__name__, str(error))
server.shutdown()
server.server_close()
