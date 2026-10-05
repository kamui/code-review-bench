rg_binary=$1
mode=$2
autoload -Uz compinit
compinit -D
unfunction _rg 2>/dev/null
unset '_comps[rg]'
if [[ $mode == ksh ]]; then
  setopt ksh_arrays
fi
print -r -- "ksh_arrays=${options[ksharrays]}"
source <("$rg_binary" --generate complete-zsh)
source_status=$?
print -r -- "source_status=$source_status"
print -r -- "rg_registration=${_comps[rg]-NONE}"
print -r -- "rg_function_exists=${+functions[_rg]}"
print -r -- "shell_continues=yes"
