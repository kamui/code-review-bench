# Completion loading

## Finding: Separate completion registration from test invocation

The new footer in `crates/core/flags/complete/rg.zsh` uses this condition at lines 441-444:

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

The `_rg` function itself is defined earlier in the same generated file. The new absent-`compdef` case serves the existing `ci/test-complete` implementation: `get_comp_args` sources the completion file in a subshell, sets `_RG_COMPLETE_LIST_ARGS=1`, and expects source-time execution to invoke `_rg` and print its argument specs. This makes the generated completion file's production load behavior depend on a test harness convention. A shell that sources the script before running `compinit` also has no `compdef`, so sourcing immediately runs `_rg` outside the completion system. In a focused reproduction, `zsh -fc 'source crates/core/flags/complete/rg.zsh'` exited 1 with `_rg:341: command not found: _arguments`.

This is a boundary problem, not just an inelegant condition: “test wants the completion function executed” and “the shell is sourcing a completion definition” are distinct operations, but the footer guesses between them from a global function-table detail. That is brittle as test setup or initialization order changes, and it turns a normal source operation into a call that assumes completion context.

## Worked code-judo proposal

Give the test the invocation it actually needs. Change the test helper's list-args path to source the file to define `_rg`, then call `_rg` explicitly with `_RG_COMPLETE_LIST_ARGS=1` set. Keep the generated script's footer responsible only for its two production roles: call `_rg` when the completion system is invoking the autoloaded `_rg` function, and register it with `compdef` when an already initialized shell sources the file. Do not use absence of `compdef` as a third, implicit “run the completion now” mode. The FAQ should say to initialize zsh completion (`compinit`) before the process-substitution `source` line, since a script sourced before initialization cannot register with `compdef`.

This removes a test-only behavior inference from a generated runtime script and makes each caller own its intent. It also makes the limitation of dynamic sourcing explicit instead of allowing initialization order to select an accidental execution path.

## Evidence and measurements

- Reviewed `git diff main...review-head -- FAQ.md crates/core/flags/complete/rg.zsh`; the only completion logic change is the eight-line footer branch at `crates/core/flags/complete/rg.zsh:437-445`.
- Read `ci/test-complete`: `get_comp_args` runs `(_RG_COMPLETE_LIST_ARGS=1 source $1)`, relying on the current footer to invoke `_rg` during `source`.
- Read `_rg`'s list-args handling at `crates/core/flags/complete/rg.zsh:345` and its use of completion helpers including `_arguments`.
- Ran `zsh -fc 'source crates/core/flags/complete/rg.zsh'` from the repository root. It exited 1 with `_rg:341: command not found: _arguments`, confirming that a missing `compdef` selects an invalid source-time execution path in an uninitialized shell.
- `crates/core/flags/complete/rg.zsh` is 645 lines; `FAQ.md` is 1063 lines in the head checkout. This change does not push either file across the 1000-line threshold.

## Verification status

The focused source behavior was executed and the failure reproduced. The full `ci/test-complete` option comparison was not run because it requires a built `rg`. The run policy disallows building/testing the Rust crate because dependencies are not vendored. No files in the clone were modified.
