"""Start `python -m django runserver --noreload`, request /q once, stop it, print what each side showed.

Usage: PYTHONPATH=<django checkout>:<this directory> python runserver_probe.py <http port>
The database settings come from M1_OPTIONS, M1_HOST and M1_PORT, as for the other commands.
"""
import html
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

port = sys.argv[1]
server = subprocess.Popen(
    [sys.executable, "-m", "django", "runserver", "--noreload", f"127.0.0.1:{port}", "--settings", "m1proj.settings"],
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
    text=True,
)
time.sleep(4)
print(f"  runserver process after 4 s: {'running' if server.poll() is None else f'exited with {server.returncode}'}")
if server.poll() is None:
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/q", timeout=60) as response:
            print(f"  GET /q: {response.status} {response.read().decode()[:200]}")
    except urllib.error.HTTPError as e:
        page = e.read().decode(errors="replace")
        title = re.search(r"<title>(.*?)</title>", page, re.S)
        value = re.search(r'<pre class="exception_value">(.*?)</pre>', page, re.S)
        print(f"  GET /q: HTTP {e.code}; error page title: {html.unescape(title.group(1)).strip() if title else None!r}")
        print(f"          exception value on the page: {html.unescape(value.group(1)) if value else None!r}")
    except Exception as e:
        print(f"  GET /q: {type(e).__name__}: {e}")
    server.terminate()
out = server.communicate(timeout=20)[0]
lines = [line for line in out.splitlines() if line.strip()]
print("  what runserver printed (traceback lines dropped, last line of each traceback kept):")
keep = [l for l in lines if not l.startswith("  ") and not l.startswith("Traceback")]
for line in keep[:25]:
    print(f"    | {line[:200]}")
