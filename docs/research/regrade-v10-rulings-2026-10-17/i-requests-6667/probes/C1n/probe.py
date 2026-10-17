"""Run each case of the missing-default-bundle question against one requests source tree.

    python probe.py SRC_DIR WORK_DIR [--importtime N]

SRC_DIR is a tree's `src` directory. WORK_DIR holds certs/ from make-certs.sh and receives
scratch files. Every case runs in a fresh `python -I` process that puts SRC_DIR first on
sys.path, so the cases do not share state. Local plain HTTP and HTTPS servers are started here;
the HTTPS server's certificate is signed by the private CA in certs/ca.pem.
"""
import http.server
import json
import os
import shutil
import ssl
import statistics
import subprocess
import sys
import threading
import zipfile

SRC, WORK = (os.path.abspath(p) for p in sys.argv[1:3])
IMPORTTIME_RUNS = int(sys.argv[sys.argv.index("--importtime") + 1]) if "--importtime" in sys.argv else 0
CERTS = os.path.join(WORK, "certs")


class Quiet(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b"hello"
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def serve(tls):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Quiet)
    if tls:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(os.path.join(CERTS, "server.pem"), os.path.join(CERTS, "server.key"))
        server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server.server_address[1]


HTTP_PORT, HTTPS_PORT = serve(False), serve(True)
HTTP_URL, HTTPS_URL = f"http://127.0.0.1:{HTTP_PORT}/", f"https://127.0.0.1:{HTTPS_PORT}/"
CA = os.path.join(CERTS, "ca.pem")

# Fixtures for the default CA location.
MISSING = os.path.join(WORK, "no-such-dir", "cacert.pem")
FAKE_CERTIFI = os.path.join(WORK, "distro-certifi")
DISTRO_PATH = os.path.join(WORK, "etc", "pki", "tls", "certs", "ca-bundle.crt")
UNREADABLE = os.path.join(WORK, "unreadable", "cacert.pem")
ZIP = os.path.join(WORK, "zipped", "deps.zip")
ZIP_TMP_STALE = os.path.join(WORK, "tmp-stale")
ZIP_TMP_CLEAN = os.path.join(WORK, "tmp-clean")
CADIR = os.path.join(CERTS, "cadir")


def fixtures():
    import certifi

    real = certifi.where()
    os.makedirs(os.path.join(FAKE_CERTIFI, "certifi"), exist_ok=True)
    with open(os.path.join(FAKE_CERTIFI, "certifi", "__init__.py"), "w") as handle:
        # The shape of a distribution's patched certifi: where() names the system bundle.
        handle.write(f"def where():\n    return {DISTRO_PATH!r}\n\n\ndef contents():\n    with open(where()) as f:\n        return f.read()\n")
    os.makedirs(os.path.dirname(UNREADABLE), exist_ok=True)
    if os.path.exists(UNREADABLE):
        os.chmod(UNREADABLE, 0o600)
    shutil.copyfile(real, UNREADABLE)
    os.chmod(UNREADABLE, 0o000)
    os.makedirs(os.path.dirname(ZIP), exist_ok=True)
    with zipfile.ZipFile(ZIP, "w") as archive:
        archive.write(real, "certifi/cacert.pem")
    for tmp in (ZIP_TMP_STALE, ZIP_TMP_CLEAN):
        shutil.rmtree(tmp, ignore_errors=True)
        os.makedirs(tmp)
    stale = os.path.join(ZIP_TMP_STALE, "cacert.pem")
    shutil.copyfile(real, stale)
    os.chmod(stale, 0o000)  # stands for a copy another user extracted and this user cannot read


SETUPS = {
    "real certifi": ("", {}),
    "certifi.where() repointed to a missing file": (f"import certifi; certifi.where = lambda: {MISSING!r}", {}),
    "distribution-style certifi whose where() names a missing system bundle": (f"sys.path.insert(0, {FAKE_CERTIFI!r})", {}),
    "certifi.where() repointed to an unreadable file (mode 000)": (f"import certifi; certifi.where = lambda: {UNREADABLE!r}", {}),
    "certifi.where() names a member of a zip; an unreadable cacert.pem already sits in the temp directory": (
        f"import certifi; certifi.where = lambda: {os.path.join(ZIP, 'certifi', 'cacert.pem')!r}", {"TMPDIR": ZIP_TMP_STALE}),
    "certifi.where() names a member of a zip; empty temp directory": (
        f"import certifi; certifi.where = lambda: {os.path.join(ZIP, 'certifi', 'cacert.pem')!r}", {"TMPDIR": ZIP_TMP_CLEAN}),
    "certifi.where() repointed to a hashed CA directory holding the server's CA": (f"import certifi; certifi.where = lambda: {CADIR!r}", {}),
    "ssl extension unimportable": ("sys.modules['_ssl'] = None", {}),
}

ACTIONS = {
    "import requests": "pass",
    "GET http:// (plain HTTP)": f"r = requests.get({HTTP_URL!r}); print('ok', r.status_code)",
    "GET https:// verify=False": f"r = requests.get({HTTPS_URL!r}, verify=False); print('ok', r.status_code)",
    "GET https:// verify=<CA file>": f"r = requests.get({HTTPS_URL!r}, verify={CA!r}); print('ok', r.status_code)",
    "GET https:// default verify=True": f"r = requests.get({HTTPS_URL!r}); print('ok', r.status_code)",
    "GET https:// default verify=True, REQUESTS_CA_BUNDLE=<CA file>": f"r = requests.get({HTTPS_URL!r}); print('ok', r.status_code)",
    "load_verify_locations calls during import": "",
}

CLIENT = """
import sys, os, traceback, warnings
warnings.simplefilter('ignore')
sys.path.insert(0, {src!r})
{setup}
stage = 'import'
try:
    {count_setup}
    import requests
    {count_report}
    stage = 'request'
    {action}
    if stage == 'request' and {is_import!r}:
        print('ok')
except BaseException as exc:
    tb = traceback.extract_tb(exc.__traceback__)
    origin = next((f for f in reversed(tb) if 'requests' + os.sep in f.filename), tb[-1])
    message = str(exc).replace(chr(10), ' ')[:240]
    print(f'{{type(exc).__name__}} at {{stage}}: {{message}}  [raised via {{os.path.relpath(origin.filename, {src!r}) if origin.filename.startswith({src!r}) else os.path.basename(origin.filename)}}:{{origin.lineno}}]')
"""

COUNT_SETUP = """import ssl as _ssl_mod
    calls = []
    _orig = _ssl_mod.SSLContext.load_verify_locations
    def _counting(self, *a, **k):
        calls.append(a or k)
        return _orig(self, *a, **k)
    _ssl_mod.SSLContext.load_verify_locations = _counting"""


def run_case(setup_name, action_name):
    setup, extra_env = SETUPS[setup_name]
    counting = action_name == "load_verify_locations calls during import"
    action = ACTIONS[action_name] or "pass"
    code = CLIENT.format(
        src=SRC,
        setup=setup,
        action=action,
        is_import=action_name == "import requests",
        count_setup=COUNT_SETUP if counting else "pass",
        count_report="print('calls', len(calls))" if counting else "pass",
    )
    env = {k: v for k, v in os.environ.items() if k not in ("REQUESTS_CA_BUNDLE", "CURL_CA_BUNDLE", "SSL_CERT_FILE", "SSL_CERT_DIR", "PYTHONPATH")}
    env.update(extra_env)
    env["NO_PROXY"] = "*"
    if "REQUESTS_CA_BUNDLE" in action_name:
        env["REQUESTS_CA_BUNDLE"] = CA
    out = subprocess.run([sys.executable, "-I", "-c", code], capture_output=True, text=True, env=env, cwd=WORK, timeout=60)
    text = (out.stdout.strip() or out.stderr.strip().splitlines()[-1:] and out.stderr.strip().splitlines()[-1]) or f"exit {out.returncode}"
    return text


def importtime(runs):
    selves = []
    for _ in range(runs):
        code = f"import sys; sys.path.insert(0, {SRC!r}); import requests"
        out = subprocess.run([sys.executable, "-I", "-X", "importtime", "-c", code], capture_output=True, text=True, cwd=WORK)
        for line in out.stderr.splitlines():
            parts = [p.strip() for p in line.split("|")]
            if len(parts) == 3 and parts[2] == "requests.adapters":
                selves.append(int(parts[0].split()[-1]))
    return selves


def main():
    fixtures()
    import urllib3
    import certifi

    print(f"python {sys.version.split()[0]}; urllib3 {urllib3.__version__}; certifi {certifi.__version__}; {ssl.OPENSSL_VERSION}")
    print(f"tree {os.path.basename(os.path.dirname(SRC))}")
    plan = [
        ("real certifi", ["import requests", "GET http:// (plain HTTP)", "GET https:// verify=<CA file>", "load_verify_locations calls during import"]),
        ("certifi.where() repointed to a missing file", [a for a in ACTIONS if a != "load_verify_locations calls during import"]),
        ("distribution-style certifi whose where() names a missing system bundle", ["import requests", "GET http:// (plain HTTP)", "GET https:// verify=False", "GET https:// default verify=True"]),
        ("certifi.where() repointed to an unreadable file (mode 000)", ["import requests", "GET http:// (plain HTTP)", "GET https:// verify=False", "GET https:// default verify=True"]),
        ("certifi.where() names a member of a zip; an unreadable cacert.pem already sits in the temp directory", ["import requests", "GET http:// (plain HTTP)", "GET https:// verify=<CA file>"]),
        ("certifi.where() names a member of a zip; empty temp directory", ["import requests", "GET http:// (plain HTTP)"]),
        ("certifi.where() repointed to a hashed CA directory holding the server's CA", ["import requests", "GET https:// default verify=True"]),
        ("ssl extension unimportable", ["import requests", "GET http:// (plain HTTP)"]),
    ]
    for setup_name, actions in plan:
        print(f"\n## {setup_name}")
        for action_name in actions:
            print(f"- {action_name}: {run_case(setup_name, action_name)}")
    if IMPORTTIME_RUNS:
        selves = importtime(IMPORTTIME_RUNS)
        print(f"\n## real certifi, -X importtime self time of requests.adapters over {len(selves)} fresh imports")
        print(f"- median {statistics.median(selves)} us; min {min(selves)} us; max {max(selves)} us")
    os.chmod(UNREADABLE, 0o600)
    os.chmod(os.path.join(ZIP_TMP_STALE, "cacert.pem"), 0o600)


main()
