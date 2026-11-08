"""Local HTTPS server: asks for a client certificate (optional) and answers with the common name it received.

Usage: python -I server.py <material dir>. Prints its port on the first line of stdout, and exits when its parent does.
"""
import http.server
import os
import ssl
import sys
import threading
import time

directory = sys.argv[1]
parent = os.getppid()


def exit_with_parent():
    while os.getppid() == parent:
        time.sleep(0.5)
    os._exit(0)


threading.Thread(target=exit_with_parent, daemon=True).start()


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        cert = self.connection.getpeercert()
        name = dict(item[0] for item in cert["subject"])["commonName"] if cert else "none"
        body = name.encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(os.path.join(directory, "server.crt"), os.path.join(directory, "server.key"))
context.load_verify_locations(os.path.join(directory, "ca.pem"))
context.verify_mode = ssl.CERT_OPTIONAL
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
server.request_queue_size = 128
server.socket = context.wrap_socket(server.socket, server_side=True)
print(server.server_address[1], flush=True)
server.serve_forever()
