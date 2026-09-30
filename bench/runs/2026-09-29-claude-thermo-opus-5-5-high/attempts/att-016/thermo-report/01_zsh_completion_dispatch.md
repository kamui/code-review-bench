# 01 — zsh completion load-mode dispatch (`crates/core/flags/complete/rg.zsh`)

Scope: the nine-line tail added at `crates/core/flags/complete/rg.zsh:437-445`, its interaction with `ci/test-complete:16`, and the generator `crates/core/flags/complete/zsh.rs` (unchanged; it embeds `rg.zsh` verbatim apart from the `!ENCODINGS!` substitution, so every line of the tail ships in `rg --generate complete-zsh`).

## The change

Before the PR the file ended with an unconditional `_rg "$@"`. That is the correct shape for a file that compinit autoloads from `$fpath`: zsh uses the whole file as the body of `_rg`, so the first call redefines `_rg` and the helpers and then calls the real `_rg`. When the same file is `source`d from a `.zshrc`, though, that final call runs outside any completion context and fails, which is the problem in issue #2956.

The PR replaces the call with:

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

The file can be loaded in three different ways, and this one predicate has to tell them apart:

| Load mode | What should happen | What the predicate checks |
| --- | --- | --- |
| Autoloaded by compinit (file installed as `_rg` in `$fpath`) | run `_rg "$@"` | `$funcstack[1] == _rg` |
| Sourced by a user after `compinit` | `compdef _rg rg` | falls to `else` |
| Sourced by `ci/test-complete` with `_RG_COMPLETE_LIST_ARGS=1` and no compsys | run `_rg` so it dumps its specs | `! $+functions[compdef]` |

The review thread on the PR shows why the third clause exists. The author first wrote the two-way version (`funcstack` or `compdef`). `ci/test-complete` failed, and the reviewer then suggested adding `|| (( ! $+functions[compdef] ))` so the harness would keep working.

## Verification (executed)

I ran scratch zsh 5.8.1 scripts under `clone-work/scratch/`, using a copy of `rg.zsh` with `!ENCODINGS!` replaced by `utf-8` (`rg-gen.zsh`). Each ran with `zsh -f`.

1. **Sourced before `compinit`** (`t1.zsh`: `source ./rg-gen.zsh`). The output was `_rg:341: command not found: _arguments`, and `source` returned 1. If the completion-system functions are already on `fpath`/autoloaded, the same path gives the issue's exact error, `_arguments:comparguments:NNN: can only be called from completion function`. I observed that when I invoked the body outside a completion context (`t6.zsh` P4). **Verified: the PR's `! compdef` branch reproduces the bug it closes whenever the `source` line comes before `compinit`.**
2. **Sourced after `compinit`** (`t2.zsh`). `source` returned 0 and `_comps[rg]=_rg`. **Verified: the headline scenario works.**
3. **Load-mode probes** (`t3.zsh`, a file that prints `funcstack` and `zsh_eval_context`):
   - sourced inside a function: `funcstack=./probe.zsh,f evalctx=toplevel,shfunc,file`
   - `source <(...)`: `funcstack=/proc/self/fd/11 evalctx=toplevel,file`
   - autoloaded as `_probe`: `funcstack=_probe evalctx=toplevel,shfunc,loadautofunc`
   - autoloaded as `_other`: `funcstack=_other evalctx=toplevel,shfunc,loadautofunc`

   **Verified: `zsh_eval_context[-1] == loadautofunc` identifies "I am being autoloaded" regardless of the file name. `funcstack[1] == _rg` only works if the file is installed under the one name `_rg`.**
4. **Installed under a different name** (`t5.zsh`: the file copied to `$fpath/_ripgrep`, compinit loaded, `_ripgrep` invoked the way the completer would). The body did *not* run. The call returned 0 and only registered `compdef _rg rg`. **Verified: a misnamed install gives an empty first TAB instead of completions.**
5. **Harness-style load** (`t4.zsh`: `( _RG_COMPLETE_LIST_ARGS=1 source ./rg-gen.zsh )`). It dumps the specs (`+`, `(exclusive)`, `(: * -)-h[...]`). **Verified: the harness passes only because of the `! compdef` clause.**

## Finding 01-A — test-harness mode is encoded as production dispatch, and it re-opens #2956

The `(( ! $+functions[compdef] ))` clause does nothing for any real user. Its only purpose is to keep `ci/test-complete` green. It works because the harness happens to source the file in a shell where compsys is not loaded. In production, "sourced and `compdef` does not exist" means only one thing: the user put `source <(rg --generate complete-zsh)` before `compinit`, which is a very common `.zshrc` ordering. For that user the new branch does the worst available thing. It calls `_rg` outside a completion context and prints the same cryptic `_arguments` error the PR was meant to remove (verification 1). The harness also no longer tests either real load mode. It tests a third mode that exists only for the harness. So the predicate is spaghetti in the plain sense: one boolean expression mixes up "am I autoloaded?", "is compsys ready?", and "am I under test?", and the reader has to reconstruct all three from the review thread.

**Code-judo proposal.** Base the dispatch on *how the file was loaded*, and let the harness own its own needs:

```zsh
# This file is either autoloaded from $fpath by compinit (in which case the
# file *is* the body of the completion function) or sourced directly, e.g.
# `source <(rg --generate complete-zsh)` after `compinit`.
if [[ $zsh_eval_context[-1] == loadautofunc ]]; then
  _rg "$@"
else
  compdef _rg rg
fi
```

and in `ci/test-complete:16`:

```zsh
( _RG_COMPLETE_LIST_ARGS=1; compdef() { : }; source $1; _rg )
```

This makes the harness-only branch in shipped code go away. The harness explicitly stubs `compdef` and calls `_rg` itself, the way it already sets `_RG_COMPLETE_LIST_ARGS`. A user who sources before `compinit` now gets zsh's normal `command not found: compdef`, which points at the real mistake, instead of a completion-engine error from inside `_arguments`. If the maintainers want a friendlier message, a single `(( $+functions[compdef] )) || { print -ru2 'rg: run compinit before sourcing completions'; return 1 }` guard before the `compdef` line is still easier to read than the current mixed predicate. I checked a variant of this with a message (`t6.zsh` P1–P4, `proposed.zsh`). Sourced before compinit, it prints the guidance and returns 1. Sourced after compinit, it registers `_rg`. The harness-style load with the stub/explicit call dumps the specs. Autoloaded as `_ripgrep`, it runs the body.

Status: **verified by execution** (current behaviour and proposed behaviour). I did not run the real `ci/test-complete` because it needs a built `rg`, which the execution policy makes unavailable. The harness change above is checked only in the scratch simulation.

## Finding 01-B — `$funcstack[1] == _rg` hard-codes the install filename

`funcstack[1]` is the name of the autoloaded function, and that comes from the *file name* on `$fpath`, not from the `#compdef rg` header or the `_rg() { ... }` definition inside the file. The predicate therefore depends silently on the file being installed as exactly `_rg`. Verification 4 shows that under any other name (`_ripgrep`, or a packager's `_rg.zsh`-derived name) the autoload path falls to `compdef _rg rg`. The first completion attempt returns nothing, and only later attempts work, because `_comps[rg]` has by then been rebound. That is the "magic string" failure mode: behaviour depends on a coincidence between two names that the code never states. The same `zsh_eval_context[-1] == loadautofunc` test from 01-A removes the coupling, so both findings have one remedy.

Status: **verified by execution** (`t5.zsh`).

## Finding 01-C — the explanatory comment misdescribes the branch

The comment at `rg.zsh:437` says "Don't run the completion function when being sourced by itself." The code *does* run the completion function when sourced, whenever `compdef` is missing, which is the harness case and the sourced-before-compinit case. After that the comment only gives two URLs. The file is otherwise carefully documented (the top-of-file header and the long "ZSH COMPLETION REFERENCE" section), but it never names the load modes this tail now has to support. The header at `rg.zsh:1-13` still describes only the fpath/autoload use. This is a legibility issue, not a blocker. Replace the comment with a two-line statement of the load-mode contract, as in the 01-A proposal, and mention the `source` usage in the header next to the existing `ci/test-complete` note.

Status: **verified by reading** against executed behaviour.

## Not flagged

- `rg.zsh` goes from 637 to 645 lines, well under the 1k threshold.
- `zsh.rs` needs no change. The `include_str!` + `replace` generator is the right single source of truth, and the fix correctly lives in the shell script and not in the Rust generator.
