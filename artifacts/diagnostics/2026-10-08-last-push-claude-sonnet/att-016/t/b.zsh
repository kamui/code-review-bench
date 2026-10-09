source ./rg.zsh; print "B: before compinit rc=$?"
autoload -Uz compinit; compinit -u -d /tmp/zd2
print "B: _comps[rg]=${_comps[rg]}"
