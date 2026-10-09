autoload -Uz compinit; compinit -u -d /tmp/zd1
source ./rg.zsh; print "A: after compinit source: ${_comps[rg]}"
