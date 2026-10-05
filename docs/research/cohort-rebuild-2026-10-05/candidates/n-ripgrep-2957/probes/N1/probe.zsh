#!/usr/bin/env zsh
# Usage: probe.zsh /path/to/built/rg
#
# Each case runs in its own `zsh -f` process with an empty HOME and a cleared
# environment. The packaged completion directory is dropped from fpath because
# this machine has a distribution ripgrep whose own `_rg` would otherwise
# register rg completion before the probe starts.

emulate -R zsh
setopt err_return no_unset

rg_bin=${1:a}
[[ -x $rg_bin ]] || { print -u2 "not executable: $rg_bin"; exit 2 }

work=$(mktemp -d ${TMPDIR:-/tmp}/n1-probe.XXXXXX)
trap 'rm -rf $work' EXIT
mkdir -p $work/home/.zsh-complete $work/bin
ln -s $rg_bin $work/bin/rg
$rg_bin --generate complete-zsh > $work/home/.zsh-complete/_rg
print -r -- 'print -r -- "funcstack[1] seen by the sourced file: <$funcstack[1]>"' > $work/home/.zsh-complete/_show

print -r -- "rg: $($rg_bin --version | head -1)"
print -r -- "zsh: $(zsh -f -c 'print $ZSH_VERSION')"
print -r -- "generated script sha256: $(sha256sum < $work/home/.zsh-complete/_rg | cut -d' ' -f1)"
print -r -- "last lines of the generated script before the reference section:"
grep -n -B1 -A8 '^# Don.t run the completion function\|^_rg "\$@"$' $work/home/.zsh-complete/_rg | sed 's/^/    /'
print

run_case() {
  local title=$1 before_compinit=$2 with_compinit=$3 command=$4
  {
    print -r -- 'fpath=(${fpath:#*vendor-completions*})'
    print -r -- $before_compinit
    (( with_compinit )) && print -r -- 'autoload -Uz compinit && compinit -D'
    print -r -- $command
    print -r -- 'print -r -- "exit status of the command: $?"'
    print -r -- 'print -r -- "rg completion registered (_comps[rg]): ${_comps[rg]-<nothing>}"'
    print -r -- 'print -r -- "_rg function defined: $+functions[_rg]"'
  } > $work/case.zsh
  print -r -- "=== $title"
  print -r -- "    command: $command"
  env -i HOME=$work/home ZDOTDIR=$work/home PATH=$work/bin:/usr/bin:/bin TERM=dumb \
    zsh -f $work/case.zsh 2>&1 | sed "s#$work#<tmp>#g; s/^/    /" || true
  print
}

print -r -- "--- How zsh names a sourced file in funcstack (mechanism check, no ripgrep code involved)"
run_case "bare name"            ''  0 'cd ~/.zsh-complete && source _show'
run_case "./ prefix"            ''  0 'cd ~/.zsh-complete && source ./_show'
run_case "absolute path"        ''  0 'source ~/.zsh-complete/_show'
run_case "process substitution" ''  0 'source <(cat ~/.zsh-complete/_show)'

print -r -- "--- Sourcing the completion script after compinit; ~/.zsh-complete is NOT in fpath"
run_case "A. bare name from its own directory"   '' 1 'cd ~/.zsh-complete && source _rg'
run_case "B. ./ prefix from its own directory"   '' 1 'cd ~/.zsh-complete && source ./_rg'
run_case "C. absolute path"                      '' 1 'source ~/.zsh-complete/_rg'
run_case "D. documented form (FAQ), process substitution" '' 1 'source <(rg --generate complete-zsh)'
run_case "E. eval form (used by the author in the review thread)" '' 1 'eval "$(rg --generate complete-zsh)"'
run_case "F. dot command with bare name"         '' 1 'cd ~/.zsh-complete && . _rg'
run_case "J. bare name, but the file was saved under a name other than _rg" \
  '' 1 'cd ~/.zsh-complete && cp _rg rg-completion.zsh && source rg-completion.zsh'

print -r -- "--- Same, but the directory was added to fpath before compinit, as the FAQ's recommended method says"
run_case "G. bare name from its own directory, directory already in fpath" \
  'fpath=($HOME/.zsh-complete $fpath)' 1 'cd ~/.zsh-complete && source _rg'
run_case "H. control: no source at all, directory in fpath" \
  'fpath=($HOME/.zsh-complete $fpath)' 1 'true'

print -r -- "--- Control: bare-name source before compinit (compdef does not exist yet)"
run_case "I. bare name, no compinit" '' 0 'cd ~/.zsh-complete && source _rg'
