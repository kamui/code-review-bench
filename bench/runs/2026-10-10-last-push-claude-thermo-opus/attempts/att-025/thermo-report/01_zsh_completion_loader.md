# 01 — zsh completion loader (`crates/core/flags/complete/rg.zsh`)

Scope: the nine added lines at `crates/core/flags/complete/rg.zsh:437-445`, and their
interaction with the unchanged test harness `ci/test-complete`.

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

## How everything below was verified

`cargo build` and `ci/test-complete` are unavailable in this environment, so the checked-in
`rg.zsh` was used directly (the only difference from `rg --generate complete-zsh` output is
the `!ENCODINGS!` placeholder inside one array, which does not affect loading). All runs used
zsh 5.8.1 with `zsh -f`, a private `HOME`/`ZDOTDIR`, and scratch scripts kept outside the
clone under `clone-work/scratch/` (`harness.zsh`, `run2.zsh`, `run3.zsh`, `pty.zsh`).

One environmental trap is worth recording: this machine has a packaged
`/usr/share/zsh/vendor-completions/_rg`, so `compinit` registers `rg` on its own. Every
registration check below therefore strips that directory from `fpath` first
(`fpath=(${fpath:#*vendor-completions*})`) and confirms `_comps[rg]` is unset before the
file under test is loaded.

## What the guard does in each load mode (measured)

| # | How the file is loaded | `$funcstack[1]` | `compdef` defined | Branch taken | Observed result |
| --- | --- | --- | --- | --- | --- |
| 1 | autoloaded from `fpath` as `_rg` by compinit | `_rg` | yes | run `_rg` | `_comps[rg]=_rg`; `_RG_COMPLETE_LIST_ARGS=1 _rg` prints 341 specs |
| 2 | `source <(…)` after `compinit` | `/proc/self/fd/NN` | yes | `compdef _rg rg` | rc=0, `_comps[rg]=_rg`, `_rg is a shell function from /proc/self/fd/13` |
| 3 | `eval "$(…)"` after `compinit` | `(eval)` | yes | `compdef _rg rg` | rc=0, `_comps[rg]=_rg` |
| 4 | `source <(…)` from inside a wrapper function, after `compinit` | file path | yes | `compdef _rg rg` | rc=0, `_comps[rg]=_rg` |
| 5 | `source <(…)` **before** `compinit` | `/proc/self/fd/NN` | no | run `_rg` | `_rg:341: command not found: _arguments`, rc=1, nothing registered |
| 6 | `eval "$(…)"` **before** `compinit` | `(eval)` | no | run `_rg` | same error as row 5 |
| 7 | `ci/test-complete` (`( _RG_COMPLETE_LIST_ARGS=1 source $1 )`) | file path | no | run `_rg` | 341 specs printed (what the harness wants) |
| 8 | same harness, but `compdef` already defined in the environment | file path | yes | `compdef _rg rg` | **0 lines** → harness would report `Failed to get comp_args` |
| 9 | `cd ~/.zsh-complete && source _rg` after `compinit` | `_rg` | yes | run `_rg` | `_arguments:comparguments:325: can only be called from completion function`, rc=1 |

An end-to-end check through a pty (`pty.zsh`: interactive `zsh -f -i`, type
`rg --generate=complete-z<TAB>`, read the line back) confirms the PR's headline claim and
both failure cases:

```
--- [rg.zsh, source after compinit] line after TAB:
rg --generate=complete-zsh
--- [rg.zsh, source before compinit] line after TAB:
rg --generate=complete-z^G
--- [rg-base.zsh, source after compinit] line after TAB:      (merge-base version)
rg --generate=complete-z^G
```

So the feature works for the path in the PR's test plan (rows 2–4), and the fpath install is
unaffected (row 1). The findings are about rows 5–8.

## Finding F1 — the second disjunct is a test-harness accommodation living in the shipped script

**Where:** `crates/core/flags/complete/rg.zsh:441` (`|| (( ! $+functions[compdef] ))`).

**Status:** verified by execution (rows 5, 6, 7, 8 above).

**What is wrong.** The condition has two halves with unrelated purposes. `$funcstack[1] == _rg`
answers the real question — "am I being run as the autoloaded completion function, or merely
being read by `source`/`eval`?" — and is the widely used idiom for dual-mode zsh completion
files. `! $+functions[compdef]` answers nothing about load mode. It was added because
`ci/test-complete` sources the file from a non-interactive script, where `compdef` does not
exist, and still expects `_rg` to be invoked so it can dump its option specs (review threads
6–7 in the packet say exactly this). In other words: the shipped completion script infers
"I am probably inside the test harness" from the absence of an unrelated function.

That inference is wrong for every real user it applies to. The only non-test situation in
which a file is being sourced and `compdef` is missing is a `.zshrc` that loads the snippet
before `compinit` (or never calls `compinit`). The guard maps that situation to "run the
completion function right now, at shell startup, outside any completion context", and the
user gets:

```
_rg:341: command not found: _arguments
```

on every new shell, with `rg` left unregistered and TAB dead (pty run above). That is the
same class of failure as issue #2956 — a completion function executed outside the
completion system, reported by an internal helper the user has never heard of — with a
different message. The PR removed that failure for one ordering and reintroduced it for the
other, and the FAQ text added in the same PR sends users to `.zshrc` without stating which
ordering is required (see `02_faq_zsh_instructions.md`).

The same heuristic also makes the harness itself depend on ambient state. Row 8: if `compdef`
happens to be defined in the environment the harness runs in, the file silently takes the
`compdef _rg rg` branch, prints nothing, and `ci/test-complete` fails with
`Failed to get comp_args` — a message that points nowhere near the cause. `#!/usr/bin/env zsh`
scripts read `.zshenv`, so this is reachable on a developer machine whose `.zshenv` sets up
completion; I have no reason to think it occurs in the project's CI.

**Code-judo.** The harness's need is "call `_rg` with `_RG_COMPLETE_LIST_ARGS=1`". It can do
that itself, by loading the file the same way the completion system does — as the body of a
function named `_rg`. Then the shipped tail collapses to the plain two-branch idiom, the
condition has one concept in it, and the test exercises the production load path (autoload
semantics) instead of a third path that exists only for the test.

Shipped file (`rg.zsh`), replacing lines 437-445:

```zsh
# This file is loaded in one of two ways. Normally it's autoloaded from fpath
# as the function `_rg`, in which case its body is the whole file and we need
# to call the real `_rg` defined above. Alternatively it can be sourced or
# eval'd after compinit, e.g. `source <(rg --generate complete-zsh)`, in which
# case we must not run the completion function, only register it.
if [[ $funcstack[1] == _rg ]]; then
  _rg "$@"
else
  compdef _rg rg
fi
```

Harness (`ci/test-complete`, `get_comp_args`), one line changed:

```zsh
    # Load the file as the body of `_rg`, exactly as autoloading it would
    ( functions[_rg]="$(<$1)"; _RG_COMPLETE_LIST_ARGS=1 _rg )
```

Measured with `harness.zsh` / `run2.zsh`, under the harness's own
`emulate zsh -o extended_glob -o no_function_argzero -o no_unset`:

| Harness loader | Tail in `rg.zsh` | `compdef` in env | Lines of specs |
| --- | --- | --- | --- |
| PR (`source $1`) | PR | no | 341 |
| PR (`source $1`) | PR | yes | **0** |
| `functions[_rg]="$(<$1)"; _rg` | two-branch | no | 341 |
| `functions[_rg]="$(<$1)"; _rg` | two-branch | yes | 341 |
| `functions[_rg]="$(<$1)"; _rg` | PR (unchanged) | no | 341 |
| `compdef() { }; source $1; _rg` | two-branch | no | 341 |

The three 341-line outputs were compared with `cmp` and are byte-identical to the PR's
output, so the option-parity check sees exactly the same data. The fifth row shows the
harness change is safe to land on its own, ahead of the tail change. The last row is an
alternative harness shape (stub `compdef`, then call `_rg` explicitly) for anyone who finds
assigning to `functions[...]` too clever; it is equally correct, slightly less faithful to
the production path.

With the two-branch tail, the other rows behave as follows (also measured): fpath autoload
still registers and runs (`_comps[rg]=_rg`, 341 specs); `source` after `compinit` still
registers (rc=0, `_comps[rg]=_rg`); and `source` before `compinit` now fails with

```
/proc/self/fd/11:444: command not found: compdef
```

which names the actual missing prerequisite, and is the familiar failure for any
completion script that is sourced before the completion system is initialised. That is a
behaviour change only in a case that is already broken, and it replaces an obscure error
with a self-explanatory one. If the maintainers want to go one step further, the `else`
branch could test `$+functions[compdef]` and print a one-line "run compinit first" hint to
stderr, but that is optional polish; the important part is that the missing-`compdef` case
stops pretending to be the autoload case.

## Finding F4 — the comment explains half the condition and outsources the rest to two URLs

**Where:** `crates/core/flags/complete/rg.zsh:437-440`.

**Status:** verified by reading; no execution needed.

`crates/core/flags/complete/zsh.rs` justifies hand-maintaining this script partly because
"there is a wealth of documentation in the zsh script explaining how it works". The new
block is the one place in the file that does not meet that bar. "Don't run the completion
function when being sourced by itself" is ambiguous (sourced *by* itself? sourced *on its
own*?), it describes only the `funcstack` half, and it says nothing about why a missing
`compdef` should cause `_rg` to run — which is the non-obvious half, and the half a future
maintainer is most likely to "simplify" away and break `ci/test-complete`. The two bare
`See …` links make the reader leave the repository to recover a rationale that fits in
three lines. The file header (lines 3-12) also still describes a single way of using the
file.

If F1 is adopted the comment shown above replaces it and the problem disappears with the
second disjunct. If F1 is not adopted, the comment must at minimum say that the
`! $+functions[compdef]` case exists for `ci/test-complete`, which sources this file
without the completion system loaded and relies on `_rg` being called.

## Things checked and deliberately not escalated

- **Row 9, `source _rg` by bare relative name.** `$funcstack[1]` for a sourced file is the
  argument as typed, so sourcing a file literally named `_rg` from its own directory is
  misread as the autoload case and reproduces the original #2956 error. Sourcing by any path
  containing a slash is fine (`source ~/.zsh-complete/_rg` → rc=0, registered). This is a
  limit of the name-comparison idiom itself rather than of this PR's version of it, and it
  is contrived enough that I would only mention it. `$zsh_eval_context[-1] == loadautofunc`
  would discriminate precisely, but it would break the `functions[_rg]=…` harness loader and
  the file still carries compatibility code for zsh < 5.4, so I am not proposing it.
- **`"$@"` when sourced.** In the PR's fallback branch, `_rg "$@"` forwards whatever
  positional parameters the *sourcing* context has (in `ci/test-complete` that is the path to
  `rg.zsh`). `_rg` ignores them in list mode, and the behaviour predates the PR.
- **File size.** `rg.zsh` goes 637 → 645 lines; no threshold concern.
- **Canonical layer.** The tail of the hand-written `rg.zsh` is the right home for load-mode
  dispatch; `zsh.rs` stays a one-line `include_str!` and nothing leaked into Rust.
- **`release.yml` / `ci/build-and-publish-m2`.** Both still write the generated script to
  `complete/_rg` for packaging; row 1 covers that path and it is unaffected.
