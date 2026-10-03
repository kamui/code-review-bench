emulate zsh -o unset
for review_fpath_entry in $fpath; do
  [[ -f $review_fpath_entry/_rg ]] && fpath=(${fpath:#$review_fpath_entry})
done
autoload -Uz compinit
if [[ $1 == late-fpath ]]; then
  compinit -D -i
  print -r -- "before_fpath=${_comps[rg]-MISSING}"
  fpath=($PWD/zsh-probes/head $fpath)
  print -r -- "after_fpath=${_comps[rg]-MISSING}"
else
  fpath=($PWD/zsh-probes/head $fpath)
  compinit -D -i
  print -r -- "before_init_fpath_registered=${_comps[rg]-MISSING}"
fi
