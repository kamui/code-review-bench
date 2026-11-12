"""Regenerate what `rg --generate complete-zsh` prints at one commit and report the facts N1 rests on.

Usage: python3 -I probe.py <git-dir> <sha>

Rebuilds the script as crates/core/flags/complete/zsh.rs does
(include_str!("rg.zsh").replace("!ENCODINGS!", ENCODINGS.trim_end())), prints its SHA-256 for
comparison with the earlier round's zsh runs, and lists every line that runs when the file is
read (top level, outside any function body), so the reader can see what sourcing the file does.
"""
import hashlib, subprocess, sys

git_dir, sha = sys.argv[1:3]

def show(path):
    return subprocess.run(["git", "-C", git_dir, "show", f"{sha}:{path}"],
                          check=True, capture_output=True, text=True).stdout

template = show("crates/core/flags/complete/rg.zsh")
encodings = show("crates/core/flags/complete/encodings.sh")
assert template.count("!ENCODINGS!") == 1
script = template.replace("!ENCODINGS!", encodings.rstrip())
print(f"commit {sha}")
print(f"generated script sha256 {hashlib.sha256(script.encode()).hexdigest()}  lines {script.count(chr(10))}")

depth = 0
top = []
for n, line in enumerate(script.split("\n"), 1):
    s = line.strip()
    if depth == 0 and s and not s.startswith("#"):
        top.append((n, line))
    if s.endswith("{"):
        depth += 1
    elif s.startswith("}") and depth:
        depth -= 1
print("lines that run at top level when the file is read:")
for n, line in top:
    print(f"  {n}: {line}")
defined = [l.split("(")[0] for _, l in top if l.rstrip().endswith("() {")]
print("functions defined:", " ".join(defined))
body = script.split("\n_rg() {", 1)[1].split("\n}\n", 1)[0]
print("the _rg function body contains the funcstack check:", "funcstack" in body)
print("the _rg function body contains `compdef`:", "compdef" in body)
