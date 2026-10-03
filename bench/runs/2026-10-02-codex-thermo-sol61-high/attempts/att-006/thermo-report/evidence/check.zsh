emulate zsh -o extended_glob -o no_function_argzero -o unset
local_version=$1
scenario=$2
completion_file=$PWD/zsh-probes/$local_version/_rg
# Keep packaged ripgrep completions from masking registration failures.
for review_fpath_entry in $fpath; do
  [[ -f $review_fpath_entry/_rg ]] && fpath=(${fpath:#$review_fpath_entry})
done
case $scenario in
  list)
    get_comp_args() { setopt local_options unset; ( _RG_COMPLETE_LIST_ARGS=1 source $1 ); }
    get_comp_args $completion_file
    ;;
  source)
    autoload -Uz compinit
    compinit -D -i
    source <(cat $completion_file)
    print -r -- "source_result=$? registered=${_comps[rg]}"
    _RG_COMPLETE_LIST_ARGS=1 _rg | wc -l
    ;;
  autoload)
    fpath=($completion_file:h $fpath)
    autoload -Uz compinit
    compinit -D -i
    print -r -- "registered=${_comps[rg]}"
    _RG_COMPLETE_LIST_ARGS=1 _rg > "$PWD/zsh-probes/$local_version-first-args.txt"
    wc -l < "$PWD/zsh-probes/$local_version-first-args.txt"
    _RG_COMPLETE_LIST_ARGS=1 _rg > "$PWD/zsh-probes/$local_version-second-args.txt"
    wc -l < "$PWD/zsh-probes/$local_version-second-args.txt"
    ;;
  before-init)
    source <(cat $completion_file)
    print -r -- "source_result=$? compdef_present=$+functions[compdef]"
    autoload -Uz compinit
    compinit -D -i
    print -r -- "registered_after_init=${_comps[rg]-MISSING}"
    ;;
  literal-faq)
    autoload -Uz compinit
    compinit -D -i
    rg() { cat $completion_file; }
    $ source <(rg --generate complete-zsh)
    print -r -- "source_result=$?"
    ;;
  normal-dispatch)
    autoload -Uz compinit
    compinit -D -i
    _arguments() { print -r -- "arguments_called:$*"; return 0; }
    source <(cat $completion_file)
    print -r -- "source_result=$? registered=${_comps[rg]}"
    _rg
    print -r -- "completion_result=$?"
    ;;
esac
