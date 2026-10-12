"""Usage: python run_all.py <django checkout> <label> <sock dir> <port> > result-<label>.txt
Runs probe.py once per case in its own process and records standard output and standard error."""
import os
import re
import subprocess
import sys

checkout, label, host, port = sys.argv[1:5]
here = os.path.dirname(os.path.abspath(__file__))
cases = [
    ("bad-tz", "off", "default"),
    ("bad-tz", "on", "default"),
    ("bad-tz", "on", "root-console"),
    ("bad-tz", "on", "django-only"),
    ("bad-tz", "on", "root-error"),
    ("bad-db", "off", "default"),
    ("bad-db", "on", "default"),
    ("bad-password", "off", "default"),
    ("bad-password", "on", "default"),
    ("server-down", "off", "default"),
    ("server-down", "on", "default"),
    ("none", "off", "default"),
    ("none", "on", "default"),
    ("bad-tz", "default", "default"),
]
env = dict(os.environ, PYTHONPATH=checkout)
version = subprocess.run(
    [sys.executable, "-c", "import django, psycopg, psycopg_pool; print(django.get_version(), 'psycopg', psycopg.__version__, 'psycopg_pool', psycopg_pool.__version__)"],
    env=env, capture_output=True, text=True,
).stdout.strip()
print(f"{label}: django {version}")
for failure, pool, logging_mode in cases:
    done = subprocess.run(
        [sys.executable, os.path.join(here, "probe.py"), failure, pool, logging_mode, host, port],
        env=env, capture_output=True, text=True, timeout=180,
    )
    print()
    print(f"## failure={failure} pool={pool} logging={logging_mode}")
    print(done.stdout.rstrip().replace(host, "<scratch>/sock"))
    lines = [line.replace(host, "<scratch>/sock") for line in done.stderr.splitlines()]
    print(f"standard error: {len(lines)} line(s)")
    counts = {}
    for line in lines:
        key = re.sub(r"\d+\.\d+ sec", "N sec", line)
        counts[key] = counts.get(key, 0) + 1
    for key, n in list(counts.items())[:8]:
        print(f"    {n} x {key[:300]}")
