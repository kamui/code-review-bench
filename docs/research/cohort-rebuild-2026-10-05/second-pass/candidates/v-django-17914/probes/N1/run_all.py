import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[5]
output = root / "docs/research/cohort-rebuild-2026-10-05/second-pass/candidates/v-django-17914"
scratch = Path("<scratch>")
clone = scratch / "clone"
pgbin = Path("/usr/lib/postgresql/16/bin")
with (scratch / "postgres-run.log").open("w") as log:
    server = subprocess.Popen([
        str(pgbin / "postgres"), "-D", str(scratch / "pgdata"), "-p", "55439",
        "-k", str(scratch), "-h", "127.0.0.1",
    ], stdout=log, stderr=log)
    try:
        for _ in range(100):
            ready = subprocess.run([str(pgbin / "pg_isready"), "-h", "127.0.0.1", "-p", "55439"], capture_output=True)
            if ready.returncode == 0:
                break
            if server.poll() is not None:
                raise RuntimeError((scratch / "postgres-run.log").read_text())
            time.sleep(0.1)
        else:
            raise RuntimeError("PostgreSQL did not start")
        for label, revision in [
            ("base", "bcccea3ef31c777b73cba41a6255cd866bf87237"),
            ("head", "fad334e1a9b54ea1acb8cce02a25934c5acfe99f"),
        ]:
            subprocess.run(["git", "-C", str(clone), "checkout", "--detach", revision], check=True, capture_output=True)
            env = {**os.environ, "PYTHONPATH": str(clone), "PYTHONDONTWRITEBYTECODE": "1",
                   "DOSSIER_REVISION": revision, "DOSSIER_SQLITE_PATH": str(scratch / "probe.sqlite3")}
            for group in ["N1", "N2", "Q3", "Q4"]:
                interpreter = scratch / ("venv32" if group == "Q3" else "venv310") / "bin/python"
                result = subprocess.run([str(interpreter), str(output / "probes" / group / "probe.py")],
                                        env=env, capture_output=True, text=True, timeout=40)
                (output / "probes" / group / f"result-{label}.txt").write_text(
                    result.stdout + result.stderr + f"\nprocess exit: {result.returncode}\n")
                print(label, group, result.returncode, result.stdout, result.stderr, flush=True)
            if label == "head":
                result = subprocess.run([str(scratch / "venv32/bin/python"), str(output / "probes/N2/probe.py")],
                                        env=env, capture_output=True, text=True, timeout=40)
                (output / "probes/N2/result-head-pool32.txt").write_text(result.stdout + result.stderr)
                print("head pool 3.2 control", result.returncode, result.stdout, result.stderr, flush=True)
    finally:
        server.terminate()
        server.wait(timeout=15)
