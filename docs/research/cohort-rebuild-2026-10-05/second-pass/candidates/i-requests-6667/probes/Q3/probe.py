import argparse
import datetime
import ipaddress
import json
import os
from pathlib import Path
import ssl
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import NameOID


def material(directory, key_size=2048, valid_host=True):
    directory.mkdir(parents=True, exist_ok=True)
    key = rsa.generate_private_key(public_exponent=65537, key_size=key_size)
    name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "local probe")])
    now = datetime.datetime.now(datetime.timezone.utc)
    certificate = (
        x509.CertificateBuilder().subject_name(name).issuer_name(name)
        .public_key(key.public_key()).serial_number(x509.random_serial_number())
        .not_valid_before(now - datetime.timedelta(days=1))
        .not_valid_after(now + datetime.timedelta(days=2))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .add_extension(x509.SubjectAlternativeName([
            x509.DNSName("localhost"), x509.IPAddress(ipaddress.ip_address("127.0.0.1"))
        ] if valid_host else [x509.DNSName("wrong.invalid")]), critical=False).sign(key, hashes.SHA256()))
    (directory / "cert.pem").write_bytes(certificate.public_bytes(serialization.Encoding.PEM))
    (directory / "key.pem").write_bytes(key.private_bytes(
        serialization.Encoding.PEM, serialization.PrivateFormat.TraditionalOpenSSL,
        serialization.NoEncryption()))
    return directory / "cert.pem", directory / "key.pem"


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = json.dumps({"cipher": self.connection.cipher(), "tls": self.connection.version(),
                           "client_certificate_present": bool(self.connection.getpeercert())}).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def serve(directory, ciphers=None, key_size=2048, valid_host=True, verify_client=False):
    certificate, key = material(directory, key_size, valid_host)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = context.maximum_version = ssl.TLSVersion.TLSv1_2
    if ciphers:
        context.set_ciphers(ciphers)
    context.load_cert_chain(certificate, key)
    if verify_client:
        context.load_verify_locations(certificate)
        context.verify_mode = ssl.CERT_OPTIONAL
    server.socket = context.wrap_socket(server.socket, server_side=True)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, str(certificate)


def request(url, verify):
    import requests
    try:
        with requests.Session() as session:
            session.trust_env = False
            response = session.get(url, verify=verify, timeout=4)
            print("request", response.status_code, response.text)
    except Exception as error:
        print("request_error", type(error).__name__, str(error))


def child(case, url, certificate):
    import certifi
    import urllib3
    from urllib3.contrib import pyopenssl
    from urllib3.util import ssl_ as utility

    certifi.where = lambda: certificate
    if case.startswith("early"):
        pyopenssl.inject_into_urllib3()
    import requests
    original_context = getattr(requests.adapters, "_preloaded_ssl_context", None)
    print("before_injection_context", type(original_context).__module__, type(original_context).__name__)
    if not case.startswith("early"):
        pyopenssl.inject_into_urllib3()
    print("injected", utility.IS_PYOPENSSL, "factory", utility.SSLContext.__module__, utility.SSLContext.__name__)
    original_wrap = urllib3.connection.ssl_wrap_socket

    def record_wrap(*args, **kwargs):
        context = kwargs.get("ssl_context")
        print("supplied_context", type(context).__module__, type(context).__name__,
              "check_hostname", getattr(context, "check_hostname", None))
        result = original_wrap(*args, **kwargs)
        print("socket_backend", type(result).__module__, type(result).__name__)
        return result

    urllib3.connection.ssl_wrap_socket = record_wrap
    verify = False if case.endswith("false") else certificate if case.endswith("path") else True
    request(url, verify)
    if original_context is not None:
        print("after_context_check_hostname", original_context.check_hostname)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--scratch", type=Path)
    parser.add_argument("--child", nargs=3)
    arguments = parser.parse_args()
    if arguments.child:
        child(*arguments.child)
        return
    print("python", sys.version.replace("\n", " "))
    import urllib3
    print("urllib3", urllib3.__version__, "openssl", ssl.OPENSSL_VERSION)
    server, certificate = serve(arguments.scratch)
    url = f"https://127.0.0.1:{server.server_port}/"
    for case in ["late-true", "late-false", "late-path", "early-true"]:
        print("CASE", case, flush=True)
        subprocess.run([sys.executable, __file__, "--child", case, url, certificate], check=True)
    for case in ["late-untrusted", "late-wrong-host"]:
        negative_server, negative_certificate = serve(arguments.scratch / case, valid_host=case != "late-wrong-host")
        negative_url = f"https://127.0.0.1:{negative_server.server_port}/"
        trust_certificate = certificate if case == "late-untrusted" else negative_certificate
        print("CASE", case, flush=True)
        subprocess.run([sys.executable, __file__, "--child", case, negative_url, trust_certificate], check=True)
        negative_server.shutdown()
        negative_server.server_close()
    server.shutdown()
    server.server_close()


if __name__ == "__main__":
    main()
