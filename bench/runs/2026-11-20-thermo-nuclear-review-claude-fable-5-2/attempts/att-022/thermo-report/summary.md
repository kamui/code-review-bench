# Thermo-nuclear review — BurntSushi/ripgrep#2957

"feat(completion): support sourcing zsh completion dynamically"
Range reviewed: `79cbe89deb1151e703f4d91b19af9cdcc128b765..855bfa6cdae4f4fe8762f892fc4957635397083e` (`git diff main...review-head`), 2 files: `crates/core/flags/complete/rg.zsh` (+9 −1) and `FAQ.md` (+20 −3).

## Verdict

The change does what the PR says in the two mainstream cases, and I found nothing high priority, no security issue and no feature-gate leak. There are four actionable findings: one medium and three low. All four come from the same design choice: the new trailer in `rg.zsh` guesses how it was loaded from `$funcstack[1] == _rg` and from whether a `compdef` function exists, and the FAQ text does not state the preconditions that guess relies on.

## What was checked and how

`cargo build`, `cargo test` and `ci/test-complete` were unavailable in this run, so no real `rg` binary was used. Instead I produced the generated script from the head and base versions of `rg.zsh` (with `!ENCODINGS!` substituted, as `zsh.rs` does) and exercised it with zsh 5.8.1 under `zsh -f`, in scratch files outside the clone. A stub `rg` on `PATH` printed the script for `--generate complete-zsh`. Tab completion was driven through a real interactive zsh in a pty (`zsh/zpty`), typing `rg --generate=complete-zs<TAB>`.

Scenarios that behave correctly at head:

- Autoloaded from `fpath` as `_rg` after `compinit` (the packaged path: release archive, deb `vendor-completions/_rg`, brew): first Tab completes to `complete-zsh`. Unchanged from base.
- `source <(rg --generate complete-zsh)` after `compinit`: no error, `_comps[rg]` is `_rg`, first Tab completes. On base this printed `_arguments:comparguments:325: can only be called from completion function`.
- `eval "$(rg --generate complete-zsh)"` after `compinit`: same good result.
- A stand-in for `ci/test-complete`'s `get_comp_args` (same `emulate` options, same subshell `source`) in a clean environment: still lists the argument specs (341 lines at both base and head).

The remaining scenarios are the findings below.

## Findings

### 1. [Medium] Sourcing the script before `compinit` runs `_rg` at shell start-up, prints a misleading error and registers nothing — and the new FAQ text does not say the line must come after `compinit`

**Where:** `crates/core/flags/complete/rg.zsh` lines 441–445; `FAQ.md` lines 131–136.

**Evidence:** The new condition is `if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then _rg "$@"`. The second clause exists so that `ci/test-complete` (which sources the file with no completion system loaded) still reaches `_rg`. But "no `compdef` function" is also exactly the state of a user's shell when the sourced line sits above `compinit` in `.zshrc` (or above the framework line that calls it), or when the user never calls `compinit` at all. The FAQ now tells users to "add the following to your `$HOME/.zshrc` file" with no ordering requirement. Reproduced with `zsh -f`: sourcing the generated script and then running `compinit` prints `_rg:341: command not found: _arguments` (the line number varies with the encodings list), `source` returns 1, and `_comps[rg]` stays unset after `compinit`; in the pty run, Tab on `rg --generate=complete-zs` only rang the bell. (This was with the distribution's own `vendor-completions/_rg` removed from `fpath`; where a packaged `_rg` is also installed, it masks the missing registration but the start-up error still prints.)

**Failure:** A user who follows the FAQ but places the line before `compinit` gets an error on every shell start that names `_arguments` rather than the real cause, and gets no ripgrep completion from the sourced script at all, because `compdef _rg rg` is never executed. The script actively runs the completion function outside a completion context in this case, which is the very thing the PR set out to stop. The scope is limited to that ordering (lines appended to the end of a typical framework-managed `.zshrc` work), which is why this is medium rather than high. Either the FAQ needs to state "after `compinit`", or the fallback to `_rg "$@"` should be keyed on something specific to the test harness (for example `_RG_COMPLETE_LIST_ARGS`) rather than on `compdef` being absent.

### 2. [Low] The `$funcstack[1] == _rg` test makes first-use completion depend on the autoload name being exactly `_rg`; under any other name the first Tab silently does nothing

**Where:** `crates/core/flags/complete/rg.zsh` line 441 (and the `else` branch at lines 443–444).

**Evidence:** Before this change the trailer was an unconditional `_rg "$@"`, so the file worked as an autoloaded completion under any file name carrying the `#compdef rg` tag, and when evaluated from inside another completion function. At head, when the completion system is loaded and the calling function is not literally named `_rg`, the trailer takes the `else` branch and runs `compdef _rg rg` in the middle of a completion attempt instead of completing. Reproduced in the pty harness with the head script installed in `fpath` as `_ripgrep`: the first Tab on `rg --generate=complete-zs` rang the bell and inserted nothing, a second Tab completed. The same file from base completed on the first Tab. The same first-Tab failure reproduced with a lazy loader, `_lazy_rg() { eval "$(rg --generate complete-zsh)" }; compdef _lazy_rg rg`. The test also misfires the other way: `source _rg` by bare name (file found in the current directory or on `path`) makes `$funcstack[1]` equal `_rg`, and the original `_arguments:comparguments:325: can only be called from completion function` error comes back, while `source ./_rg` works.

**Failure:** A behaviour regression for anyone who installs or wraps the generated script under a name other than `_rg`: the first completion attempt in every new shell produces no matches and no message. Everything this repository ships (release archive, deb, brew formula) uses the name `_rg`, so the packaged paths are unaffected; that is why this is low.

### 3. [Low] `ci/test-complete` now fails if a `compdef` function exists in the environment it runs in

**Where:** `crates/core/flags/complete/rg.zsh` line 441, as consumed by `ci/test-complete` lines 8–17 and 54.

**Evidence:** `ci/test-complete` is a `#!/usr/bin/env zsh` script, so it reads the developer's `.zshenv`. `get_comp_args` runs `( _RG_COMPLETE_LIST_ARGS=1 source $1 )` and expects `_rg` to print its argument specs. While the file is being sourced, `$funcstack[1]` is the file path, not `_rg`, so the only thing that still routes to `_rg "$@"` is `compdef` being undefined. I ran a stand-in with the same `emulate` options and the same `get_comp_args`, with a `.zshenv` that runs `compinit`: the head script yielded 0 argument specs (it ran `compdef _rg rg` and printed nothing), the base script yielded 341. In the real script that is the `Failed to get comp_args` exit.

**Failure:** A developer whose `.zshenv` (or a global zshenv) initialises the completion system, or otherwise defines a `compdef` function, can no longer run the completion consistency check locally; it fails with a message that does not point at the cause. Initialising completion in zshenv is uncommon and CI is not affected, hence low. Checking `_RG_COMPLETE_LIST_ARGS` in the trailer would make the harness independent of the environment.

### 4. [Low] New FAQ zsh instructions: the `.zshrc` snippet carries a `$ ` prompt marker, the `fpath` line omits its ordering requirement, and one sentence is ungrammatical

**Where:** `FAQ.md` lines 124–139.

**Evidence:** (a) Line 135 is introduced as text to "add … to your `$HOME/.zshrc` file" but reads `$ source <(rg --generate complete-zsh)`. The `fpath` snippet seven lines above, also file content, correctly has no marker. A file containing the line as written fails with `command not found: $` (exit 127) and loads nothing — reproduced. (b) Lines 124–129 say to add `fpath=($HOME/.zsh-complete $fpath)` to `.zshrc` without saying it must precede `compinit`. Reproduced: with the assignment before `compinit`, `_comps[rg]` is `_rg`; with it after, `_comps[rg]` is unset and nothing reports why. (c) Line 138, "while this approach is easier to setup, is generally slower", is missing its subject ("it is generally slower"; also "set up").

**Failure:** Readers who copy the documented lines literally, or who append the `fpath` line at the bottom of an existing `.zshrc`, end up with a start-up error or with silently absent completions from the method the FAQ calls recommended. Documentation only, so low.

## Other things examined, no finding

- **Security:** the change adds no new input handling. Sourcing `rg`'s output at shell start executes whatever `rg` is first on `PATH`, which is inherent to the pattern the user opts into and no different from the other tools cited in the issue.
- **Feature gates and devex of building:** no flags, environment variables, build steps or packaging paths change. `zsh.rs` still embeds the file with `include_str!` and only substitutes `!ENCODINGS!`; the release workflow, `ci/build-and-publish-m2`, the deb asset list in `Cargo.toml` and the brew formula all still write or install the file as `_rg`.
- **Portability of the new line:** `$funcstack` and `$functions` come from `zsh/parameter`; if that module is unavailable both expand empty and the trailer falls back to `_rg "$@"`, i.e. base behaviour. The condition is safe under `no_unset`, which `ci/test-complete` sets.
- **Frameworks that predefine a `compdef` stub before `compinit`** (to queue registrations) take the `compdef _rg rg` branch, which is the desired outcome.

## Not verified

- Nothing was run against a real `rg` binary, and the real `ci/test-complete` was not executed; the harness result in finding 3 and the clean-environment check come from a stand-in that copies its `emulate` line and `get_comp_args` verbatim.
- Only zsh 5.8.1 was available; older (< 5.4) and newer zsh versions were not exercised.
