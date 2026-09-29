# 01 — zsh completion script (`crates/core/flags/complete/rg.zsh`)

Scope: the 9-line change at the tail of the script (lines 437-446 at head `855bfa6`), replacing the unconditional `_rg "$@"`.

## Measurements and commands

The Rust crate cannot be built here, and `ci/test-complete` needs a built `rg`, so I could not run it. I read the script and ran scratch zsh experiments under `clone-work/zt`. Each run used `zsh -f` with an isolated `HOME`, and the script under test was the head version of `rg.zsh`.

| Scenario | Command shape | Observed |
| --- | --- | --- |
| A. `source rg.zsh` before `compinit` | `zsh -f -c "source rg.zsh; echo rc=$?"` | `_rg:341: command not found: _arguments`, rc=1 |
| B. `source rg.zsh` after `compinit` | `compinit; source rg.zsh` | rc=0, `$_comps[rg]` is `_rg` |
| C. `source rg.zsh` and then `compinit` | | Same error as A, but `$_comps[rg]` is `_rg` afterwards |
| D. `_rg` on `fpath`, autoloaded by `compinit` | | Registered as `_rg`, function autoloadable |
| E. `_RG_COMPLETE_LIST_ARGS=1 source rg.zsh` (what `ci/test-complete` does) | | Dumps the arg specs as before |
| F, G. Sourced from inside a function, and from inside a function named `_rg` | | Registers `_rg` as expected |

The head-branch condition is `[[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] ))`. It picks between calling `_rg "$@"` and calling `compdef _rg rg`.

## Finding 1 — the "no `compdef`" arm keeps the original bug alive and encodes a test-harness accommodation in production code (confirmed by execution)

The `(( ! $+functions[compdef] ))` arm at `rg.zsh:441` sends every source that happens before `compinit` down the `_rg "$@"` path. That is exactly the failure that issue #2956 reported. In scenario A, `source rg.zsh` with no completion system loaded prints `_rg:341: command not found: _arguments` and returns 1. The message differs from the issue's `comparguments` wording, but the failure is the same.

This matters because the FAQ text added in the same PR tells users to put `source <(rg --generate complete-zsh)` in `$HOME/.zshrc`. Many `.zshrc` files load `compinit` late, after plugin managers, or in a framework-controlled step. For those users the documented one-liner fails with the error the PR set out to fix. The PR's test plan ran the command in an interactive shell where `compinit` had already run, which is scenario B and never touches this arm.

The comment above the condition also misdescribes the behavior. It says "Don't run the completion function when being sourced by itself", but the code runs `_rg` in two of its three cases. Neither the comment nor the FAQ tells the reader that `compdef` must already exist. The comment does not mention the third case, "no completion system present", which exists only so that `ci/test-complete` keeps working. That script sources the file with `_RG_COMPLETE_LIST_ARGS=1`, and `_rg` itself dumps the specs and returns early.

So production control flow keeps a fallback whose sole purpose is a CI harness. It silently turns a real "no completion system" situation into a runtime error instead of a defined outcome.

### Code-judo proposal

Make the file's tail describe three explicit situations, and give the harness its own hook instead of overloading the `compdef` probe.

1. Autoloaded from `fpath` (`$funcstack[1] == _rg`): call `_rg "$@"`, as today.
2. Sourced with the completion system available: `compdef _rg rg`.
3. Sourced with no completion system: do nothing, or `autoload -Uz compinit && compinit` first. Do not call `_rg`.

The harness already sets `_RG_COMPLETE_LIST_ARGS`. `_rg` could check that variable before the arms, or the tail could gate on it:

```zsh
if [[ $funcstack[1] == _rg || -n $_RG_COMPLETE_LIST_ARGS ]]; then
  _rg "$@"
elif (( $+functions[compdef] )); then
  compdef _rg rg
fi
```

With that shape, the "no `compdef`" case is a silent no-op instead of a confusing `_arguments` failure. The harness dependency is named where it is used. The comment can then say what actually happens.

An alternative is to use `(( $+functions[compdef] )) || { autoload -Uz compinit; compinit }` in the sourced branch. That fully removes the ordering trap, but it makes a completion script initialize the user's completion system, which is more invasive. Recommend the no-op arm plus documentation.

Verification status: confirmed by running scenarios A, B and C. I did not run `ci/test-complete` (unavailable). Scenario E only confirms the list-args dump path still works when sourced as the harness does.

## Finding 2 — stale, misleading comment and duplicated trailing logic (low severity, direct fix)

`rg.zsh:437-440` says the function is not run "when being sourced by itself", but the branch runs `_rg` whenever `compdef` is absent. The `See ...issues/2956` and `pulls/2957` URLs point at discussion, not at the condition's rationale. Replace the comment with one sentence per arm: "autoloaded: run"; "sourced with compinit loaded: register with `compdef`"; and the harness case, whichever way Finding 1 is resolved. Reference links are fine as a supplement, but the reader should not have to open a PR thread to learn why three conditions exist.

Verification status: read-only, confirmed against the current text.

## Notes

Checked and found fine: `$funcstack[1] == _rg` correctly detects the autoload case (scenario D). The `#compdef rg` header is unchanged, so `compinit` registration through `fpath` still works. Using `$+functions[compdef]` instead of `type compdef` is the right idiom.
