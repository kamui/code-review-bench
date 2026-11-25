# Thermo-nuclear review — BurntSushi/ripgrep#2957

"feat(completion): support sourcing zsh completion dynamically"
Range `79cbe89deb1151e703f4d91b19af9cdcc128b765..855bfa6cdae4f4fe8762f892fc4957635397083e` (2 files, +29 / −4).

## Verdict

The change does what it sets out to do. Four low-priority findings follow: two in the new FAQ text, two in the new guard at the bottom of `rg.zsh`. Nothing here is a security issue, and nothing breaks the standard install path.

## What was checked

The Rust crate could not be built in this environment, so `ci/test-complete` itself was not run. Instead I drove a real interactive `zsh -f -i` (zsh 5.8.1) through a pty, with a stub `rg` that prints the script, typed `rg --line-b<TAB>` and read back the edit buffer. The system's own `_rg` was removed from `fpath` so only the script under review could answer. Each scenario was run against the head script and, where relevant, the merge-base script.

| Scenario | Merge-base | Head |
| --- | --- | --- |
| Saved as `_rg` on `fpath`, then `compinit` (packaged install) | completes | completes |
| `compinit`, then `source <(rg --generate complete-zsh)` | error, no completion | completes |
| `source <(...)`, then `compinit` | error, no completion | error, no completion |
| Saved on `fpath` under another name (`_ripgrep`, `#compdef rg`) | completes on first Tab | first Tab does nothing, second completes |
| Lazy wrapper `_rg() { eval "$(rg --generate complete-zsh)" }` or `source <(...)` | completes on first Tab | first Tab does nothing, second completes |
| `ci/test-complete`-style `( _RG_COMPLETE_LIST_ARGS=1 source rg.zsh )`, no `compdef` | 341 lines | 341 lines |
| Same, with `compdef` defined via `.zshenv` | 341 lines | 0 lines |

The PR's headline case works, the packaged `_rg` install is unchanged, and the test harness in a clean environment still gets its argument list. `crates/core/flags/complete/zsh.rs` only substitutes `!ENCODINGS!`, and the release scripts (`ci/build-and-publish-m2`, `.github/workflows/release.yml`) all write the file as `_rg`, so shipped archives are unaffected.

## Findings

### 1. FAQ does not say the new `source` line must come after `compinit`

**Priority: Low.** `FAQ.md` lines 131–136 (and lines 124–129 for the mirror-image ordering).

The FAQ tells readers to add `source <(rg --generate complete-zsh)` to `.zshrc` with no ordering requirement. The script only registers itself when `compdef` already exists (`crates/core/flags/complete/rg.zsh:441`); otherwise it falls into the `_rg "$@"` branch. Running `source <(rg --generate complete-zsh); autoload -Uz compinit; compinit` against the head script prints `_rg:341: command not found: _arguments` and leaves `rg --line-b<TAB>` uncompleted, on every Tab. The same happens with no `compinit` at all, which is the state of a zsh with no existing completion setup. A reader who puts the line above their `compinit` call (or above the line that sources a framework which calls it) gets that error on every shell start and no ripgrep completions, and the FAQ gives no hint why. The recommended `fpath` method has the opposite requirement, also unstated: adding `fpath=($HOME/.zsh-complete $fpath)` after `compinit` produced no completion in the same test. One sentence stating that the `fpath` line goes before `compinit` and the `source` line after it would close this.

### 2. FAQ snippet meant for `.zshrc` carries a `$ ` prompt prefix

**Priority: Low.** `FAQ.md` lines 134–136.

The text says "add the following to your `$HOME/.zshrc` file" and the block that follows reads `$ source <(rg --generate complete-zsh)`. The `fpath` block a few lines up (line 128), which is also meant for `.zshrc`, has no prefix. Sourcing a file containing the line exactly as printed gives `command not found: $`; `_rg` is not defined and `_comps[rg]` stays unset. A reader who copies the block verbatim gets an error on every shell start and no completions. Dropping the `$ ` from line 135 fixes it.

### 3. Guard only recognises a completion call when the top frame is literally named `_rg`

**Priority: Low.** `crates/core/flags/complete/rg.zsh` lines 437–445.

When `compdef` exists, the script runs the completion only if `$funcstack[1] == _rg`; in every other case it calls `compdef _rg rg` and returns without producing matches. That test is true for the standard autoload from an `fpath` file named `_rg`, but false in two cases where the script is being run by the completion system and the merge-base script completed on the first Tab:

- The file is on `fpath` under another name with the same `#compdef rg` tag (tested as `_ripgrep`): `funcstack[1]` is `_ripgrep`.
- A lazy-loading wrapper such as `_rg() { eval "$(rg --generate complete-zsh)" }; compdef _rg rg`, or the same with `source <(...)`: `funcstack[1]` is `(eval)` or the `/proc/self/fd/N` path, with `_rg` one frame down. Before this PR that wrapper was the working way to get self-updating completions.

In both cases the first Tab after shell start now does nothing; the second works because `_rg` has been redefined and re-registered. The failure heals itself, which is why this is Low, but it is a regression for those setups. `$+compstate` is 1 inside a completion widget (including inside a nested `eval`) and 0 outside, so testing it instead of the function name covers both cases: with the condition changed to `(( $+compstate )) || [[ -n ${_RG_COMPLETE_LIST_ARGS-} ]] || (( ! $+functions[compdef] ))`, every scenario in the table above completed on the first Tab.

### 4. `ci/test-complete` gets no argument list if `compdef` is defined in the developer's environment

**Priority: Low.** `crates/core/flags/complete/rg.zsh` line 441, as consumed by `ci/test-complete` line 17.

The harness runs `( _RG_COMPLETE_LIST_ARGS=1 source $1 )` and relies on the script calling `_rg` so that it prints its `_arguments` specs. The new guard uses "no `compdef` function" as the signal that it is being run this way. `ci/test-complete` is a `#!/usr/bin/env zsh` script, so it reads `.zshenv`; if that file runs `compinit` or otherwise defines `compdef`, the guard takes the `compdef _rg rg` branch and prints nothing. I reproduced the harness's `emulate` line and `get_comp_args` in a scratch script: the head script yields 341 lines normally and 0 lines with a `.zshenv` that runs `compinit`, where the merge-base script yields 341 in both. With zero lines the script reaches its `Failed to get comp_args` exit. CI and most developer machines have no `compdef` at that point, so this affects few people; honouring `_RG_COMPLETE_LIST_ARGS` in the guard, as in the condition under finding 3, removes the dependency on the environment.

## Not reported as findings

- `FAQ.md` line 138 reads "is easier to setup, is generally slower"; it wants "it is generally slower" (and "set up"). Wording only.
- With `setopt nounset no_eval_lineno`, an `eval` of the script at top level fails on `$funcstack[1]` ("parameter not set"). The option combination is rare enough that I did not count it.
