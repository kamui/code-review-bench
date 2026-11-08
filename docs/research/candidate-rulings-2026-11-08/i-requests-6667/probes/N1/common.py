"""Shared set-up for the N1 probes: certificates, a stand-in certifi and a local HTTPS server.

The server asks for a client certificate (optional) and answers each request with the common name of the
certificate it received, or "none". It answers HTTP/1.0, so every request opens a new connection and a new
TLS handshake. It runs in its own process, so its threads do not compete with the client's for the GIL.
"""
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))


def make_material(directory):
    import trustme

    os.makedirs(directory, exist_ok=True)
    ca = trustme.CA()
    ca.cert_pem.write_to_path(os.path.join(directory, "ca.pem"))
    server = ca.issue_cert("localhost", "127.0.0.1")
    server.private_key_pem.write_to_path(os.path.join(directory, "server.key"))
    server.cert_chain_pems[0].write_to_path(os.path.join(directory, "server.crt"))
    for name in ("client-a", "client-b"):
        client = ca.issue_cert(name + "@example.test", common_name=name)
        client.private_key_pem.write_to_path(os.path.join(directory, name + ".key"))
        client.cert_chain_pems[0].write_to_path(os.path.join(directory, name + ".crt"))
    with open(os.path.join(directory, "garbage.pem"), "w") as f:
        f.write("this file holds no certificate and no key\n")
    fake = os.path.join(directory, "standin", "certifi")
    os.makedirs(fake, exist_ok=True)
    with open(os.path.join(fake, "__init__.py"), "w") as f:
        f.write("def where():\n    return %r\n\ndef contents():\n    return open(where()).read()\n" % os.path.join(directory, "ca.pem"))
    return {name: os.path.join(directory, name) for name in os.listdir(directory)}


def start_server(directory):
    process = subprocess.Popen([sys.executable, "-I", os.path.join(HERE, "server.py"), directory],
                               stdout=subprocess.PIPE, text=True)
    port = int(process.stdout.readline())
    return process, port


def use_requests(tree, directory):
    sys.path.insert(0, os.path.join(directory, "standin"))
    sys.path.insert(0, os.path.join(tree, "src"))
    import certifi
    import requests

    assert certifi.where() == os.path.join(directory, "ca.pem"), certifi.where()
    assert requests.__file__.startswith(os.path.join(tree, "src")), requests.__file__
    return requests


def describe(error):
    text = f"{type(error).__name__}: {error}"
    return text if len(text) < 220 else text[:217] + "..."
