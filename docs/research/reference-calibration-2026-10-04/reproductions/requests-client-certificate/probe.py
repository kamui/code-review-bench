"""Does a client certificate given to one request get sent on a later request that supplied none?

Two local HTTPS servers ask for a client certificate (optional) and report whether they received one.
Request 1 goes to server A with a client certificate. Request 2 goes to server B without one.
"""
import http.server, json, ssl, sys, threading

import requests


def serve(port):
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            cert = self.connection.getpeercert()
            subject = dict(item[0] for item in cert["subject"])["commonName"] if cert else None
            body = json.dumps({"client_certificate": subject}).encode()
            self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
        def log_message(self, *args):
            pass
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain("server.crt", "server.key")
    context.load_verify_locations("ca.crt")
    context.verify_mode = ssl.CERT_OPTIONAL
    server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


a, b = serve(0), serve(0)
url_a, url_b = (f"https://localhost:{s.server_address[1]}/" for s in (a, b))
print("requests", requests.__version__, "from", requests.__file__.split("/repro-b/")[1])
first = requests.get(url_b).json()
print("request 0 to server B, no certificate supplied ->", first)
one = requests.get(url_a, cert=("client.crt", "client.key")).json()
print("request 1 to server A, certificate supplied     ->", one)
two = requests.get(url_b).json()
print("request 2 to server B, no certificate supplied ->", two)
leaked = two["client_certificate"] is not None
print("RESULT:", "LEAK: server B received the client certificate from request 1" if leaked else "no leak")
sys.exit(1 if leaked else 0)
