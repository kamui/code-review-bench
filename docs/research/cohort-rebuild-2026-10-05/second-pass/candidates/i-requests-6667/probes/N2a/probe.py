import argparse
import os
from pathlib import Path
import ssl
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "N1"))
from probe import request, serve


def child(case, url, certificate, directory):
    import certifi
    certifi.where = lambda: certificate
    if case == "keylog-early":
        os.environ["SSLKEYLOGFILE"] = str(Path(directory) / "keys.log")
    import requests
    from urllib3.util import ssl_ as utility
    if case.startswith("cipher-compatibility"):
        utility.DEFAULT_CIPHERS = "AES128-SHA:@SECLEVEL=0"
    elif case.startswith("cipher-restriction"):
        utility.DEFAULT_CIPHERS = "ECDHE-RSA-AES128-GCM-SHA256"
    elif case.startswith("keylog"):
        os.environ["SSLKEYLOGFILE"] = str(Path(directory) / "keys.log")
    elif case == "cert-file-env":
        os.environ["SSL_CERT_FILE"] = str(Path(directory) / "does-not-exist.pem")
    elif case == "certifi-where":
        certifi.where = lambda: str(Path(directory) / "does-not-exist.pem")
    elif case == "adapter-bundle":
        requests.adapters.DEFAULT_CA_BUNDLE_PATH = str(Path(directory) / "does-not-exist.pem")
    print("case", case, "shared_context", hasattr(requests.adapters, "_preloaded_ssl_context"))
    request(url, certificate if case.endswith("path") else True)
    keylog = Path(directory) / "keys.log"
    print("keylog_has_handshake_secrets", keylog.exists() and any(
        line.startswith("CLIENT_RANDOM") for line in keylog.read_text().splitlines()))
    if hasattr(requests.adapters, "_preloaded_ssl_context"):
        context = requests.adapters._preloaded_ssl_context
        print("context_cipher_names", [item["name"] for item in context.get_ciphers()])
        print("context_keylog_filename", getattr(context, "keylog_filename", None))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--scratch", type=Path)
    parser.add_argument("--child", nargs=4)
    parser.add_argument("--keylog-only", action="store_true")
    arguments = parser.parse_args()
    if arguments.child:
        child(*arguments.child)
        return
    import urllib3
    print("python", sys.version.replace("\n", " "))
    print("urllib3", urllib3.__version__, "openssl", ssl.OPENSSL_VERSION)
    cases = ["keylog-true", "keylog-path", "keylog-early"] if arguments.keylog_only else [
        "cipher-compatibility", "cipher-restriction", "cipher-compatibility-path", "cipher-restriction-path",
        "cert-file-env", "certifi-where", "adapter-bundle"]
    for case in cases:
        directory = arguments.scratch / case
        directory.mkdir(parents=True, exist_ok=True)
        compatibility = case.startswith("cipher-compatibility")
        ciphers = "AES128-SHA:@SECLEVEL=0" if compatibility else "ECDHE-RSA-AES256-GCM-SHA384"
        server, certificate = serve(directory, ciphers, key_size=1024 if compatibility else 2048)
        url = f"https://127.0.0.1:{server.server_port}/"
        print("CASE", case, flush=True)
        subprocess.run([sys.executable, __file__, "--child", case, url, certificate, str(directory)], check=True)
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
