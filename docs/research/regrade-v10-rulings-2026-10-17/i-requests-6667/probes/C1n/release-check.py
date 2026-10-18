import os, subprocess, sys
work = sys.argv[1]
missing = os.path.join(work, "no-such-dir", "cacert.pem")
for target in sys.argv[2:]:
    code = f"""
import sys; sys.path.insert(0, {target!r})
import certifi; certifi.where = lambda: {missing!r}
try:
    import requests
    print('import ok, requests', requests.__version__)
except BaseException as e:
    print(type(e).__name__, str(e)[:100])
"""
    out = subprocess.run([sys.executable, "-I", "-c", code], capture_output=True, text=True)
    print(f"- requests {os.path.basename(target)} from PyPI, certifi.where() repointed to a missing file: {out.stdout.strip() or out.stderr.strip()[-200:]}")
