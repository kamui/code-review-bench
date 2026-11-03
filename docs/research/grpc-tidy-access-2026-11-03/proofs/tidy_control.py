"""Show that the offline `go mod tidy -diff` form reports an untidy module, outside the sandbox.

    python3 -I tidy_control.py HEAD_TREE GOMODCACHE OUT

HEAD_TREE is an export of the review head. Each case copies it, makes one edit to stats/opencensus, and runs the
form in the profile's offline environment. OUT records each case's exit code and output.
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

MODULE = 'stats/opencensus'


def drop_line(path, needle):
    lines = path.read_text().splitlines(keepends=True)
    kept = [line for line in lines if needle not in line]
    assert len(kept) == len(lines) - 1, needle
    path.write_text(''.join(kept))


CASES = [
    ('unchanged', None, None),
    ('require dropped', 'go.mod', 'go.opencensus.io v0.24.0'),
    ('go.sum line dropped', 'go.sum', 'go.opencensus.io v0.24.0 h1:'),
]

tree, cache, out = (pathlib.Path(argument).resolve() for argument in sys.argv[1:])
results = []
for name, file, needle in CASES:
    with tempfile.TemporaryDirectory() as scratch:
        copy = pathlib.Path(scratch) / 'tree'
        shutil.copytree(tree, copy, symlinks=True)
        if file:
            drop_line(copy / MODULE / file, needle)
        env = dict(os.environ, GOMODCACHE=str(cache), GOCACHE=str(pathlib.Path(scratch) / 'gocache'),
                   GOFLAGS='-mod=readonly', GOPROXY='off', GOSUMDB='off', GOTOOLCHAIN='local')
        argv = ['go', 'mod', 'tidy', '-C', MODULE, '-diff']
        done = subprocess.run(argv, cwd=copy, env=env, capture_output=True, text=True)
        results.append({'case': name, 'edit': {'file': file, 'dropped': needle} if file else None, 'argv': argv,
                        'exit_code': done.returncode, 'stdout': done.stdout, 'stderr': done.stderr})
out.write_text(json.dumps(results, indent=1) + '\n')
