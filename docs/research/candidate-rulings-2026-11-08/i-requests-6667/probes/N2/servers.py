"""Local servers for the N2 probe, in one process of their own.

Usage: python -I servers.py <material dir>
Prints one JSON line: {"private": port, "default": port, "proxy": port}.
- "private": HTTPS, its certificate signed by a private CA that is not in the default CA file.
- "default": HTTPS, its certificate signed by the CA the default CA file holds.
Both answer with the TLS version of the connection.
- "proxy": a plain-HTTP proxy that accepts CONNECT and relays bytes; each tunnel is counted, and GET /count
  answers with the number of tunnels so far.
The process exits when its parent does.
"""
import http.server
import json
import os
import socket
import socketserver
import ssl
import sys
import threading
import time

directory = sys.argv[1]
parent = os.getppid()
tunnels = 0
lock = threading.Lock()


def exit_with_parent():
    while os.getppid() == parent:
        time.sleep(0.5)
    os._exit(0)


class Version(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = self.connection.version().encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def https(name):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(os.path.join(directory, name + ".crt"), os.path.join(directory, name + ".key"))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Version)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server.server_address[1]


def pipe(source, sink):
    try:
        while data := source.recv(65536):
            sink.sendall(data)
    except OSError:
        pass
    finally:
        for end in (source, sink):
            try:
                end.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass


class Proxy(socketserver.StreamRequestHandler):
    def handle(self):
        global tunnels
        line = self.rfile.readline().decode("latin-1")
        while self.rfile.readline() not in (b"\r\n", b"\n", b""):
            pass
        method, target = line.split()[:2]
        if method == "GET" and target.endswith("/count"):
            body = str(tunnels).encode()
            self.wfile.write(b"HTTP/1.0 200 OK\r\nContent-Length: %d\r\n\r\n" % len(body) + body)
            return
        if method != "CONNECT":
            self.wfile.write(b"HTTP/1.0 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n")
            return
        host, port = target.rsplit(":", 1)
        upstream = socket.create_connection((host, int(port)))
        with lock:
            tunnels += 1
        self.wfile.write(b"HTTP/1.1 200 Connection established\r\n\r\n")
        self.wfile.flush()
        relay = threading.Thread(target=pipe, args=(upstream, self.connection), daemon=True)
        relay.start()
        pipe(self.connection, upstream)
        relay.join()


threading.Thread(target=exit_with_parent, daemon=True).start()
proxy = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Proxy)
proxy.daemon_threads = True
threading.Thread(target=proxy.serve_forever, daemon=True).start()
print(json.dumps({"private": https("private"), "default": https("default"), "proxy": proxy.server_address[1]}), flush=True)
threading.Event().wait()
