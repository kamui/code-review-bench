"""Check that the v4 tools-module form reports missing go.sum entries, outside the sandbox.

    python3 -I tools_module_control.py PREPARED SCRATCH OUT

PREPARED holds the `clone/` and `clone-cache/` that `bench-tools provision prepare` made for u-grpc-go-6919 with
its v3 supplement. SCRATCH must be empty. Each case copies `clone/test/tools` into SCRATCH, drops at most one go.sum
line and runs `go list -C <copy> -tags tools -deps -e -json .` with the profile's offline Go environment. A last case
runs the `-modfile=test/tools/go.mod` form on `./test/tools` from the clone root. OUT records each case's exit code
and every distinct package error with its count.
"""

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

WHITESPACE = re.compile(r"\s*")
prepared, scratch, out = map(pathlib.Path, sys.argv[1:4])
assert not any(scratch.iterdir()), "SCRATCH must be empty"
environment = {
    **os.environ,
    "GOMODCACHE": str(prepared / "clone-cache/toolchain-extra/gomodcache"),
    "GOCACHE": str(scratch / "gocache"),
    "GOFLAGS": "-mod=readonly",
    "GOPROXY": "off",
    "GOSUMDB": "off",
    "GOTOOLCHAIN": "local",
}
cases = [
    ("review head go.sum", None),
    ("without a module hash", "honnef.co/go/tools v0.4.6 h1:"),
    ("without a go.mod hash", "golang.org/x/tools v0.14.0/go.mod "),
]


def errors_of(argv, cwd):
    done = subprocess.run(argv, cwd=cwd, env=environment, capture_output=True, text=True)
    errors = {}
    decoder = json.JSONDecoder()
    text = done.stdout
    index = WHITESPACE.match(text).end()
    while index < len(text):
        package, index = decoder.raw_decode(text, index)
        index = WHITESPACE.match(text, index).end()
        for error in [package.get("Error")] + package.get("DepsErrors", []):
            if error:
                errors[error["Err"]] = errors.get(error["Err"], 0) + 1
    return {
        "exit_code": done.returncode,
        "stderr": done.stderr,
        "errors": [{"err": err, "count": count} for err, count in sorted(errors.items())],
    }


results = []
for number, (name, dropped) in enumerate(cases, 1):
    module = scratch / f"tools-{number}"
    shutil.copytree(prepared / "clone/test/tools", module)
    sums = module / "go.sum"
    sums.chmod(0o644)
    lines = sums.read_text().splitlines(keepends=True)
    kept = [line for line in lines if dropped is None or not line.startswith(dropped)]
    assert len(lines) - len(kept) == (0 if dropped is None else 1)
    sums.write_text("".join(kept))
    argv = ["go", "list", "-C", str(module), "-tags", "tools", "-deps", "-e", "-json", "."]
    results.append({
        "case": name,
        "dropped_go_sum_line": dropped,
        "argv": ["go", "list", "-C", f"<scratch>/tools-{number}", *argv[4:]],
        **errors_of(argv, scratch),
    })
argv = ["go", "list", "-modfile=test/tools/go.mod", "-tags", "tools", "-deps", "-e", "-json", "./test/tools"]
results.append({"case": "-modfile from the clone root", "dropped_go_sum_line": None, "argv": argv,
                **errors_of(argv, prepared / "clone")})
for result in results:
    print(result["case"], "exit", result["exit_code"], "distinct errors", len(result["errors"]), flush=True)
out.write_text(json.dumps(results, indent=1, ensure_ascii=False) + "\n")
