#!/bin/sh
# Usage: gen.sh <mirror.git> <sha> <out-file>
# Produces what `rg --generate complete-zsh` prints at <sha>: the template
# crates/core/flags/complete/rg.zsh with the line !ENCODINGS! replaced by
# crates/core/flags/complete/encodings.sh minus trailing whitespace
# (crates/core/flags/complete/zsh.rs: include_str!("rg.zsh").replace("!ENCODINGS!", ENCODINGS.trim_end())).
set -eu
mirror=$1; sha=$2; out=$3
tmp=$(mktemp -d)
git -C "$mirror" show "$sha:crates/core/flags/complete/rg.zsh" > "$tmp/rg.zsh"
git -C "$mirror" show "$sha:crates/core/flags/complete/encodings.sh" > "$tmp/enc"
python3 -I -c '
import sys
t=open(sys.argv[1]).read(); e=open(sys.argv[2]).read().rstrip()
assert t.count("!ENCODINGS!")==1
open(sys.argv[3],"w").write(t.replace("!ENCODINGS!", e))
' "$tmp/rg.zsh" "$tmp/enc" "$out"
rm -rf "$tmp"
