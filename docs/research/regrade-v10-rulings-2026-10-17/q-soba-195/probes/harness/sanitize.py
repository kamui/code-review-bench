"""Usage: python3 -I sanitize.py WORKDIR PORT < raw > clean

Replaces the work and scratch paths, the home directory, the port, dates, times and bundle timestamps so that runs
at the two commits can be compared line by line."""
import os
import re
import sys

work, port = sys.argv[1], sys.argv[2]
scratch = os.path.dirname(work.rstrip("/"))
home = os.path.expanduser("~")
for line in sys.stdin:
    line = line.replace(work, "<W>").replace(f"127.0.0.1:{port}", "127.0.0.1:<PORT>")
    line = line.replace(scratch, "<S>").replace(home, "~")
    line = re.sub(r"\d{4}/\d\d/\d\d \d\d:\d\d:\d\d", "<date> <time>", line)
    line = re.sub(r"\.\d{14}\.", ".<timestamp>.", line)
    line = re.sub(r"(took|in) [0-9.]+(ms|µs|s)\b", r"\1 <duration>", line)
    sys.stdout.write(line)
