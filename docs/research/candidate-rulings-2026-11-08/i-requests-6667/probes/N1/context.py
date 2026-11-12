"""What a failed load_cert_chain leaves in a context that already holds a client certificate.

Usage: python -I context.py <material dir>
One context from urllib3's create_urllib3_context(), the call requests uses for its shared context, trusts the
local CA and holds client A's certificate and key. Each case then tries one more load_cert_chain on that same
context, and a new handshake with the server shows which certificate the context now presents. A fresh context
is made for each case.
"""
import os
import socket
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import describe, make_material, start_server  # noqa: E402

directory = sys.argv[1]
m = make_material(directory)
from urllib3.util.ssl_ import create_urllib3_context  # noqa: E402

process, port = start_server(directory)


def presented(context):
    with socket.create_connection(("127.0.0.1", port)) as raw:
        with context.wrap_socket(raw, server_hostname="localhost") as tls:
            tls.sendall(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
            data = b""
            while chunk := tls.recv(4096):
                data += chunk
    return data.split(b"\r\n\r\n", 1)[1].decode()


cases = [
    ("client B's certificate with client A's key (key does not match)", m["client-b.crt"], m["client-a.key"]),
    ("client B's certificate with a key file that does not exist", m["client-b.crt"], os.path.join(directory, "no-such.key")),
    ("client B's certificate with a key file holding no key", m["client-b.crt"], m["garbage.pem"]),
    ("a certificate file holding no certificate, with client A's key", m["garbage.pem"], m["client-a.key"]),
    ("client B's certificate and no key file (the certificate file holds no key)", m["client-b.crt"], None),
    ("client B's certificate and key (loads; for comparison)", m["client-b.crt"], m["client-b.key"]),
]
try:
    context = create_urllib3_context()
    context.load_verify_locations(m["ca.pem"])
    print("fresh context, nothing loaded: presents", presented(context))
    for label, certfile, keyfile in cases:
        context = create_urllib3_context()
        context.load_verify_locations(m["ca.pem"])
        context.load_cert_chain(m["client-a.crt"], m["client-a.key"])
        before = presented(context)
        try:
            context.load_cert_chain(certfile, keyfile)
            load = "loads"
        except Exception as error:  # noqa: BLE001
            load = "fails: " + describe(error)
        try:
            after = presented(context)
        except Exception as error:  # noqa: BLE001
            after = "handshake fails: " + describe(error)
        print(f"- {label}\n    before: presents {before}; load_cert_chain {load}\n    after: presents {after}")
finally:
    process.kill()
