import ssl, threading, socket, http.server, requests, requests.adapters as A
ca="ca.pem"
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        c=self.connection.getpeercert()
        body=(str(c.get('subject')) if c else "NO CLIENT CERT").encode()
        self.send_response(200); self.send_header("Content-Length",str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(*a): pass
srv=http.server.HTTPServer(("127.0.0.1",0),H)
sc=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); sc.load_cert_chain("srv.pem","srv.key"); sc.load_verify_locations(ca); sc.verify_mode=ssl.CERT_OPTIONAL
srv.socket=sc.wrap_socket(srv.socket,server_side=True)
threading.Thread(target=srv.serve_forever,daemon=True).start()
port=srv.server_address[1]
# test-only: trust the test CA in the shared preloaded context (stand-in for a publicly trusted server)
A._preloaded_ssl_context.load_verify_locations(ca)
url=f"https://localhost:{port}/"
s1=requests.Session()
print("req1 (Session1, cert=alice):", s1.get(url,cert=("cli.pem","cli.key")).text)
s2=requests.Session()
print("req2 (Session2, NO cert, verify=True):", s2.get(url).text)
