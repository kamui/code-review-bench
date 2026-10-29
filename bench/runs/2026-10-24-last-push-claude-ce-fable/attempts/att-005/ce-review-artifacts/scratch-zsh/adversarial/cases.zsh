#!/usr/bin/env zsh
# Each case runs in a fresh `zsh -f`. Stubs for compdef/_arguments let us see which
# branch of the trailing guard executes without a real completion widget.
S=${0:a:h}
F=$S/fp/_rg

hr() { print -r -- "===== $1" }

hr "A: source BEFORE compinit (compdef undefined), interactive-like"
zsh -f -c '
  source '"$F"'
  print -r -- "rc=$? ; _rg defined: $+functions[_rg] ; compdef defined: $+functions[compdef]"
' 2>&1

hr "A2: source BEFORE compinit, then compinit; does rg get a completer?"
zsh -f -c '
  source '"$F"' 2>/dev/null
  autoload -Uz compinit; compinit -u -D 2>/dev/null
  print -r -- "_comps[rg]=${_comps[rg]:-<unset>}"
' 2>&1

hr "B: source AFTER compinit via process substitution"
zsh -f -c '
  autoload -Uz compinit; compinit -u -D
  source <(cat '"$F"')
  print -r -- "rc=$? ; _comps[rg]=${_comps[rg]:-<unset>}"
' 2>&1

hr "B2: eval after compinit"
zsh -f -c '
  autoload -Uz compinit; compinit -u -D
  eval "$(cat '"$F"')"
  print -r -- "rc=$? ; _comps[rg]=${_comps[rg]:-<unset>}"
' 2>&1

hr "C: zsh-style autoload (-Uz) from fpath; first invocation"
zsh -f -c '
  fpath=('"$S"'/fp $fpath)
  compdef() { print -r -- "STUB compdef called: $* (funcstack=${funcstack[*]})" }
  _arguments() { print -r -- "STUB _arguments called (funcstack=${funcstack[*]})"; return 0 }
  autoload -Uz _rg
  _rg; print -r -- "rc=$?"
  _rg; print -r -- "second rc=$?"
' 2>&1

hr "D: ksh-style autoload (setopt kshautoload; autoload -U without -z)"
zsh -f -c '
  setopt kshautoload
  fpath=('"$S"'/fp $fpath)
  compdef() { print -r -- "STUB compdef called: $* (funcstack=${funcstack[*]})" }
  _arguments() { print -r -- "STUB _arguments called (funcstack=${funcstack[*]})"; return 0 }
  autoload -U _rg
  _rg; print -r -- "rc=$?"
  _rg; print -r -- "second rc=$?"
' 2>&1

hr "E: zsh-style autoload under a different filename (_ripgrep) with #compdef rg"
zsh -f -c '
  fpath=('"$S"'/fp $fpath)
  compdef() { print -r -- "STUB compdef called: $* (funcstack=${funcstack[*]})" }
  _arguments() { print -r -- "STUB _arguments called (funcstack=${funcstack[*]})"; return 0 }
  autoload -Uz _ripgrep
  _ripgrep; print -r -- "first rc=$?"
  _ripgrep; print -r -- "second rc=$?"
' 2>&1

hr "F: relative-path source of a file literally named _rg after compinit"
( cd "$S/fp" && zsh -f -c '
  autoload -Uz compinit; compinit -u -D
  print -r -- "funcstack during source test:"
  source _rg
  print -r -- "rc=$? ; _comps[rg]=${_comps[rg]:-<unset>}"
' 2>&1 )

hr "G: harness simulation (ci/test-complete style), compdef undefined"
zsh -f -c '
  emulate zsh -o extended_glob -o no_function_argzero -o no_unset
  get_comp_args() { setopt local_options unset; ( _RG_COMPLETE_LIST_ARGS=1 source $1 ) }
  out=( ${(f)"$( get_comp_args '"$F"' )"} )
  print -r -- "lines=$#out first=${out[1]}"
' 2>&1

hr "G2: harness simulation when ~/.zshenv already ran compinit (compdef defined)"
zsh -f -c '
  autoload -Uz compinit; compinit -u -D
  emulate zsh -o extended_glob -o no_function_argzero -o no_unset
  get_comp_args() { setopt local_options unset; ( _RG_COMPLETE_LIST_ARGS=1 source $1 ) }
  out=( ${(f)"$( get_comp_args '"$F"' )"} )
  print -r -- "lines=$#out"
' 2>&1

hr "H: source after compinit with setopt nounset (user shell option)"
zsh -f -c '
  autoload -Uz compinit; compinit -u -D
  setopt nounset
  source <(cat '"$F"')
  print -r -- "rc=$? ; _comps[rg]=${_comps[rg]:-<unset>}"
' 2>&1

hr "I: funcstack[1] value when sourcing via <() and in toplevel"
zsh -f -c '
  print -r -- "toplevel funcstack=(${funcstack[*]}) n=$#funcstack"
  f=$(mktemp); print "print -r -- \"in source funcstack[1]=\$funcstack[1]\"" > $f
  source <(cat $f); source $f; eval "$(cat $f)"; rm $f
' 2>&1
