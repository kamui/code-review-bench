# 01 — zsh completion load-mode dispatch (`crates/core/flags/complete/rg.zsh`, `ci/test-complete`)

Carries the evidence for Findings 1 and 2 in `summary.md`.

## What changed

At the merge-base the file ended with an unconditional call:

```zsh
_rg "$@"
```

At the head (`rg.zsh:437-445`) it ends with:

```zsh
# Don't run the completion function when being sourced by itself.
#
# See https://github.com/BurntSushi/ripgrep/issues/2956
# See https://github.com/BurntSushi/ripgrep/pull/2957
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

The file is consumed in three ways, and the tail has to pick the right action for each:

1. Autoloaded from `fpath` by `compinit` (the release archives and `ci/build-and-publish-m2` write it as `complete/_rg`). The file is the function body, so it must call `_rg "$@"`.
2. Sourced or eval'd from a `.zshrc` (the new feature). It must only register: `compdef _rg rg`.
3. Sourced by `ci/test-complete:16` as `( _RG_COMPLETE_LIST_ARGS=1 source $1 )`, with no completion system loaded. The harness expects `_rg` to run and print its specs.

The head condition encodes case 1 as "the calling function is named `_rg`" and case 3 as "`compdef` is not defined". Neither is the fact the code cares about.

## Method

Cargo was unavailable, so the generated script was reproduced by substituting the `!ENCODINGS!` placeholder in `rg.zsh` with two encoding names, for both the head and the merge-base version. The substitution is the only thing `zsh.rs::generate` does.

All runs used `zsh 5.8.1` with `-f` and a private `ZDOTDIR`/`HOME`. This machine ships `/usr/share/zsh/vendor-completions/_rg`, which masks results by registering its own `_rg`, so that directory was removed from `fpath` for the interactive runs.

Interactive behaviour was checked by starting `zsh -f -i` under `zsh/zpty`, running the setup line, typing `rg --generate=complete-zs` followed by TAB, and reading the screen; then TAB again. Scripts are in `../scratch/t/` (`pty.zsh`, `autoload.zsh`, `probe.zsh`, `harness.zsh`).

## Load-mode matrix (verified)

| Load mode | Merge-base | Head | Proposed |
| --- | --- | --- | --- |
| `fpath`, file named `_rg`, after `compinit` | first TAB completes | first TAB completes | first TAB completes |
| `fpath`, file named `_ripgrep`, after `compinit` | first TAB completes | first TAB beeps, second TAB completes | first TAB completes |
| `source <(...)` after `compinit` | `_arguments:comparguments:325: can only be called from completion function`, no completion | completes | completes |
| `eval "$(...)"` after `compinit` | not run | completes | completes |
| `source <(...)` before `compinit` | `_rg:341: command not found: _arguments` | `_rg:341: command not found: _arguments`, no completion | `/proc/self/fd/11:444: command not found: compdef`, no completion |
| harness body, no `compdef` | 341 specs | 341 specs | 341 specs (with the harness change below) |
| harness body, `compdef` defined | 341 specs | 0 specs | 341 specs (with the harness change below) |

The third row is the PR's stated goal and it is met.

## Finding 1 — evidence

The `(( ! $+functions[compdef] ))` clause is the only reason the harness row still passes at the head. The review thread on `rg.zsh:438` states that the clause was added because `ci/test-complete` "didn't like" the two-branch form. The comment in the file says only "Don't run the completion function when being sourced by itself", which explains the `else` branch and says nothing about why a missing `compdef` means "run it".

Consequences, both reproduced:

- A user who puts the `source` line above `compinit` in `.zshrc` has no `compdef`, so the tail calls `_rg "$@"` at shell start. Output: `_rg:341: command not found: _arguments`, and no completion is registered afterwards. That is the same class of failure as the issue being closed, with a message that points at line 341 of the completion function rather than at the ordering mistake.
- With `compdef` defined before the harness body runs (for example a `zshenv` that loads `compinit`), the head returns zero specs. `ci/test-complete` would then stop at `Failed to get comp_args`. The merge-base returned 341 in the same setup, so the PR made the harness depend on the ambient shell state.

## Finding 2 — evidence

`zsh -f t/autoload.zsh <dir>` puts the directory first in `fpath`, runs `compinit`, and calls whatever `_comps[rg]` names with `_RG_COMPLETE_LIST_ARGS=1`:

```
== head, file installed as _ripgrep
before: _comps[rg]=_ripgrep
first call printed 0 lines
after first call: _comps[rg]=_rg
second call printed 341 lines

== merge-base, file installed as _ripgrep
before: _comps[rg]=_ripgrep
first call printed 341 lines
```

The pty run agrees: at the head the first TAB produced a bell and no insertion, and the second TAB completed `complete-zsh`.

Every install path inside this repository names the file `_rg` (`ci/build-and-publish-m2:38`, `.github/workflows/release.yml:222,252,346`), so this affects third-party packaging and hand installs, not the release artifacts. It is still a behaviour the file had before the PR and silently lost.

`t/probe.zsh` shows what zsh reports in each mode:

```
autoloaded as _probe      ctx=[toplevel shfunc loadautofunc] fs1=[_probe]
autoloaded as _other      ctx=[toplevel shfunc loadautofunc] fs1=[_other]
source file               ctx=[toplevel file]                fs1=[<path>]
source <(...)             ctx=[toplevel file]                fs1=[/proc/self/fd/11]
eval "$(...)"             ctx=[toplevel eval]                fs1=[(eval)]
sourced inside a function ctx=[toplevel shfunc file]         fs1=[<path>]
autoloaded via a wrapper  ctx=[toplevel shfunc shfunc loadautofunc] fs1=[_probe2]
```

The last element of `zsh_eval_context` is `loadautofunc` exactly when the file is being loaded as an autoload body, independent of its name.

## Worked proposal

`rg.zsh` tail:

```zsh
# When autoloaded from fpath, this file is the body of the completion function,
# so run it. When sourced or eval'd instead, e.g.
#
#     source <(rg --generate complete-zsh)
#
# only register it. That requires compinit to have run already.
#
# See https://github.com/BurntSushi/ripgrep/issues/2956
if [[ $zsh_eval_context[-1] == loadautofunc ]]; then
  _rg "$@"
else
  compdef _rg rg
fi
```

`ci/test-complete`, in `get_comp_args`:

```zsh
    # Sourcing the file only registers the function, so stub out the
    # registration and call it ourselves. Do this in a sub-shell to avoid any
    # weirdness
    ( compdef() { }; source $1; _RG_COMPLETE_LIST_ARGS=1 _rg )
```

This leaves one condition on one fact, puts the test accommodation in the test, and removes the dependence on both the file name and the ambient `compdef`. The "Proposed" column above was produced with exactly these two bodies.

Sourcing before `compinit` still fails under the proposal, since registration cannot work without `compinit`. The message becomes `command not found: compdef`, which names the missing piece. Finding 3 covers documenting the order.

## Not verified

- The zsh release that introduced `zsh_eval_context` and its `loadautofunc` value. The file strips argument groups for zsh 4 and 5.0–5.3, so it claims support for versions I could not test. If the parameter is missing on a supported version, `[[ $funcstack[1] == _rg ]]` alone, with the same harness change, resolves Finding 1 and keeps Finding 2 as a known limitation.
- `ci/test-complete` itself was not run (needs a built `rg`); `t/harness.zsh` copies its option setup and `get_comp_args` body.
- Only zsh 5.8.1 on Linux was exercised.
