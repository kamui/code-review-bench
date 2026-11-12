#!/bin/sh
# Usage: ksh-funcstack.sh <zsh-root> <scratch-dir>
# Shows what the head's two unbraced tests expand to with KSH_ARRAYS on, in a sourced file,
# and which branch the head's `if` takes there, with compdef defined (a stub) and not.
set -eu
zr=$1; d=$2
zsh=$zr/usr/bin/zsh
mp="module_path=($zr/usr/lib/x86_64-linux-gnu/zsh/5.9)"
mkdir -p "$d"
cat > "$d/probe.zsh" <<'Z'
print -r -- "unbraced funcstack[1] -> $funcstack[1]"
print -r -- "braced \${funcstack[0]} -> ${funcstack[0]-<unset>}  braced \${funcstack[1]} -> ${funcstack[1]-<unset>}"
print -r -- "unbraced +functions[compdef] -> $+functions[compdef]"
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  print -r -- THEN_BRANCH
else
  print -r -- ELSE_BRANCH
fi
print -r -- "after-if status=$?"
Z
run() { label=$1; shift; "$zsh" -f -c "$mp; $*" 2>&1 | sed "s|$d|<scratch>|g; s|^|$label: |"; }
run "ksh_arrays, compdef defined, sourced at top level" "setopt ksh_arrays; compdef() { :; }; source $d/probe.zsh"
run "ksh_arrays, compdef defined, sourced from function _rg" "setopt ksh_arrays; compdef() { :; }; _rg() { source $d/probe.zsh }; _rg"
run "ksh_arrays, no compdef, sourced at top level" "setopt ksh_arrays; source $d/probe.zsh"
run "default options, compdef defined, sourced at top level" "compdef() { :; }; source $d/probe.zsh"
printf 'emulate ksh\n' > "$d/em-ksh"; printf 'emulate sh\n' > "$d/em-sh"
run "after sourcing a file that runs emulate ksh" "source $d/em-ksh; [[ -o ksharrays ]] && print -r -- KSH_ARRAYS=on || print -r -- KSH_ARRAYS=off"
run "after sourcing a file that runs emulate sh" "source $d/em-sh; [[ -o ksharrays ]] && print -r -- KSH_ARRAYS=on || print -r -- KSH_ARRAYS=off"
