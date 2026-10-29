S=${0:a:h}
run() { print -r -- "=== $1"; shift; zsh -f -c "$@" 2>&1 | grep -v _docker | sed 's/^/    /' | head -8 }
print -r -- 'print -r -- "ctx=[$zsh_eval_context[-1]] fs=[$funcstack[1]]"' > $S/fp/_probe2
run "ctx probes" "cd $S/fp; source <(cat _probe2); source _probe2; eval \"\$(cat _probe2)\"; fpath=($S/fp \$fpath); autoload -Uz _probe2; _probe2; setopt ksh_autoload; autoload -z _probe3; cp _probe2 _probe3; _probe3"
run "alt guard: relative source after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; cd $S/fp2; source _rg; print rc=\$? reg=\${_comps[rg]}"
run "alt guard: procsubst source after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; source <(cat $S/fp2/_rg); print rc=\$? reg=\${_comps[rg]}"
run "alt guard: autoload zsh-style" "fpath=($S/fp2 \$fpath); autoload -Uz compinit; compinit -u -d $S/zcd; _RG_COMPLETE_LIST_ARGS=1 _rg | head -2"
run "alt guard: autoload ksh-style" "fpath=($S/fp2 \$fpath); autoload -Uz compinit; compinit -u -d $S/zcd; setopt ksh_autoload; autoload -z _rg; _RG_COMPLETE_LIST_ARGS=1 _rg | head -2"
run "alt guard: ci harness" "emulate zsh -o extended_glob -o no_function_argzero -o no_unset; get_comp_args() { setopt local_options unset; ( _RG_COMPLETE_LIST_ARGS=1 source \$1 ) }; get_comp_args $S/fp2/_rg | wc -l"
run "orig: ksh_arrays + procsubst source after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; setopt ksh_arrays; source <(cat $S/fp/_rg); print rc=\$? reg=\${_comps[rg]}"
run "orig: source inside a wrapper function after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; load() { source <(cat $S/fp/_rg) }; load; print rc=\$? reg=\${_comps[rg]}"
