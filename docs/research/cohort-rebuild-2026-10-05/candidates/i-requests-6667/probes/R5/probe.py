"""R5: `import requests` on a Python that has no ssl module.

No interpreter built without ssl was available. The nearest thing is run
instead: the C extension `_ssl` is made unimportable before anything else is
imported, so `import ssl` raises ImportError exactly as it does on such a
build. urllib3 and requests/__init__.py both guard `import ssl` for this case.
The child then imports requests and makes a plain http:// request to a local
server. A control child runs the same steps with ssl left in place.
"""
import os
import subprocess
import sys
import textwrap

CHILD = textwrap.dedent(
    """
    import sys
    if {block!r}:
        sys.modules["_ssl"] = None  # `import ssl` now raises ImportError, as on a build without OpenSSL
    try:
        import ssl
        print("  import ssl: works")
    except ImportError as exc:
        print(f"  import ssl: ImportError: {{exc}}")
    import threading, traceback
    from http.server import BaseHTTPRequestHandler, HTTPServer

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args): pass
        def do_GET(self):
            self.send_response(200); self.send_header("Content-Length", "5"); self.end_headers()
            self.wfile.write(b"hello")

    server = HTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        import requests
    except BaseException as exc:
        frames = traceback.extract_tb(exc.__traceback__)
        where = next((f for f in reversed(frames) if "requests" in f.filename and "adapters" in f.filename), frames[-1])
        print(f"  import requests: RAISED {{type(exc).__name__}}: {{exc}}")
        print(f"    raised while executing {{where.filename.split('requests')[-1].lstrip('/')}} line {{where.lineno}}: {{where.line}}")
        sys.exit(0)
    print(f"  import requests: works (requests {{requests.__version__}})")
    try:
        response = requests.get(f"http://127.0.0.1:{{server.server_address[1]}}/", timeout=5)
        print(f"  plain http:// GET: OK {{response.status_code}} {{response.text}}")
    except BaseException as exc:
        print(f"  plain http:// GET: RAISED {{type(exc).__name__}}: {{exc}}")
    """
)

import urllib3  # noqa: E402  (parent only: for the banner)

print(f"python {sys.version.split()[0]}")
print(f"urllib3 {urllib3.__version__}")
print(f"commit under test: {os.environ.get('PROBE_LABEL', '?')}")
print("-" * 60)
for block, title in ((False, "control: ssl available"), (True, "ssl module unavailable")):
    print(f"{title}:")
    out = subprocess.run([sys.executable, "-W", "ignore", "-c", CHILD.format(block=block)],
                         capture_output=True, text=True)
    sys.stdout.write(out.stdout)
    if out.returncode:
        print(f"  child exited {out.returncode}: {out.stderr.strip().splitlines()[-1:]}")
