#!/usr/bin/env zsh
# Usage: zsh -f eval_context.zsh
#
# Does not involve ripgrep. Shows what zsh reports to a file about how it is
# being run: through `source` under several spellings, and through autoload
# from fpath under two file names. Runs in a throwaway HOME.

emulate -R zsh
work=$(mktemp -d ${TMPDIR:-/tmp}/eval-context.XXXXXX)
trap 'rm -rf $work' EXIT
cat > $work/inner.zsh <<'INNER'
mkdir -p $HOME/fn && cd $HOME/fn
print -r -- 'print -r -- "    funcstack[1]=<$funcstack[1]>  zsh_eval_context=<$zsh_eval_context>"' > _rg
print "sourced by bare name (source _rg):";   source _rg
print "sourced with ./ (source ./_rg):";      source ./_rg
print "sourced via process substitution:";    source <(cat _rg)
cp _rg _ripgrep
fpath=($HOME/fn $fpath); autoload -Uz _rg _ripgrep
print "autoloaded from fpath as _rg:";        _rg
print "autoloaded from fpath as _ripgrep:";   _ripgrep
INNER
mkdir $work/home
env -i HOME=$work/home PATH=/usr/bin:/bin zsh -f $work/inner.zsh
print "exit status: $?"
