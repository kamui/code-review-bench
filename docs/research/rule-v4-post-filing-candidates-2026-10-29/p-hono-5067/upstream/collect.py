"""Fetch the upstream records for group M1 and save each raw response with its command.

python3 -I collect.py <name> <gh api arguments...>
writes upstream/<name>.json (the raw response) and upstream/<name>.request.json (the command).
Read-only GitHub calls only.
"""
import json
import pathlib
import subprocess
import sys

OUT = pathlib.Path(__file__).resolve().parent
name, args = sys.argv[1], sys.argv[2:]
command = ['gh', 'api', *args]
done = subprocess.run(command, text=True, capture_output=True)
(OUT / (name + '.json')).write_text(done.stdout)
(OUT / (name + '.request.json')).write_text(json.dumps({'command': command, 'returncode': done.returncode, 'stderr': done.stderr}, indent=2) + '\n')
print(name, 'exit', done.returncode, len(done.stdout), 'bytes', done.stderr.strip()[:200])
