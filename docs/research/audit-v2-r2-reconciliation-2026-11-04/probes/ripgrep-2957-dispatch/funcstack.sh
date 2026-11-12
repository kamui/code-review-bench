#!/bin/sh
# Usage: funcstack.sh <zsh-root> <scratch-dir>
# <zsh-root> holds the extracted Debian zsh 5.9-8 packages (usr/bin/zsh, its modules).
# Prints $funcstack[1] as seen by a line of a sourced file when the file is sourced at
# top level, from a function named _rg, from a function named other, and evaluated
# from a function named _rg.
set -eu
zr=$1; d=$2
zsh=$zr/usr/bin/zsh
mp="module_path=($zr/usr/lib/x86_64-linux-gnu/zsh/5.9)"
mkdir -p "$d"
printf 'print -r -- "funcstack[1]=${funcstack[1]-<unset>}  funcstack=(${funcstack[*]})"\n' > "$d/probe.zsh"
run() { label=$1; shift; "$zsh" -f -c "$mp; $*" 2>&1 | sed "s|$d|<scratch>|g; s|^|$label: |"; }
run "top-level source" "source $d/probe.zsh"
run "function _rg sources it" "_rg() { source $d/probe.zsh }; _rg"
run "function other sources it" "other() { source $d/probe.zsh }; other"
run "function _rg evals it" "_rg() { eval \"\$(<$d/probe.zsh)\" }; _rg"
