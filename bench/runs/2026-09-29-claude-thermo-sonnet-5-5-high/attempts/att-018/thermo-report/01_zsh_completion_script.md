# Detail 01: `crates/core/flags/complete/rg.zsh` dispatch

## Change

Old tail: `_rg "$@"`. New tail (lines 437-445):

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

## Measurements (scratch zsh, stub `_rg` calling `_arguments`)

- `zsh -f -c 'source t.zsh'` (no compinit): `_rg: command not found: _arguments`. In a real completion-less shell the real `_rg` fails the same way, on its first `_arguments` call.
- `source <(cat t.zsh)` with no compinit: same error, exit status 127.
- After `compinit`, `source t.zsh`: exit 0, and `_comps[rg]` is `_rg`. This is the intended path.
- Autoload from `fpath` (file is `_rg`), then `_rg`: `funcstack[1]` is `_rg`, so the body runs. This is fine.
- Sourced inside a wrapper function after `compinit`: `compdef` registers `_rg`. This is fine.

## Finding 1 (verified by execution)

The second disjunct is meant to cover "no completion system." In that case running `_rg` is guaranteed to fail for real users. The only legitimate consumer of that branch is `ci/test-complete`, which runs `( _RG_COMPLETE_LIST_ARGS=1 source $1 )` in a shell that has no `compdef`. The PR thread on this line says the arm was added so that script passes. Two unrelated intents (a user sourcing before `compinit`, and a test dumping arg specs) share one condition, and the user case gets the wrong behavior.

### Code-judo proposal

The script already has a purpose-built test hook (`_RG_COMPLETE_LIST_ARGS`, `rg.zsh:30` and `:347`). Use it:

```zsh
if [[ $funcstack[1] == _rg || $_RG_COMPLETE_LIST_ARGS == (1|t*|y*) ]]; then
  _rg "$@"                     # autoloaded as _rg, or test harness
elif (( $+functions[compdef] )); then
  compdef _rg rg               # sourced with the completion system loaded
else
  # sourced before compinit: nothing to register with yet
fi
```

For the last case, either do nothing and document that `compinit` must come first, or defer registration (for example, append to `precmd_functions` for one shot). Nothing now hides the harness inside the user-facing condition, and the user case can no longer surface the original error. This changes test-only behavior: the harness keeps working through its existing variable.

Verification status: the failure is confirmed by execution. The proposed rewrite was not executed against the real `ci/test-complete`, which is unavailable (needs a built `rg`).

## Finding 4 (minor)

The comment at lines 437-440 ("Don't run the completion function when being sourced by itself") does not describe either condition and defers to two URLs. Replace it with a per-case comment as in the proposal.
