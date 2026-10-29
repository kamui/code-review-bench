#!/usr/bin/env zsh
S=${0:a:h}
mkdir -p $S/fpE
cp $S/fp/_rg $S/fpE/_ripgrep

print -r -- "===== A2 clean: source BEFORE compinit, then compinit (no system _rg on fpath)"
zsh -f -c '
  fpath=( ${fpath:#*(vendor|site)*} )
  source '"$S"'/fp/_rg
  print -r -- "source rc=$?"
  autoload -Uz compinit; compinit -u -D
  print -r -- "_comps[rg]=${_comps[rg]:-<unset>}"
' 2>&1

print -r -- "===== E real compinit: only _ripgrep (with #compdef rg) on fpath"
zsh -f -c '
  fpath=( '"$S"'/fpE ${fpath:#*(vendor|site)*} )
  autoload -Uz compinit; compinit -u -D
  print -r -- "after compinit _comps[rg]=${_comps[rg]:-<unset>}"
  _arguments() { print -r -- "REAL-PATH _arguments called (funcstack=${funcstack[*]})"; return 0 }
  $_comps[rg]; print -r -- "first call rc=$? now _comps[rg]=${_comps[rg]:-<unset>}"
  $_comps[rg]; print -r -- "second call rc=$?"
' 2>&1

print -r -- "===== E-base: same but with BASE version of the tail (_rg \"\$@\")"
mkdir -p $S/fpEbase
sed -e '/^if \[\[ \$funcstack\[1\] == _rg \]\]/,/^fi$/c\
_rg "$@"' $S/fp/_rg > $S/fpEbase/_ripgrep
grep -n '^_rg "\$@"' $S/fpEbase/_ripgrep
zsh -f -c '
  fpath=( '"$S"'/fpEbase ${fpath:#*(vendor|site)*} )
  autoload -Uz compinit; compinit -u -D
  _arguments() { print -r -- "BASE _arguments called (funcstack=${funcstack[*]})"; return 0 }
  $_comps[rg]; print -r -- "first call rc=$?"
' 2>&1

print -r -- "===== A-base: BASE version sourced before compinit (for comparison)"
cp $S/fpEbase/_ripgrep $S/fpEbase/_rg
zsh -f -c '
  fpath=( ${fpath:#*(vendor|site)*} )
  source '"$S"'/fpEbase/_rg
  print -r -- "source rc=$?"
' 2>&1
