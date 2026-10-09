setopt ksh_arrays
autoload -Uz compinit; compinit -u -d /tmp/zd4
source ./rg.zsh; print "D: ${_comps[rg]}"
