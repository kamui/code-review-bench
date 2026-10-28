# Thermo nuclear review: ripgrep PR 2957 (`main...review-head`)

Scope: `FAQ.md` (+20/−3) and `crates/core/flags/complete/rg.zsh` (+9/−1). I couldn't build rg or run `ci/test-complete`. I checked the zsh behaviour with scratch scripts that use a stub `_rg` and the same guard as the diff.

## Findings

### 1. Medium: `source <(rg --generate complete-zsh)` still fails if it runs before `compinit`, and the FAQ doesn't say so
**File:** `crates/core/flags/complete/rg.zsh:441-446`, with the FAQ text at `FAQ.md:131-139`.

The new guard is `if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then _rg "$@"; else compdef _rg rg; fi`.
`compdef` is only defined once `compinit` has run. This is what I traced and tested:

- **After `compinit`:** `funcstack[1]` is not `_rg` (it is the sourced file or `/dev/fd/N`) and `compdef` exists, so the script registers `compdef _rg rg`. This works, and it is the case the PR was written to fix.
- **Before `compinit`:** `compdef` is undefined, so the second clause is true and the script calls `_rg "$@"` at source time. I confirmed in scratch zsh that the stub `_rg` ran during `source` with stack `_rg ./stub.zsh`. Real `_rg` starts by calling `_arguments`, so the user would get the original `_arguments: can only be called from completion function` error from issue #2956. `compdef _rg rg` is never run, so rg completion isn't registered by this source line.
- **Why this matters:** the `(( ! $+functions[compdef] ))` clause exists so `ci/test-complete` can source the file with no completion system loaded. In a real `.zshrc`, though, `compinit` is often run at the end, as plugin managers and many dotfiles do. A user who follows the new FAQ advice and puts the `source` line above `compinit` gets the same error as before, with no hint about why.

The FAQ block at `FAQ.md:131-139` gives no ordering requirement. Suggested fix: say in the FAQ that the line must come after `compinit`. Alternatively, make the script detect an uninitialised completion system and fall back to something safe. The test-only branch could also be keyed to an explicit variable such as the existing `_RG_COMPLETE_LIST_ARGS`, instead of the absence of `compdef`.

### 2. Low: the `source` snippet in `FAQ.md` has a literal `$ ` prompt prefix
**File:** `FAQ.md:133-136`.

The block says "add the following to your `$HOME/.zshrc` file" and then shows it as `$ source <(rg --generate complete-zsh)`. The `$ ` prompt marker is correct for the shell-command blocks above it, but this block is file content. The `fpath=(...)` block just before it correctly has no prefix. If someone pastes the line verbatim into `.zshrc`, zsh runs `$` as a command, which fails with `command not found: $`, and the `source` that follows never runs. The `$ ` prefix should be removed for consistency with the `fpath` example.

### 3. Low (docs wording): the FAQ sentence at `FAQ.md:138-139` is ungrammatical and the claim isn't measured
**File:** `FAQ.md:138-139`.

"…while this approach is easier to setup, is generally slower than the previous method…" has a broken clause: "is generally slower" has no subject, and the noun should be "set up". The claim that it "will add more time to loading your shell prompt" is plausible, but the PR thread only mentions about 4 ms. The sentence is also missing the self-updating upside that was raised in review. This is cosmetic.

## Checked and found fine
- **Autoload from `fpath`:** the file is run as a function named `_rg`, so `funcstack[1] == _rg` and `_rg "$@"` runs as before. Scratch test confirmed the stack is `_rg _rg`.
- **Sourcing after `compinit`:** it registers `compdef _rg rg` with no error.
- **`ci/test-complete`:** it runs `( _RG_COMPLETE_LIST_ARGS=1 source $1 )` with no `compdef`, so the second clause keeps it calling `_rg`. This was not executed because `rg` can't be built here.
- **`no_unset` / `extended_glob`:** `$funcstack[1]` and `$+functions[compdef]` are safe under the options the script sets.
- No security issues: the diff adds no new input handling or execution paths.
