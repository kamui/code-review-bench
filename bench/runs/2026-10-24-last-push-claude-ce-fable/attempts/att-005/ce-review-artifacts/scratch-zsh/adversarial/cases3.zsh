#!/usr/bin/env zsh
S=${0:a:h}
mkdir -p $S/fpCtx
# Probe file: prints context at the tail position, in each loading style.
cat > $S/fpCtx/_probe <<'EOF'
#compdef probe
_probe() { : }
print -r -- "ctx=${zsh_eval_context[-1]} full=(${zsh_eval_context[*]}) funcstack[1]=${funcstack[1]-} argzero=$0"
EOF
cp $S/fpCtx/_probe $S/fpCtx/_rg

print -r -- "== zsh-style autoload (-Uz), called as _probe"
zsh -f -c 'fpath=('"$S"'/fpCtx $fpath); autoload -Uz _probe; _probe' 2>&1
print -r -- "== ksh-style autoload"
zsh -f -c 'setopt kshautoload; fpath=('"$S"'/fpCtx $fpath); autoload -U _probe; _probe' 2>&1
print -r -- "== source via <()"
zsh -f -c 'source <(cat '"$S"'/fpCtx/_probe)' 2>&1
print -r -- "== source relative _rg from inside dir"
zsh -f -c 'cd '"$S"'/fpCtx; source _rg' 2>&1
print -r -- "== eval"
zsh -f -c 'eval "$(cat '"$S"'/fpCtx/_probe)"' 2>&1
print -r -- "== source from within a shell function (harness style)"
zsh -f -c 'f() { ( source $1 ) }; f '"$S"'/fpCtx/_probe' 2>&1
