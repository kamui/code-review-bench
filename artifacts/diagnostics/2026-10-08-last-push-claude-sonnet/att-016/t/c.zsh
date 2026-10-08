autoload -Uz compinit; compinit -u -d /tmp/zd3
eval "$(cat ./rg.zsh)"; print "C: ${_comps[rg]}"
f(){ source ./rg.zsh }; f; print "C2: ${_comps[rg]}"
