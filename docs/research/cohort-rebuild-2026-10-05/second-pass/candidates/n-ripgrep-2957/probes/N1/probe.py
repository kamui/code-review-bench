import os
import pty
import select
import signal
import subprocess
import sys
import time
from pathlib import Path

binary = str(Path(sys.argv[1]).resolve())
work = Path(sys.argv[2]).resolve()
root = Path(__file__).resolve().parent
work.mkdir(parents=True, exist_ok=True)
generated = subprocess.check_output([binary, "--generate", "complete-zsh"])
(work / "_rg").write_bytes(generated)
print("BINARY_VERSION=" + subprocess.check_output([binary, "--version"], text=True).splitlines()[0])
(root / ("generated-" + sys.argv[4] + ".zsh")).write_bytes(generated)
(root / ("source-" + sys.argv[4] + ".zsh")).write_bytes(Path(sys.argv[3]).read_bytes())
print("GENERATED_SAVED=generated-" + sys.argv[4] + ".zsh")
for number, line in enumerate(generated.decode().splitlines(), 1):
    if "$funcstack" in line:
        print("GENERATED_GUARD_LINE=" + str(number) + " " + line)
for mode in ["normal", "ksh"]:
    print("\nNONINTERACTIVE_SOURCE mode=" + mode, flush=True)
    result = subprocess.run(["zsh", "-f", str(root / "source.zsh"), binary, mode], capture_output=True, text=True)
    print("stdout:\n" + result.stdout + "stderr:\n" + result.stderr + "process_exit=" + str(result.returncode), flush=True)

for route, mode in [("source", "normal"), ("source", "ksh"), ("autoload", "normal"), ("autoload", "ksh")]:
    print("\nINTERACTIVE_TAB route=" + route + " mode=" + mode, flush=True)
    pid, master = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "dumb"
        os.environ["ZDOTDIR"] = str(work)
        os.execvp("zsh", ["zsh", "-f"])
    transcript = bytearray()
    def read_until(token, timeout=10):
        deadline = time.monotonic() + timeout
        start = len(transcript)
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master], [], [], min(.2, max(0, deadline - time.monotonic())))
            if ready:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    return False
                if not chunk:
                    return False
                transcript.extend(chunk)
                if token in transcript[start:]:
                    return True
        return False
    try:
        os.write(master, b"PS1='PROBE> '\n")
        if not read_until(b"PROBE> "):
            raise RuntimeError("initial prompt did not arrive")
        command = "source " + str(root / "interactive.zsh") + " " + binary + " " + mode + " " + route + " " + str(work) + "\n"
        os.write(master, command.encode())
        if not read_until(b"PROBE> "):
            raise RuntimeError("configured prompt did not arrive")
        os.write(master, b"rg --generate=complete-zs\t")
        time.sleep(.5)
        os.write(master, b"\x18")
        if not read_until(b"BUFFER_CAPTURE=", 5):
            raise RuntimeError("buffer capture did not arrive")
        read_until(b"PROBE> ", 2)
        os.write(master, b"exit\n")
        read_until(b"unlikely-final-token", .3)
    finally:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        os.waitpid(pid, 0)
        os.close(master)
    print(transcript.decode(errors="replace").replace("\r", ""), flush=True)
