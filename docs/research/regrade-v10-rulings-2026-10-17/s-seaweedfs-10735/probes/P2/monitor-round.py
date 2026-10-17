#!/usr/bin/env python3
"""Print, in order, the commands on keys under /p2race that Redis received in one round of the
race loop of end-to-end.sh: from the marker `ECHO round-<n>` up to the first GET by the checks
that follow the round (the checks run after both requests have returned, so the block is cut at
the first command from a client that issued neither request). Each line: the client's port
(one per HTTP request's connection is not guaranteed; the filer pools connections) and the command,
with the stored entry bytes left out.

    python3 -I monitor-round.py <monitor file> <round>
"""
import re
import sys

path, rnd = sys.argv[1], sys.argv[2]
line_re = re.compile(r'^(\d+\.\d+) \[\d+ [\d.]+:(\d+)\] (.*)$')
inside = False
out = []
for raw in open(path, encoding="utf-8", errors="replace"):
    m = line_re.match(raw.rstrip("\n"))
    if not m:
        continue
    ts, port, cmd = m.groups()
    if cmd.startswith('"echo" "round-'):
        if inside:
            break
        inside = cmd == f'"echo" "round-{rnd}"'
        continue
    if not inside:
        continue
    parts = re.findall(r'"((?:[^"\\]|\\.)*)"', cmd)
    if len(parts) < 2 or not parts[1].startswith("/p2race"):
        continue
    name = parts[0].upper()
    if name not in ("GET", "SET", "DEL", "ZADD", "ZREM"):
        continue
    shown = [name] + [p if not (name == "SET" and i == 1) else "<entry bytes>" for i, p in enumerate(parts[1:])]
    if name == "SET":
        shown = shown[:3] + shown[3:]
    out.append(f"{ts} client:{port} " + " ".join(shown))
for line in out:
    print(line)
