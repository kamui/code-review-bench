S=${0:a:h}
run() { print -r -- "=== $1"; shift; zsh -f -c "$@" 2>&1 | sed 's/^/    /' | head -12 }

# (a) source via process substitution WITHOUT compinit
run "a: source <(cat _rg) without compinit" "source <(cat $S/fp/_rg); print rc=\$?; print funcs: \${+functions[_rg]} \${+functions[compdef]}"

# (b) source via process substitution WITH compinit, then list args via _rg
run "b: source <(cat _rg) after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; source <(cat $S/fp/_rg); print rc=\$?; print compdef-registered: \${_comps[rg]}; _RG_COMPLETE_LIST_ARGS=1 _rg | head -3"

# (b2) eval form
run "b2: eval \"\$(cat _rg)\" after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; eval \"\$(cat $S/fp/_rg)\"; print rc=\$?; print compdef-registered: \${_comps[rg]}"

# (c) fpath autoload, zsh-style (-U), via compinit registration
run "c: fpath autoload (zsh style)" "fpath=($S/fp \$fpath); autoload -Uz compinit; compinit -u -d $S/zcd; print registered: \${_comps[rg]}; _RG_COMPLETE_LIST_ARGS=1 _rg | head -3; print second-call:; _RG_COMPLETE_LIST_ARGS=1 _rg | head -1"

# (d) fpath autoload, ksh-style
run "d: fpath autoload (ksh_autoload)" "fpath=($S/fp \$fpath); autoload -Uz compinit; compinit -u -d $S/zcd; setopt ksh_autoload; autoload +X _rg 2>/dev/null; autoload -z _rg; _RG_COMPLETE_LIST_ARGS=1 _rg | head -3"

# (e) source by bare relative filename from inside the dir, after compinit
run "e: cd dir; source _rg (relative) after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; cd $S/fp; source _rg; print rc=\$?"
run "e2: cd dir; source ./_rg after compinit" "autoload -Uz compinit; compinit -u -d $S/zcd; cd $S/fp; source ./_rg; print rc=\$?"

# (f) ci/test-complete harness emulation
run "f: ci/test-complete emulation" "emulate zsh -o extended_glob -o no_function_argzero -o no_unset; get_comp_args() { setopt local_options unset; ( _RG_COMPLETE_LIST_ARGS=1 source \$1 ) }; get_comp_args $S/fp/_rg | wc -l; get_comp_args $S/fp/_rg | head -3"

# (g) what funcstack[1] looks like in each loading mode
print -r -- 'print -r -- "funcstack1=[$funcstack[1]]"' > $S/fp/_probe
run "g: funcstack[1] probes" "cd $S/fp; source <(cat _probe); source _probe; source ./_probe; eval \"\$(cat _probe)\"; fpath=($S/fp \$fpath); autoload -Uz _probe; _probe"
