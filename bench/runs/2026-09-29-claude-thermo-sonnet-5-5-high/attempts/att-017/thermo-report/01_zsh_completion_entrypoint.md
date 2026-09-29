# Detail 01: `crates/core/flags/complete/rg.zsh` entry point

Scope: the 9-line change at lines 437-446. Context: the file used to end with an unconditional `_rg "$@"`, which works when zsh autoloads the file from `fpath` (the file body becomes the `_rg` function body) and when `ci/test-complete` sources it. It fails when a user sources it into an interactive shell, which is what issue #2956 reports.

## Method

I extracted the head version of `rg.zsh` into a scratch directory, replaced `!ENCODINGS!` with a dummy value, and ran zsh 5.8.1 with `zsh -f` (no user config) and an isolated `compinit -d` dump file. Nothing was added to the clone. `ci/test-complete` and a built `rg` were unavailable, so I imitated its `( _RG_COMPLETE_LIST_ARGS=1 source $file )` call.

## Measurements

| Scenario | Command shape | Result |
| --- | --- | --- |
| A. source before `compinit` | `zsh -f -c 'source ./rg.zsh'` | `_rg:341: command not found: _arguments`, rc=1 |
| B. source after `compinit` | `compinit; source <(cat rg.zsh)` | rc=0, `_comps[rg]` is `_rg` |
| D. harness style, no `compinit` | `( _RG_COMPLETE_LIST_ARGS=1 source ./rg.zsh )` | prints the argument specs (as intended) |
| E. harness style, `compinit` loaded | same as D, after `compinit` | prints nothing, exit 0 |
| F. sourced from inside another function | `f(){ source ./rg.zsh; }; f` after `compinit` | registers `_rg` correctly |
| G. autoloaded from `fpath` as `_rg` | `fpath=(dir $fpath); compinit` | function autoloaded, registered by `#compdef` |

## Finding 1: `compdef` absence is a proxy for the test harness

`$+functions[compdef]` is 0 in exactly two situations: the CI harness, and any user whose `.zshrc` sources the script before `compinit`. The code treats both as "run the completion function now". For the harness that is intended. For the user it reproduces the failure from #2956 with a different message (scenario A). The feature therefore works only for users who happen to have their `.zshrc` in the right order, and the failure for everyone else is still an internal `_arguments` error that says nothing about `compinit`.

### Code-judo proposal

Make the two intents explicit. The script already has a documented test hook, `_RG_COMPLETE_LIST_ARGS`, used at lines 30 and 347. Use it:

```zsh
if [[ $funcstack[1] == _rg || $_RG_COMPLETE_LIST_ARGS == (1|t*|y*) ]]; then
  _rg "$@"
elif (( $+functions[compdef] )); then
  compdef _rg rg
else
  print -ru2 -- 'rg completion: run compinit before sourcing this script'
fi
```

This deletes the implicit third state, keeps CI working, and gives a real message for the ordering mistake. If the maintainers prefer no output, the last branch can be a documented no-op. Either choice is better than falling through into a function that cannot work. I ran this variant in the scratch directory: sourcing before `compinit` prints the message, sourcing after `compinit` registers `_rg`, and dump mode prints the specs both with and without `compinit`. I did not run it against `ci/test-complete`, since that needs a built binary.

Verification status: confirmed by execution (scenarios A, D, E).

## Finding 2: dump mode bypassed when a completion system is loaded

Scenario E: with `compdef` defined, `_RG_COMPLETE_LIST_ARGS=1 source` reaches `compdef _rg rg` and never runs `_rg`, so the dump at line 347 never executes. Output is empty and exit status is 0. In `ci/test-complete`, `comp_args` would be empty and every option would be reported as missing from the completion function, or the comparison would be meaningless. That is a hard-to-diagnose failure mode rather than an outright error. CI today is unaffected only because the script's shebang shell does not load `compinit`. The proposal above fixes this, because dump mode no longer depends on `compdef` being absent.

Verification status: confirmed by execution (scenario E). The CI script itself was not run.

## Notes

The PR thread shows the author first wrote a simpler check, `funcstack[1] = _rg` with `type compdef`, and the reviewer supplied the current condition as the more idiomatic and faster one. The idiom is fine on its own terms. The concern here is only what the second clause stands in for.
