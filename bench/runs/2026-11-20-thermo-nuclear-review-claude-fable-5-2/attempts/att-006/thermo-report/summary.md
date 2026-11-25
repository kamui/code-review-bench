# Thermo-nuclear review — BurntSushi/ripgrep#2957

"feat(completion): support sourcing zsh completion dynamically"
Range `79cbe89..855bfa6` (`main...review-head`), 2 files: `FAQ.md`, `crates/core/flags/complete/rg.zsh`.

## Verdict

The change does what it sets out to do: after `compinit`, `source <(rg --generate complete-zsh)` registers the completion and the existing install paths keep working. There are no security issues, no high-priority findings, and no regression for packaged installs. Three findings follow: one Medium (the new FAQ instructions fail with an error in any `.zshrc` that has not run `compinit` first) and two Low.

## How this was checked

`cargo build` and `ci/test-complete` were unavailable, so the generated script was reproduced outside the clone exactly as `zsh.rs` does it (`rg.zsh` with `!ENCODINGS!` replaced by `encodings.sh`), for both the head and the merge-base. A stub `rg` served that script for `--generate complete-zsh` and a two-line list for `--type-list`. Each scenario ran in a real interactive `zsh -f` (zsh 5.8.1) driven through a pty, pressing TAB on `rg --generate=complete-zs`, `rg --no-colu` and `rg -t ru`. The distro's own `/usr/share/zsh/vendor-completions/_rg` was removed from `fpath` so it could not mask results.

| Scenario | Base | Head |
| --- | --- | --- |
| `_rg` autoloaded from `fpath` (packaged install) | completes | completes |
| Same, with `setopt ksh_autoload` | completes | completes |
| `compinit`, then `source <(rg --generate complete-zsh)` | `_arguments:comparguments:325: can only be called from completion function`, no completion | completes, no error |
| `compinit`, then `eval "$(rg --generate complete-zsh)"` | not run | completes |
| `compinit` with `setopt nounset`, then `source <(...)` | not run | completes |
| `source <(...)` before `compinit`, or with no `compinit` | not run | `_rg:341: command not found: _arguments`, no completion (finding 1) |
| `fpath=(dir $fpath)` placed after `compinit` | not run | no completion, no error (finding 1) |
| Script autoloaded from `fpath` under the name `_ripgrep` | first TAB completes | first TAB dead, later TABs complete (finding 2) |
| Lazy wrapper `_rg() { source <(rg --generate complete-zsh) }; compdef _rg rg` | first TAB completes | first TAB dead, later TABs complete (finding 2) |
| `ci/test-complete`'s `get_comp_args`, replicated in a plain script | 341 specs | 341 specs |
| Same, with `compdef` already defined in the shell | 341 specs | 0 specs (finding 2) |

The CI option-comparison path is therefore unaffected in a normal environment: a non-interactive zsh has no `compdef`, so the fallback branch still runs `_rg` and dumps the specs. The real `ci/test-complete` script itself was not run, because it needs a built `rg`.

## Findings

### 1. [Medium] The new FAQ zsh instructions omit the `compinit` requirement, so the one-liner errors on every shell start when it runs before `compinit`

**File:** `FAQ.md` lines 123–139 (the behaviour comes from the fallback branch at `crates/core/flags/complete/rg.zsh` lines 441–442)

**Evidence.** The FAQ now tells users to "add the following to your `$HOME/.zshrc` file: `source <(rg --generate complete-zsh)`" and says nothing about `compinit`. The guard added in `rg.zsh` only takes the `compdef _rg rg` branch when `compdef` already exists; otherwise `(( ! $+functions[compdef] ))` is true and it calls `_rg "$@"` directly, outside any completion context. zsh does not load the completion system by itself, so `compdef` exists only once `compinit` has run. In a `zsh -f` shell with the line placed before `compinit` (or with no `compinit` at all), sourcing printed `_rg:341: command not found: _arguments`, returned status 1, left `_comps[rg]` unset, and TAB on `rg --generate=complete-zs` completed nothing, even though `compinit` ran afterwards. The same ordering gap applies to the "recommended" half: `fpath=($HOME/.zsh-complete $fpath)` placed after `compinit` left `_comps[rg]` unset and gave no completion, silently.

**Failure.** A user who follows the FAQ literally, in a `.zshrc` that has not already initialised completion above that line (no `compinit`, or the line placed above `compinit` or above the framework line that runs it), gets a cryptic `command not found: _arguments` on every shell start and no `rg` completion. This is the same class of failure issue #2956 was opened about, reached through the path the FAQ now documents. It is not a regression for existing installs, since the base script failed in every sourced case, which is why this is Medium and not higher. The FAQ should state that both snippets must come after/before `compinit` respectively (source line after it, `fpath` line before it). The script could also print a clear message instead of invoking `_rg` when it is sourced without `compdef`, as long as the `_RG_COMPLETE_LIST_ARGS` test path keeps working.

### 2. [Low] The guard infers "being sourced" from the function name and the mere existence of `compdef`, so some completion-time loads now lose the first completion and the CI dump can come back empty

**File:** `crates/core/flags/complete/rg.zsh` lines 441–445

**Evidence.** The condition is `[[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] ))`. Inside a completion `compdef` always exists, so the function only runs when the innermost frame is literally named `_rg`. Two loads that worked on the first TAB at the merge-base do not on the head:

- The generated file autoloaded from `fpath` under any other name. The `#compdef rg` tag makes any `_name` file valid; installed as `_ripgrep`, `$funcstack[1]` is `_ripgrep`, the `else` branch runs `compdef _rg rg`, and the first TAB after shell start completes nothing. Later TABs work.
- A lazy-loading wrapper, `_rg() { source <(rg --generate complete-zsh) }; compdef _rg rg`. `source` pushes its own frame (the measured `funcstack` was `<file> _rg`), so `$funcstack[1]` is the file path and the first TAB is likewise dead.

The same heuristic affects `ci/test-complete`: `get_comp_args` runs `( _RG_COMPLETE_LIST_ARGS=1 source $1 )` and depends on the file calling `_rg`. Replicating that function in a shell where `compdef` was already defined returned 0 specs on the head against 341 on the base, which would make the script exit with "Failed to get comp_args".

**Failure.** One silently dropped completion per shell session for non-`_rg` installs and lazy-load wrappers, and a spurious `ci/test-complete` failure for a developer whose `.zshenv` runs `compinit`. All three need unusual setups: every packaging path in this repo installs the file as `_rg` (`Cargo.toml` deb asset, `pkg/brew/ripgrep-bin.rb`, release workflow), and CI's non-interactive zsh has no `compdef`. Hence Low. Replacing the name check with a completion-context check, `(( $+compstate )) || (( ! $+functions[compdef] ))`, restored the first TAB in both cases and kept `source <(...)` working in the same harness; it does not address the `ci/test-complete` case, which would need `_RG_COMPLETE_LIST_ARGS` added to the condition.

### 3. [Low] The snippet meant to be pasted into `.zshrc` carries a `$ ` prompt prefix, and the sentence after it is missing a word

**File:** `FAQ.md` lines 134–139

**Evidence.** The text says to "add the following to your `$HOME/.zshrc` file" and the block reads `$ source <(rg --generate complete-zsh)`. The sibling block that is also meant for `.zshrc`, `fpath=($HOME/.zsh-complete $fpath)` at line 128, correctly has no prompt. A file containing the line as printed fails with `command not found: $` (status 127) when zsh runs it. The following sentence, "while this approach is easier to setup, is generally slower than the previous method", is missing "it" before "is generally slower".

**Failure.** A user who copies the block verbatim into `.zshrc` gets `command not found: $` at every shell start and no completion. The `$ ` convention is used elsewhere in this FAQ for commands typed at a prompt, so most readers will strip it, which keeps this at Low.

## Checked and not reported

- **Packaged installs** (`_rg` in `fpath`, including `ksh_autoload`): behaviour is unchanged from the merge-base.
- **`zsh.rs` generation**: untouched, and the `!ENCODINGS!` substitution is unaffected by the new lines.
- **Shell options at source time**: `setopt nounset` works. `setopt ksh_arrays` prints one `bad output format specification` message but still registers the completion; not reported because interactive `ksh_arrays` is vanishingly rare.
- **Security**: the change adds no new input handling, command execution or file access; `compdef _rg rg` registers a function defined in the same file.
- **Devex and feature gates**: no build, environment or gating changes.
