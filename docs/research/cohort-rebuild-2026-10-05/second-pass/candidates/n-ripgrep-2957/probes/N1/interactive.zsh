rg_binary=$1
mode=$2
route=$3
completion_dir=$4
PS1='PROBE> '
PS2='CONT> '
if [[ $route == autoload ]]; then
  fpath=("$completion_dir" /usr/share/zsh/functions/Completion/Unix /usr/share/zsh/functions/Completion/Base /usr/share/zsh/functions/Completion/Zsh /usr/share/zsh/functions/Completion)
fi
autoload -Uz compinit
compinit -D
if [[ $route == source ]]; then
  unfunction _rg 2>/dev/null
  unset '_comps[rg]'
fi
if [[ $mode == ksh ]]; then
  setopt ksh_arrays
fi
if [[ $route == source ]]; then
  source <("$rg_binary" --generate complete-zsh)
  print -r -- "SOURCE_STATUS=$? REGISTRATION=${_comps[rg]-NONE}"
fi
function capture_buffer {
  print -r -- "BUFFER_CAPTURE=$BUFFER"
  print -r -- "REGISTRATION_CAPTURE=${_comps[rg]-NONE} KSH_CAPTURE=${options[ksharrays]}"
  zle send-break
}
zle -N capture_buffer
bindkey '^X' capture_buffer
